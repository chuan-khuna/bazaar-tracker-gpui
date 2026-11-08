//! The Advisor tab of the detail island, the chat icons that open it, and the glue between the
//! tab's state (`tracking::advisor::state`) and the client's threads (`tracking::advisor::client`).
//!
//! The tab follows the selected Run: its Conversations newest first, the chosen one's messages,
//! and the box to ask in. Each message is sent with the situation now (the latest board read for
//! the Live run, else the Run's last Snapshot; or, with an Event attached from the Run tab's Event
//! table and shown as a chip in the box, the situation at that Event), shown under the question as a collapsed Context row
//! (`tracking::advisor::context`) that opens to the situation drawn as the Run tab draws a Snapshot,
//! with its raw JSON a toggle away. A reply is drawn from `tracking::advisor::reply`
//! in full: thinking, tool lines, light markdown, the recommendation card (its rows, bars, the
//! source runs on hover and expanded in the card, and the source line), the Advisor's question,
//! follow-up buttons and the usage line. Under the box, the model picker's button shows the model
//! the next message goes to; it opens the picker (`overlays/model_picker.rs`) drawn here too.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, Bounds, ClickEvent, Context, Div, Pixels, FontStyle, FontWeight, HighlightStyle, SharedString, Stateful, StyledText, Transformation, div, percentage, prelude::*, px, rgb,
};

use crate::tracking::advisor::ask_user::AskUser;
use crate::tracking::advisor::attachment::Attachment;
use crate::tracking::advisor::client::Update;
use crate::tracking::advisor::context::{self, ContextView, OfferCard, Sent};
use crate::tracking::advisor::conversation::Conversation;
use crate::tracking::advisor::markdown::{self, LineKind};
use crate::tracking::advisor::reply::{End, Piece, Question, Reply, Thinking, ToolCall, ToolStatus, Turn};
use crate::tracking::advisor::situation;
use crate::overlays::Overlay;
use crate::overlays::model_picker::{self, ModelPicker, Row};
use crate::tracking::advisor::state::{BACKEND_MODEL, ChatIcon, DetailTab, LIST_ROWS, Request, icon_run};
use crate::tracking::advisor::usage::Stats;
use crate::tracking::run::board::Grid;
use crate::ui::TrackerApp;
use crate::ui::run::board::tier_colour;
use crate::ui::widgets::{self, IconName, Tone};

/// The group name of a Conversation's row, so that its ✕ shows while the pointer is over it.
const CONVERSATION_ROW: &str = "conversation-row";

/// A Conversation row's line: the height of its ✕ button, so every row is as tall.
const CONVERSATION_LINE_HEIGHT: f32 = 22.;

/// A Conversation row with its padding: the open list is [`LIST_ROWS`] of these tall.
const CONVERSATION_ROW_HEIGHT: f32 = CONVERSATION_LINE_HEIGHT + 4.;

/// How long a message may be, in characters.
const INPUT_MAX: usize = 4000;

/// The font of inline code and of a tool line's JSON.
use widgets::MONO;

/// How often the tab is drawn again while a reply runs, so `Thinking… Ns` counts.
const TICK: Duration = Duration::from_millis(250);

