---
name: 0003 Content reuse state model
status: Accepted
date: 2026-04-30
---

## Context

FauniSearch 需要让同一份内容在不同路径、不同库、不同 source root 下复用处理结果。一个常见场景是目录被重命名，例如 `data/example/lib` 改成 `data/example/lib1`。路径变了，新的 `Source` 必须存在；但文件字节没有变，PDF 页、视频片段、Unit 和 embedding 不应该重新创建。

当前实现里，`Content`、`VectorSpace` 和 `ContentIndex` 仍然带 `library_id`，`content_id` 由顺序号生成，部分内容定位还直接包含路径。结果是同一文件换路径后会生成新的 Content、Asset、Unit 和 ContentIndex，系统只能重新处理和重新写向量。

这不符合 Content-centered model 的目标。更小的模型应该把位置事实和内容事实分开：`Source` 表示某个库在某个 `source_uri` 看到内容；`Content` 表示 Source 原始内容身份；`Asset` 和 `Unit` 表示内容内部结构和索引执行单元；位置由单独关系表承接。

## Decision

FauniSearch 将采用基于 Source Content 复用的状态模型。`Content` 使用随机 UUID 作为主键，不把 hash 直接作为 `content_id`。`Content` 只表示 Source 原始内容身份，不保存 `content_type`、`media_type` 或 `payload_ref`。内容判定只在 Source Content 层做；Asset 和 Unit 不再单独做字节指纹计算。

Source Content 的身份判定分两层：

- 快速层使用 `size_bytes + fast_fingerprint` 找候选。`fast_fingerprint` 可以是首尾采样哈希或其他低成本方法，只用于候选筛选。
- 精确层使用 SHA-256。SHA-256 懒计算，仅在存在候选且需要确认合并时计算。没有候选时可以创建 `sha256 = NULL` 的 provisional Content。

没有 SHA-256 的两个 Content 不得直接合并。只有 SHA-256 相等，才允许把新 Source 指向已有 Source Content。若候选无法通过现有 Source 或存储位置计算 SHA-256，应创建新的 provisional Content，而不是冒险合并。

Asset 不保存路径，也不表示 Source 内的位置。Asset 表示 Source Content 内部的结构化资产，例如 PDF 第 3 页、视频 00:10-00:20 片段、图片整体或文本块。Source 到 Asset 的位置关系由 `source_asset_locations` 表表达。

Unit 表示 Asset 下的索引执行单元，例如 page image unit、keyframe image unit、text unit。Unit 不再引用 Unit Content。Unit 自身就是可 embedding 的稳定执行单元，由 `asset_id + unit_type + locator_json + derivation_signature` 复用。

`derivation_signature` 是派生规则的版本指纹。它包含会影响派生结果的处理规则，例如 PDF renderer、DPI、视频切片策略、关键帧策略或文本 chunk 策略。相同规则必须得到相同 signature；会改变 Asset 或 Unit 结果的规则变化必须得到不同 signature。

Embedding 时仍然需要把 Unit 物化成模型可接受的输入，例如 PDF 页图、视频关键帧或文本片段。这个物化结果可以是临时文件、内存字节或可清理缓存，不进入核心状态模型。若未来需要持久化派生载荷，应作为 Unit materialization 缓存单独引入，而不是把派生载荷重新建模为 Content。

为了避免每次遇到相同 Content 都重新跑完整处理链路，系统为 Content 增加端到端索引完成标记。`content_e2e_index_states` 只记录某个 Content 在某个 `pipe_signature` 和 `vector_space_id` 下是否已经完成索引。它不保存 Asset / Unit 计数、物化载荷状态或 per-unit 向量引用；这些仍由 Asset、Unit 和 UnitIndex 表达。

最小关系如下：

```text
Library -> SourceRoot
Library -> Source
Source -> Source Content

Source Content -> Asset
Asset -> Unit
Source Content + PipeSignature + VectorSpace -> ContentE2eIndexState

Source -> SourceAssetLocation -> Asset

Unit + VectorSpace -> UnitIndex
UnitIndex.vector_ref -> retrieval backend payload
```

## State Tables

### `contents`

`contents` 是全局表，不带 `library_id`。

```text
content_id uuid primary key
size_bytes nullable
fast_fingerprint nullable
sha256 nullable
created_at_ms
```

`content_id` 是稳定对象 ID，不承载内容判定语义。`sha256` 是可选精确身份字段；`sha256 IS NULL` 表示该 Content 仍是 provisional。若 `sha256` 存在，应建立唯一约束，避免 verified Content 重复。

