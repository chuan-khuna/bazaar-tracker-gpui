//! The Advisor tab of the detail island, the chat icons that open it, and the glue between the
//! tab's state (`tracking::advisor::state`) and the client's threads (`tracking::advisor::client`).
//!
//! The tab follows the selected Run: its Conversations newest first, the chosen one's messages,
//! and the box to ask in. Each message is sent with the situation now (the latest board read for
//! the Live run, else the Run's last Snapshot).

use gpui::{Context, Div, SharedString, Stateful, div, prelude::*, px, rgb};

use crate::tracking::advisor::client::Update;
use crate::tracking::advisor::conversation::{Conversation, Entry, Role};
use crate::tracking::advisor::situation;
use crate::tracking::advisor::state::{ChatIcon, DetailTab, Request, icon_run};
use crate::ui::TrackerApp;
use crate::ui::widgets::{self, IconName, Tone};

/// The group name of a Conversation's row, so that its ✕ shows while the pointer is over it.
const CONVERSATION_ROW: &str = "conversation-row";

/// How long a message may be, in characters.
const INPUT_MAX: usize = 4000;

impl TrackerApp {
    /// Hand each request to the client's threads.
    fn ask_advisor(&self, requests: Vec<Request>) {
        for request in requests {
            self.advisor_client.ask(request);
        }
    }

    /// Something the client's threads reported.
    pub(in crate::ui) fn on_advisor_update(&mut self, update: Update, cx: &mut Context<Self>) {
        let requests = match update {
            Update::Conversations { run_id, result } => self.advisor.on_conversations(&run_id, result),
            Update::Created { run_id, text, situation, result } => self.advisor.on_created(&run_id, text, situation, result),
            Update::Messages { conversation_id, result } => {
                self.advisor.on_messages(&conversation_id, result);
                self.advisor_scroll.scroll_to_bottom();
                Vec::new()
            }
            Update::Event { conversation_id, event } => {
                self.advisor.on_event(&conversation_id, &event);
                self.advisor_scroll.scroll_to_bottom();
                Vec::new()
            }
            Update::Ended { conversation_id, result } => {
                self.advisor.on_ended(&conversation_id, result);
                Vec::new()
            }
            Update::Deleted { conversation_id, result } => self.advisor.on_deleted(&conversation_id, result),
        };
        self.ask_advisor(requests);
        cx.notify();
    }

    /// A chat icon: select its Run, open the detail island on the Advisor tab, and show the Run's
    /// newest Conversation (or none, so the first message makes one). The box takes the typing.
    pub(in crate::ui) fn open_advisor(&mut self, icon: ChatIcon<'_>, cx: &mut Context<Self>) {
        let Some(run_id) = icon_run(icon, self.session.live()).map(str::to_owned) else { return };
        // Asked before selecting, so following the selection has nothing more to ask.
        let requests = self.advisor.open(&run_id, true);
        self.ask_advisor(requests);
        self.select_run(&run_id, cx);
        self.detail_open = true;
        self.detail_tab = DetailTab::Advisor;
        self.advisor_input_focused = true;
        self.odds_search_focused = false;
        cx.notify();
    }

    /// Open one of the detail island's tabs. Opening the Advisor shows the selected Run's
    /// Conversations, and asks the backend again if it wasn't usable.
    pub(in crate::ui) fn set_detail_tab(&mut self, tab: DetailTab, cx: &mut Context<Self>) {
        self.detail_tab = tab;
        if tab == DetailTab::Advisor
            && let Some(run_id) = self.session.selected().map(str::to_owned)
        {
            let requests = self.advisor.open(&run_id, false);
            self.ask_advisor(requests);
        }
        cx.notify();
    }

    /// While the Advisor tab is open, follow a change of selected Run.
    pub(in crate::ui) fn advisor_follow_selection(&mut self) {
        if !(self.detail_open && self.detail_tab == DetailTab::Advisor) {
            return;
        }
        if let Some(run_id) = self.session.selected().map(str::to_owned)
            && self.advisor.run_id() != Some(run_id.as_str())
        {
            let requests = self.advisor.open(&run_id, false);
            self.ask_advisor(requests);
        }
    }

