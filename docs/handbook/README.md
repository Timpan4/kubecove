# Engineering Handbook

Short, canonical rules for working in this repo. The handbook is the source of truth; module READMEs and `AGENTS.md` link back here. If you find a contradiction with another doc, the handbook wins — open a PR to fix the other doc.

## When to consult what

- **About to add a file or folder** → [code-organization.md](code-organization.md). Tells you where it goes and when to make a new folder.
- **A file is getting long** → [file-size-and-split.md](file-size-and-split.md). Soft/hard caps and how to split.
- **About to replace, rename, or remove something** → [hygiene.md](hygiene.md). Dead-code, orphan-file, and duplicate-component policy.
- **About to open a PR or finish an agent task** → [pr-checklist.md](pr-checklist.md). Tickbox list.

## What this handbook is not

- Not the product vision — see [product-vision.md](../product-vision.md).
- Not the target architecture diagram — see [architecture-blueprint.md](../architecture-blueprint.md).
- Not a list of locked-in decisions — see [decisions/](../decisions/).

The handbook is rules and guardrails. Keep each page short enough to read in a minute.
