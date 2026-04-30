# 002 状态与数据模型 (State and Data Model)

定义 FauniSearch 的广义应用状态与数据模型，明确系统承载哪些逻辑实体与状态族、它们如何关联、由谁承担事实源，以及丢失后如何恢复或替代。

本专题采用 Content-centered core model。核心链路固定为 `Source Content -> Asset -> Unit -> UnitIndex`。`Content` 只表示 Source 原始内容身份，`Asset` 表示内容内部结构，`Unit` 表示模型执行单元，`UnitIndex` 承接 per-unit 索引就绪状态。`ContentE2eIndexState` 记录某个 Source Content 在某条处理链路和某个 VectorSpace 下已经完成端到端索引。

## 关键术语 (Terminology)

- 库（Library）
- 库配置（LibraryConfig）
- 库来源根（Library Source Root）
- 来源类型（Source Kind）
- 摄取模式（Ingestion Mode）
- 内容（Content）
- 来源（Source）
- 来源资产位置（SourceAssetLocation）
- 资产（Asset）
- 单元（Unit）
- 向量空间（VectorSpace）
- 单元索引（UnitIndex）
- 内容端到端索引状态（ContentE2eIndexState）
- 定位符（Locator）
- 事实源（Truth Source）

## 范围

- 核心对象、配置状态、辅助状态、临时状态与运行时状态的逻辑模型
- `Content`、`Source`、`SourceAssetLocation`、`Asset`、`Unit`、`VectorSpace`、`UnitIndex` 与 `ContentE2eIndexState` 的身份语义和关系边界
- 对象关系、逻辑唯一性、复用边界与搜索定位出口
- 各类状态的事实源归属、可恢复性与保留语义

范围外：

- 数据库表、字段、ORM 映射与迁移
- 摄取流程、索引生命周期、生效切换与失败恢复
- 提供方配置、模型选择的解析算法与运行时探测细节
- 对外公开接口与请求响应形状
- 任务调度策略、worker 并发与执行流程

物理落点承接：

- 各状态族的物理持久化分层、记录族映射、文件系统布局与迁移版本由 [007-storage-and-persistence](../007-storage-and-persistence/spec.md) 定义

## 设计原则

- 身份先于存储（Identity Before Storage）：先定义对象和状态在逻辑上的唯一性与归属，再谈具体存储方式
- 单一事实源（Single Source of Truth）：每类状态只应有一个稳定事实源，其他表示只能是派生视图、缓存、引用或临时副本
- 内容身份保持干净（Clean Content Identity）：`Content` 只描述 Source 原始内容身份，不保存来源位置、类型解释、派生结果、模型能力或索引策略
- 定位与索引分离（Location Separate From Embedding）：`Asset` 面向用户最终定位，`Unit` 面向模型执行和 embedding
- 来源位置外置（Location Outside Asset）：`Asset` 不保存路径或库级位置；某个 Asset 出现在某个 Source 中的位置由 `SourceAssetLocation` 表达
- 能力判断外置（Capability Outside Content）：是否能直接 embedding 由索引策略根据内容类型配置和 provider/model 能力判断，不写入 `Content`
- 搜索范围显式（Explicit Search Scope）：搜索请求中的范围选择必须作为结构化会话状态显式存在，而不是隐含等同于“当前库”
- 可恢复性显式（Recoverability Is Explicit）：每类状态都应明确是持久保留、可重建，还是纯临时驻留

## 稳定记录与状态族