    /// The situation to send with a message about the Advisor's Run.
    fn advisor_situation(&self) -> serde_json::Value {
        let Some(run_id) = self.advisor.run_id() else { return serde_json::Value::Null };
        let Some(run) = self.session.run(run_id) else { return serde_json::Value::Null };
        // The Advisor follows the selected Run, whose Events are loaded.
        let events = if self.session.selected() == Some(run_id) { self.session.events() } else { &[] };
        situation::for_run(run, self.session.is_live(run_id), self.live.board(), events, &self.names).to_json()
    }

    fn send_advisor(&mut self, cx: &mut Context<Self>) {
        let situation = self.advisor_situation();
        let requests = self.advisor.send(situation);
        if !requests.is_empty() {
            self.advisor_scroll.scroll_to_bottom();
        }
        self.ask_advisor(requests);
        cx.notify();
    }

    /// A key while the box takes the typing: Enter sends, Shift+Enter starts a new line, Escape lets go.
    pub(in crate::ui) fn advisor_key(&mut self, key: &str, typed: Option<&str>, shift: bool, cx: &mut Context<Self>) {
        match key {
            "enter" if shift => self.type_advisor("\n"),
            "enter" => self.send_advisor(cx),
            "escape" => self.advisor_input_focused = false,
            "backspace" => {
                self.advisor.input.pop();
            }
            _ => {
                if let Some(typed) = typed.filter(|typed| !typed.chars().any(char::is_control)) {
                    self.type_advisor(typed);
                }
            }
        }
        cx.notify();
    }

    /// Add `text` to the box, up to `INPUT_MAX` characters. Pasted line endings become `\n`.
    pub(in crate::ui) fn type_advisor(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let room = INPUT_MAX.saturating_sub(self.advisor.input.chars().count());
        self.advisor.input.extend(text.chars().filter(|c| *c == '\n' || !c.is_control()).take(room));
    }

