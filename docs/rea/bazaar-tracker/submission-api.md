# BazaarDBTracker: Event Submission and the HTTP API

This is a static deep dive into `BazaarDBCompanion.Submission.EventSubmissionClient` (and its nested `RunDto`), `EndpointConfig` and `SecretStore` in `references/BazaarDBTracker-beta-Portable/current/BazaarDBTracker.dll` (1.0.0-beta.7, SHA-256 `ab1734c4…f362d`, MVID `f3dd3a4a-…`). The app was never run, and no request was sent to any endpoint.

**Method.** The REA `inspect_managed_members` dump (`ev_aaf4c467…`) supplied the file offset of every method body, plus the MemberRef, Field and #US tables. A small throwaway CIL disassembler then read the raw IL bytes straight from the DLL and resolved the tokens. TypeRef tokens used in exception clauses were resolved by parsing the TypeRef table directly; for example, `0x0100013e` is `System.OperationCanceledException`. The disassembler is `scratchpad/cil.js` and its output is in `scratchpad/sub_il.txt` and `scratchpad/nested_il.txt`. Async state machines were read through their `MoveNext` bodies.

How each claim is marked:
- **[V]**: verified by reading the CIL instruction stream: branches, constants and call targets.
- **[I]**: inferred from .NET framework semantics or names, not from this assembly's code.

Shorthand: `ESC` = `EventSubmissionClient`, `ES` = `Persistence.EventStore`.

---

## 1. The request

| Item | Value | |
|---|---|---|
| Method | `POST` (`HttpMethod.Post`) | [V] |
| URL | `EndpointConfig.BatchUrl`, default `https://adira.bzdb.network/api/tracker-v2/runs/batch` | [V] |
| Other endpoints | **None.** This is the only HTTP request the submission code builds. | [V] |
| Body | The UTF-8 JSON batch (see §2), gzip-compressed, sent as `ByteArrayContent` | [V] |
| `Content-Type` | `application/json` (built with `new MediaTypeHeaderValue("application/json")`, so no charset parameter) | [V] |
| `Content-Encoding` | `gzip` | [V] |
| `Authorization` | `Bearer <apiKey>`, set per request with `new AuthenticationHeaderValue("Bearer", _apiKey)` | [V] |
| `User-Agent` | `BazaarDBCompanion/<AppInfo.Version>`, set on `DefaultRequestHeaders` → `BazaarDBCompanion/1.0.0-beta.7` | [V] |
| Accept / other headers | None added explicitly | [V] (bounded to ESC) |

**Gzip.** `GzipUtf8(string)` calls `Encoding.UTF8.GetBytes`, then writes the bytes into `new GZipStream(memoryStream, (CompressionLevel)0 /*Optimal*/, leaveOpen: true)`, disposes the gzip stream and returns `ms.ToArray()`. This produces a standard single-member gzip with no chunking. [V]

**Bearer token source.** `_apiKey` is the string the ctor receives (`MainWindowViewModel.savedKey`, which comes from `SecretStore.Load()`). `UpdateApiKey(key)` replaces it when the user clicks Save, which goes through `MainWindowViewModel.SaveKey` → `SecretStore.Save` → `client.UpdateApiKey`. The key is sent as-is: it is not trimmed or encoded. [V] There is no OAuth, no token refresh, and no step that validates the key against the server. [V]

**`AppInfo.Version`.** This is `AssemblyInformationalVersion`, truncated at the first `+` so build metadata is stripped. If that is blank, it falls back to `Assembly.GetName().Version.ToString(3)`, then to `"0.0.0"`. [V]

**HttpClient configuration** (`ESC..ctor`) [V]:
- `SocketsHttpHandler { PooledConnectionLifetime = 2 min, MaxConnectionsPerServer = 2 }`
- `HttpClient.Timeout = 110 s`
- The ctor starts the send loop with `Task.Run(() => SendLoop(_cts.Token))`.
- `Dispose()` cancels the CTS, waits up to 3 s for the loop (and swallows `AggregateException`), then disposes the HttpClient, CTS and semaphore.

---

## 2. Batch JSON schema (`schemaVersion: 1`)

`BuildBatchBody(batch)` concatenates strings in this order [V]:

