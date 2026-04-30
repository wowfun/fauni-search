---
name: 0004 Row-level vector prefiltering
status: Accepted
date: 2026-04-30
---

## Context

FauniSearch 的新状态模型把内容结构和来源位置拆开。`Asset` 和 `Unit` 是全局可复用对象，`SourceAssetLocation` 才表达某个 Asset 出现在某个 Source 的哪里。同一份 PDF 内容可以同时出现在 `data/example/lib` 和 `data/example/lib1`，这两个路径应复用同一组 Asset、Unit 和 UnitIndex，但搜索结果要按当前 scope 返回不同的 Source location。

这带来一个检索性能问题。若每次搜索都先在整个 VectorSpace 中做向量召回，再回 SQLite 按 library、folder、`source_uri`、asset type 或 time range 过滤，大库和多库场景会浪费大量相似度计算。`faus find <folder>` 更明显：用户只想在一个小 folder 内找资料，但全空间召回会扫描许多当前 folder 根本不可返回的 Unit。

当前 Qdrant 查询实现没有 filter，下游只能后过滤。这个方式简单，但不能作为长期检索策略。

## Decision

FauniSearch 必须支持向量相似度计算前的行级预过滤。预过滤的业务真相仍来自 SQLite 中的 `Source`、`SourceAssetLocation`、`Asset`、`Unit` 和 `UnitIndex`。检索后端只接收可下推的候选行集合或稳定 Unit / Asset 条件。

搜索计划先在 SQLite 根据 `search_scope`、filters、active `SourceAssetLocation` 和 active `UnitIndex` 求出 eligible UnitIndex。对窄范围查询，系统把 `UnitIndex.vector_ref.point_id` 或等价的不透明 point locator 下推给检索后端，用 `has_id` 或 point-id allow-list 做行级预过滤。检索后端只在这些候选点中计算向量相似度。

稳定的 Unit / Asset 属性可以作为 payload filter 直接下推，比如：

```text
unit_id
unit_type
asset_id
asset_type
```

Qdrant 或其他检索后端只保存检索执行所需的最小向量点信息。稳定最小 payload 包括：

```text
unit_id
asset_id
unit_type
asset_type
```

检索后端还必须保存 point id 和向量数据。point id 对应 `UnitIndex.vector_ref.point_id` 或等价不透明 locator；向量数据可以包含多向量表示和用于 prefetch 的 dense vector。若一个物理 collection 承载多个 VectorSpace，可以把 `vector_space_id` 作为 payload 或 namespace locator 的一部分；若每个 VectorSpace 使用独立 namespace，则不要求重复保存。

`unit_locator`、`asset_locator` 等字段只能作为性能缓存或 debug hint。它们不得替代 SQLite 中的 Asset、Unit 或 SourceAssetLocation 事实。

Source 位置事实不作为向量点 payload 的业务真相，包括：

```text
source_uri
source_id
source_root_id
library_id
source_visibility
source_status
job_id
content_e2e_index_state
```

这些字段可以在未来作为性能冗余写入检索后端，但查询结果必须仍回 SQLite 校验。SQLite 是 Source 位置、可见性和 location 展开的事实源。

预过滤采用自适应策略。候选 point 数量低于实现阈值时，下推 point allow-list。候选过大时，系统退回全空间召回加 SQLite 后过滤，避免巨大 filter 请求本身拖慢查询。阈值属于实现配置，不进入稳定协议。

## Examples

### `faus find data/example/lib1`

`faus find` 的 search scope 很窄。系统先在 SQLite 找到 `file://.../data/example/lib1/` 下 active Source，再通过 SourceAssetLocation 找到 Asset，通过 Asset 找到 Unit，通过 UnitIndex 找到 point IDs。

检索请求下推这些 point IDs。Qdrant 只在这个 folder 对应的候选点里计算相似度。返回命中后，系统仍回 SQLite 展开 `locations[]`，保证结果指向 `data/example/lib1` 下的 Source。

### `asset_type=document_page`

如果查询只需要文档页，`asset_type=document_page` 可以直接作为 payload filter 下推。若查询同时限定了 folder，系统可以组合两类预过滤：

```text
asset_type = document_page
point_id in eligible folder point IDs
```

前者是稳定 Asset 属性，后者是 SQLite 从当前 search scope 解析出的候选行集合。

### 同一 PDF 出现在两个路径

同一 PDF 内容可能出现在两个路径：

```text
file:///data/example/lib/report.pdf
file:///data/example/lib1/report.pdf
```

它们共享 Asset、Unit 和 UnitIndex。Unit 向量点不保存单一 `source_uri` 作为事实源，因为这个 Unit 同时服务两个 Source location。

当 search scope 是 `lib1` 时，SQLite 只把 `lib1` 对应的 SourceAssetLocation 解析进候选集合。向量命中 Unit 后，结果只展开 `lib1` 的 location。若 search scope 同时覆盖两个路径，同一个 Asset 命中后可以展开两个 locations。

## Consequences

查询计划需要携带 eligible point IDs 或预过滤摘要。执行层要能把这些摘要转换为检索后端 filter，并在 debug 输出中报告是否使用 prefilter、候选点数量、下推字段和 fallback 原因。

Qdrant payload 应保留 Unit / Asset 级稳定字段，例如 `unit_id`、`unit_type`、`asset_id` 和 `asset_type`。Source 位置字段不得成为检索后端中的业务真相，避免路径重命名、跨库复用和 SourceAssetLocation 变化导致 payload 漂移。

查询命中后，服务端必须用 `unit_id` 或 `asset_id` 回到 SQLite，沿 `Unit -> Asset -> SourceAssetLocation -> Source` 还原 `source_uri`、locator、preview 和 `locations[]`。检索后端返回的 payload 只能加速定位结构化记录，不能直接作为最终位置响应。

搜索结果仍必须回 SQLite 做最终可见性校验和 `locations[]` 展开。检索后端的 filter 是性能优化，不替代结构化存储的事实源职责。

实现需要处理两个执行路径：小候选集合走 point allow-list 预过滤，大候选集合走全空间召回再后过滤。两条路径必须返回相同的业务结果，只允许性能和 debug 信息不同。

## Alternatives Considered

### 永远全空间召回后过滤

这条路实现简单，也不会把 Source 位置事实复制到检索后端。但大库、多库和 `faus find <folder>` 会产生大量无效相似度计算，top-k 还可能被当前 scope 外的命中挤占，需要更大的 overfetch 才能得到足够结果。

### 把 Source 位置字段全部冗余进向量 payload

把 `source_uri`、`library_id`、`source_id` 和 `source_root_id` 写进向量点 payload，可以直接用 Qdrant payload filter 过滤。问题是一个 Unit 可以对应多个 SourceAssetLocation。路径重命名、库间复用或 Source 失效都要求同步更新向量 payload，检索后端会变成第二份位置事实源。

### 只按 `asset_type` / `unit_type` 做 payload filter

这种方式有价值，但只能过滤内容形态，不能表达 folder、library、Source visibility 或 path prefix。它应作为 point allow-list 的补充，而不是替代。

### 为每个 Source 或 folder 创建独立 collection

独立 collection 能天然隔离查询范围，但会破坏 VectorSpace 的全局复用边界。相同 Unit 会被复制进多个 collection，路径重命名和跨库复用会重新引入重复向量和重复维护成本。
