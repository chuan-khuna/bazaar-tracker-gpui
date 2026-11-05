# Upload

How the tracker uploads saved Events to Bazaar DB: which Events go, how a batch is built and sent,
how failures back off, and where the secret key lives. The code is in
`crates/bazaar-tracker/src/upload/` (`uploader.rs`, `batch.rs`, `transport.rs`, `secret.rs`) and
`crates/bazaar-tracker/src/history/uploads.rs`. The terms (Run, Event) are in
[GLOSSARY.md](../../GLOSSARY.md). What the original BazaarDBTracker sends is in
[submission-api.md](../rea/bazaar-tracker/submission-api.md).

> **To be removed.** Run data is to stay in the local `history.db` only, so this feature is to be
> deleted rather than turned on. See
> [2026-11-02.001-remove-upload-feature](../records/2026-11-02.001-remove-upload-feature.md).

## Off by default

Nothing is sent unless both hold:

- run sync is on: `Uploader::set_enabled(true)`. A new `Uploader` starts off.
- a secret key is saved, and it is not blank.

Otherwise `drain_once` returns `Drain::Idle` without reading the history.

The app has a `run_sync` setting in `settings.toml`, `false` by default, saved like any other
setting. Nothing in the app reads it or starts an `Uploader` yet: the upload code is built and
tested in the crate, but no window, menu or thread turns it on.

What the real server accepts is not known, so the body format is our own and has not been tried
against the real server.

## Which Events are sendable

An Event is sent once, and only once. `History::waiting_events(limit, stale_before_ms)` returns the
Events that meet every rule, oldest first (by `seq`):

| Rule | Why |
|---|---|
| `upload_status = 0` (waiting) | Sent and set-aside Events are never tried again. |
| The Run's id does not start with `local-` | A Run starts under a `local-` id until the game's real id is known. Once rebound, its Events follow it (`ON UPDATE CASCADE`) and become sendable. |
| The Run has ended (`ended = 1`), **or** its newest Event is at least 6 hours old | A Run still being played waits. A Run abandoned without an end goes after six hours of quiet (`stale_after`). |

`stale_before_ms` is `now_ms - 6 h`. Examples from the tests:

- A Run whose last Event was an hour ago sends nothing; six hours later both its Events go.
- An ended Run and a live one: only the ended Run's Events go.
- An ended Run under `local-abc` sends nothing; after `rebind_run("local-abc", "real-1")` both Events
  go, and the batch names Run `real-1`.

## A batch

| Limit | Value | Name |
|---|---|---|
| Events in one batch | 200 | `MAX_BATCH_EVENTS` |
| Event JSON in one batch, before compression | about 2,000,000 bytes | `MAX_BATCH_PAYLOAD_BYTES` |

`waiting_events` takes up to 200; `limit_by_size` then keeps Events while their JSON fits under the
byte cap. It stops at the first Event that would go over, even if a later one is small, and always
keeps the first Event, even when it is over the cap by itself. Exactly at the cap fits.

```
sizes 400, 400, 400, 100 with a cap of 1000  →  keeps the first two (800); the third would make 1200
sizes 5000, 100 with a cap of 1000           →  keeps the first alone
```

`build_body` writes the JSON. Each Event's JSON goes in exactly as `history.db` serialises it, not
re-encoded:

```json
{"schema_version":1,"client":"BazaarTracker","client_version":"0.1.0",
 "runs":[{"id":"r1","started_ms":5,"ended":true,…}],
 "events":[{…event a…},{…event b…}]}
```

`runs` holds each distinct Run of the batch's Events, read with `get_run`. The original tracker
writes `schemaVersion` and `clientVersion` in camelCase, with PascalCase Run fields; this body
does not copy that.

`gzip` compresses the body as one standard gzip stream (it starts `1f 8b`).

## Sending

`transport.rs` puts the network behind a trait, so the uploader's decisions are tested without one:

```rust
pub trait Transport: Send + Sync {
    fn post(&self, request: &Request<'_>) -> Result<Response, TransportError>;
}
```

`UreqTransport` is the real one. It `POST`s with a 110 s timeout (`REQUEST_TIMEOUT`, as the original
tracker) and these headers:

| Header | Value |
|---|---|
| `Content-Type` | `application/json` |
| `Content-Encoding` | `gzip` |
| `Authorization` | `Bearer <key>` |
| `User-Agent` | `BazaarTracker/<version>` |

An HTTP error status is an answer (`Response { status, body }`), not an error. `TransportError` is
only `Timeout` or `Other(message)`.

The endpoint is `BAZAAR_TRACKER_UPLOAD_ENDPOINT` if set to something non-blank, else
`https://adira.bzdb.network/api/tracker-v2/runs/batch`, with any trailing `/` removed.

## What each answer does