```
{"schemaVersion":1,"clientVersion":<JsonSerializer.Serialize(AppInfo.Version)>,"runs":<JsonSerializer.Serialize(runDtos, RunSummaryJson)>,"events":[<payloadJson_1>,<payloadJson_2>,...]}
```

- `schemaVersion` is the literal integer `1`. [V]
- `clientVersion` is a JSON string. [V]
- `runs` holds the distinct `UnsentEvent.BzRunId` values in the batch. Their `ES.GetRunSummaries(ids)` rows (`SELECT id, startedUtc, endedUtc, outcome, eventCount, day, victories, losses, hero FROM runs WHERE id IN (…)`) are mapped to `RunDto`. [V] `UnsentEvent.BzRunId` is `events.runId` from the DB row, not the payload field. The two are the same once a run has been rebound. [V]
- `events` holds the stored `events.payloadJson` strings, inserted **verbatim** and comma-joined. The client does not re-serialize or validate them. [V]

### 2.1 `RunDto` (in `runs[]`)

`RunSummaryJson` is `new JsonSerializerOptions { DefaultIgnoreCondition = WhenWritingNull (3) }`. It sets **no naming policy**, so keys are PascalCase, and no `JsonPropertyName` attribute exists anywhere in the assembly. [V] Null properties are **omitted**. [V for the option, I for STJ behaviour]

| Key | Type | Source | |
|---|---|---|---|
| `BzRunId` | string | `runs.id`: the game's RunId, never `local-…` (see §3.4) | [V] |
| `StartedUtc` | string | `RunSummary.StartedUtc.ToString("o")`, round-trip ISO-8601 such as `2026-10-30T12:34:56.1234567Z` | [V]; the `Z` suffix is [I] and depends on `ParseUtc` returning Kind=Utc |
| `EndedUtc` | string? | `EndedUtc?.ToString("o")`; omitted while the run is still open | [V] |
| `Outcome` | string? | `runs.outcome`, written as `RunEndedEvent.Outcome.ToString()` → `"Win"`, `"Loss"` or `"Unknown"` | [V] call edges; values from the `CombatOutcome` enum |
| `PlayerHero` | string? | `runs.hero` | [V] |
| `Day` | int? | `runs.day` | [V] |
| `Victories` | int? | `runs.victories` | [V] |
| `Losses` | int? | `runs.losses` | [V] |

`RunSummary.EventCount` and `LastEventUtc` are **not** sent. [V]

### 2.2 Event objects (in `events[]`)

These are written in `MainWindowViewModel.OnEventDetected` as `JsonSerializer.Serialize(ev, ev.GetType(), WireJson)`. `WireJson` = `{ Converters = [JsonStringEnumConverter], DefaultIgnoreCondition = WhenWritingNull }`, with no naming policy. That means PascalCase keys, the runtime type's full property set, enums as strings, and nulls omitted. [V]

Common `GameEvent` fields [V, from backing-field types]: `EventId` (Guid; a `Guid.NewGuid()` call site exists), `Type` (`GameEventType` as a string, e.g. `"CombatEnd"`), `TimestampUtc` (DateTime), `BzRunId` (string), `Day` (int?) and `Hour` (int?). The per-type fields (`RunStartedEvent.PlayMode/BuildId/…`, `CombatEndEvent.Outcome/VictoriesAfter/…`, `CombatFramesEvent.Frames`, and so on) belong to the event-model deep dive. The upload client treats them as opaque.

`RunReboundEvent` is **never stored or uploaded**. `OnEventDetected` handles it by calling `ES.RebindRun(old, new)` and `client.Enqueue(0)`, then returns. [V]

### 2.3 Example body (before gzip)

This example is illustrative: the values are made up and the key order inside events is not significant.

```json
{
  "schemaVersion": 1,
  "clientVersion": "1.0.0-beta.7",
  "runs": [
    {"BzRunId":"7f3c…","StartedUtc":"2026-10-30T12:00:01.0000000Z","EndedUtc":"2026-10-30T12:41:10.0000000Z",
     "Outcome":"Win","PlayerHero":"Vanessa","Day":9,"Victories":10,"Losses":3}
  ],
  "events": [
    {"EventId":"0b6c…","Type":"RunStarted","TimestampUtc":"2026-10-30T12:00:01.0000000Z","BzRunId":"7f3c…","Day":1,"Hour":0,
     "PlayMode":"…","BuildId":"…","PlayerHero":"Vanessa","PlayerRank":"…"},
    {"EventId":"9d21…","Type":"CombatEnd","TimestampUtc":"…","BzRunId":"7f3c…","Outcome":"Win","VictoriesAfter":1,"LossesAfter":0}
  ]
}
```

