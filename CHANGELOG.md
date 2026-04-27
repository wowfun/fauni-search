# CHANGELOG

## 2026-04-27
### Changed
- Reworked Settings around runtime-config overlays for provider/model and content-type CRUD, including inheritance restore states and cached provider runtime probes.
- Added modeld-backed local model orchestration with `faus serve --model`, modeld-aware local scripts/status, Qwen3-VL backend wiring, and timestamped rotating `modeld.log`.

## 2026-04-26
### Added
- Added the public App OpenAPI contract and the initial `faus` product CLI covering runtime, Web, library, jobs, and import workflows.
- Added `faus sources` and flag-based `faus search`.
- Added explicit dev smoke coverage for the headless `faus serve` runtime and local script alignment.

### Changed
- Reworked the Rust HTTP/API boundary, route discovery, `/runtime/status`, `faus serve`, `faus web`, and local wrapper responsibilities.
- Replaced the single-row durable state JSON snapshot with structured SQLite tables for persistent library state.
- Improved `faus` CLI structure, help text, client-side diagnostics, and direct App API workflows.
- Simplified the Search and background-task UI, including Inventory-owned imports, status-capsule job progress, and stable video result thumbnails during polling.

## 2026-04-25
### Changed
- Removed the `工具` drawer from the shell: the sidebar now keeps `Search / 库管理 / 设置`, the status capsule opens `Settings > 诊断`, and Inventory owns folded library maintenance beside refresh and rescan.
- Unified the shared workspace UI across Search, Inventory, and Settings: current-library headers now use one toolbar shell, object rows use shared list and detail primitives, and local button, tag, and detail variants were removed.
- Consolidated Inventory source management into one control surface with one library-level `刷新当前库 / 重扫当前库` pair, folded source-root management, folded advanced rules, and cleaner source rows.
- Improved Inventory readability and stability: long paths get more room, counts no longer crowd the path, normal status no longer repeats on each row, and unchanged detail previews stay mounted across polling.
- Unified selected-state styling and shell status semantics, restored green `Ready` text, bounded `.env.dev` Playwright Qdrant cleanup, and moved the remaining frontend `legacy.ts` code into `ui/src/app/*`.

## 2026-04-24
### Changed
- Added `008` UI/UX reduction rules for minimal working surfaces and less repeated explanation.
- Applied those rules to the non-search workspaces: Inventory moved to a compact current-library toolbar with cleaner source rows, Settings dropped chapter-level filler, and the old utility drawer lost duplicate status cards.
- Updated the closest Playwright workspace coverage to match that ownership model, including library creation in `库管理`, Settings-owned overrides, and maintenance flows through the old utility drawer.

## 2026-04-23
### Changed
- Split the shipped frontend monolith onto a dedicated module layout without changing product IA: `ui/src/main.ts` is now a thin bootstrap over `ui/src/app/*`, frontend types moved behind `ui/src/types/*`, the old `ui/src/style.css` monolith was replaced by `ui/src/styles/index.css` plus domain partials, and the Playwright helper monolith was broken into domain-specific `ui/tests/e2e/helpers/*` modules with a stable barrel entry.
- Continued the `020` frontend architecture split by downgrading `ui/src/app/core.ts` and `ui/src/app/events.ts` into thin compatibility/barrel layers over domain modules, lifting shared preview/library-context/jobs/bridge rendering into `ui/src/app/render/shared/*`, and finishing the second-round Playwright helper split so `search` and `workspace` scenarios now live in real subdomain files instead of hidden legacy monoliths.
- Preserved current single-library multimedia search compatibility during that refactor by continuing to send the legacy top-level `library_id` expected by the current `/search/image`, `/search/video`, and `/search/document` handlers, and by normalizing missing `library_id` fields on single-library search results so detail loading and “reuse as query” actions still work after the module split.
- Refined the shipped `008` Search shell to match the new prototype direction: the top status capsule now uses `Ready`, the empty-state headline is centered, the text composer is a true single-line strip with the lens aligned to the input, the formal `Search` action sits beside the library scope selector, and active search feedback is reduced to a simple `搜索中...` line above the results surface.
- Simplified the shipped `008` Search reading surface: redundant result/detail headings and helper copy were removed, `document_page` result rows now use a lightweight static `PDF` placeholder instead of live PDF iframe thumbnails, duplicate document/page cues were dropped from the thumbnail area, and preview/reuse actions now live only on result items instead of being repeated in the detail panel.

