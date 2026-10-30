# BazaarDBTracker: persistence and the SQLite schema

Scope: `BazaarDBCompanion.Persistence.EventStore`, `RunSummary`, `EventSummary`, `EventDetail`, `UnsentEvent` and `SyncStatus` in `references/BazaarDBTracker-beta-Portable/current/BazaarDBTracker.dll` (SHA-256 `ab1734c4…f362d`). This doc builds on [overview.md](overview.md) §3.6 and §5.3.

**Method.** Static analysis only. REA `inspect_managed_members` gave the metadata, the call edges and the `#US` literals. I then decoded every method body's CIL from the reported file offsets with a small decoder and resolved the tokens against that same metadata. I did not run the app and did not open any real `history.db`.

**Labels.** **[V]** means verified from the CIL or literals. **[I]** means inferred. The SQL below is verbatim, except that the binary uses CRLF line endings and they are shown here as LF.

---

## 1. Store shape and lifetime

- `public sealed class EventStore : IDisposable` has these fields: `_connectionString`, `_writeConnection` (a `SqliteConnection`), `_writeLock` (a plain `object`), and `static readonly TimeSpan StaleRunUploadAge = TimeSpan.FromHours(6)`. **[V]** (`.cctor`: `ldc.i4.6; TimeSpan.FromHours`)
- `.ctor(string path)`: when `path` is null, it uses `Path.Combine(GetFolderPath(ApplicationData /*0x1A*/), "BazaarDBCompanion", "history.db")`. It then calls `Directory.CreateDirectory(dirname)`. The connection string comes from `SqliteConnectionStringBuilder { DataSource = path, Mode = 0 /*ReadWriteCreate*/ }`. The constructor opens **one long-lived write connection** and runs this on it: **[V]**
  ```sql
  PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=3000;
  ```
  The `ldc.i4.s 26` constant is now byte-checked, which resolves the overview's [I] about the `%APPDATA%` path.
- `MainWindowViewModel..ctor` calls `new EventStore(null)` and then `EnsureSchema()` explicitly. The constructor itself does not create the schema. **[V]**
- **Reads** (`OpenRead`): every read method opens a **new** `SqliteConnection(_connectionString)`, runs `PRAGMA busy_timeout=3000;` on it, uses it, and disposes it. Reads take no lock. Reads do not set `foreign_keys` or `journal_mode`. Neither matters for reads, and WAL mode persists in the file. **[V]** Microsoft.Data.Sqlite pools these connections by default. **[I]**
- **Writes** all go through `_writeConnection` while holding `lock(_writeLock)`. This applies to `EnsureSchema`, `InsertEvent`, `MarkBatchSent`, `MarkBatchDeadLetter`, `RebindRun`, `ClearRunEnd`, `DeleteRun` and `Dispose`. **[V]**
- `Dispose()` disposes the write connection under the lock. **[V]**
- `ParseUtc(s)` is `DateTime.Parse(s, null, DateTimeStyles.RoundtripKind /*128*/)`. **[V]**
- **Time format.** Every timestamp is written as `DateTime.ToString("o")`, the round-trip ISO-8601 format. With `Kind=Utc` this looks like `2025-01-02T03:04:05.1234567Z`. **[V]** That the values are always `Kind=Utc` is **[I]**. It matters because `GetBatchSendable` and `ListRuns` compare and sort these as **strings**.

### Transactions [V]
| Method | Transaction |
|---|---|
| `InsertEvent` | `BeginTransaction`, then UpsertRunInternal, then seq, then INSERT, then eventCount++, then `Commit`. If an exception is thrown, the uncommitted transaction is disposed, which rolls it back. |
| `RebindRun` | One transaction for three commands. |
| `DeleteRun` | One transaction for two DELETEs. |
| `EnsureSchema` (DDL, migrations, backfill) | No explicit transaction. Each `Execute` autocommits, and the whole thing runs under the lock. |
| `MarkBatchSent`, `MarkBatchDeadLetter`, `ClearRunEnd` | A single autocommit statement under the lock. |

---

## 2. DDL and migrations