/// The width of a mini Board in a Context row.
const CONTEXT_BOARD: f32 = 280.;

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
            Update::Created { run_id, text, situation, model, result } => self.advisor.on_created(&run_id, text, situation, model, result),
            Update::Models { result } => {
                self.advisor.on_models(result);
                Vec::new()
            }
            Update::Messages { conversation_id, result } => {
                self.advisor.on_messages(&conversation_id, result);
                self.advisor_scroll.scroll_to_bottom();
                Vec::new()
            }
            Update::Event { conversation_id, event } => {
                self.advisor.on_event(&conversation_id, &event, Instant::now());
                self.advisor_scroll.scroll_to_bottom();
                Vec::new()
            }
            Update::Ended { conversation_id, result } => {
                self.advisor.on_ended(&conversation_id, result, Instant::now());
                Vec::new()
            }
            Update::Deleted { conversation_id, result } => self.advisor.on_deleted(&conversation_id, result),
        };
        self.ask_advisor(requests);
        self.tick_advisor(cx);
        cx.notify();
    }

    /// Open the model picker, and ask for the list again. `for_default`: opened from Settings, so
    /// choosing sets the default.
    pub(in crate::ui) fn open_model_picker(&mut self, for_default: bool, cx: &mut Context<Self>) {
        self.advisor_input_focused = false;
        self.advisor_ask_focused = false;
        let requests = self.advisor.open_picker();
        self.ask_advisor(requests);
        self.open(Overlay::ModelPicker(ModelPicker::new(for_default)), cx);
    }

    /// Use `model` for the next messages (`None`: the backend's own), and with `default` save it
    /// as the default model too. The picker closes; opened from Settings, Settings opens again.
    pub(in crate::ui) fn choose_advisor_model(&mut self, model: Option<String>, default: bool, cx: &mut Context<Self>) {
        let from_settings = matches!(&self.overlay, Some(Overlay::ModelPicker(picker)) if picker.for_default);
        self.advisor.choose_model(model.as_deref());
        if default {
            self.change_settings(|settings| settings.advisor_model = model.unwrap_or_default(), cx);
        }
        if from_settings {
            self.open(Overlay::Settings(Default::default()), cx);
        } else {
            self.dismiss(cx);
            self.advisor_input_focused = true;
        }
        cx.notify();
    }

    /// While a reply runs, draw the tab again every `TICK`, so the thinking time counts up.
    fn tick_advisor(&mut self, cx: &mut Context<Self>) {
        if self.advisor_ticking || !self.advisor.replying() {
            return;
        }
        self.advisor_ticking = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let going = this
                    .update(cx, |this, cx| {
                        let going = this.advisor.replying();
                        this.advisor_ticking = going;
                        if going && this.detail_open && this.detail_tab == DetailTab::Advisor {
                            cx.notify();
                        }
                        going
                    })
                    .unwrap_or(false);
                if !going {
                    break;
                }
            }
        })
        .detach();
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
        self.advisor_ask_focused = false;
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

    /// An Event row's chat icon: open the Advisor tab on a new Conversation of the selected Run,
    /// with that Event attached to the first message.
    pub(in crate::ui) fn open_advisor_event(&mut self, event_id: &str, cx: &mut Context<Self>) {
        let Some(run_id) = self.session.selected().map(str::to_owned) else { return };
        let Some(attachment) = Attachment::new(&run_id, self.session.events(), event_id, &self.names) else { return };
        let requests = self.advisor.open_event(attachment);
        self.ask_advisor(requests);
        self.detail_open = true;
        self.detail_tab = DetailTab::Advisor;
        self.advisor_input_focused = true;
        self.advisor_ask_focused = false;
        self.odds_search_focused = false;
        cx.notify();
    }

    /// The situation to send with a message about the Advisor's Run: at the attached Event when
    /// there is one, else now.
    fn advisor_situation(&self) -> serde_json::Value {
        let Some(run_id) = self.advisor.run_id() else { return serde_json::Value::Null };
        let Some(run) = self.session.run(run_id) else { return serde_json::Value::Null };
        // The Advisor follows the selected Run, whose Events are loaded.
        let events = if self.session.selected() == Some(run_id) { self.session.events() } else { &[] };
        if let Some(attached) = self.advisor.attached().filter(|attached| attached.run_id == run_id)
            && let Some(at_event) = situation::for_event(run, events, &attached.event_id, &self.names)
        {
            return at_event.to_json();
        }
        situation::for_run(run, self.session.is_live(run_id), self.live.board(), events, &self.names).to_json()
    }

    fn send_advisor(&mut self, cx: &mut Context<Self>) {
        let situation = self.advisor_situation();
        let requests = self.advisor.send(situation);
        self.sent_advisor(requests, cx);
    }

    /// After a message was handed to the state: run its requests and follow the reply down.
    fn sent_advisor(&mut self, requests: Vec<Request>, cx: &mut Context<Self>) {
        if !requests.is_empty() {
            self.advisor_scroll.scroll_to_bottom();
        }
        self.ask_advisor(requests);
        self.tick_advisor(cx);
        cx.notify();
    }

    /// Stop: close the reply's stream, keeping what arrived.
    fn stop_advisor(&mut self, cx: &mut Context<Self>) {
        let requests = self.advisor.stop(Instant::now());
        self.ask_advisor(requests);
        cx.notify();
    }

    /// A follow-up button: send its question.
    fn send_follow_up(&mut self, question: &str, cx: &mut Context<Self>) {
        let situation = self.advisor_situation();
        let requests = self.advisor.follow_up(question, situation);
        self.sent_advisor(requests, cx);
    }

    /// The `ask_user` card's Send.
    fn send_answer(&mut self, call_id: &str, cx: &mut Context<Self>) {
        let situation = self.advisor_situation();
        let requests = self.advisor.answer(call_id, situation);
        if !requests.is_empty() {
            self.advisor_ask_focused = false;
        }
        self.sent_advisor(requests, cx);
    }

    /// A key while the box (or the `ask_user` card's text box) takes the typing: Enter sends,
    /// Shift+Enter starts a new line in the box, Escape lets go.
    pub(in crate::ui) fn advisor_key(&mut self, key: &str, typed: Option<&str>, shift: bool, cx: &mut Context<Self>) {
        if self.advisor_ask_focused {
            self.ask_key(key, typed, cx);
            return;
        }
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

    /// A key while the `ask_user` card's text box takes the typing: one line, Enter sends the answer.
    fn ask_key(&mut self, key: &str, typed: Option<&str>, cx: &mut Context<Self>) {
        match key {
            "enter" => {
                if let Some((call_id, _)) = self.advisor.transcript().waiting_question() {
                    self.send_answer(&call_id, cx);
                }
            }
            "escape" => self.advisor_ask_focused = false,
            "backspace" => {
                let mut text = self.advisor.ask_text().to_owned();
                text.pop();
                self.advisor.set_ask_text(text);
            }
            _ => {
                if let Some(typed) = typed.filter(|typed| !typed.chars().any(char::is_control)) {
                    self.type_advisor(typed);
                }
            }
        }
        cx.notify();
    }

    /// Add `text` to the box, up to `INPUT_MAX` characters. Pasted line endings become `\n`. While
    /// the `ask_user` card's text box takes the typing, it goes there on one line.
    pub(in crate::ui) fn type_advisor(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if self.advisor_ask_focused {
            let mut answer = self.advisor.ask_text().to_owned();
            let room = INPUT_MAX.saturating_sub(answer.chars().count());
            answer.extend(text.chars().map(|c| if c == '\n' { ' ' } else { c }).filter(|c| !c.is_control()).take(room));
            self.advisor.set_ask_text(answer);
            return;
        }
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
                let command = self.advisor.status_command().map(|command| {
                    let id = SharedString::from("copy-serve-command");
                    let tip = if self.just_copied(&id) { "Copied" } else { "Copy" };
                    let button = widgets::copy_button(t, id.clone(), self.just_copied(&id)).tooltip(widgets::hint(t, vec![tip.into()])).on_click(cx.listener(move |this, _, _, cx| {
                        this.copy_text(id.clone(), command, cx);
                    }));
                    div().flex().items_center().gap_0p5().child(widgets::inline_code(t, command)).child(button)
                });
                div()
                    .flex_none()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .mx_3()
                    .my_2()
                    .px_2()
                    .py_1p5()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(t.error))
                    .text_xs()
                    .text_color(rgb(t.error))
                    .child(status)
                    .children(command)
            }))
            .child(self.render_advisor_messages(cx))
            .child(self.render_advisor_input(cx))
    }

    fn render_conversation_list(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let new = widgets::icon_button(t, "conversation-new", widgets::icon(IconName::Plus, t.subtle))
            .tooltip(widgets::hint(t, vec!["New conversation".into()]))
            .on_click(cx.listener(|this, _, _, cx| {
                // The header around it folds the list; `+` only starts a new Conversation.
                cx.stop_propagation();
                this.advisor.new_conversation();
                this.advisor_input_focused = true;
                cx.notify();
            }));
        let collapsed = self.advisor.list_collapsed();
        let header = div()
            .id("conversations-heading")
            .flex()
            .items_center()
            .gap_1p5()
            .min_h(px(widgets::BADGE_HEIGHT))
            .px_3()
            .pt_2()
            .pb_1()
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
                this.advisor.toggle_list();
                cx.notify();
            }))
            .child(widgets::section_title(t, if collapsed { "▸ CONVERSATIONS" } else { "▾ CONVERSATIONS" }))
            .child(div().flex_1().min_w_0().truncate().text_xs().text_color(rgb(t.subtle)).children(self.advisor.list_summary().map(|summary| format!("· {summary}"))))
            .child(div().flex_none().child(new));
        let list = div().flex().flex_col().flex_none().border_b_1().border_color(rgb(t.overlay)).child(header);
        if collapsed {
            self.conversations_scrolled.replace(None);
            return list;
        }
        // Scroll the shown Conversation into view once, when the list opens or it changes.
        let target = self.advisor.scroll_target();
        if target.map(|(_, id)| id) != self.conversations_scrolled.borrow().as_deref() {
            if let Some((index, _)) = target {
                self.conversations_scroll.scroll_to_item(index);
            }
            self.conversations_scrolled.replace(target.map(|(_, id)| id.to_owned()));
        }
        let rows: Vec<_> = self.advisor.conversations().iter().map(|conversation| self.render_conversation_row(conversation, cx)).collect();
        let empty = rows.is_empty();
        list.child(
            div()
                .relative()
                .child(
                    div()
                        .id("conversations")
                        .flex()
                        .flex_col()
                        .max_h(px(LIST_ROWS as f32 * CONVERSATION_ROW_HEIGHT + 6.))
                        .px_1p5()
                        .pb_1p5()
                        .overflow_y_scroll()
                        .track_scroll(&self.conversations_scroll)
                        .when(empty, |list| list.child(div().px_1p5().py_1().text_xs().text_color(rgb(t.muted)).child("None yet: the first question starts one.")))
                        .when(self.advisor.selected().is_none() && !empty, |list| list.child(div().px_1p5().py_1().text_xs().text_color(rgb(t.accent)).child("New conversation")))
                        .children(rows),
                )
                .child(widgets::scrollbar(t, "conversations-scrollbar", &self.conversations_scroll)),
        )
    }

    fn render_conversation_row(&self, conversation: &Conversation, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = self.theme;
        let id = conversation.id.as_str();
        let overlay = t.overlay;
        let selected = self.advisor.selected() == Some(id);
        // While the Conversation asks to be deleted, a strip under it holds the question and its
        // buttons, and the ✕ hides.
        let asking = self.advisor.deleting() == Some(id);
        let select_id = id.to_owned();
        let delete = if asking { None } else { Some({
            let ask_id = id.to_owned();
            let button = widgets::icon_button(t, SharedString::from(format!("conversation-delete-{id}")), widgets::icon(IconName::X, t.subtle))
                .tooltip(widgets::hint(t, vec!["Delete this conversation".into()]))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.advisor.ask_delete(&ask_id);
                    cx.notify();
                }));
            div().flex_none().invisible().group_hover(CONVERSATION_ROW, |style| style.visible()).child(button)
        }) };
        let strip = if !asking { None } else { Some({
            let delete_id = id.to_owned();
            let cancel = widgets::text_button(t, SharedString::from(format!("conversation-delete-cancel-{id}")), "Cancel", t.highlight_med, true).on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.advisor.cancel_delete();
                cx.notify();
            }));
            let delete = widgets::delete_button(t, SharedString::from(format!("conversation-delete-confirm-{id}"))).on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                let requests = this.advisor.delete(&delete_id);
                this.ask_advisor(requests);
                cx.notify();
            }));
            widgets::delete_strip(t, SharedString::from(format!("conversation-delete-strip-{id}")), "Delete this Conversation? Its messages are deleted too.", cancel, delete).mb_1()
        }) };
        div()
            .id(SharedString::from(format!("conversation-{id}")))
            .group(CONVERSATION_ROW)
            .flex()
            .flex_col()
            .px_1p5()
            .py_0p5()
            .rounded_md()
            .cursor_pointer()
            .when(selected || asking, |row| row.bg(rgb(overlay)))
            .hover(move |style| style.bg(rgb(overlay)))
            .on_click(cx.listener(move |this, _, _, cx| {
                let requests = this.advisor.select(&select_id);
                this.ask_advisor(requests);
                cx.notify();
            }))
            .child(
                div()
                    .id(SharedString::from(format!("conversation-line-{id}")))
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .h(px(CONVERSATION_LINE_HEIGHT))
                    .tooltip(widgets::hint(t, vec![conversation.label()]))
                    .child(div().flex_1().min_w_0().truncate().text_sm().text_color(rgb(if selected { t.text } else { t.subtle })).child(conversation.title.clone()))
                    .children(conversation.time().map(|time| widgets::badge(time, t.subtle, Tone::Soft, t)))
                    .children(delete),
            )
            .children(strip)
    }

    fn render_advisor_messages(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let turns = &self.advisor.transcript().turns;
        let now = Instant::now();
        let last = turns.len().saturating_sub(1);
        // While a reply is on its way and nothing of it shows it at work.
        let working = self.advisor.replying()
            && match turns.last() {
                Some(Turn::Reply(reply)) => reply.end == End::Streaming && !reply.busy() && reply.pieces().is_empty(),
                _ => true,
            };
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
                    .gap_3()
                    .px_3()
                    .py_2()
                    .overflow_y_scroll()
                    .track_scroll(&self.advisor_scroll)
                    .when(self.advisor.transcript().is_empty() && !working, |list| list.child(div().text_xs().text_color(rgb(t.muted)).child("Ask about this run: what to buy, what to keep, what goes together.")))
                    .children(turns.iter().enumerate().map(|(index, turn)| match turn {
                        Turn::User(question) => self.render_question(index, question, cx).into_any_element(),
                        Turn::Reply(reply) => self.render_reply(index, reply, index == last, now, cx).into_any_element(),
                    }))
                    .when(working, |list| list.child(div().flex().items_center().gap_1p5().text_xs().text_color(rgb(t.muted)).child(spinner(t.muted, "advisor-working")).child("Working…"))),
            )
            .child(widgets::scrollbar(t, "advisor-scrollbar", &self.advisor_scroll))
    }

    /// The player's message on the right, and under it the collapsed Context row of the situation
    /// sent with it.
    fn render_question(&self, turn: usize, question: &Question, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        div()
            .flex()
            .flex_col()
            .items_end()
            .gap_1()
            .child(div().max_w(px(320.)).px_2().py_1p5().rounded_md().bg(rgb(t.overlay)).child(plain_lines(&question.text, t.text)))
            .child(self.render_context(turn, &context::view(question.situation.as_ref(), &self.names), cx))
    }

    /// `Context · D14 H6 · 10/10 slots`, collapsed; open, the situation as sent: the player's
    /// numbers, the Board, Stash and skills as the Run tab draws them (mini Boards), the Offer, the
    /// opponent, and a JSON toggle with a copy button.
    fn render_context(&self, turn: usize, view: &ContextView, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let key = format!("{turn}.context");
        let header = self.render_detail_header(&key, widgets::icon(IconName::Paperclip, t.muted).into_any_element(), view.summary.clone(), t.muted, cx).justify_end();
        // Built here, while open, as its JSON toggle needs `cx` too.
        let body = self.advisor.is_open(&key).then(|| {
            let body = div().w_full().flex().flex_col().gap_2().p_2().rounded_md().border_1().border_color(rgb(t.highlight_med)).text_xs();
            match &view.sent {
                None => body.child(div().text_color(rgb(t.muted)).child("No context sent")),
                Some(sent) => self.render_sent(&key, sent, body, cx),
            }
        });
        self.render_collapsible(&key, header, move || body.unwrap_or_else(div), cx).w_full()
    }

    /// The situation's parts, in `body`.
    fn render_sent(&self, key: &str, sent: &Sent, body: Div, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let label = |text: &'static str| div().font_weight(FontWeight::BOLD).text_color(rgb(t.muted)).child(text);
        let facts = |facts: &[(&'static str, String)]| {
            div().flex().flex_wrap().gap_x_3().gap_y_0p5().children(facts.iter().map(|(name, value)| div().flex().gap_1().child(div().text_color(rgb(t.muted)).child(*name)).child(div().text_color(rgb(t.text)).child(value.clone()))))
        };
        let grid = |name: &'static str, id: String, grid: &Grid| {
            div().flex().flex_col().gap_0p5().child(label(name)).child(if grid.tiles.is_empty() { div().text_color(rgb(t.muted)).child("empty") } else { self.render_mini_grid(id, grid, CONTEXT_BOARD) })
        };
        let json_key = format!("{key}.json");
        let json_open = self.advisor.is_open(&json_key);
        let toggle = div()
            .id(SharedString::from(format!("context-json-{key}")))
            .flex()
            .items_center()
            .gap_1()
            .cursor_pointer()
            .text_color(rgb(if json_open { t.text } else { t.subtle }))
            .tooltip(widgets::hint(t, vec![if json_open { "Hide the JSON sent" } else { "Show the JSON sent" }.into()]))
            .on_click(cx.listener(move |this, _, _, cx| {
                // Not the body's double-click: two quick clicks here toggle the JSON twice.
                cx.stop_propagation();
                this.advisor.toggle(&json_key);
                cx.notify();
            }))
            .child(widgets::icon(IconName::Braces, t.muted))
            .child("JSON");
        let json = json_open.then(|| {
            let id = SharedString::from(format!("copy-context-{key}"));
            let copied = self.just_copied(&id);
            let text = sent.json.clone();
            let copy = widgets::copy_button(t, id.clone(), copied).tooltip(widgets::hint(t, vec![if copied { "Copied" } else { "Copy" }.into()])).on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.copy_text(id.clone(), &text, cx);
            }));
            div().relative().px_2().py_1().rounded(px(4.)).bg(rgb(t.highlight_low)).font_family(MONO).child(plain_lines(&sent.json, t.subtle)).child(div().absolute().top_0p5().right_0p5().child(copy))
        });
        body.child(facts(&sent.facts))
            .children(sent.board.as_ref().map(|board| grid("BOARD", format!("context-board-{key}"), board)))
            .children(sent.stash.as_ref().map(|stash| grid("STASH", format!("context-stash-{key}"), stash)))
            .children(sent.skills.as_ref().map(|skills| self.render_skills(&format!("context-{key}"), skills)))
            .children(sent.offer.as_ref().map(|offer| {
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(label("OFFER"))
                    .when(offer.is_empty(), |list| list.child(div().text_color(rgb(t.muted)).child("nothing")))
                    .children(offer.iter().map(|card| self.render_offer_card(card)))
            }))
            .children(sent.opponent.as_ref().map(|opponent| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(label("OPPONENT"))
                    .child(facts(&opponent.facts))
                    .child(if opponent.board.tiles.is_empty() { div().text_color(rgb(t.muted)).child("no board") } else { self.render_mini_grid(format!("context-opponent-{key}"), &opponent.board, CONTEXT_BOARD) })
                    .when(!opponent.skills.is_empty(), |them| them.child(self.render_skills(&format!("context-opponent-{key}"), &opponent.skills)))
            }))
            .child(div().flex().child(toggle))
            .children(json)
    }

    /// A card on offer: a small square of its art in its tier's border, its name and its kind.
    fn render_offer_card(&self, card: &OfferCard) -> Div {
        let t = self.theme;
        div()
            .flex()
            .items_center()
            .gap_1p5()
            .child(
                div()
                    .flex_none()
                    .size(px(18.))
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(tier_colour(t, card.tier)))
                    .bg(rgb(t.highlight_low))
                    .overflow_hidden()
                    .children(card.art.clone().map(|art| gpui::img(art).size_full().object_fit(gpui::ObjectFit::Cover))),
            )
            .child(div().text_color(rgb(t.text)).child(card.name.clone()))
            .when(!card.kind.is_empty(), |row| row.child(div().text_color(rgb(t.muted)).child(card.kind.clone())))
    }

    /// One reply in full: how it ended, its pieces in order, its error, the follow-up buttons
    /// (on the newest reply only) and the usage line.
    fn render_reply(&self, turn: usize, reply: &Reply, newest: bool, now: Instant, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let follow_ups = if newest && reply.end == End::Done { reply.follow_ups() } else { Vec::new() };
        let answer = self.advisor.transcript().answer_after(turn).map(str::to_owned);
        div()
            .flex()
            .flex_col()
            .gap_1p5()
            .children(match &reply.end {
                End::Stopped => Some(widgets::badge("Stopped", t.muted, Tone::Soft, t)),
                End::Failed(_) => Some(widgets::badge("Failed", t.error, Tone::Soft, t)),
                End::Streaming | End::Done => None,
            })
            .children(reply.pieces().into_iter().map(|piece| match piece {
                Piece::Thinking { key, thinking } => self.render_thinking(format!("{turn}.{key}"), thinking, now, cx).into_any_element(),
                Piece::Tool { key, call } => self.render_tool(format!("{turn}.{key}"), call, cx).into_any_element(),
                Piece::Text(text) => render_markdown(t, text).into_any_element(),
                Piece::Suggest { key, card } => self.render_recommendation(format!("{turn}.{key}"), &card, cx).into_any_element(),
                Piece::AskUser { call_id, ask } => self.render_ask_user(call_id, &ask, answer.as_deref(), cx).into_any_element(),
            }))
            .children(match &reply.end {
                End::Failed(error) => Some(div().text_xs().text_color(rgb(t.error)).child(error.clone())),
                _ => None,
            })
            .when(!follow_ups.is_empty(), |column| {
                column.child(div().flex().flex_col().items_start().gap_1().pt_1().children(follow_ups.into_iter().enumerate().map(|(index, question)| {
                    let overlay = t.overlay;
                    let busy = self.advisor.busy() || self.advisor.replying();
                    let sent = question.clone();
                    div()
                        .id(SharedString::from(format!("follow-up-{turn}-{index}")))
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .max_w_full()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(t.highlight_med))
                        .text_xs()
                        .text_color(rgb(if busy { t.muted } else { t.subtle }))
                        .when(!busy, |button| button.cursor_pointer().hover(move |style| style.bg(rgb(overlay))).on_click(cx.listener(move |this, _, _, cx| this.send_follow_up(&sent, cx))))
                        .child(widgets::icon(IconName::CornerDownRight, t.muted))
                        .child(question)
                })))
            })
            .children(reply.stats.as_ref().map(|stats| render_usage(t, turn, stats)))
    }

    /// The header of a thinking block or tool line: a chevron, an icon and the label; a click opens it.
    fn render_detail_header(&self, key: &str, icon: AnyElement, label: String, colour: u32, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = self.theme;
        let open = self.advisor.is_open(key);
        let toggled = key.to_owned();
        let tip = if open { vec!["Collapse".into(), "Or double-click the text".into()] } else { vec!["Expand".into()] };
        div()
            .id(SharedString::from(format!("detail-{key}")))
            .flex()
            .items_center()
            .gap_1()
            .cursor_pointer()
            .text_xs()
            .text_color(rgb(colour))
            .tooltip(widgets::hint(t, tip))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.advisor.toggle(&toggled);
                cx.notify();
            }))
            .child(widgets::icon(if open { IconName::ChevronDown } else { IconName::ChevronRight }, t.muted))
            .child(icon)
            .child(label)
    }

    /// A thinking block or tool line: its header, and its body while open. Double-clicking the
    /// body collapses it (a single click does nothing); if the header is then above the messages'
    /// visible area, they scroll so it shows.
    fn render_collapsible(&self, key: &str, header: Stateful<Div>, body: impl FnOnce() -> Div, cx: &mut Context<Self>) -> Div {
        // Where the header was last painted, in window coordinates.
        let header_bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::default();
        let painted = header_bounds.clone();
        let collapsed = key.to_owned();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .on_children_prepainted(move |bounds, _, _| painted.set(bounds.first().copied()))
            .child(header)
            .when(self.advisor.is_open(key), |block| {
                block.child(div().id(SharedString::from(format!("detail-body-{key}"))).child(body()).on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    if event.click_count() != 2 || !this.advisor.collapse(&collapsed) {
                        return;
                    }
                    // Collapsing only shrinks what is below the header, so it stays where it is.
                    if let Some(header) = header_bounds.get() {
                        let viewport = this.advisor_scroll.bounds();
                        if header.top() < viewport.top() {
                            let mut offset = this.advisor_scroll.offset();
                            offset.y += viewport.top() - header.top() + px(8.);
                            this.advisor_scroll.set_offset(offset);
                        }
                    }
                    cx.notify();
                })))
            })
    }

    /// `Thinking… Ns` while it streams, then `Thought for Ns`; collapsed, opening to the text.
    fn render_thinking(&self, key: String, thinking: &Thinking, now: Instant, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let icon = if thinking.streaming() { spinner(t.muted, SharedString::from(format!("thinking-{key}"))).into_any_element() } else { widgets::icon(IconName::Brain, t.muted).into_any_element() };
        let header = self.render_detail_header(&key, icon, thinking.label(now), t.muted, cx);
        self.render_collapsible(&key, header, || div().ml(px(18.)).pl_2().border_l_1().border_color(rgb(t.highlight_med)).text_xs().child(plain_lines(&thinking.text, t.subtle)), cx)
    }

    /// A tool line: a spinner, ✓ or ✕, and the tool's name; collapsed, opening to its arguments and
    /// its result (or error) as JSON.
    fn render_tool(&self, key: String, call: &ToolCall, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let status = call.status();
        let icon = match status {
            ToolStatus::Running => spinner(t.accent, SharedString::from(format!("tool-{key}"))).into_any_element(),
            ToolStatus::Done => widgets::icon(IconName::Check, t.success).into_any_element(),
            ToolStatus::Failed => widgets::icon(IconName::X, t.error).into_any_element(),
        };
        let name = if call.name.is_empty() { "Tool call".to_owned() } else { call.name.clone() };
        let field = |title: &'static str, value: String, colour: u32| {
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(rgb(t.muted)).child(title))
                .child(div().px_2().py_1().rounded(px(4.)).bg(rgb(t.highlight_low)).text_xs().font_family(MONO).child(plain_lines(&value, colour)))
        };
        let header = self.render_detail_header(&key, icon, name, if status == ToolStatus::Failed { t.error } else { t.subtle }, cx);
        self.render_collapsible(
            &key,
            header,
            || {
                let result = call.result_text().map(|result| if status == ToolStatus::Failed { field("Error", result, t.error) } else { field("Result", result, t.subtle) });
                div().ml(px(18.)).flex().flex_col().gap_1().child(field("Arguments", call.arguments_text(), t.subtle)).children(result)
            },
            cx,
        )
    }

    /// The Advisor's question: a choice per row (radio, or checkbox with several), a text box with
    /// `allowFreeText`, and Send. Once answered it is locked, showing the answer read back.
    fn render_ask_user(&self, call_id: &str, ask: &AskUser, answer: Option<&str>, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let answered = answer.map(|answer| ask.read(answer));
        let draft = self.advisor.draft();
        let mine = draft.call_id == call_id;
        let disabled = answered.is_some() || self.advisor.busy() || self.advisor.replying();
        let overlay = t.overlay;
        let choices = ask.options.iter().map(|option| {
            let chosen = match &answered {
                Some(answered) => answered.picked.contains(&option.id),
                None => mine && draft.picked.contains(&option.id),
            };
            let icon = match (ask.multiple, chosen) {
                (true, true) => IconName::SquareCheck,
                (true, false) => IconName::Square,
                (false, true) => IconName::CircleDot,
                (false, false) => IconName::Circle,
            };
            let (call, id) = (call_id.to_owned(), option.id.clone());
            div()
                .id(SharedString::from(format!("ask-{call_id}-{}", option.id)))
                .flex()
                .items_center()
                .gap_1p5()
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(rgb(if chosen { t.accent } else { t.highlight_med }))
                .text_sm()
                .text_color(rgb(if chosen { t.text } else { t.subtle }))
                .when(answered.is_some() && !chosen, |row| row.opacity(0.4))
                .when(!disabled, |row| {
                    row.cursor_pointer().hover(move |style| style.bg(rgb(overlay))).on_click(cx.listener(move |this, _, _, cx| {
                        this.advisor.ask_pick(&call, &id);
                        cx.notify();
                    }))
                })
                .child(widgets::icon(icon, if chosen { t.accent } else { t.muted }))
                .child(option.label.clone())
        });
        let kind = if ask.multiple { "Multiple choice" } else { "Single choice" };
        let typed = if mine { draft.typed.as_str() } else { "" };
        let none = std::collections::BTreeSet::new();
        let can_send = !disabled && !ask.answer(if mine { &draft.picked } else { &none }, typed).is_empty();
        let focused = self.advisor_ask_focused && !disabled;
        let form = (answered.is_none()).then(|| {
            let call = call_id.to_owned();
            let text = if typed.is_empty() {
                div().flex().items_center().gap_1().when(focused, |line| line.child(widgets::caret(t))).child(div().text_color(rgb(t.muted)).child(if ask.multiple { "Add anything else…" } else { "Or type your own answer…" }))
            } else {
                div().flex().items_center().child(typed.to_owned()).when(focused, |line| line.child(widgets::caret(t)))
            };
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .when(ask.allow_free_text, |form| {
                    form.child(
                        div()
                            .id(SharedString::from(format!("ask-text-{call_id}")))
                            .flex_1()
                            .min_w_0()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(if focused { t.accent } else { t.highlight_med }))
                            .text_sm()
                            .overflow_hidden()
                            .when(!disabled, |field| field.cursor_text())
                            .when(disabled, |field| field.opacity(0.5))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.advisor_ask_focused = true;
                                this.advisor_input_focused = false;
                                this.odds_search_focused = false;
                                cx.notify();
                            }))
                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                if this.advisor_ask_focused {
                                    this.advisor_ask_focused = false;
                                    cx.notify();
                                }
                            }))
                            .child(text),
                    )
                })
                .when(!ask.allow_free_text, |form| form.justify_end())
                .child(
                    div()
                        .id(SharedString::from(format!("ask-send-{call_id}")))
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(if can_send { t.accent } else { t.overlay }))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(if can_send { t.background } else { t.muted }))
                        .when(can_send, |button| button.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| this.send_answer(&call, cx))))
                        .child(widgets::icon(IconName::Send, if can_send { t.background } else { t.muted }))
                        .child("Send"),
                )
        });
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(rgb(t.highlight_med))
            .bg(rgb(t.background))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .text_xs()
                    .text_color(rgb(t.muted))
                    .child(widgets::icon(IconName::MessageCircleQuestionMark, t.accent))
                    .child("Question")
                    .child(widgets::badge(kind, t.accent, Tone::Soft, t)),
            )
            .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(rgb(t.text)).child(ask.question.clone()))
            .child(div().flex().flex_col().gap_1().children(choices))
            .children(answered.as_ref().filter(|answered| !answered.rest.is_empty()).map(|answered| div().text_xs().text_color(rgb(t.subtle)).child(plain_lines(&format!("You answered: {}", answered.rest), t.subtle))))
            .children(form)
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
        // Send becomes Stop while this app's message is on its way.
        let send = if self.advisor.can_stop() {
            widgets::icon_button(t, "advisor-stop", widgets::icon(IconName::CircleStop, t.error)).tooltip(widgets::hint(t, vec!["Stop".into(), "Keeps what has arrived".into()])).on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.stop_advisor(cx);
            }))
        } else {
            let can_send = !input.trim().is_empty() && !self.advisor.busy() && !self.advisor.replying();
            widgets::icon_button(t, "advisor-send", widgets::icon(IconName::SendHorizontal, if can_send { t.accent } else { t.muted }))
                .tooltip(widgets::hint(t, vec!["Send (Enter)".into()]))
                .on_click(cx.listener(|this, _, _, cx| this.send_advisor(cx)))
        };
        // The attached Event, as a chip over the text: ✕ sends the next message with the situation now.
        let chip = self.advisor.attached().map(|attached| {
            let remove = widgets::icon_button(t, "advisor-detach", widgets::icon(IconName::X, t.subtle)).size(px(16.)).tooltip(widgets::hint(t, vec!["Remove".into(), "Send with the situation now".into()])).on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.advisor.detach();
                cx.notify();
            }));
            div()
                .flex()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .min_w_0()
                        .pl_1p5()
                        .pr_0p5()
                        .py_0p5()
                        .rounded(px(4.))
                        .border_1()
                        .border_color(rgb(t.highlight_med))
                        .bg(rgb(t.highlight_low))
                        .text_xs()
                        .text_color(rgb(t.subtle))
                        .child(widgets::icon(IconName::Paperclip, t.muted))
                        .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(attached.label.clone()))
                        .child(remove),
                )
        });
        let text = div().flex().flex_col().gap_1().children(chip).child(text);
        div().flex_none().flex().flex_col().gap_1().px_3().pb_2().pt_1().child(
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
                    this.advisor_ask_focused = false;
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
        .child(self.render_model_button(cx))
    }

    /// Under the box: the model the next message goes to, which opens the model picker. It shows
    /// the saved default even while the backend can't be reached.
    fn render_model_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let overlay = t.overlay;
        div().flex().child(
            div()
                .id("advisor-model")
                .flex()
                .items_center()
                .gap_1()
                .px_1p5()
                .py_0p5()
                .rounded_md()
                .cursor_pointer()
                .text_xs()
                .text_color(rgb(t.subtle))
                .hover(move |style| style.bg(rgb(overlay)))
                .tooltip(widgets::hint(t, vec!["Choose the model".into(), "For the next messages; Shift+Enter in the list also makes it the default".into()]))
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.open_model_picker(false, cx);
                }))
                .child(widgets::icon(IconName::Cpu, t.muted))
                .child(div().max_w(px(320.)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(self.advisor.model_label()))
                .child(widgets::icon(IconName::ChevronDown, t.muted)),
        )
    }

    /// The model picker, near the top of the window like the theme picker: a search field, the
    /// rows around the highlighted one, and what Enter and Shift+Enter do.
    pub(in crate::ui) fn render_model_picker(&self, picker: &ModelPicker, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let rows = model_picker::rows(&picker.query, self.advisor.models(), self.advisor.backend_model());
        let saved = self.settings.get().advisor_model.clone();
        let current = self.advisor.model().map(str::to_owned);
        let shown = model_picker::visible(picker.selected, rows.len());
        let mut list = div().flex().flex_col().p_1();
        for index in shown.clone() {
            let row = rows[index].clone();
            let (label, detail) = match &row {
                Row::Backend(Some(id)) => (format!("{BACKEND_MODEL} ({id})"), String::new()),
                Row::Backend(None) => (BACKEND_MODEL.to_owned(), String::new()),
                Row::Model(option) => (option.id.clone(), if option.name.is_empty() || option.name == option.id { String::new() } else { option.name.clone() }),
                Row::Typed(id) => (format!("Use \"{id}\""), "not in the list".to_owned()),
            };
            let model = row.model().map(str::to_owned);
            let is_default = model.as_deref().unwrap_or_default() == saved;
            let is_current = model == current;
            let default = picker.for_default;
            list = list.child(
                div()
                    .id(gpui::SharedString::from(format!("model-{index}")))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .when(index == picker.selected, |row| row.bg(rgb(t.overlay)))
                    .hover(|style| style.bg(rgb(t.overlay)))
                    .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| this.choose_advisor_model(model.clone(), default || event.modifiers().shift, cx)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(div().text_color(rgb(if is_current { t.accent } else { t.text })).overflow_hidden().whitespace_nowrap().text_ellipsis().child(label))
                            .when(!detail.is_empty(), |cell| cell.child(div().text_xs().text_color(rgb(t.muted)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(detail))),
                    )
                    .when(is_default, |row| row.child(div().flex_none().text_xs().text_color(rgb(t.muted)).child("default"))),
            );
        }
        if rows.is_empty() {
            list = list.child(div().px_2().py_1().text_sm().text_color(rgb(t.muted)).child("No matching models"));
        }
        let footer = if picker.for_default {
            "Enter sets the default model".to_owned()
        } else {
            let count = if rows.len() > shown.len() { format!("{} of {} · ", shown.end, rows.len()) } else { String::new() };
            format!("{count}Enter uses it · Shift+Enter also makes it the default")
        };
        let placeholder = if self.advisor.models().is_empty() { "Type a model id…" } else { "Search models, or type a model id…" };
        div().absolute().top(px(56.)).left_0().right_0().flex().justify_center().child(
            widgets::panel(t, "model-picker")
                .w(px(440.))
                .child(widgets::query_field(t, &picker.query, placeholder))
                .child(list)
                .child(div().px_3().py_1p5().border_t_1().border_color(rgb(t.overlay)).text_xs().text_color(rgb(t.muted)).child(footer)),
        )
    }
}

