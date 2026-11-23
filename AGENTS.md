# Agent instructions

## Context

- Read `CONTRIBUTING.md` for build, check and local-patch conventions.
- Read `AGENTS_TODO_AND_STATUS.md` for the active batch, validation and handoff; tasks live one per file in `TODO/<epic>/` (Todo app vault format).
- User decisions are inline `DECISION (user, <date>)` comments at the code they govern; find them with `grep -rn 'DECISION (' apps crates` before re-asking.
- Vault (Markdown-folder) work: read the module docs at the top of `apps/todo/schema/src/components/vault.rs` and `crates/organiser-vault/src/lib.rs`, and use the `vault-format` skill.
- Read relevant sections and source files only; avoid unrelated history or full-file dumps.

## Working rules

- Preserve user and other-agent changes. Do not stage, commit, reset, or create branches unless requested.
- Work in finite batches with explicit scope and acceptance checks.
- Fix root causes and add regression coverage where practical.
- Ask before making unresolved product, API, or interaction decisions. Do not invent policy.
- Avoid unrelated refactors. Reformatting is fine (`cargo fmt --all`).
- Never claim tests, visual verification, or audits were completed without evidence.

## Architecture invariants

- Layering: primitives → components → apps. Shared logic lives in `crates/` (`organiser-vault`, `organiser-qt`, ...), not copied between apps.
- Every GUI is Kirigami (Qt/KDE) and follows the Plasma style; no app themes and no libadwaita look. Logic lives in the app's Qt-free `core.rs`, `backend.rs` stays a thin cxx-qt shell, QML only draws. Colours that are user data (tags, epics, sticky palette) are the only hard-coded colours.
- A vault never loses user files: deletes are soft (`.archive/`), conflicts keep both versions, imported (symlinked) folders are never removed or written outside their task files.
- `weaver` is pinned by `rev` in the root `Cargo.toml`; `.cargo/config.toml` is a local-only patch and `Cargo.lock` must not be committed with it active.

## Execution

- Use the relevant skill for specialised workflows (`bounded-sprint`, `vault-format`).
- Run targeted tests before the workspace: `cargo test -p <crate>`, then `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`.
- Compilation does not establish visual or interaction correctness.
- Record actual validation evidence and handoff in `AGENTS_TODO_AND_STATUS.md`; record unresolved work as `TODO/` task files. Keep no history log.
- Finish with a concise summary of changes, verification, and remaining issues.