### 2.1 Base DDL (`EnsureSchema`, which runs it before `MigrateSchema` and then `BackfillRunStats`) [V]
```sql
CREATE TABLE IF NOT EXISTS runs (
  id          TEXT PRIMARY KEY,
  startedUtc  TEXT NOT NULL,
  endedUtc    TEXT,
  outcome     TEXT,
  realRunId   TEXT,
  eventCount  INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS events (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  runId         TEXT NOT NULL REFERENCES runs(id),
  seq           INTEGER NOT NULL,
  timestampUtc  TEXT NOT NULL,
  type          TEXT NOT NULL,
  payloadJson   TEXT NOT NULL,
  syncStatus    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS ix_events_run  ON events(runId, seq);
CREATE INDEX IF NOT EXISTS ix_events_sync ON events(syncStatus) WHERE syncStatus <> 1;
CREATE INDEX IF NOT EXISTS ix_runs_started ON runs(startedUtc);
```

### 2.2 `MigrateSchema` [V]
It reads `PRAGMA user_version;` **once** into `v`. It then runs each step whose guard is true, in order. Every `ALTER` is guarded by `ColumnExists(table, col)`, which runs `PRAGMA table_info(<table>);` and compares column 1 to the column name ordinal-ignore-case (`StringComparison` 5). A fresh database has `v = 0`, so it runs every step.

| Guard | Statements | Effect |
|---|---|---|
| `v < 2` | `ALTER TABLE events ADD COLUMN attemptCount INTEGER NOT NULL DEFAULT 0;` then `ALTER TABLE events ADD COLUMN nextRetryUtc TEXT;` then `PRAGMA user_version = 2;` | Adds the legacy per-event retry columns. |
| `v < 3` | For each of `day`, `victories` and `losses`: `"ALTER TABLE runs ADD COLUMN " + col + " INTEGER;"`. Then `PRAGMA user_version = 3;` | Adds the run stats columns. |
| `v < 4` | `ALTER TABLE runs ADD COLUMN hero TEXT;` then `PRAGMA user_version = 4;` | Adds the hero column. |
| `v < 5` | `$"ALTER TABLE events ADD COLUMN {name} {decl};"` for `(batchSyncStatus, "INTEGER NOT NULL DEFAULT 0")`, `(batchAttemptCount, "INTEGER NOT NULL DEFAULT 0")` and `(batchNextRetryUtc, "TEXT")`. Then `CREATE INDEX IF NOT EXISTS ix_events_batchsync ON events(batchSyncStatus) WHERE batchSyncStatus <> 1;` and `PRAGMA user_version = 5;` | Adds the current batch-sync bookkeeping. |
| `v < 6` | `UPDATE events SET batchSyncStatus = 3`<br>`WHERE batchSyncStatus IN (0, 2)`<br>`  AND json_extract(payloadJson, '$.EventId') IS NULL;` then `PRAGMA user_version = 6;` | Dead-letters old events that have no `EventId`, because the batch API can't take them. |

The final schema is at `user_version` 6. **[V]**

- `runs`: `id, startedUtc, endedUtc, outcome, realRunId, eventCount, day, victories, losses, hero`
- `events`: `id, runId, seq, timestampUtc, type, payloadJson, syncStatus, attemptCount, nextRetryUtc, batchSyncStatus, batchAttemptCount, batchNextRetryUtc`

