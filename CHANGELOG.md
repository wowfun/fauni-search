# CHANGELOG

## 2026-04-09
### Added
- Added the `110-image-search` topic docs (`spec.md`, `plan.md`, `testing.md`) and the first runnable image-search slice across the Rust app, Python sidecar, shared workspace UI, and `scripts/local/smoke-image-search.sh`.

### Changed
- Moved shared non-text query-mode workspace rules into `008/search-workspace.md`, keeping `110-image-search` focused on image-specific planning.
- Tightened image-query v1 contracts and validation around `not_ready`, invalid uploads, temporary query assets, and cleanup of expired query-image files.
- Expanded image-query inputs from temporary uploads to reusable library objects, covering both `image` and `document_page` with locator-aware embeddings and smoke coverage.
- Added clipboard paste support for shared `Image` mode on top of the existing temporary query-image upload flow.

## 2026-04-08
### Added
- Added `specs/010-local-operations-and-automation/spec.md` plus `scripts/local/status.sh` and `scripts/local/check.sh` as the first dedicated local-operations and automation slice.
- Added the first Playwright UI smoke for the current `100-text-search` happy path, including `--dev` runtime reuse/self-start behavior and stable UI test selectors.

### Changed
- Added `.env` / `.env.dev` profile separation, `--dev`, `run.sh --detach`, JSON status/smoke output, pid-aware stopping, and automatic Qdrant startup to make local service orchestration reproducible and automation-friendly.
- Hardened the text-search execution path with async imports, chunked Qdrant writes, longer first-load timeouts, and real multi-page PDF `document_page` indexing via locator-aware `document_embedding`.
- Upgraded the workspace from a basic flow to a three-column UI with app-served previews, persistent smoke PDF previews, and per-result score display as a same-response ranking hint.
- Expanded the Playwright UI validation slice to cover explicit `not_ready` feedback and invalid-import rejection feedback while continuing to reuse or self-start only the isolated `--dev` runtime.
- Split shared search-workspace UI constraints out of `100-text-search` planning into `specs/008-ui-ux/search-workspace.md`, keeping `008` as the UI fact-source entry point and leaving `100` with text-specific current-stage rules.

## 2026-04-06
### Added
- Added the remaining base specs plus the long-lived `100-text-search` topic docs, establishing the current fact-source layout.
- Added standardized local runtime assets, minimal operator docs, and the TATDQA fixture set needed to build and validate the first text-search loop.
- Added the first runnable text-search MVP slices across the Rust app, Python sidecar, UI workspace, model download/smoke tooling, and Qdrant-backed search path.

### Changed
- Reorganized the base specs into cleaner fact-source boundaries, normalized terminology and naming, and tightened repository-wide spec/testing/documentation rules.
- Tightened the public search and interface contracts around `not_ready`, result/detail separation, preview delivery, library/import/detail APIs, and sidecar protocol shape.
- Standardized the local runtime around `.venv`, `scripts/local/*`, root env-driven ports, repo-local tool bootstrap, Hugging Face download controls, and predictable startup/health behavior.
- Evolved the app from placeholders to a working text-search workspace with real ColQwen + Qdrant indexing/search, operator docs, and a browser-driven validation path.

## 2026-04-05
### Added
- Added the initial `fauni-search` specs baseline covering foundation, architecture, state/data-model, ingestion/indexing, and search.

### Changed
- Established the initial specs hierarchy, terminology rules, and fact-source discipline for the project.