| 记录 / 状态族 | 关键字段 | 稳定关系 | 主事实源 | 保留 / 恢复语义 |
| --- | --- | --- | --- | --- |
| 库 (Library) | `library_id`、`display_name`、`lifecycle_state`、可选 `archived_at_ms`、默认配置引用 | 是库级作用域边界，拥有库配置、来源根、来源与辅助状态 | 结构化存储 | 长期保留，不自动重建 |
| 库配置 (LibraryConfig) | `library_id`、内容类型绑定、刷新策略、摄取 / 搜索 / 索引默认值、库级模型覆盖 | 与单个库一一对应，为摄取、索引构建、搜索与模型选择提供库级输入 | 配置事实源或结构化存储，按专题定义 | 长期保留，不自动重建 |
| 库来源根 (Library Source Root) | `source_root_id`、`library_id`、根定位、`enabled`、状态、`watch_state`、接入边界、覆盖边界摘要 | 属于单个库，定义扫描入口与覆盖边界 | 结构化存储 | 可通过重新探测或重新扫描修复部分状态 |
| 库来源根规则 (Library Source Root Rule) | `rule_id`、`source_root_id`、规则模式、规则表达、可选扩展名过滤 | 属于单个库来源根，参与包含 / 排除覆盖判定 | 结构化存储 | 长期保留；规则 DSL 与优先级算法不在本专题固定 |
| 内容 (Content) | `content_id`、`size_bytes`、可选 `fast_fingerprint`、可选 `sha256`、`created_at_ms` | 表示 Source 原始内容身份，不保存类型解释、来源位置或载荷位置 | 结构化存储 | 长期保留；`sha256` 可懒计算补齐 |
| 来源 (Source) | `source_id`、`library_id`、可选 `source_root_id`、`source_uri`、可选 `relative_path`、`source_type`、`media_type`、状态、`source_content_id` | 属于单个库，表示库内来源位置，并引用原始 `Content` | 结构化存储 | 可通过重新扫描与内容确认重建部分记录，但不作为默认恢复路径 |
| 资产 (Asset) | `asset_id`、`source_content_id`、`asset_type`、`locator_json`、`derivation_signature` | 表示 Source Content 内部的结构化资产，例如图片整体、PDF 页、视频片段或文本块 | 结构化存储 | 可由 Source Content 与处理规则重建；跨 Source 复用 |
| 来源资产位置 (SourceAssetLocation) | `source_id`、`asset_id`、`locator_json`、可选 `source_position`、`visibility` | 表示某个全局 Asset 出现在某个 Source 的哪里 | 结构化存储 | 随 Source 可见性变化更新；不承载向量索引状态 |
| 单元 (Unit) | `unit_id`、`asset_id`、`unit_type`、`derivation_signature`、可选 `locator_json` | 表示实际用于模型编码的执行单元，例如页图、关键帧、文本单元 | 结构化存储 | 可由 Asset 与处理规则重建 |
| 向量空间 (VectorSpace) | `vector_space_id`、`provider_id`、`model_id`、`model_version`、可选 `model_revision`、`vector_type`、可选 adapter / preprocessing signature | 表示某个模型的一种具体向量表示空间；同一空间内分数和向量可比较 | 配置解析结果；可被结构化记录引用 | 可由配置与运行时能力重新解析；不承载 active / retired |
| 单元索引 (UnitIndex) | `unit_id`、`vector_space_id`、`status`、`visibility`、可选 `vector_ref`、可选 `job_id`、错误摘要 | 表示某个 Unit 在某个 VectorSpace 下的索引就绪状态 | 结构化存储中的索引记录；向量载荷在检索后端 | 已提交索引长期保留；retired 进入清理窗口 |
| 内容端到端索引状态 (ContentE2eIndexState) | `content_id`、`pipe_signature`、`vector_space_id`、`indexed_at_ms` | 表示某个 Source Content 在指定处理链路和 VectorSpace 下已经完成 Asset / Unit / UnitIndex 提交 | 结构化存储 | 只在完成后写入；缺失时不得走复用快路径 |
| 提供方配置与模型选择状态 (Provider/Model Selection State) | provider 配置、内容类型绑定、模型选择、覆盖层级 | 附着在全局默认与库级覆盖上 | 配置文件事实源 | repo 基线与 runtime 覆盖长期保留；不以 `state.sqlite` 为 durable truth |
| 任务状态 (Job State) | `job_id`、任务类型、所有者引用、阶段状态、检查点引用 | 可指向库、来源根、来源、资产、单元或索引流程 | 当前切片中属于运行时状态 | 当前 restart 语义下重启清空，不自动恢复执行 |
| 搜索历史记录 (Search History Record) | `search_history_id`、`library_id`、查询类型、过滤摘要、时间戳 | 属于单个库，可关联搜索输入与调试追踪 | 非当前 restart-durable subset | 当前切片不要求跨 restart 保留 |
| 收藏记录 (Favorite Record) | `favorite_id`、`library_id`、目标对象引用、创建时间 | 属于单个库，指向被收藏的稳定对象 | 非当前 restart-durable subset | 当前切片不要求跨 restart 保留 |
| 临时查询资产 (Temporary Query Asset) | `temp_asset_id`、查询类型、来源、可选查询定位摘要、过期窗口 | 服务于图片 / 视频 / 文档查询的临时输入链路 | Rust 主服务管理的临时资产存储区 | 纯临时；丢失后需重新上传或重新选择 |
| 运行时驻留状态 (Runtime Resident State) | 运行时 / 提供方配置档引用、健康状态、设备分配、已加载模型或后端摘要、有效能力快照 | 属于 Python sidecar、modeld 或其他运行时进程，不承载长期业务事实 | 运行时进程自身 | 可通过重连、重载或重新探测恢复 |