---

## 3. Sending, responses and retry

### 3.1 Which events are sendable: `ES.GetBatchSendable(limit: 200, now: UtcNow)` [V]

```sql
SELECT e.id, e.runId, e.type, e.payloadJson, e.batchAttemptCount
FROM events e JOIN runs r ON r.id = e.runId
WHERE e.batchSyncStatus IN (0, 2)
  AND json_extract(e.payloadJson, '$.EventId') IS NOT NULL
  AND json_extract(e.payloadJson, '$.BzRunId') IS NOT NULL
  AND json_extract(e.payloadJson, '$.BzRunId') NOT LIKE 'local-%'
  AND (e.batchNextRetryUtc IS NULL OR e.batchNextRetryUtc <= $now)
  AND (r.endedUtc IS NOT NULL
       OR (SELECT MAX(e2.timestampUtc) FROM events e2 WHERE e2.runId = r.id) <= $stale)
ORDER BY e.id LIMIT $limit;
```

- `$stale` = `now - ES.StaleRunUploadAge`, where `StaleRunUploadAge` = **6 hours** (`TimeSpan.FromHours(6)`). [V] A run is uploaded only after it ends, or once it has had no new events for 6 h.
- `batchSyncStatus` values: 0 Pending, 1 Sent, 2 Failed, 3 DeadLetter. The submission path writes only 1 (`MarkBatchSent`: `UPDATE events SET batchSyncStatus = 1, batchNextRetryUtc = NULL WHERE id IN (…)`) and 3 (`MarkBatchDeadLetter`, one id). [V] In the SQL literals that were searched, no code path sets status 2, `batchAttemptCount` or `batchNextRetryUtc`. Those columns are read but are effectively vestigial, and retry is global, not per event. [V, bounded to the #US SQL literals] A migration also dead-letters old rows that lack `$.EventId`. [V]

### 3.2 Size and count limits

| Limit | Value | |
|---|---|---|
| Rows fetched per drain iteration | **200** (`GetBatchSendable(200, …)`) | [V] |
| Request size cap (`LimitBySize`) | Takes events in order while the running sum of `UTF8.GetByteCount(payloadJson)` stays ≤ **2,000,000** bytes. It stops at the first event that would exceed the cap. The first event is **always** included, even if it alone is larger. The cap counts only payload bytes, before gzip, and ignores the envelope and `runs`. | [V] |
| Error text clamp | API error strings are cut to 200 chars plus `…` | [V] |

### 3.3 Response handling: `SendBatch(batch, ct)` returns a `bool`, where true means "back off"

| Outcome | Action | Returns | |
|---|---|---|---|
| empty batch | none | `false` | [V] |
| `IsSuccessStatusCode` (2xx) | `MarkBatchSent(all ids)`. **The response body is not read.** | `false` | [V] |
| **413** and `batch.Count > 1` | `SplitAndRetry` | the split's result | [V] |
| **413** and `Count == 1` | `MarkBatchDeadLetter(batch[0].Id)`, raise `ErrorOccurred("<Type> event too large for the server (<N> KB) — skipped")` | `false` | [V] |
| any other non-2xx (400/401/403/429/5xx…) | Read the body as a string. `ErrorOccurred("Batch of <n> refused (<HttpStatusCode>), backing off" + HttpErrorText)`. No events are marked, so they stay pending. | `true` | [V]; the status is formatted as its enum name, e.g. `Unauthorized`, which is [I] |
| `OperationCanceledException` while the loop's `ct` is cancelled | rethrow (shutdown) | n/a | [V] |
| `OperationCanceledException` otherwise, i.e. the 110 s HttpClient timeout, with `Count > 1` | `SplitAndRetry` | the split's result | [V] |
| any other `Exception`, including a timeout on a 1-event batch, DNS failure or connection refused | `ErrorOccurred("Batch upload error, backing off: " + ex.Message)` | `true` | [V] |

**`SplitAndRetry`** sets `mid = Count / 2`. It awaits `SendBatch(batch.Take(mid))`; if that returns true, it returns true without sending the second half. Otherwise it returns `SendBatch(batch.Skip(mid))`. The split recurses, so repeated 413 responses or timeouts halve the batch down to single events. [V]

**`Retry-After` is ignored**, and 429 is treated the same as any other refusal. [V: no header access in the code]

**`ExtractApiError(body)`** parses the body as JSON. If the root is an object, it returns the first string-valued property among `message`, `error`, `detail` and `title`. Failing that, it returns `error.message` when `error` is an object. Each value is clamped. A `JsonException` or a blank body returns null. `HttpErrorText` appends `": " + msg` when one is found. [V]

### 3.4 Drain and loop cadence

**`DrainBatchOnce(ct)`** [V]:
1. If `!_enabled` or `_apiKey` is blank, return `false` (an idle tick).
2. Repeat:
   1. `GetBatchSendable(200, now)`. Stop if it returns nothing.
   2. `r = SendBatch(LimitBySize(batch))`.
   3. Set `anyChanged |= !r`.
   4. If `r` is true, set `serverRefused = true` and stop.

   The loop also stops when `ct` is cancelled.
3. If `anyChanged`, raise `StatusChanged`. The VM then refreshes the run list, which shows the "synced" state.
4. Return `serverRefused`.

**`SendLoop(ct)`** runs until `ct` is cancelled. Each pass [V]:

```
failed = await DrainBatchOnce(ct)          // catch OCE when ct cancelled → exit; catch Exception → ErrorOccurred("Sync loop error: …"), failed = true
_connFailures = failed ? _connFailures + 1 : 0
delay = _connFailures <= 0 ? IdleWake (15 s) : BackoffDelay(_connFailures)
await _wake.WaitAsync(delay, ct)           // catch OCE → exit
```

**Backoff** [V]: `BackoffDelay(n) = min(BaseRetryDelay * 2^min(n-1, 8), MaxRetryDelay)`, where `BaseRetryDelay` = **15 s** and `MaxRetryDelay` = **1 h**. That gives 15 s, 30 s, 1 m, 2 m, 4 m, 8 m, 16 m, 32 m, then 1 h from the 9th failure on (15 s × 256 = 64 m, clamped to 60 m). There is no jitter. [V]

**Wake-ups.** `_wake` is `SemaphoreSlim(0, 1)`. `Nudge()` releases it and swallows `SemaphoreFullException`. [V] These calls Nudge:
- `Enqueue(long)`, called after every stored event and after a rebind. The argument is ignored. [V]
- `SetEnabled(true)` [V]
- `UpdateApiKey`, which also resets `_connFailures = 0`. [V]

The consequence: **a Nudge cuts any backoff sleep short.** During active play, every new event triggers an immediate retry, even after a 401. Only the counter keeps growing. [V, from the control flow]

**Enable switch.** The `_enabled` field starts as `true` in the field initializer. The VM ctor then calls `SetEnabled(settings.SyncEnabled)`. [V]

### 3.5 Server run ids and rebinding

**The server never returns run ids. The client does not read a successful response body at all.** [V] The `BzRunId` comes from the game itself, read from memory as `_cachedRunInitialized.<RunId>`; see the overview. Until that id is known, events are stored under a synthetic `local-…` run id, and `GetBatchSendable` excludes them (`NOT LIKE 'local-%'`). When `GameTracker` learns the real id, it emits `RunReboundEvent{OldBzRunId, BzRunId}`. The VM then calls `ES.RebindRun(old, new)`, which does the following [V]:
- Moves the events with `runId = $new, seq = seq + $seqBase, payloadJson = json_set(payloadJson,'$.BzRunId',$new)`.
- Merges into an existing `$new` run, clearing `endedUtc`/`outcome` and coalescing `day/victories/losses/hero`, or renames the run row.
- Calls `client.Enqueue(0)`, which makes those events eligible immediately once the run ends or goes stale.

**The server has to be idempotent on `EventId`** [I]. A timeout after the server has committed the batch leaves the events pending, so they are re-sent, possibly split differently. The same applies when a 2xx response is lost.

---

## 4. `SecretStore`

| | |
|---|---|
| Path | `Path.Combine(GetFolderPath(ApplicationData /*26*/), "BazaarDBCompanion", "secret.dat")` → `%APPDATA%\BazaarDBCompanion\secret.dat` [V] |
| Save (Windows) | Creates the directory, then `ProtectedData.Protect(UTF8(key), optionalEntropy: null, scope: 0 = CurrentUser)` → `File.WriteAllBytes`. This is **DPAPI, user scope, no entropy.** [V] |
| Save (non-Windows) | `File.WriteAllText(path, key)`: **plain text**, with no file-mode hardening. [V] |
| Load | Returns null if the file is missing. Otherwise it uses DPAPI `Unprotect` + UTF-8 on Windows, or `ReadAllText` elsewhere. A catch-all returns `null` on any error, for example a blob from another user or machine. [V] |
| Deletion | `SecretStore` has no clear or delete method. [V] An empty key is rejected in the UI with "Key cannot be empty". [V] |

With no key, the VM shows "No key saved — runs are kept locally until one is set", and `DrainBatchOnce` stays idle. [V]

## 5. `EndpointConfig`

`record EndpointConfig(string BatchUrl)`. `Default()` = `(Environment.GetEnvironmentVariable("BAZAARDB_BATCH_API_ENDPOINT") ?? "https://adira.bzdb.network/api/tracker-v2/runs/batch").TrimEnd('/')`. [V]

- Only the null check applies. An env var set to an empty string gives `BatchUrl = ""`, which makes every send throw and back off. [V for the code, I for the runtime effect]
- The ctor's `endpoints` parameter defaults to `Default()`, and the VM passes `null`. [V]
- `BAZAARDB_BATCH_API_ENDPOINT` is the only env var in the submission area. `BAZAARDB_UPDATE_URL`, which belongs to `Updater`, is unrelated. [V]

---

## 6. Rust porting notes

These notes are design suggestions, not observations.

- **HTTP.** Use one `reqwest::Client` with `.user_agent(format!("BazaarDBCompanion/{ver}"))`, `.timeout(Duration::from_secs(110))`, `.pool_idle_timeout(...)` and `pool_max_idle_per_host(2)`. Compress the body yourself with `flate2::write::GzEncoder` and set `Content-Encoding: gzip` and `Content-Type: application/json` manually. Do **not** rely on reqwest's `gzip` feature, which only decompresses responses. Use `.bearer_auth(key)`.
- **Body.** Build the envelope by string concatenation of the stored payload JSON (`serde_json::value::RawValue`), as the original does, so payloads go out byte-for-byte. `RunDto` needs `#[serde(rename_all = "PascalCase", skip_serializing_if = "Option::is_none")]`. Dates should be RFC 3339 with 7 fractional digits to match .NET `"o"`. The server probably accepts any ISO-8601 [I].
- **Loop.** Run a `tokio` task with a `tokio::sync::Notify` as `_wake` and `tokio::select!` over `notify.notified()`, `sleep(delay)` and cancellation. Keep the 15 s idle wake, the 15 s → 1 h doubling backoff and the 200-row / 2 MB caps. The recursive halving on 413 or timeout needs `Box::pin` for the async recursion. Treat 413 on a single event as dead-letter and every other non-2xx as back-off. Consider honouring `Retry-After` and adding jitter, which would be an improvement.
- **Secret.** Use the `keyring` crate (Windows Credential Manager, macOS Keychain, Secret Service) as the store. To import an existing install, read `%APPDATA%\BazaarDBCompanion\secret.dat` with `windows::Win32::Security::Cryptography::CryptUnprotectData` (null entropy, user scope).
- **Env override.** Read `BAZAARDB_BATCH_API_ENDPOINT` with `std::env::var`, trim the trailing `/`, and treat an empty value as unset. This is better than the original's behaviour.

## 7. Unknowns

- The server-side contract is unknown: the response body schema on success, per-event rejection semantics and dedup behaviour. Nothing in the client reveals these, and no traffic was observed.
- The exact `DateTimeKind` from `ES.ParseUtc` has not been byte-checked, so whether `"o"` emits `Z` or an offset is unconfirmed.
- The generic instantiations behind MethodSpec tokens (`0x2b…`) were not resolved. Examples are `JsonSerializer.Serialize<List<RunDto>>` and the `AppendFormatted<HttpStatusCode>` type argument. They were inferred from context.
- It is unknown whether any non-submission code path sets `batchSyncStatus = 2` or `batchNextRetryUtc`. The search was bounded to #US SQL literals and found none.