`Content` 不保存类型解释和载荷位置。类型解释由 `Source.source_type` 与 `Source.media_type` 承接；载荷位置由 `Source.source_uri` 或后续缓存层承接。

### `sources`

`sources` 是库内位置表。

```text
source_id primary key
library_id
source_root_id nullable
source_uri
relative_path nullable
source_type
media_type
status
status_reason nullable
source_content_id
observed_size_bytes nullable
observed_modified_at_ms nullable
```

`source_uri` 是 Source 的规范化定位字符串，例如 `file:///home/user/project/report.pdf` 或后续外部 URI。`relative_path` 只在 Source 属于某个 source root 时存在，用于 root 内唯一性和展示。`Source` 可以因为路径变化而新增或更新，但它引用的 `source_content_id` 可以复用已有 Content。

### `assets`

`assets` 是全局内容结构表，不带 `library_id`，也不保存 path。

```text
asset_id uuid primary key
source_content_id
asset_type
locator_json
derivation_signature
```

唯一性由以下字段决定：

```text
source_content_id + asset_type + locator_json + derivation_signature
```

同一 PDF 内容的第 3 页只有一个 Asset。它可以出现在多个 Source 中，具体出现位置由 `source_asset_locations` 表表达。

### `content_e2e_index_states`

`content_e2e_index_states` 是 Content 的端到端索引完成标记表。

```text
content_id
pipe_signature
vector_space_id
indexed_at_ms
```

唯一性固定为：

```text
content_id + pipe_signature + vector_space_id
```

`pipe_signature` 表示从 Source Content 推导 Asset、Unit 和模型输入的整体处理规则，可以包含 PDF 页枚举策略、视频切片策略、文本 chunk 策略和默认 Unit 派生规则。

这张表只在索引完成后写入。行存在表示该 Content 在该 `pipe_signature + vector_space_id` 下已经完成 Asset / Unit 生成、Unit embedding、UnitIndex 写入，并且这些状态已经提交。行缺失就表示不能走完成快路径，系统必须重新执行或恢复端到端索引流程。

UnitIndex 仍然保留。`content_e2e_index_states` 是粗粒度完成标记，UnitIndex 保存每个 Unit 的 `vector_ref` 和搜索证据。

### `units`

`units` 是全局索引执行单元表，不带 `library_id`，也不保存 path。

```text
unit_id uuid primary key
asset_id
unit_type
derivation_signature
locator_json nullable
```

唯一性由以下字段决定：

```text
asset_id + unit_type + derivation_signature + locator_json
```

Unit 的唯一性就是索引输入身份。需要 embedding 时，系统按 Unit 的 Asset、locator 和 `derivation_signature` 生成模型输入；生成结果可以临时存在，不是核心 durable truth。

### `source_asset_locations`

`source_asset_locations` 表达某个 Source 中出现了某个 Asset，以及如何向用户定位。

```text
source_id
asset_id
locator_json
source_position nullable
visibility
```

唯一性至少应包含：

```text
source_id + asset_id
```

搜索命中 Asset 后，按当前 search scope 过滤 locations。结果返回的是 Source + locator，而不是把 Asset 本身当成路径位置。

### `vector_spaces`

`vector_spaces` 是全局表，不带 `library_id`。

```text
vector_space_id primary key
provider_id
model_id
model_version
model_revision nullable
vector_type
adapter_signature nullable
```

VectorSpace 只表示哪些向量可以比较，不保存 active/staging、collection 名或库级状态。

### `unit_indexes`

`unit_indexes` 是全局表，不带 `library_id`。

```text
unit_id
vector_space_id
status
visibility
vector_ref nullable
job_id nullable
error_summary nullable
```

唯一性固定为：

```text
unit_id + vector_space_id
```

它表示某个 Unit 在某个 VectorSpace 下是否已经可检索。Source、Asset 和 Location 不承载向量可见性。

## Source Transaction Boundary

Content 命中不等于 Source 可以跳过处理。系统必须先确认 `content_e2e_index_states(content_id, pipe_signature, vector_space_id)` 存在，才允许把新 Source 走完成快路径。

Source 级处理采用提交边界：

```text
1. 扫描 Source，确认或创建 Source Content
2. 检查 ContentE2eIndexState(content_id, pipe_signature, vector_space_id)
3. 若完成标记存在，复用已有 Asset、Unit 和 UnitIndex，只提交 Source、SourceAssetLocation，并让 Source active
4. 若完成标记缺失，执行端到端索引：生成缺失 Asset / Unit，物化 Unit 输入，执行 embedding，写入向量
5. 在一个 SQLite transaction 中提交 Source、SourceAssetLocation、Asset、Unit、UnitIndex ready 和 ContentE2eIndexState
6. Source 变为 active
```

