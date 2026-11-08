## Agent skills

### Issue tracker

Issues and specs live as local markdown files under `.scratch/<feature-slug>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`), recorded as a `Status:` line in each issue file. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

## Project docs

### Documents

Documents about this project, including how it is implemented, live in `docs/documents/<slug>.md` (or `.html`). `docs/documents/INDEX.md` is the entry point: it lists every document as one bullet, `- [Title](<slug>.md) — one-line summary`.

- Read the index before working on an area, and open the documents relevant to it.
- When you add, rename or remove a document, update `INDEX.md` in the same change.
- Keep documents in step with the code they describe: if a change makes a document wrong, fix the document in the same change.

### HTML documents

When a document (or a record) is generated as HTML:

- **Theme:** Rosé Pine Moon (dark) and Dawn (light) only, with an icon theme switcher: Auto (monitor), Moon (moon), Dawn (sun). Each name is the icon's tooltip and `aria-label`.
- **Code blocks:** highlight with Shiki; one code-theme icon button cycles Rosé Pine, Catppuccin and Everforest.
- **Diagrams:** write them in Mermaid and render with Pele ([pele.run/api](https://pele.run/api)). Make them expandable and zoomable, with expand, close and zoom icon controls.
- **Styling:** small border radius (about 3–6px), and thin scrollbars styled to match the theme.
- Include a table of contents.
- **Thai body text:** use a looped Thai font (e.g. Noto Sans Thai Looped).

`docs/records/2026-11-08.001-build-advisor-agent.html` follows all of these; start a new HTML document from it.

### Records

Discussion records live at `docs/records/yyyy-mm-dd-<slug>.md`, dated the day the discussion took place. Each record captures one discussion: what was discussed, what was decided, and any open questions.