### 2.3 `BackfillRunStats` (runs on every startup after migration, autocommit) [V]
```sql
UPDATE runs SET
  victories = COALESCE(
    victories,
    (SELECT json_extract(e.payloadJson, '$.VictoriesAfter') FROM events e
     WHERE e.runId = runs.id AND e.type = 'CombatEnd'
       AND json_extract(e.payloadJson, '$.VictoriesAfter') IS NOT NULL
     ORDER BY e.seq DESC LIMIT 1),
    (SELECT json_extract(e.payloadJson, '$.Victories') FROM events e
     WHERE e.runId = runs.id AND e.type = 'RunEnded' LIMIT 1)),
  losses = COALESCE(
    losses,
    (SELECT json_extract(e.payloadJson, '$.LossesAfter') FROM events e
     WHERE e.runId = runs.id AND e.type = 'CombatEnd'
       AND json_extract(e.payloadJson, '$.LossesAfter') IS NOT NULL
     ORDER BY e.seq DESC LIMIT 1),
    (SELECT json_extract(e.payloadJson, '$.Losses') FROM events e
     WHERE e.runId = runs.id AND e.type = 'RunEnded' LIMIT 1)),
  day = COALESCE(
    day,
    (SELECT json_extract(e.payloadJson, '$.Day') FROM events e
     WHERE e.runId = runs.id AND e.type = 'CombatEnd'
       AND json_extract(e.payloadJson, '$.Day') IS NOT NULL
     ORDER BY e.seq DESC LIMIT 1),
    (SELECT json_extract(e.payloadJson, '$.Day') FROM events e
     WHERE e.runId = runs.id AND e.type = 'RunEnded' LIMIT 1)),
  hero = COALESCE(
    hero,
    (SELECT json_extract(e.payloadJson, '$.PlayerHero') FROM events e
     WHERE e.runId = runs.id AND e.type = 'RunStarted'
       AND json_extract(e.payloadJson, '$.PlayerHero') IS NOT NULL
     LIMIT 1),
    (SELECT json_extract(e.payloadJson, '$.PlayerHero') FROM events e
     WHERE e.runId = runs.id AND e.type = 'CombatStart'
       AND json_extract(e.payloadJson, '$.PlayerHero') IS NOT NULL
     ORDER BY e.seq LIMIT 1))
WHERE victories IS NULL OR losses IS NULL OR day IS NULL OR hero IS NULL;
```
This tells us the payload JSON uses **PascalCase** property names (`EventId`, `BzRunId`, `VictoriesAfter`, `PlayerHero`, …). It also tells us `events.type` holds the `GameEventType` enum name (`'CombatEnd'`, `'RunStarted'`, …). **[V]**

### 2.4 Dead and legacy columns [V, from an exhaustive scan of the literals]
- `runs.realRunId`: no SQL ever reads or writes it.
- `events.syncStatus`: `InsertEvent` writes `0` and nothing ever updates it. Its `ix_events_sync` index is therefore dead weight.
- `events.attemptCount` and `events.nextRetryUtc`: no SQL ever touches them after they are added.
- `events.batchAttemptCount`: `GetBatchSendable` reads it into `UnsentEvent.AttemptCount`, but **no SQL increments it**.
- `events.batchNextRetryUtc`: only ever set to `NULL`, by MarkBatchSent and MarkBatchDeadLetter. `GetBatchSendable` honours it, but nothing in this build ever sets a future value.
- `batchSyncStatus = 2` (Failed) is **never written**. Only `0` (the default), `1` and `3` are written.

So retry and backoff live in memory in `EventSubmissionClient` (the strings "backing off"), not in the database. **[V]** for the absence of SQL writes. **[I]** that it is purely in-memory; the submission doc covers that side.

---

## 3. Every SQL statement, by public method

`$x` are `SqliteParameter`s added with `AddWithValue`. "ev" means the `GameEvent` argument.

### `long InsertEvent(GameEvent ev, string payloadJson)`, write lock and transaction [V]
1. `UpsertRunInternal(tx, ev)` runs. Its statements are listed below.
2. `SELECT COALESCE(MAX(seq), 0) + 1 FROM events WHERE runId = $r;` binds `$r = ev.BzRunId` and yields `seq` (int).
3. This statement binds `$r = ev.BzRunId`, `$seq = seq`, `$ts = ev.TimestampUtc.ToString("o")`, `$type = ev.Type.ToString()` (the `GameEventType` name) and `$payload = payloadJson`. Its scalar result is the returned `long` id.
   ```sql
   INSERT INTO events (runId, seq, timestampUtc, type, payloadJson, syncStatus)
   VALUES ($r, $seq, $ts, $type, $payload, 0);
   SELECT last_insert_rowid();
   ```
4. `UPDATE runs SET eventCount = eventCount + 1 WHERE id = $r;`
5. `Commit`.