## 核心对象与关系

- `Content` 是 Source 原始内容身份。`content_id` 使用随机 UUID，不把 hash 直接作为主键
- `Content` 不保存 `content_type`、`media_type` 或载荷位置。类型解释由 `Source.source_type` 与 `Source.media_type` 承接；原始字节位置由 `Source.source_uri` 或后续缓存层承接
- `Source` 是库内来源位置。它记录某个库在某个 `source_uri` 看到内容，并通过 `source_content_id` 引用原始 `Content`
- `Asset` 是 Source Content 内部的结构化资产。它不属于某个 Source，不保存路径，逻辑唯一性来自 `source_content_id + asset_type + locator_json + derivation_signature`
- `SourceAssetLocation` 是 Source 与 Asset 的位置关系。搜索结果通过它把命中的 Asset 还原到具体 Source 位置
- `Unit` 是为模型能力和索引策略生成的执行单元。它不再引用 Unit Content；Unit 自身就是稳定 embedding 输入身份
- `VectorSpace` 是模型和向量类型形成的全局向量表示空间。它回答哪些向量可以直接比较，不回答某个库当前是否 active
- `UnitIndex` 是 `unit_id + vector_space_id` 的索引就绪状态。向量命中来自 `UnitIndex` 指向的检索载荷，搜索结果返回 `Source + Asset`
- `ContentE2eIndexState` 是粗粒度完成标记。它只表示某个 `content_id + pipe_signature + vector_space_id` 已经完整生成 Asset / Unit、完成 embedding 并提交 UnitIndex
- `vector_ref` 是 `UnitIndex` 指向检索后端具体向量点或向量载荷位置的不透明引用。它不表达 VectorSpace、active / retired、Source、Asset 或 Unit 关系

最小关系固定为：

```text
Source.source_content_id -> Content
Content + pipe_signature + vector_space_id -> ContentE2eIndexState
Content -> Asset
Source -> SourceAssetLocation -> Asset
Asset -> Unit
Unit + vector_space_id -> UnitIndex
UnitIndex.vector_ref -> backend vector point or vector payload
```

## 典型对象形态

### 图片

图片 Source 形成 image Asset，再形成 image Unit。Asset 与 Unit 可以复用同一个 Source Content 结构，但 Unit 不再单独引用 Content。

```text
Source(photo.png, source_uri=file:///data/photo.png) -> Content(photo bytes)
Content(photo bytes) -> Asset(image, locator=image)
Asset(image) -> Unit(image_unit)
Unit(image_unit) + image vector_space_id -> UnitIndex
Source(photo.png) -> SourceAssetLocation(image Asset, locator=image)
ContentE2eIndexState(photo bytes, image_pipe:v1, image vector_space_id)
```

### PDF

PDF Source 引用 PDF 文件内容。PDF 页是可定位 Asset；当前模型若不能直接 embed PDF 页对象，索引策略把页面渲染成 page image Unit。渲染图是临时物化输入或可清理缓存，不成为新的核心 Content。

```text
Source(report.pdf, source_uri=file:///data/report.pdf) -> Content(pdf bytes)
Content(pdf bytes) -> Asset(document_page, locator=page 3)
Asset(page 3) -> Unit(page_image_unit)
Unit(page_image_unit) + image vector_space_id -> UnitIndex
Source(report.pdf) -> SourceAssetLocation(page 3 Asset, locator=page 3)
ContentE2eIndexState(pdf bytes, pdf_page_image_pipe:v1, image vector_space_id)
```