/// Text kept as typed, one row per line.
fn plain_lines(text: &str, colour: u32) -> Div {
    div().flex().flex_col().text_color(rgb(colour)).children(text.split('\n').map(|line| div().min_h(px(14.)).child(line.to_owned())))
}

/// A spinning loader.
fn spinner(colour: u32, id: impl Into<gpui::ElementId>) -> impl IntoElement {
    widgets::icon(IconName::LoaderCircle, colour).with_animation(id, Animation::new(Duration::from_secs(1)).repeat(), |icon, delta| icon.with_transformation(Transformation::rotate(percentage(delta))))
}

/// The Advisor's text with light markdown: bold, italics, inline code, bullet and numbered lists.
fn render_markdown(t: &crate::theme::Theme, text: &str) -> Div {
    div().flex().flex_col().gap_0p5().text_sm().text_color(rgb(t.text)).children(markdown::parse(text).into_iter().map(|line| {
        let indent = |depth: usize| px(depth as f32 * 14.);
        let styled = styled_line(t, &line);
        match &line.kind {
            LineKind::Plain if line.text.is_empty() => div().h(px(6.)),
            LineKind::Plain => div().child(styled),
            LineKind::Bullet { depth } => div().flex().gap_1p5().pl(indent(*depth)).child(div().flex_none().text_color(rgb(t.muted)).child("•")).child(div().flex_1().min_w_0().child(styled)),
            LineKind::Numbered { depth, number } => div().flex().gap_1p5().pl(indent(*depth)).child(div().flex_none().text_color(rgb(t.muted)).child(format!("{number}."))).child(div().flex_1().min_w_0().child(styled)),
        }
    }))
}