## 2026-04-22
### Changed
- Removed the remaining `demo import` / `run demo` assumptions from the shipped Search surface and its closest browser harnesses, so the formal product path now goes through real library creation, real import, and submitted search only.
- Landed the main `008` workspace direction in the shipped UI and prototypes: the shell now uses a lighter brand/status app bar plus left sidebar, Search is composer-first with scope-aware current-library and `所有库` text search, and the supplemental prototype was rebuilt/split around the same software-style IA with `Ready`, `Unified · Native · Powerful`, and the quieter search-first layout.
- Completed a broader Search reading-flow cleanup: readiness copy now distinguishes missing source roots, missing content, config blockers, and in-flight jobs; empty results and config failures use the same reason model; `所有库` results can be grouped, focused, and reused in place; cross-library reuse switches back into the hit library when a query mode requires a bound library; and the results surface now has clearer grouped/focused/default reading states.
- Reshaped `Inventory` / `库管理` and `Settings` into fuller workspaces: `库管理` is now preview-first with a dedicated current-library management/readiness band and direct jump into `Settings > 当前库覆盖`, while `Settings` moved to a chapter-first IA with section roles, summary metrics, and a stronger overview-before-editor flow.
- Reworked the closest Playwright helpers and regressions to match the shipped `008` shell and Search behavior, including live filter toggles, non-demo searchability setup, the new left-sidebar `工具` entry, and the cross-library Search flows described above.

## 2026-04-21
### Changed
- Added and iterated on the first standalone `008` Search HTML prototype, then split it into dedicated `Ready` empty/results pages and trimmed the extra explanatory shell filler so it reads more like the intended product UI.
- Landed the main `008-ui-ux` shell transition in the Vite workspace: Search moved from a three-column chrome to a stage-first desktop layout with a compact app bar, shared utility drawer, compressed reading-state Search stage, and denser result/detail surfaces, while Inventory and Settings shifted to the new list/detail and sectioned flows.
- Expanded the `008` control plane and workspace actions with explicit cancel / retry / resume job flows, library archive / restore, library rename / delete, and maintenance actions such as `rebuild` and `cleanup_retired_vector_spaces`, all backed by focused Rust and Playwright coverage.
- Tightened the surrounding stability and language work for that rollout: current-library popovers now stay anchored across polling, utilities no longer collapse on refresh, prep-state and invalid-time-range regressions were realigned to the live UI, and the main shell/product copy was normalized to Chinese task language where the shipped surface now expects it.
- Renamed the top-level search debug summary field from `repr_kind` to `vector_type` and aligned smoke-script JSON summaries plus operator specs with the new name.
- Added a black-box `search_api` success-path test with an in-process Qdrant stub so imported libraries now verify `debug.vector_type`, `target_content_types`, and the absence of legacy search debug fields at the HTTP layer.

## 2026-04-20
### Changed
- Started the config-file cutover for provider/model/runtime selection with tracked `fauni.config.json`, merged `${APP_RUNTIME_DIR}/runtime-config.json`, `local_sidecar.active_model + version` resolution, config-backed Settings writes, and explicit legacy-runtime cutover tooling for default and `--dev` environments.
- Continued the library/content-type control-plane migration by splitting `library_id` from `display_name`, moving search targeting and skipped-search diagnostics to `target_content_types` / `unsupported_content_types`, and removing the remaining public `index_lines`-style contract remnants from library, settings, and search surfaces.
- Advanced the internal `index_line -> vector_space` refactor by deriving `vector_space_id` from config-backed model state, executing import/source-action/search work per space, preserving partial-success activations under failed jobs, and immediately retiring superseded active spaces on content-type rebinds.
- Added durable retired-`vector_space` inventory, a maintenance cleanup loop, runtime-health snapshots, and library-scoped vector-space diagnostics so execution state, provider probes, and stale Qdrant namespaces are observable and eventually reaped instead of being silently dropped.
- Finished the tracked `ui/` TypeScript cutover for Vite/Playwright support files, split the old monolithic smoke suite into domain specs, promoted `display_name` and runtime-health/vector-space diagnostics in the shared workspace, and added dedicated local `runtime-health` / `check-e2e` operator entry points.
- Removed the remaining compatibility shims for library `name`, sidecar `target_index_lines`, and durable `name` / `active_index_lines`, and added explicit legacy-runtime cleanup tooling for archived runtime data and stale Qdrant collections.
- Standardized the active terminology on `global_content_type` / `library_content_type` / `settings_model_test` and `multi_vector_late_interaction`, updated the related specs/UI/docs, and added a terminology audit to catch regressions.

## 2026-04-19
### Changed
- Renamed the runtime model env contract from `TEXT_SEARCH_*` to `EMBEDDING_*` across local scripts, Rust/provider bootstrap, sidecar runtime loading, test harnesses, and operator docs, while keeping `download-model.sh` aligned with the new names.
- Added the first runnable `005-provider-capabilities-and-profiles` slice across the Rust app and Settings workspace, covering durable provider/model defaults, library overrides, resolved-model summaries, `remote_http` as a configurable-but-not-executable shell, and black-box API/UI coverage for the new control surface.
- Simplified the public `005` surface to a strict pre-stable `provider_id + model_id` contract by removing `selection_kind`, `variant`, and the unused `region` field, tightening `multivector` model validation, and surfacing the exact active model more directly across Settings and shared provider summaries.
- Reframed the current `005` capability surface around `EmbeddingCapabilities`, separating native embedding facts from runtime adapters, limiting Settings model tests to native `text` / `image` inputs, and fixing the local-sidecar draft-test flow so runtime-derived connection details stay display-only.
- Extended Settings model tests to support an optional second native input with independent modality selection, returning both inputs' vectors plus cosine similarity derived from their pooled vectors for same-modality and cross-modality diagnostics.
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