The caller is `MainWindowViewModel.OnEventDetected`, which computes `payloadJson = JsonSerializer.Serialize(ev, ev.GetType(), WireJson)`. `DrainPersistRetryQueue` is the other caller (see §4.4). Callers then call `client.Enqueue(id)`. **[V]**

### `UpsertRunInternal(SqliteTransaction tx, GameEvent ev)` (private, runs inside the InsertEvent transaction) [V]
1. This always runs. `startedUtc` is the timestamp of the **first persisted event** of the run, whatever its type.
   ```sql
   INSERT INTO runs (id, startedUtc)
   VALUES ($id, $started)
   ON CONFLICT(id) DO NOTHING;
   ```
   It binds `$id = ev.BzRunId` and `$started = ev.TimestampUtc.ToString("o")`.
2. If `ev is RunEndedEvent re`, this runs with `$ended = ev.TimestampUtc.ToString("o")`, `$outcome = re.Outcome.ToString()` (the `CombatOutcome` name) and `$id = ev.BzRunId`:
   `UPDATE runs SET endedUtc = $ended, outcome = $outcome WHERE id = $id;`
3. The hero comes from `RunStartedEvent.PlayerHero` or `CombatStartEvent.PlayerHero`. If it is not null, this runs with `$hero = hero` and `$id = ev.BzRunId`. The first hero wins:
   `UPDATE runs SET hero = COALESCE(hero, $hero) WHERE id = $id;`
4. `(day, vict, loss)` comes from `RunEndedEvent` as `(Day, Victories, Losses)` or from `CombatEndEvent` as `(Day, VictoriesAfter, LossesAfter)`. Every other event type gives all-null. If any of the three has a value, this runs with each null bound as `DBNull`. Non-null values overwrite, so the latest wins:
   ```sql
   UPDATE runs SET
     day       = COALESCE($day, day),
     victories = COALESCE($vict, victories),
     losses    = COALESCE($loss, losses)
   WHERE id = $id;
   ```

### `IReadOnlyList<UnsentEvent> GetBatchSendable(int limit, DateTime now)`, a read connection with no lock [V]
```sql
SELECT e.id, e.runId, e.type, e.payloadJson, e.batchAttemptCount
FROM events e
JOIN runs r ON r.id = e.runId
WHERE e.batchSyncStatus IN (0, 2)
  AND json_extract(e.payloadJson, '$.EventId') IS NOT NULL
  AND json_extract(e.payloadJson, '$.BzRunId') IS NOT NULL
  AND json_extract(e.payloadJson, '$.BzRunId') NOT LIKE 'local-%'
  AND (e.batchNextRetryUtc IS NULL OR e.batchNextRetryUtc <= $now)
  AND (r.endedUtc IS NOT NULL
       OR (SELECT MAX(e2.timestampUtc) FROM events e2 WHERE e2.runId = r.id) <= $stale)
ORDER BY e.id
LIMIT $limit;
```
- It binds `$now = now.ToString("o")`, `$stale = (now - 6h).ToString("o")` and `$limit = limit`.
- Each row becomes `new UnsentEvent(Id: col0, BzRunId: col1 /*events.runId*/, Type: col2, PayloadJson: col3, AttemptCount: col4)`.
- The caller is `EventSubmissionClient.DrainBatchOnce`, which passes `limit = 200` and `now = DateTime.UtcNow` and then calls `LimitBySize` and `SendBatch`. It loops until the result is empty, the token is cancelled, or the server refuses the batch.

### `MarkBatchSent(IReadOnlyList<long> ids)`, write lock [V]
- If `ids.Count == 0` it returns immediately.
- Otherwise it runs this, where `InClause` builds `$b0,$b1,…` and binds each id:
  ```sql
  UPDATE events SET batchSyncStatus = 1, batchNextRetryUtc = NULL WHERE id IN ($b0,$b1,…);
  ```
- `SendBatch` calls it when the HTTP response has `IsSuccessStatusCode`.