/// A line's text with its bold, italic and code ranges styled.
fn styled_line(t: &crate::theme::Theme, line: &markdown::Line) -> StyledText {
    let highlights: Vec<_> = line
        .runs
        .iter()
        .map(|(range, emphasis)| {
            let style = HighlightStyle {
                font_weight: emphasis.bold.then_some(FontWeight::BOLD),
                font_style: emphasis.italic.then_some(FontStyle::Italic),
                background_color: emphasis.code.then(|| rgb(t.overlay).into()),
                color: emphasis.code.then(|| rgb(t.accent).into()),
                ..HighlightStyle::default()
            };
            (range.clone(), style)
        })
        .collect();
    let fonts: Vec<_> = line.runs.iter().filter(|(_, emphasis)| emphasis.code).map(|(range, _)| (range.clone(), SharedString::from(MONO))).collect();
    StyledText::new(line.text.clone()).with_highlights(highlights).with_font_family_overrides(fonts)
}

/// The usage line under a reply: model · time · tokens · cost · tools, the breakdown on hover.
fn render_usage(t: &crate::theme::Theme, turn: usize, stats: &Stats) -> Stateful<Div> {
    let parts = stats.line();
    let icons = [Some(IconName::Cpu), Some(IconName::Clock), None, Some(IconName::Coins), Some(IconName::Wrench)];
    div()
        .id(SharedString::from(format!("usage-{turn}")))
        .flex()
        .flex_wrap()
        .items_center()
        .gap_x_3()
        .gap_y_0p5()
        .pt_0p5()
        .font_family(MONO)
        .text_size(px(11.))
        .text_color(rgb(t.muted))
        .tooltip(widgets::hint(t, stats.breakdown()))
        .children(parts.into_iter().zip(icons).map(|(part, icon)| div().flex().items_center().gap_1().children(icon.map(|icon| widgets::icon(icon, t.muted).size(px(11.)))).child(part)))
}
