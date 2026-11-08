//! The Advisor tab's side of the Build Advisor, with no drawing: the HTTP client that talks to
//! `just advisor serve` on a background thread (`client`), the SSE lines it parses into events
//! (`sse`), the situation sent with every message (`situation`), the Conversations and the text of
//! the selected one (`conversation`), and what the tab shows and asks for next (`state`).
//!
//! The API and its events are documented in docs/documents/advisor.md ("The backend").

pub mod client;
pub mod conversation;
pub mod situation;
pub mod sse;
pub mod state;