### `MarkBatchDeadLetter(long id)`, write lock [V]
```sql
UPDATE events SET batchSyncStatus = 3, batchNextRetryUtc = NULL WHERE id = $id;
```
`SendBatch` calls it on HTTP **413** when the batch holds exactly one event; it passes `batch[0].Id` and logs "… event too large for the server (… KB) — skipped". On a 413 with a batch of more than one event, it calls `SplitAndRetry` instead.

### `IReadOnlyList<RunSummary> GetRunSummaries(IReadOnlyCollection<string> ids)`, read [V]
- If the collection is empty it returns `Array.Empty`.
- Otherwise it binds `$s0…$sN` in enumeration order and runs:
  ```sql
  SELECT id, startedUtc, endedUtc, outcome, eventCount, day, victories, losses, hero
  FROM runs
  WHERE id IN ($s0,$s1,…);
  ```
  There is no ORDER BY.
- `LastEventUtc` is always `null` here.
- The caller is `EventSubmissionClient.BuildBatchBody`, which uses it for the per-run metadata in the batch body.

### `RebindRun(string oldId, string newId)`, write lock and transaction [V]
1. This yields `seqBase`, or `-1` when `$new` has no run row:
   ```sql
   SELECT (SELECT COALESCE(MAX(seq), 0) FROM events WHERE runId = $new)
   WHERE EXISTS(SELECT 1 FROM runs WHERE id = $new);
   ```
2. This binds `$seqBase = max(seqBase, 0)`:
   ```sql
   UPDATE events SET
     runId = $new,
     seq = seq + $seqBase,
     payloadJson = json_set(payloadJson, '$.BzRunId', $new)
   WHERE runId = $old;
   ```
3. If `seqBase >= 0` (the new run already exists), it **merges**:
   ```sql
   UPDATE runs SET
     eventCount = eventCount + (SELECT eventCount FROM runs WHERE id = $old),
     endedUtc = NULL,   -- the rebind only ever happens on a LIVE run
     outcome  = NULL,
     day       = COALESCE((SELECT day FROM runs WHERE id = $old), day),
     victories = COALESCE((SELECT victories FROM runs WHERE id = $old), victories),
     losses    = COALESCE((SELECT losses FROM runs WHERE id = $old), losses),
     hero      = COALESCE(hero, (SELECT hero FROM runs WHERE id = $old))
   WHERE id = $new;
   DELETE FROM runs WHERE id = $old;
   ```
   Otherwise it **renames**: `UPDATE runs SET id = $new WHERE id = $old;`
4. `Commit`.

### `ClearRunEnd(string id)`, write lock [V]
`UPDATE runs SET endedUtc = NULL, outcome = NULL WHERE id = $id;`

### `DeleteRun(string id)`, write lock and transaction [V]
`DELETE FROM events WHERE runId = $id;` and then `DELETE FROM runs WHERE id = $id;`. Both bind `$id`. The child rows go first, which satisfies the foreign key. The caller is the UI's `DeleteRunCommand` (`MainWindowViewModel.DeleteRun`). Afterwards it clears the selection if that run was selected and calls `RefreshRuns()`.

### `bool IsRunEnded(string id)` and `bool HasRunStarted(string id)`, read [V]
- `SELECT EXISTS(SELECT 1 FROM runs WHERE id = $id AND endedUtc IS NOT NULL);` returns true when the scalar equals 1.
- `SELECT EXISTS(SELECT 1 FROM events WHERE runId = $id AND type = 'RunStarted');`

### `IReadOnlyList<RunSummary> ListRuns(int limit)`, read [V]
```sql
SELECT id, startedUtc, endedUtc, outcome, eventCount, day, victories, losses, hero,
       (SELECT e.timestampUtc FROM events e
        WHERE e.runId = runs.id ORDER BY e.seq DESC LIMIT 1) AS lastEventUtc
FROM runs
ORDER BY startedUtc DESC
LIMIT $limit;
```
Its only caller is `MainWindowViewModel.RefreshRuns`, which passes `limit = 200`.

### `HashSet<string> GetSyncedRunIds()`, read [V]
```sql
SELECT DISTINCT r.id
FROM runs r
JOIN events e ON e.runId = r.id
WHERE r.id NOT LIKE 'local-%'
  AND e.batchSyncStatus = 1;
```