搜索命中 page image Unit 后，结果返回 `report.pdf` 的第 3 页，而不是返回渲染图文件本身。

### 视频

视频 Source 引用视频文件内容。视频片段是可定位 Asset；当前模型若不能直接 embed 视频片段，但支持图片，索引策略从片段抽关键帧并生成 image Unit。

```text
Source(clip.mp4, source_uri=file:///data/clip.mp4) -> Content(video file bytes)
Content(video file bytes) -> Asset(video_segment, locator=00:10-00:20)
Asset(video_segment) -> Unit(keyframe_image_unit)
Unit(keyframe_image_unit) + image vector_space_id -> UnitIndex
Source(clip.mp4) -> SourceAssetLocation(video segment Asset, locator=00:10-00:20)
ContentE2eIndexState(video file bytes, video_keyframe_pipe:v1, image vector_space_id)
```

搜索命中 keyframe image Unit 后，结果返回 `clip.mp4` 的 `00:10-00:20` 片段。keyframe 只是匹配证据和索引输入，不是用户最终要操作的位置。

## 身份与唯一性

- 库的稳定身份由 `library_id` 承载；库配置与其共享库级作用域，不单独扩展库身份
- `display_name` 是库的展示元数据，不参与稳定身份判定；`display_name` 更新不得改写 `library_id`
- 库生命周期状态属于库自身的结构化元数据；当前稳定最小值至少包括 `active` 与 `archived`
- `archived` 是可逆软生命周期状态，不得改变 `library_id`，也不得隐式等同于物理删除
- 创建库时，`library_id` 可以由用户显式指定；若未指定，则应由系统从 `display_name` 生成稳定 slug，并在冲突时自动追加后缀
- `Content` 的身份采用两阶段精确判定：`size_bytes + fast_fingerprint` 只用于低成本候选筛选，SHA-256 才是最终合并依据
- SHA-256 懒计算。没有候选时可以创建 `sha256 = NULL` 的 provisional Content；只有候选需要确认合并时才必须计算 SHA-256
- 两个 Content 只有在 SHA-256 相等时才允许合并。没有 SHA-256 的 Content 不得仅凭快速 fingerprint 合并
- 快速 fingerprint 不得作为最终身份事实源；不做 perceptual near-duplicate、语义相似或 embedding 相似合并
- `Source` 的逻辑唯一性由库、来源根、`source_uri`、摄取模式与原始内容确认共同决定；不同库之间不共享 `source_id`
- 同一库中同一 `source_uri` 的内容更新应更新或替换对应 Source 的内容引用与派生边界，而不是制造不可解释的重复来源
- `Asset` 的逻辑唯一性由 `source_content_id + asset_type + locator_json + derivation_signature` 决定；它不脱离 Source Content 单独存在
- `SourceAssetLocation` 的逻辑唯一性至少包含 `source_id + asset_id`；它只表达位置和展示顺序，不表达向量可见性
- `Unit` 的逻辑唯一性由 `asset_id + unit_type + derivation_signature + locator_json` 决定；它不替代 Asset 的定位身份
- `VectorSpace` 的逻辑唯一性由 provider、model、version / revision、`vector_type` 与必要 adapter / preprocessing signature 决定
- `VectorSpace` 不承载 active / retired、visibility、job、UnitIndex 状态或物理 collection 名
- `UnitIndex` 的逻辑唯一性由 `unit_id + vector_space_id` 决定；同一 Unit 在同一 VectorSpace 下应复用索引结果
- `ContentE2eIndexState` 的逻辑唯一性由 `content_id + pipe_signature + vector_space_id` 决定；它是 Source Content 复用快路径的前置条件
- `pipe_signature` 表示从 Source Content 推导 Asset、Unit 和模型输入的整体处理规则；会改变 Asset、Unit 或模型输入的规则变化必须形成新的 `pipe_signature`
- `derivation_signature` 表示具体 Asset 或 Unit 派生规则；会改变单项派生结果的规则变化必须形成新的 `derivation_signature`
- `locator` 只承载定位角色；首批定位形式包括页号、单图标识、片段序号与时间范围
- 对 `document_page` 而言，`locator` 中的页号与页标签必须来自真实文档页序，而不是占位值或推测值
- 对 `video_segment` 而言，`locator` 中的 `start_ms` / `end_ms` 必须来自同一源视频的真实时间轴范围，而不是占位值或运行时临时猜测
- provider/model 选择状态、任务状态、搜索历史记录、收藏记录与临时查询资产都是状态记录；它们的稳定标识只服务于记录归属与引用，不扩展核心对象身份规则

