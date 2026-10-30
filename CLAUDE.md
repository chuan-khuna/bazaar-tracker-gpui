## Agent skills

### Issue tracker

Issues and specs live as local markdown files under `.scratch/<feature-slug>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`), recorded as a `Status:` line in each issue file. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

## Project docs

### Documents

Project-related reference documents live at `docs/documents/<slug>.md`. `docs/documents/INDEX.md` lists every document as one bullet: `- [Title](<slug>.md) — one-line summary`. When you add, rename or remove a document, update `INDEX.md` too.

### Records

Discussion records live at `docs/records/yyyy-mm-dd-<slug>.md`, dated the day the discussion took place. Each record captures one discussion: what was discussed, what was decided, and any open questions.