### `IReadOnlyList<EventSummary> ListEvents(string runId)`, read [V]
```sql
SELECT id, seq, timestampUtc, type, batchSyncStatus, payloadJson
FROM events
WHERE runId = $r
ORDER BY seq;
```
It returns every event, with no limit. Each row becomes `EventSummary(Id, Seq, ParseUtc(ts), Type, (SyncStatus)batchSyncStatus, PayloadJson)`.

### `EventDetail? LoadEvent(long id)`, read [V]
```sql
SELECT id, runId, seq, timestampUtc, type, payloadJson, batchSyncStatus
FROM events
WHERE id = $id;
```
It returns `null` when there is no row. **It has no callers in the assembly**, so it is dead code. **[V]** from the static call edges; reflection was not checked.

### Record types [V] (all `sealed record class`, with positional constructors in this order)
- `RunSummary(string Id, DateTime StartedUtc, DateTime? EndedUtc, string? Outcome, int EventCount, int? Day, int? Victories, int? Losses, string? Hero, DateTime? LastEventUtc)`
- `EventSummary(long Id, int Seq, DateTime TimestampUtc, string Type, SyncStatus SyncStatus, string PayloadJson)`
- `EventDetail(long Id, string BzRunId, int Seq, DateTime TimestampUtc, string Type, string PayloadJson, SyncStatus SyncStatus)`
- `UnsentEvent(long Id, string BzRunId, string Type, string PayloadJson, int AttemptCount)`
- `enum SyncStatus : int { Pending, Sent, Failed, DeadLetter }`. The names and their declaration order are **[V]**. The values 0, 1, 2 and 3 are **[I]**: the Constant table was not decoded. Those values agree with every comparison in the code (`IN (0,2)`, `= 1`, `= 3`, and the UI lambdas compare against `0`, `2` and `3`).

---

## 4. Run ids, rebinding, stale runs and closing runs

### 4.1 `local-` ids [V]
- `GameTracker.StartRunLocked(info)` sets `_bzRunId = info?.BzRunId ?? $"local-{Guid.NewGuid():N}"`. That is `local-` followed by 32 lowercase hex characters. This happens when the run-init memory read yields no server id.
- `GameTracker.IsLocalRunId(s)` is `s != null && s.StartsWith("local-", StringComparison.Ordinal /*4*/)`.

### 4.2 Rebinding to a server id [V]
- `ResolveRunInitLocked(info)` does nothing when `info.BzRunId == _bzRunId`. If the current id is local and the run has not ended, it calls `RebindRunLocked`. Otherwise it calls `EndRunLocked(…, false)` and then `StartRunLocked(info)`.
- `RebindRunLocked` sets `_bzRunId = newId` and emits `RunReboundEvent { Type = 9, TimestampUtc = UtcNow, BzRunId = newId, OldBzRunId = oldId }`.
- `MainWindowViewModel.OnEventDetected` handles a `RunReboundEvent` by calling `store.RebindRun(old, new)` and `client.Enqueue(0)` (a nudge) and then posting `QueueRunsRefresh`. **The rebind event is not stored as a row.**
- **Likely bug.** This is **[I]**, medium confidence. SQLite was not run. In the **rename** path, which is the common case when the server id has no run row yet, step 2 moves `events.runId` to `$new` *before* step 3 renames the `runs` row. The write connection has `foreign_keys=ON`, and the foreign key is immediate, so the statement-end check should fail with `FOREIGN KEY constraint failed` whenever the local run already has events. The transaction then rolls back.
  - The exception lands in `OnEventDetected`'s outer catch. That catch re-queues `(ev, json)` into `persistRetryQueue` (see §4.4).
  - The retry later **inserts the `RunRebound` event as a row** under the new id, which creates a fresh `runs` row for it.
  - The `local-…` run is left orphaned. It never uploads, because `local-` runs are filtered out.
  - A Rust port should insert or rename the parent row first, or use `PRAGMA defer_foreign_keys=ON` inside that transaction.