## 替换与复用原则

- 当 Source 的原始 Content 发生变化时，其 SourceAssetLocation 应按新的 Source Content 处理结果替换；旧位置可以短期保留用于诊断与清理窗口，但在逻辑上应视为失效
- 跨库不共享 Source；跨库、跨路径复用通过 Source Content、Asset、Unit、UnitIndex 与 ContentE2eIndexState 完成
- 当新 Source 命中已有 Source Content，并且对应 `ContentE2eIndexState` 已存在时，系统只需提交 Source 与 SourceAssetLocation，不得重新创建重复 Asset、Unit 或 UnitIndex
- 当 `ContentE2eIndexState` 缺失时，系统不得因为 Content 已存在就跳过处理；必须重新执行或恢复端到端索引流程
- Unit 的模型输入物化结果是临时文件、内存字节或可清理缓存，不进入核心状态模型
- 来源根或规则变化导致的“退出覆盖范围”只改变 Source 覆盖状态与后续可搜索性，不创建新的 Content 身份
- 临时查询资产与运行时驻留状态只承担临时或运行时职责，不构成长期共享事实源

## 搜索定位语义

- 搜索先由 `search_scope` 和过滤器确定可见的 `SourceAssetLocation`
- 向量命中来自 `UnitIndex`；命中后通过 `Unit -> Asset -> SourceAssetLocation -> Source` 还原用户可操作位置
- 搜索结果的出口是 `Source + Asset`，而不是 Unit 或 Content
- Unit 可以作为匹配证据返回到调试字段或 `matched_units` 摘要，但用户操作位置必须回到 Asset
- 结果折叠的默认稳定粒度是 Asset；`locations[]` 用于列出可操作位置，每个位置必须能还原到具体 Source 与 Asset
- 同一 Asset 出现在多个 Source 时，向量只需要命中一次；搜索结果按当前 search scope 展开对应 locations
- `active` 搜索可见性由 UnitIndex 与 SourceAssetLocation 共同解释：UnitIndex 决定向量是否可检索，SourceAssetLocation 决定当前范围内是否存在可返回位置

## 跨重启 durable 子集

- 当前切片跨 restart 恢复的最小 durable truth 固定为：`libraries`、`library_configs`、`library_source_roots`、`library_source_root_rules`、`contents`、`sources`、`assets`、`source_asset_locations`、`units`、`vector_spaces`、`unit_indexes`、`content_e2e_index_states`
- 未提交为 active 的索引状态只在对应 job 运行期间或可恢复检查点中有意义；job 失败、中断后若不能恢复并验证成功，必须丢弃或隐藏，不得让其长期承担搜索事实
- `jobs`、`latest_job_id`、`job_order`、临时查询资产、search history、favorites 与 watcher runtime scratch 不属于当前 restart-durable subset
- provider probe cache 与 resolved model 摘要不属于当前 restart-durable subset；应用重启后需要重新探测或重新生成观察快照
- 来源根的 `watch_state`、debounce 队列、待处理路径集合与最近一次运行时错误属于 runtime scratch；应用启动后需要重新播种，而不是按上次进程内状态直接恢复
- active UnitIndex 是否真正可搜索，除结构化记录外，还取决于 `vector_ref` 指向的检索后端载荷是否仍存在

## 状态边界

