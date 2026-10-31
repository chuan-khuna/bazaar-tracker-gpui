# 17: Uploading and the secret key

**What to build:** When run sync is on and a secret key is saved, saved Events are uploaded to Bazaar DB in the background under our own name, with back-off on failure and without ever blocking tracking. Run sync is off by default. See the spec's Uploading and Secret key decisions and `docs/rea/bazaar-tracker/submission-api.md`. What the real server accepts is unknown (feasibility record, open question 3), so this ticket is verified against a fake server only.

**Blocked by:** 14 (`history.db`)

**Status:** resolved

- [x] `history.db` records which Events are sent and which the server rejected for good (a schema migration).
- [x] Sendable Events are taken in batches of up to 200 and about 2,000,000 payload bytes, always at least one.
- [x] The batch is gzip-compressed JSON, sent by `POST` with `Content-Type: application/json`, `Content-Encoding: gzip`, `Authorization: Bearer <key>` and `User-Agent: BazaarTracker/<version>`.
- [x] Failures back off; accepted Events are marked sent; permanently rejected Events are set aside.
- [x] Nothing is sent while run sync is off or no key is saved; run sync defaults to off.
- [x] The endpoint is configurable.
- [x] The secret key is stored in the Windows credential store under `BazaarTracker` in release builds and `bazaar-tracker-dev` in debug builds.
- [x] Tests (upload seam): against a fake HTTP server, covering the caps, the body and headers, back-off, marking sent, and sync off or no key.

## Comments

Implemented by an agent; verified against a fake HTTP server only, as the ticket says.

- `upload::Uploader` (`Uploader::new(history, key_source, config)`, then `set_enabled(true)`; `drain_once(now_ms)` is one pass and `start()` runs it on a thread with an `UploadHandle` for `nudge`, `restart_backoff` and `stop`). It reports `UploadNotice`s (sent, refused with the server's message, an event set aside, failed) over a channel. Run sync starts off, and nothing is sent without a non-blank key.
- `history.db` migration 2 adds `events.upload_status` (0 waiting, 1 sent, 3 refused for good); `History::waiting_events`, `set_upload_status` and `upload_status` use it.
- Batches: up to 200 Events and about 2,000,000 bytes of Event JSON, stopping at the first Event that would go over, and always at least one. The body is gzip-compressed JSON `POST`ed with `Content-Type: application/json`, `Content-Encoding: gzip`, `Authorization: Bearer <key>` and `User-Agent: BazaarTracker/<version>`.
- Failures: any status other than 2xx and 413, a network error, or a timeout of a single Event backs off, leaves the Events waiting, and the loop waits 15 s doubling up to one hour. A `413` or a timeout on a batch of more than one splits it in two and tries each half (if the first half has to back off the second is not tried); a `413` on a single Event sets it aside for good. A nudge wakes an idle uploader but does not cut a back-off short; a new key or turning sync on does (`restart_backoff`).
- The endpoint is `UploadConfig::endpoint`; its default is `BAZAAR_TRACKER_UPLOAD_ENDPOINT` if set, otherwise the original's URL `https://adira.bzdb.network/api/tracker-v2/runs/batch`. Sync is off by default, so nothing goes there until the player turns it on. Ask the Bazaar DB team (feasibility record, open question 3) before doing so.
- Secret key: `upload::SecretStore` over `keyring` (Windows Credential Manager), entry `BazaarTracker` in release builds and `bazaar-tracker-dev` in debug builds. Its test saves, replaces and clears a key in the real credential store under a throwaway entry name (`bazaar-tracker-test-<pid>`) and removes it afterwards.
- HTTP uses `ureq` (blocking, no async runtime), behind a `Transport` trait; `flate2` for gzip.

Ambiguities resolved:
- The spec's sample `settings.toml` shows `run_sync = true`, but the spec's Uploading decision, user story 71 and this ticket all say run sync is off by default. I took off as right and changed the app's `Settings` default (and its tests) from `true` to `false`; the settings overlay is a shell in this spec, so nothing else changes.
- The ticket does not say when an Event becomes sendable. I kept the original's rule (`submission-api.md` §3.1): its Run has a real id (not `local-`) and has ended or has had no Event for six hours, so a run in progress is not uploaded in pieces. Easy to drop if Bazaar DB wants live uploads.
- The body format is our own (`schema_version`, `client`, `client_version`, `runs`, `events`, with our snake_case Event JSON), since the server's contract is unknown. It is likely to need changing when Bazaar DB answers.
- The tracker service (ticket 15) is not wired to the uploader: the uploader reads `history.db` itself every 15 s (or when nudged), so tracking never waits for it.
