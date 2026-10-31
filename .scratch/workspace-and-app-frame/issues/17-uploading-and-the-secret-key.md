# 17: Uploading and the secret key

**What to build:** When run sync is on and a secret key is saved, saved Events are uploaded to Bazaar DB in the background under our own name, with back-off on failure and without ever blocking tracking. Run sync is off by default. See the spec's Uploading and Secret key decisions and `docs/rea/bazaar-tracker/submission-api.md`. What the real server accepts is unknown (feasibility record, open question 3), so this ticket is verified against a fake server only.

**Blocked by:** 14 (`history.db`)

**Status:** ready-for-agent

- [ ] `history.db` records which Events are sent and which the server rejected for good (a schema migration).
- [ ] Sendable Events are taken in batches of up to 200 and about 2,000,000 payload bytes, always at least one.
- [ ] The batch is gzip-compressed JSON, sent by `POST` with `Content-Type: application/json`, `Content-Encoding: gzip`, `Authorization: Bearer <key>` and `User-Agent: BazaarTracker/<version>`.
- [ ] Failures back off; accepted Events are marked sent; permanently rejected Events are set aside.
- [ ] Nothing is sent while run sync is off or no key is saved; run sync defaults to off.
- [ ] The endpoint is configurable.
- [ ] The secret key is stored in the Windows credential store under `BazaarTracker` in release builds and `bazaar-tracker-dev` in debug builds.
- [ ] Tests (upload seam): against a fake HTTP server, covering the caps, the body and headers, back-off, marking sent, and sync off or no key.