如果进程在步骤 4 后、步骤 5 前终止，检索后端可能留下 orphan vector，但 SQLite 中没有 `ContentE2eIndexState`，搜索不得把该 Content 视为完成。下次处理同一 Source Content 时，系统必须重新执行或恢复端到端索引流程，而不是因为 Content 已存在就跳过。

如果进程在创建 provisional Content 后终止，后续运行可以复用该 Content 作为候选，但仍必须检查 ContentE2eIndexState。`Content exists` 不能作为处理完成的判断条件。

Source 的 active 可见性由 `SourceAssetLocation + ContentE2eIndexState` 共同决定。Location 不承载向量可见性，但它决定当前 search scope 能否把命中的 Asset 还原到某个 Source 位置；UnitIndex 仍然是搜索读取具体向量引用的事实源。

## Examples

### Path rename

原始状态：

```text
Source S1 = /data/example/lib/report.pdf
Content C1 = report.pdf bytes
Asset A1 = C1 page 3
Unit U1 = A1 page image unit
UnitIndex(U1, colqwen_vector_space)
ContentE2eIndexState(C1, pdf_pipe:v1, colqwen_vector_space)
Location(S1, A1, page=3)
```

目录改名后：

```text
Source S2 = /data/example/lib1/report.pdf
S2.source_content_id = C1
Location(S2, A1, page=3)
```

系统先命中 `ContentE2eIndexState`，因此不创建新的 Asset、Unit 或 UnitIndex。搜索 scope 若是 `lib1`，命中 `U1` 后只返回 `S2` 的 location。

### PDF

同一个 PDF 的第 3 页由内容和派生规则决定：

```text
Asset key = C_pdf + document_page + page=3 + pdf_page:v1
Unit key = A_page3 + page_image + pdf_page_image:v1:dpi144
```

如果 PDF renderer 或 DPI 改变，`derivation_signature` 改变，系统会创建新的 Unit，不复用旧 embedding。

### Video

视频片段 Asset 和关键帧 Unit 分开：

```text
Asset key = C_video + video_segment + start=10000,end=20000 + video_segment:v1:fixed10s
Unit key = A_segment + keyframe_image + video_keyframe:v1:middle
```

同一个视频换路径后复用 Asset 和 Unit。关键帧策略改变后，只创建新的 Unit，Source 与原始视频 Content 不变。

### Fingerprint collision

两个大文件可能有相同 `size_bytes + fast_fingerprint`。这只会让它们互为候选。系统必须计算 SHA-256 才能合并：

```text
same size + same fast fingerprint + same sha256 -> merge
same size + same fast fingerprint + different sha256 -> create new Content
same size + same fast fingerprint + sha256 unavailable -> do not merge
```

## Consequences

路径变化只影响 Source 和 Location。只要 `ContentE2eIndexState` 存在，就不再导致 Asset、Unit 和 UnitIndex 重建。

大文件首次进入系统时可以避免立即计算 SHA-256。系统先创建 provisional Source Content；只有出现候选合并需求时，才为当前文件和候选 Content 计算 SHA-256。

Asset 和 Unit 从库内对象变成内容结构对象。它们不能再保存 `library_id`、`source_uri` 或搜索可见性。搜索结果需要通过 `source_asset_locations` 还原到当前 scope 内的 Source 位置。

错误复用的防线从“每个派生产物算 hash”变成“Source Content 精确确认 + derivation_signature 版本化”。这减少派生载荷的重复计算，但要求处理规则变化时必须同步更新 signature。

旧 schema 不能平滑迁移到该模型。落地时应继续采用破坏性 schema bump，并提示用户 reset 或 cutover runtime。

## Alternatives Considered

### `content_id = sha256`

直接用 SHA-256 做 Content 主键最简单，但会强迫大文件在首次导入时完整读一遍。这个成本对大 PDF、视频和 agent-first `find` 场景不理想。

### 对 Asset 和 Unit 都计算指纹

这能获得强身份判定，但会让 PDF 页图、视频关键帧、OCR 文本等派生产物都先物化再 hash。系统会为了判断能否复用而提前做昂贵工作。

### Asset 仍然属于 Source

这能简化搜索位置还原，但会让相同 Source Content 在新路径下重复创建 Asset 和 Unit。它只能复用 embedding，不能复用内容结构和派生边界。

### 只复用 UnitIndex

只复用 embedding 不能解决存储膨胀。相同 PDF 在多个路径下仍会生成多套 Asset 和 Unit 记录，后续诊断、清理和搜索证据都会变复杂。