### 4.3 Run start and end bookkeeping [V]
- **End.** `RunEndedEvent` sets `endedUtc` and `outcome` through UpsertRunInternal. No other code sets `endedUtc`.
- **Resume or dedupe.** For a `RunStartedEvent`, the code does `started = HasRunStarted(id); ClearRunEnd(id); if (started) { refresh; return; }`. So a duplicate `RunStarted` for a known run, for example after an app restart mid-run, re-opens the run without inserting a second `RunStarted` row. `ClearRunEnd` runs unconditionally; on a new id it is a no-op.
- **Finished-run guard.** `MainWindowViewModel` passes `id => !disposed && store.IsRunEnded(id)` into `GameManager` and then `GameTracker._isRunEnded`. `GameTracker.IsFinishedRunId(id)` checks an in-memory `_endedBzRunIds` set and then this delegate, and swallows exceptions as `false`. `EnsureRunLocked` uses it to drop a run-init whose server id already ended, so the tracker starts a fresh `local-` run instead of reviving the ended one.
- **Stale runs.** "Stale" is used only for upload eligibility. A run that never got `RunEnded` becomes uploadable once its newest `events.timestampUtc` is at least `StaleRunUploadAge = 6 h` before `now`. **Nothing ever closes it**: `endedUtc` stays NULL.
  - The UI treats a run as "live" with `IsRunLive(run) = run.EndedUtc == null && isConnected && run.LastEventUtc != null && UtcNow - LastEventUtc < 30 min`. `LiveActivityWindow = TimeSpan.FromMinutes(30)`.
  - Only index 0 of the list (the newest `startedUtc`) is ever checked for liveness.
- **Cleanup.** There is no automatic pruning, retention policy or VACUUM. Rows are deleted only by the user's `DeleteRun`. **[V]** from the absence of other `DELETE` literals.

### 4.4 Write-failure retry (in the VM, not the store) [V]
- When `InsertEvent` (or `RebindRun`) throws, `OnEventDetected` catches the exception. If `TrySerialize(ev)` succeeds and the queue holds fewer than **256** items, it enqueues `(ev, json)` into a `ConcurrentQueue`. Otherwise the event is dropped and the UI is told so through a posted lambda.
- `SchedulePersistRetry` uses a CAS flag. It runs `Task.Run`, which waits `PersistRetryDelay = 5 s` and then calls `DrainPersistRetryQueue`. That method does `TryPeek`, then `InsertEvent`, then `client.Enqueue(id)`, then `TryDequeue`, and loops. The first failure stops the loop and the method reschedules itself while the queue is not empty.
- Retried items keep the original event's `TimestampUtc`, but they get the next `seq`. So `seq` order can differ from timestamp order. **[I]**

---

## 5. Sync bookkeeping and "SyncStatus"

`events.batchSyncStatus` is the live state machine. **[V]**

| Value | Name | Written by | Meaning |
|---|---|---|---|
| 0 | Pending | The column default on INSERT | Waiting to upload. |
| 1 | Sent | `MarkBatchSent` after a 2xx | Uploaded. |
| 2 | Failed | **Never written** in this build | Still treated as retryable (`IN (0,2)`). |
| 3 | DeadLetter | `MarkBatchDeadLetter` (single-event 413), and migration v6 (events with no `EventId`) | Skipped for good. |

An event is uploadable when its status is 0 or 2 and it is not in backoff. Its payload must have `EventId` and a non-local `BzRunId`, and its run must be ended or stale (6 h). See §3.

**What the UI shows** is computed in `MainWindowViewModel`. **[V]**
- **Run list.** `RunDisplay.IsSynced` is `GetSyncedRunIds().Contains(run.Id)`. That is true when the run's id is not local and **at least one** of its events has status 1.
- **Selected-run header (`RunSyncSummary`).** It is built from `ListEvents(runId)`, where `pending = count(SyncStatus is 0 or 2)`, `skipped = count(SyncStatus == 3)` and `ongoing = run.EndedUtc == null`. The cases are checked in this order:

