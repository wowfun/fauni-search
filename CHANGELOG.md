# CHANGELOG

## 2026-04-06
### Added
- Added `specs/005-provider-capabilities-and-profiles/spec.md`, `specs/006-runtime-and-execution/spec.md`, `specs/007-storage-and-persistence/spec.md`, `specs/008-ui-ux/spec.md`, and `specs/009-interfaces-and-protocol-contracts/spec.md` as the remaining base specs for providers, execution, persistence, app experience, and interface/protocol contracts.

### Changed
- Reorganized the base hierarchy so `002-state-and-data-model` became the broad state/data-model fact source, `001-architecture` narrowed to pure architecture, `003` broadened into `003-ingestion-and-indexing`, and the remaining cross-cutting facts were split into dedicated specs for execution, persistence, UI/UX, and interface/protocol contracts.
- Normalized the base specs with Chinese-first terminology, `编号 中文名 (English)` H1 titles, cleaner cross-spec handoffs, and no explicit `V1` wording.

## 2026-04-05
### Added
- Added the initial `fauni-search` specs baseline covering foundation, architecture, state/data-model, ingestion/indexing, and search.

### Changed
- Established the initial specs hierarchy, terminology rules, and fact-source discipline for the project.