`send_batch` decides by the answer:

| Answer | What happens | Notice |
|---|---|---|
| 200–299 | Every Event in the batch is marked `Sent`. | `Sent { events }` |
| 413, more than one Event | The batch splits in two halves; each is sent the same way. | from the halves |
| 413, one Event | That Event is marked `Rejected`: set aside for good. The pass goes on. | `Rejected { event_id, event_type }` |
| Any other status | The Events stay waiting; the pass backs off. | `Refused { status, message }` |
| Timeout, more than one Event | Split in two, as for 413. | from the halves |
| Timeout, one Event | The Event stays waiting; the pass backs off. | `Failed("the upload timed out")` |
| Other transport error | The Events stay waiting; the pass backs off. | `Failed(message)` |
| The history can not be read, or a status can not be written | The pass backs off. | `Failed(…)` |

When halves are sent and the first must back off, the second is not tried. With four Events and the
server answering 413 to four and then to the first two, the requests are sized `4, 2, 1, 1, 2`.

The only permanent refusal is a 413 for a single Event. A 401, 403 or 500 never sets anything aside.

`Refused.message` is read from the server's JSON body by `api_error`: the first of `message`,
`error`, `detail` and `title` that is text, else `error.message`, cut to 200 characters plus `…`.

`drain_once(now_ms)` repeats batches until nothing is waiting (`Done { sent }`) or a batch backs off
(`Backoff { sent }`). `sent` counts accepted Events; a set-aside Event is not one.

## Backoff and the thread

`Uploader::start` runs passes on a thread named `uploader` and returns an `UploadHandle`.

| Setting | Default |
|---|---|
| `idle_wake`: wait between passes when all is well | 15 s |
| `base_delay`: the first wait after a failure | 15 s |
| `max_delay`: the longest wait | 1 hour |
| `stale_after` | 6 hours |

After `n` failed passes in a row the wait is `base_delay × 2^(n-1)`, capped at `max_delay`
(`backoff_delay`):

```
failures  1   2   3    4    5    6    7     8     9+
wait (s)  15  30  60  120  240  480  960  1920  3600
```

A pass that does not back off resets the count. The handle sends signals:

| Call | Effect |
|---|---|
| `nudge()` | New Events may be waiting. An idle uploader runs a pass now; one that is backing off keeps waiting. |
| `restart_backoff()` | The key changed or sync was turned on: forget past failures and run a pass now. |
| `stop()`, or dropping the handle | Stops the thread and joins it. |

## The secret key

`SecretStore` keeps the key in the Windows Credential Manager, never in a file, through the
`keyring` crate. The entry's user name is `secret-key`; its service name depends on the build, so
development never overwrites the real key:

| Build | Entry |
|---|---|
| Release | `BazaarTracker` |
| Debug | `bazaar-tracker-dev` |

| Method | What it does |
|---|---|
| `load()` | The key, or `None` when there is no entry |
| `save(key)` | Replaces any saved key; a blank key is refused |
| `clear()` | Deletes the entry; deleting a missing one is not an error |

The uploader reads the key through `KeySource` at the start of every pass, so a changed key is used
at the next pass. `SecretStore` is one source; tests pass a closure.

## Where it is recorded

There is no separate table. Migration 2 adds `upload_status` to `events`, with a partial index
`events_waiting_for_upload` over waiting Events (see [history-db.md](history-db.md)):

| `upload_status` | `UploadStatus` | Meaning |
|---|---|---|
| 0 | `Waiting` | Not sent yet (the default for every new Event) |
| 1 | `Sent` | The server accepted it |
| 3 | `Rejected` | Refused for good; never tried again |

`set_upload_status(ids, status)` writes a batch's outcome in one transaction; `upload_status(id)`
reads one Event's.

## Tests

| Test file | Covers |
|---|---|
| `crates/bazaar-tracker/src/upload/batch_tests.rs` | The byte cap (stops at the first misfit, exact cap, first always kept), the 200 / 2,000,000 limits, the body's JSON, an escaped version, gzip round trip |
| `crates/bazaar-tracker/src/upload/uploader_tests.rs` | Off until enabled, no key, headers and gzip body, sent once, 200 per batch, the byte cap, refusals and their message, the backoff table, 413 splitting, a single 413 set aside, `local-` Runs, stale Runs, a timeout, the first half backing off, the endpoint setting, `api_error`, and the background loop (backs off, nudge, stop) |
| `crates/bazaar-tracker/src/upload/secret_tests.rs` | Debug and release entry names, a blank key refused, and on Windows a real save, load, replace and clear under a test entry that is removed after |

`uploader_tests.rs` runs a small fake HTTP server on `127.0.0.1`, so the real `UreqTransport` is
used without leaving the machine. Run everything with `just test`.
