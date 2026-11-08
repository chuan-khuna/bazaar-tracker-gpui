//! The Advisor tab's side of the Build Advisor, with no drawing: the HTTP client that talks to
//! `just advisor serve` on a background thread (`client`), the SSE lines it parses into events
//! (`sse`), the situation sent with every message (`situation`), an Event attached to the next one
//! (`attachment`) and the Context row that shows it
//! under each question (`context`), the Conversations (`conversation`),
//! the open one's questions and replies built from saved messages and events (`reply`), with the
//! replies' light markdown (`markdown`), `ask_user` cards (`ask_user`), recommendation cards
//! (`recommendation`) and usage lines (`usage`), and what the tab shows and asks for next (`state`).
//!
//! The API and its events are documented in docs/documents/advisor.md ("The backend").

pub mod ask_user;
pub mod attachment;
pub mod client;
pub mod context;
pub mod conversation;
pub mod markdown;
pub mod recommendation;
pub mod reply;
pub mod situation;
pub mod sse;
pub mod state;
pub mod usage;