| Condition | Text shown |
|---|---|
| No events | `""` |
| Sync disabled, or the id starts with `local-` | "Local only" |
| No pending and no skipped events | "Synced" |
| Pending events and no skipped events | "Uploads when the run ends" if ongoing, otherwise `"Syncing — {pending} pending"` |
| No pending events and some skipped events | `"Synced — {skipped} skipped"` |
| Both pending and skipped events | "Uploads when the run ends" if ongoing, otherwise `"Syncing — {pending} pending · {skipped} skipped"` |

---

## 6. Query semantics behind the UI lists [V]

- **Runs list.** This is `ListRuns(200)`, ordered by `startedUtc DESC`, which is an ISO string sort. There is **no paging or offset** and no filters, so only the 200 most recently started runs are visible. The view model clears and rebuilds `Runs` on every refresh. If the user is not following the live run, it keeps the previously selected id; otherwise it selects the first run.
- **Events list.** This is `ListEvents(runId)` for the selected run: every row, ordered by `seq ASC`. The view model derives durations, encounter info and combat nodes in memory. It reselects the previously selected event id when that id still exists.
- **Batch upload.** This is ordered by `events.id ASC` (insertion order across runs), 200 at a time.
- **Batch metadata.** This is `GetRunSummaries`, in no particular order.

---

## 7. Suggested Rust layout (rusqlite, keeping the schema compatible)

```
persistence/
  mod.rs        // pub struct EventStore { write: Mutex<Connection>, path: PathBuf }
  schema.rs     // BASE_DDL (verbatim), migrate(&Connection) keyed on user_version 0→6, backfill_run_stats()
  queries.rs    // const SQL strings, verbatim from §3
  model.rs      // RunSummary, EventSummary, EventDetail, UnsentEvent, #[repr(i32)] SyncStatus {Pending=0,Sent=1,Failed=2,DeadLetter=3}
```

- **Opening.** Open the write connection with `journal_mode=WAL; foreign_keys=ON; busy_timeout=3000`. For reads, open a fresh `Connection` per call, or a small pool (`r2d2_sqlite`), with `busy_timeout=3000` and `OpenFlags::SQLITE_OPEN_READ_WRITE | CREATE` to match. Read-only would also be fine.
- **Locking and transactions.** Put every write behind the `Mutex`. Use `conn.transaction()` for `insert_event`, `rebind_run` and `delete_run`.
- **Bundled SQLite.** Use rusqlite's `bundled` feature so that `json_extract` and `json_set` are available. The schema depends on JSON1.
- **Timestamps.** Write them byte-compatible with .NET `"o"` (`%Y-%m-%dT%H:%M:%S%.7fZ`, always 7 fractional digits, `Z`). String comparisons in `GetBatchSendable` and `ORDER BY startedUtc` rely on this. Parse them leniently (RFC 3339).
- **Payload.** Store `type` as the PascalCase enum name. Keep the payload JSON in PascalCase with `EventId` and `BzRunId`, because the SQL `json_extract`s those keys.
- **Dead columns.** Keep them (`realRunId`, `syncStatus`, `attemptCount`, `nextRetryUtc`) so that existing `history.db` files open unchanged. Write `syncStatus = 0` on insert. Bump `user_version` only for new migrations (7 and up).
- **Fixes worth making.**
  - In the rebind rename path, rename `runs` first, then move `events`.
  - Optionally persist `batchAttemptCount` and `batchNextRetryUtc` if you want backoff to survive restarts. The C# build doesn't.

---

## 8. Unknowns

- The numeric values of `SyncStatus` (0 to 3) are inferred, because the Constant table was not decoded. They agree with every comparison in the code.
- That the `RebindRun` rename path fails under `foreign_keys=ON` is inferred from SQLite semantics and was not observed at runtime. How often it happens depends on whether events land under the `local-` id before the rebind.
- Whether `GameEvent.TimestampUtc` is always `DateTimeKind.Utc`. If it is not, the `"o"` strings change shape, and string comparisons with `$now` and `$stale` could misorder.
- In-memory backoff timing and `LimitBySize` and `SplitAndRetry` details belong to `EventSubmissionClient`, which is outside this doc's scope. Reflection-based callers of `LoadEvent` were not checked.
