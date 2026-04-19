# CHANGELOG

## 2026-04-19
### Changed
- Added the first runnable `005-provider-capabilities-and-profiles` slice across the Rust app and Settings workspace, covering durable provider/model defaults, library overrides, resolved-model summaries, `remote_http` as a configurable-but-not-executable shell, and black-box API/UI coverage for the new control surface.
- Simplified the public `005` surface to a strict pre-stable `provider_id + model_id` contract by removing `selection_kind`, `variant`, and the unused `region` field, tightening `multivector` model validation, and surfacing the exact active model more directly across Settings and shared provider summaries.
- Completed the current search-controls slice by adding opaque cursor pagination, `visual_unit.kind` / `path_prefix` / `source_type` / `time_range` filtering, and richer `debug` search diagnostics in the Rust app, while tightening the shared `004/009` specs and adding narrow search-response / search-plan coverage.
- Extended the shared search workspace with a lightweight search-filter dock, deterministic `Load more` pagination behavior backed by saved search snapshots, and UI coverage for filter payload wiring and local invalid-time-range rejection.

## 2026-04-19
### Changed
- Completed the UI TypeScript migration with a real `pnpm --dir ui typecheck` path, added `typescript` to the Vite workspace, tightened the local fast-check contract/docs to include UI typecheck before the existing UI build step, and fixed the remaining narrow DOM/locator typing gaps without changing workspace behavior.

## 2026-04-17
### Changed
- Migrated the Vite UI entry from plain JavaScript to TypeScript, adding typed UI state and API payload models plus a local `tsconfig`/`vite-env` baseline without changing workspace behavior.

## 2026-04-16
### Changed
- Added the external demo capability guide, clarified query-mode terminology, and rewrote `README.md` into a newcomer-first entry that points detailed operator guidance to the docs set.
- Split the UI into dedicated `Search` and `Inventory` workspaces, with focused spec and Playwright coverage for inventory filtering and narrow-screen behavior.
- Refactored the Rust crate out of the monolithic `src/lib.rs` and expanded black-box router-level integration coverage for restart persistence, source management, imports, search, and job observation.

## 2026-04-15
### Added
- Added the first restart-persistence slice for libraries and active indexes, backed by `${APP_RUNTIME_DIR}/state.sqlite`, with Rust coverage for snapshot roundtrip, restart-time id continuation, missing-index downgrade, and watcher reseeding.

### Changed
- Switched multivector indexing to stable `index_{library_id}_{index_line}` namespaces with staged Qdrant writes, alias-based activation, `on_disk: true` vectors, app/sidecar batch limits, and explicit non-compatibility for legacy runtime-token, `text_search_*`, and direct physical `index_*` collections.
- Restored durable library, source, visual-unit, and active-index state at boot while keeping jobs, temporary query assets, and watcher scratch ephemeral; tightened the shared `002/003/006/007/009/010` specs and docs around those restart and alias semantics.
- Preserved the open detail preview panel across workspace polling so unchanged image, video, and PDF previews no longer remount or reload.

## 2026-04-14
### Added
- Added the first runnable `140-library-source-management` slice across specs, app, UI, Playwright, and local smoke tooling, covering source-root CRUD, rule-based inventory, library/root `refresh` and `rescan`, and watcher-driven incremental refresh.

### Changed
- Extended the library/source model and validation around root ownership, active/inactive source state, out-of-scope invalidation, disabled-root behavior, and watcher debounce queueing.
- Preserved source-root drafts and focused editable inputs across workspace polling so in-progress UI edits are no longer overwritten or blurred.

## 2026-04-11
### Added
- Added the initial `140-library-source-management` topic docs (`spec.md`, `plan.md`, `testing.md`) for library-scoped source roots, source inventory, `refresh` / `rescan`, and watcher-driven incremental refresh.

### Changed
- Tightened the shared `002/003/008/009` fact sources around source roots, source inventory, and source-management control-plane contracts.

## 2026-04-10
### Added
- Added the first runnable `130-document-search` slice across the app, sidecar, shared workspace UI, and `scripts/local/smoke-document-search.sh`.

### Changed
- Updated the shared `Document` mode docs, troubleshooting notes, and local operator guidance around the current document-query lifecycle.

## 2026-04-09
### Added
- Added the initial `130-document-search` topic docs (`spec.md`, `plan.md`, `testing.md`) to define document-query retrieval, optional page-range inputs, and the first PDF-only current-stage scope.
- Added the `110-image-search` topic docs (`spec.md`, `plan.md`, `testing.md`) and the first runnable image-search slice across the Rust app, Python sidecar, shared workspace UI, and `scripts/local/smoke-image-search.sh`.
- Added the initial `120-video-search` topic docs plus a local video-artifact extraction helper for local-only smoke fixtures.
- Added the first runnable `120-video-search` slice across the Rust app, Python sidecar, shared workspace UI, and `scripts/local/smoke-video-search.sh`, covering video query uploads, `/search/video`, library-object reuse, optional time-range selection, and Playwright/runtime smoke coverage.

### Changed
- Moved shared non-text query-mode workspace rules into `008/search-workspace.md`, keeping `110-image-search` focused on image-specific planning.
- Extended the base `002/003/004/009` fact sources for `120-video-search`, including `video_segment` locators, `/search/video`, and video query-asset / sidecar protocol shapes.
- Extended the base `002/004/008/009` fact sources for `130-document-search`, covering document-query temp assets, `start_page/end_page` locators, `/search/document`, `document_query_embedding`, and shared `Document` mode workspace expectations.
- Refined `130-document-search` v1 around uploaded PDFs and `document_page` reuse in the workspace, numeric page-range inputs, API-level `source_id` reuse, and `video_segment` as an optional extension hit rather than a first-pass gate.
- Hardened the `120-video-search` runtime and validation path with larger upload support, explicit negative-path coverage, and reusable `video_segment` follow-up queries in smoke and workspace flows.
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