| 状态类别 | 主要实体 / 记录 | 事实源 | 可写组件 | 非事实源表示 | 丢失后处理 |
| --- | --- | --- | --- | --- | --- |
| 配置与设置 | 库配置、库来源根规则、provider configs、全局内容类型绑定与库级内容类型覆盖 | 配置事实源或结构化存储，按专题定义 | Rust 主服务 | 前端表单状态、进程内配置快照、sidecar 运行参数副本 | 不自动重建，应通过备份或重新配置恢复 |
| 核心结构化对象 | 库、库来源根、Content、Source、SourceAssetLocation、Asset、Unit、VectorSpace、UnitIndex、ContentE2eIndexState | 结构化存储 | Rust 主服务 | 读取模型、导出结果、调试视图 | 可部分重建，但不作为常规恢复路径 |
| 原始文件引用与覆盖边界 | 来源根定位、接入模式记录、Source 覆盖快照、Source URI | 结构化存储 | Rust 主服务 | 文件选择状态、临时路径列表、一次性扫描结果 | 可通过重新扫描或重新导入恢复 |
| 检索索引与向量状态 | 检索后端中的 active 向量载荷与 retired 清理候选 | 检索后端 | Rust 主服务 | UnitIndex、统计信息、调试视图 | 可通过重新编码与重建索引恢复 |
| Unit 物化载荷 | PDF 页图、OCR 文本、视频关键帧、预览图等实际文件载荷 | 临时工作区、可清理缓存或后续专门缓存层 | Rust 主服务 | 内存缓存、临时工作文件 | 可从 Source 原始字节与处理流程重建 |
| 任务状态与队列 | 任务状态、检查点引用 | 当前切片中属于 Rust 主服务运行态 | Rust 主服务 | 前端进度显示、诊断快照 | 当前 restart 语义下重启清空，不自动恢复 |
| 辅助操作记录 | 搜索历史记录、收藏记录 | 非当前 restart-durable subset | Rust 主服务 | 最近搜索列表缓存、界面收藏状态 | 当前切片不自动恢复，应通过后续专题显式定义 |
| 临时查询资产 | 临时查询资产 | Rust 主服务管理的临时资产存储区 | Rust 主服务 | 前端上传状态、sidecar 输入副本 | 不保证持久化，需要重新上传或重新选择 |
| 运行时驻留状态 | 运行时驻留状态 | Python sidecar、modeld 或所属运行时进程 | Python sidecar、modeld 或所属运行时 | Rust 侧健康快照、配置偏好、诊断视图 | 可通过重连、重载或重新探测恢复 |

- 前端 / 调用方可以持有会话级交互状态，但不承载系统共享事实源
- 当前库选择、搜索范围选择、未提交过滤器与查询草稿都属于前端 / 调用方的会话级交互状态；它们可以影响当前请求，但不构成 durable truth
- 原始文件内容本身不作为应用内共享事实源；应用内稳定承载的是其 Source URI、扫描结果、内容身份与索引完成状态
- 已启用内容类型与其绑定模型属于配置状态；某个 VectorSpace 当前是否持有 active 索引，属于 UnitIndex 状态
- 检索后端只承载检索索引与向量状态，不承载结构化业务元数据、来源、资产位置或搜索可见性真相
- Python sidecar 和 modeld 可以持有运行时驻留状态，但不承载长期持久的系统事实源

## 关联主题

- [000-foundation](../000-foundation/spec.md) 定义项目级基础约束与上游设计原则
- [001-architecture](../001-architecture/spec.md) 定义组件边界、编排关系与稳定交互路径
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 承接正式来源类型、摄取流程、UnitIndex 与 ContentE2eIndexState 的生命周期语义
- [004-search](../004-search/spec.md) 承接搜索语义、结果语义与查询期状态约束
- [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 承接 provider config、模型选择、解析顺序与运行时探测语义
- [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 承接任务执行、运行时生命周期、健康摘要与清理任务的执行语义
- [007-storage-and-persistence](../007-storage-and-persistence/spec.md) 承接主结构化存储、检索命名空间、临时资产存储区与迁移版本的物理持久化语义
- [008-ui-ux](../008-ui-ux/spec.md) 承接库、来源根、配置、任务、收藏与搜索历史的应用入口、管理体验与控制面接口族
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接公开接口的请求 / 响应契约、任务 / 健康快照编码与 sidecar 协议载荷
- [140-library-source-management](../140-library-source-management/spec.md) 承接来源根生命周期、来源规则、来源清单与 refresh / rescan / watcher 的当前阶段能力域