    /// The chat icon on a sidebar Run's row.
    pub(in crate::ui) fn row_chat_button(&self, run_id: &str, cx: &mut Context<Self>) -> gpui_base::Button {
        let t = self.theme;
        let id = run_id.to_owned();
        widgets::icon_button(t, SharedString::from(format!("chat-run-{run_id}")), widgets::icon(IconName::MessageCircle, t.subtle))
            .tooltip(widgets::hint(t, vec!["Ask the Advisor about this run".into()]))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.open_advisor(ChatIcon::Row(&id), cx);
            }))
    }

    /// The chat icon on the Main island beside the pop-out button: the Live run's, greyed out
    /// while no Run is being played.
    pub(in crate::ui) fn live_chat_button(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        if icon_run(ChatIcon::Main, self.session.live()).is_none() {
            return div().child(
                div()
                    .id("chat-live")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(22.))
                    .opacity(0.4)
                    .child(widgets::icon(IconName::MessageCircle, t.muted))
                    .tooltip(widgets::hint(t, vec!["Ask the Advisor".into(), "No run is being played".into()])),
            );
        }
        div().child(
            widgets::icon_button(t, "chat-live", widgets::icon(IconName::MessageCircle, t.subtle))
                .tooltip(widgets::hint(t, vec!["Ask the Advisor about this run".into()]))
                .on_click(cx.listener(|this, _, _, cx| this.open_advisor(ChatIcon::Main, cx))),
        )
    }

    /// The tabs along the top of the detail island.
    pub(in crate::ui) fn render_detail_tabs(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let tab = |id: &'static str, label: &'static str, which: DetailTab| {
            self.filter_button(id.to_owned(), label.to_owned(), self.detail_tab == which).on_click(cx.listener(move |this, _, _, cx| this.set_detail_tab(which, cx)))
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .px_3()
            .pt_3()
            .pb_2()
            .border_b_1()
            .border_color(rgb(t.overlay))
            .child(tab("detail-tab-run", "Run", DetailTab::Run))
            .child(tab("detail-tab-advisor", "Advisor", DetailTab::Advisor))
    }

    /// The Advisor tab: the Conversation list, the chosen one's messages, and the box.
    pub(in crate::ui) fn render_advisor(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let body = div().flex().flex_col().flex_1().min_h_0();
        if self.advisor.run_id().is_none() || self.advisor.run_id() != self.session.selected() {
            return body.child(widgets::empty_state(t, "Select a run to ask the Advisor about it."));
        }
        body.child(self.render_conversation_list(cx))
            .children(self.advisor.status().map(|status| {
                div().flex_none().mx_3().my_2().px_2().py_1p5().rounded_md().border_1().border_color(rgb(t.error)).text_xs().text_color(rgb(t.error)).child(status)
            }))
            .child(self.render_advisor_messages())
            .child(self.render_advisor_input(cx))
    }

    fn render_conversation_list(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let new = widgets::icon_button(t, "conversation-new", widgets::icon(IconName::Plus, t.subtle))
            .tooltip(widgets::hint(t, vec!["New conversation".into()]))
            .on_click(cx.listener(|this, _, _, cx| {
                this.advisor.new_conversation();
                this.advisor_input_focused = true;
                cx.notify();
            }));
        let rows: Vec<_> = self.advisor.conversations().iter().map(|conversation| self.render_conversation_row(conversation, cx)).collect();
        let empty = rows.is_empty();
        div()
            .flex()
            .flex_col()
            .flex_none()
            .border_b_1()
            .border_color(rgb(t.overlay))
            .child(div().flex().items_center().px_3().pt_2().pb_1().child(widgets::section_header([widgets::section_title(t, "CONVERSATIONS")], [div().child(new)])))
            .child(
                div()
                    .id("conversations")
                    .flex()
                    .flex_col()
                    .max_h(px(150.))
                    .px_1p5()
                    .pb_1p5()
                    .overflow_y_scroll()
                    .when(empty, |list| list.child(div().px_1p5().py_1().text_xs().text_color(rgb(t.muted)).child("None yet: the first question starts one.")))
                    .when(self.advisor.selected().is_none() && !empty, |list| list.child(div().px_1p5().py_1().text_xs().text_color(rgb(t.accent)).child("New conversation")))
                    .children(rows),
            )
    }

    fn render_conversation_row(&self, conversation: &Conversation, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = self.theme;
        let id = conversation.id.as_str();
        let overlay = t.overlay;
        let selected = self.advisor.selected() == Some(id);
        let asking = self.advisor.deleting() == Some(id);
        let (select_id, delete_id) = (id.to_owned(), id.to_owned());
        let on_delete = cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            let requests = this.advisor.delete(&delete_id);
            this.ask_advisor(requests);
            cx.notify();
        });
        let button_id = SharedString::from(format!("conversation-delete-{id}"));
        let delete = if asking {
            div()
                .id(button_id)
                .flex_none()
                .px_1p5()
                .py_0p5()
                .rounded_md()
                .bg(rgb(t.error))
                .text_color(rgb(t.background))
                .text_xs()
                .font_weight(gpui::FontWeight::BOLD)
                .cursor_pointer()
                .on_click(on_delete)
                .child("Delete?")
                .into_any_element()
        } else {
            div()
                .flex_none()
                .invisible()
                .group_hover(CONVERSATION_ROW, |style| style.visible())
                .child(widgets::icon_button(t, button_id, widgets::icon(IconName::X, t.subtle)).on_click(on_delete))
                .into_any_element()
        };
        div()
            .id(SharedString::from(format!("conversation-{id}")))
            .group(CONVERSATION_ROW)
            .flex()
            .items_center()
            .gap_1p5()
            .px_1p5()
            .py_0p5()
            .rounded_md()
            .cursor_pointer()
            .when(selected, |row| row.bg(rgb(overlay)))
            .hover(move |style| style.bg(rgb(overlay)))
            .on_click(cx.listener(move |this, _, _, cx| {
                let requests = this.advisor.select(&select_id);
                this.ask_advisor(requests);
                cx.notify();
            }))
            .tooltip(widgets::hint(t, vec![conversation.label()]))
            .child(div().flex_1().min_w_0().truncate().text_sm().text_color(rgb(if selected { t.text } else { t.subtle })).child(conversation.title.clone()))
            .children(conversation.time().map(|time| widgets::badge(time, t.subtle, Tone::Soft, t)))
            .child(delete)
    }

    fn render_advisor_messages(&self) -> Div {
        let t = self.theme;
        let entries = &self.advisor.transcript().entries;
        // While a reply is on its way and no text of it has come yet.
        let waiting = self.advisor.replying() && !entries.last().is_some_and(|entry| entry.role == Role::Assistant && !entry.text.is_empty());
        div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("advisor-messages")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .overflow_y_scroll()
                    .track_scroll(&self.advisor_scroll)
                    .when(entries.is_empty() && !waiting, |list| list.child(div().text_xs().text_color(rgb(t.muted)).child("Ask about this run: what to buy, what to keep, what goes together.")))
                    .children(entries.iter().map(|entry| self.render_entry(entry)))
                    .when(waiting, |list| list.child(div().text_xs().text_color(rgb(t.muted)).child("Thinking…"))),
            )
            .child(widgets::scrollbar(t, "advisor-scrollbar", &self.advisor_scroll))
    }

    fn render_entry(&self, entry: &Entry) -> Div {
        let t = self.theme;
        let lines = |text: &str, colour: u32| div().flex().flex_col().text_sm().text_color(rgb(colour)).children(text.split('\n').map(|line| div().min_h(px(14.)).child(line.to_owned())));
        match entry.role {
            Role::User => div().flex().justify_end().child(div().max_w(px(320.)).px_2().py_1p5().rounded_md().bg(rgb(t.overlay)).child(lines(&entry.text, t.text))),
            Role::Assistant => div()
                .flex()
                .flex_col()
                .gap_1()
                .when(!entry.tools.is_empty(), |column| column.child(div().text_xs().text_color(rgb(t.muted)).child(format!("Used {}", entry.tools.join(", ")))))
                .when(!entry.text.is_empty(), |column| column.child(lines(&entry.text, t.text))),
            Role::Error => div().text_xs().text_color(rgb(t.error)).child(entry.text.clone()),
        }
    }

    /// The box: what is typed over several lines (or what it is for), with a caret while it takes
    /// the typing, and a Send button.
    fn render_advisor_input(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let focused = self.advisor_input_focused;
        let input = &self.advisor.input;
        let caret = || div().flex_none().w(px(1.)).h(px(14.)).bg(rgb(t.text));
        let text = if input.is_empty() {
            div().flex().items_center().gap_1().when(focused, |line| line.child(caret())).child(div().text_color(rgb(t.muted)).child("Ask the Advisor… Enter sends, Shift+Enter starts a new line"))
        } else {
            let count = input.split('\n').count();
            div().flex().flex_col().children(input.split('\n').enumerate().map(|(index, line)| {
                div().flex().items_center().min_h(px(16.)).child(line.to_owned()).when(focused && index + 1 == count, |line| line.child(caret()))
            }))
        };
        let can_send = !input.trim().is_empty() && !self.advisor.busy();
        let send = widgets::icon_button(t, "advisor-send", widgets::icon(IconName::SendHorizontal, if can_send { t.accent } else { t.muted }))
            .tooltip(widgets::hint(t, vec!["Send (Enter)".into()]))
            .on_click(cx.listener(|this, _, _, cx| this.send_advisor(cx)));
        div().flex_none().px_3().pb_3().pt_1().child(
            div()
                .id("advisor-input")
                .flex()
                .items_end()
                .gap_1()
                .px_2()
                .py_1p5()
                .rounded_md()
                .border_1()
                .border_color(rgb(if focused { t.accent } else { t.highlight_med }))
                .cursor_text()
                .text_sm()
                .text_color(rgb(t.text))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.advisor_input_focused = true;
                    this.odds_search_focused = false;
                    cx.notify();
                }))
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    if this.advisor_input_focused {
                        this.advisor_input_focused = false;
                        cx.notify();
                    }
                }))
                .child(div().id("advisor-input-text").flex_1().min_w_0().max_h(px(120.)).overflow_y_scroll().child(text))
                .child(send),
        )
    }
}
