# 004 搜索 (Search)

定义 FauniSearch 的搜索语义，明确搜索查询 (Search Query) 如何在已启用内容类型上执行，并返回以 Asset 为中心的搜索结果 (Search Result)。

## 关键术语 (Terminology)

- 搜索查询（Search Query）
- 查询类型（Query Kind）
- 搜索范围（Search Scope）
- 搜索结果（Search Result）
- 结果位置（Result Location）
- 邻近上下文（Neighbor Context）
- 资产（Asset）
- 单元（Unit）
- 单元索引（UnitIndex）
- 搜索可见性（Search Visibility）
- 来源（Source）

## 范围

- 四类正式查询输入与公开搜索能力边界
- 搜索目标选择、公共控制项、过滤与分页语义
- 行级向量预过滤、后端下推与后过滤语义
- Asset 结果粒度、Unit 命中证据、混排、邻近上下文与调试输出语义
- 搜索期错误边界与显式拒绝规则

范围外：

- 索引构建、激活、检查点与恢复语义
- 核心对象身份、去重与定位符基础定义
- 提供方配置、模型选择与融合排序算法
- 视频查询大小或时长阈值的统一数值
- 搜索历史（search history）、分析（analytics）、收藏（favorites）等附属接口
- `/search/*` 的请求 / 响应编码、游标令牌与公共错误载荷形状

应用体验承接：

- 搜索工作区在应用中的壳层位置、全局导航关系与非搜索管理入口由 [008-ui-ux](../008-ui-ux/spec.md) 定义

接口契约承接：

- `/search/*` 的请求 / 响应形状、调试载荷编码与公共错误载荷由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义

## 设计原则

- 单一公开输入（Single Public Input）：每次公开搜索请求只接受一种查询输入，不在本专题中公开组合查询形状
- 启用内容类型约束（Enabled-Content-Type Constraint）：搜索只能以库内已启用内容类型为目标，不能绕过内容类型配置直接访问未启用内容类型
- Asset 优先（Asset First）：搜索结果默认返回用户可定位 Asset，而不是 Source、Content 或 Unit
- Unit 作为证据（Unit As Evidence）：向量命中来自 Unit 的 UnitIndex；Unit 可以解释为什么命中，但不替代结果位置
- 只读已提交索引（Committed Index Only）：普通搜索只读取 active UnitIndex；未提交为 active 的检索后端载荷不得返回
- 预过滤先规划（Plan Prefilter First）：搜索执行必须先基于结构化真相规划候选 UnitIndex，再决定是否向检索后端下推行级预过滤
- 分值上下文化（Contextual Score）：公开结果可以返回稳定 `score` 字段，但该值只在同一次搜索响应内表达当前后端返回的排序强弱，不承诺跨查询、跨 VectorSpace 或跨后端可直接比较
- 显式拒绝（Explicit Rejection）：不支持或不可用的搜索请求应明确拒绝，而不是静默降级、忽略或改写请求含义

## 查询类型与公开搜索入口

- 文本（text）、图片（image）、视频（video）与文档（document）是四类正式查询输入
- 对外稳定入口固定为：
  - `POST /search/text`
  - `POST /search/image`
  - `POST /search/video`
  - `POST /search/document`
- 这些端点的请求 / 响应字段形状由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接；本专题继续定义其行为语义
- 对外入口按查询类型分路，但内部可以收敛到统一查询编排模型
- 公开接口只接受单一输入；若请求同时携带多种查询输入，应返回明确“不支持”
- 图片、视频与文档查询长期上允许两类输入来源：临时上传资产与库内对象引用
- 各能力专题可以在当前阶段先只启用其中一个正式输入来源；未启用的输入来源应返回明确“不支持”，而不是静默降级
- 视频查询长期上允许携带一个可选查询时间范围；缺失时按整段视频解释，存在时按指定视频片段解释
- 视频查询若启用库内对象引用，长期上既可以通过 Source 表达“整段视频或显式时间范围”，也可以通过 `video_segment` Asset 表达“以该片段作为查询输入”
- 文档查询长期上允许携带一个可选页范围；缺失时按整份文档解释，存在时按指定文档片段解释
- 文档查询若启用库内对象引用，长期上通过 Source 表达“整份文档或显式页范围”；能力专题可以额外允许通过 `document_page` Asset 派生出单页查询，但公开请求仍应保持单一文档输入语义
- 视频查询必须受大小与时长上限约束；超限请求应被明确拒绝，但具体阈值不在本专题中固定
- 通过验证并进入执行阶段的搜索必须写入 QueryHistory。成功记录 `completed` 与结果数量；执行失败记录 `failed` 与 `error_code / error_message`；参数验证失败不写入 history
- QueryHistory 不保存完整搜索结果。调用方若要取得当前结果，应重新执行搜索

## 搜索目标与公共控制项

- 每次搜索请求都必须显式携带结构化 `search_scope`
- `search_scope` 长期上至少应支持：
  - 单库：`library`
  - 所有库：`all_libraries`
  - 预留的多库子集：`library_set`
- 当前公开切片中，`text`、`image`、`video` 与 `document` 查询都允许 `library` 与 `all_libraries`
- `all_libraries` 非文本查询必须使用全局 QueryAsset，或使用全局可解析的 `library_object.asset_id`；`source_id` 型库内对象查询仍只允许单库范围
- 当 `search_scope.kind = library` 时，默认情况下搜索会并行查询该库全部已启用内容类型，并按这些内容类型当前解析出的一个或多个 VectorSpace 并行执行后再融合排序
- 当 `search_scope.kind = all_libraries` 时，系统必须只查询已进入可搜索状态的库，并在成功响应中显式保留每个命中的来源库身份
- 请求可以通过 `target_content_types` 显式限制本次查询的内容类型子集；当前第一阶段中，这些内容类型约束必须按 `search_scope` 中实际参与的每个库分别验证
- 若请求命中未启用内容类型，应返回明确不可用状态
- 若请求显式命中，或默认会作用到任一已启用但尚未持有 active UnitIndex 与 active SourceAssetLocation 的内容类型，应返回明确未就绪状态，而不是静默忽略该内容类型、自动降级到其他内容类型或返回空结果
- 若某个已启用内容类型绑定的模型不支持当前查询输入，应跳过该内容类型，并在成功响应中的 `unsupported_content_types` 返回结构化原因；只有当全部目标内容类型都不可执行时，才允许显式失败
- 当前切片中，搜索是否支持 `text` / `image` / `document` / `video` 查询输入，必须以已解析 provider 的 Execution Input Types 判定，而不是以模型原生 `EmbeddingCapabilities.input_types` 判定
- 若多个目标内容类型解析到同一个 VectorSpace，系统应只生成一次查询 embedding 并复用到该 VectorSpace 承载的全部内容类型
- 若目标内容类型解析到多个不同 VectorSpace，系统应按 VectorSpace 分别生成查询 embedding、分别检索，再进行跨空间融合
- eligible VectorSpace 必须由 `search_scope`、目标 content types、provider capability、运行时可用性、active UnitIndex、active SourceAssetLocation 与过滤器共同决定
- 每个 eligible VectorSpace 独立执行 query embedding 和向量检索；不同 VectorSpace 的 raw score 不得直接跨空间比较
- 跨 VectorSpace 合并默认使用 rank fusion / RRF；具体常数和归一化实现不在本专题固定，但公开排序不得依赖 raw score 跨空间同尺度假设
- 搜索期提供方的能力、绑定与已解析提供方选择（Resolved Provider Selection）语义，由 [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义
- 公共控制项固定包括：`search_scope`、`filters`、`top_k`、`cursor`、`debug`，以及可选的 `target_content_types`
- 当前稳定搜索只搜索 active UnitIndex，并通过 active SourceAssetLocation 把检索后端命中还原为可返回位置；公开搜索请求不提供索引可见性切换参数
- 正式公共过滤器固定包括：`asset_type`、`path_prefix`、`source_type`、`time_range`
- 分页采用 `cursor` 语义；本专题不固定 cursor 的内部编码方式
- 当前切片中，`path_prefix` 作用于结果对象的 `source_uri` 前缀匹配；若提供多个前缀，则按“命中任一前缀即可保留”解释
- 当前切片中，`time_range` 作用于带时间定位符的 Asset；只有定位符中包含可解析 `start_ms` / `end_ms` 的时间型命中会参与该过滤，其他非时间型结果在启用 `time_range` 时应被排除
- 当前切片中的 `cursor` 用于在“同一查询输入与同一过滤条件”下继续读取已排序结果列表中的后续命中；它只承接续页语义，不承诺跨查询条件变化、跨索引重建或跨内容变更保持稳定

## 查询计划与行级预过滤

- 搜索执行必须先在主结构化存储中根据 `search_scope`、公共过滤器、active SourceAssetLocation 与全局 active UnitIndex 求出 eligible UnitIndex 集合；该集合必须已经应用 `asset_type`、`path_prefix`、`source_type` 与 `time_range` 等过滤条件
- 对窄范围查询，系统应优先把 eligible UnitIndex 对应的 `UnitIndex.vector_ref.point_id` 或等价不透明 point locator 作为 point allow-list 下推给检索后端，使向量相似度只在候选点内计算
- 稳定 Unit / Asset 属性可以作为检索后端 payload filter 下推；当前稳定字段包括 `unit_id`、`unit_type`、`asset_id` 与 `asset_type`
- Source 级过滤必须以结构化存储为事实源。`path_prefix`、`source_type`、library scope、source root scope 与 Source visibility 必须先通过 Source、SourceAssetLocation、Asset、Unit 与 UnitIndex 关系解析，不得直接依赖检索后端 payload 作为业务真相
- `time_range` 必须在 SourceAssetLocation locator 层参与 eligible UnitIndex 规划；不带可解析时间 locator 的结果在启用该过滤器时不得进入 point allow-list
- 检索后端 filter 是性能优化。检索命中返回后，服务端仍必须回到结构化存储做最终可见性校验，并通过 `Unit -> Asset -> SourceAssetLocation -> Source` 展开 `locations[]`
- 当候选 point 数量过大、后端不支持对应 filter，或下推 filter 会明显拖慢查询时，系统可以退回全 VectorSpace 召回加 SQLite 后过滤；但当前全局 VectorSpace namespace 下的 scoped search 默认必须优先使用 point allow-list，以避免窄 scope 查询在后过滤前浪费全库相似度计算
- point allow-list 预过滤路径与全空间召回后过滤路径必须返回相同业务结果；它们只允许在性能、召回内部 overfetch 与 debug 信息上存在差异
- 跨 VectorSpace 查询应分别为每个 eligible VectorSpace 生成预过滤计划；某个 VectorSpace 的下推失败不得改变其他 VectorSpace 的过滤语义

## 结果语义

- 搜索结果默认返回有序的 Asset 列表
- `document_page`、`image`、`video_segment` 与 `text_block` 允许在同一结果集中默认混排，并可按 `asset_type` 过滤
- 公共结果允许返回稳定 `score` 字段；若返回，该值只表达当前响应内的相对排序强弱，不改变“结果顺序优先于分值解释”的公开语义
- 公共结果卡片的稳定字段包括：`library_id`、`source_id`、`asset_id`、`preview`、`source_uri`、`source_type`、`asset_type`、`locator`、`cursor`，以及可选 `score`、`matched_units` 与 `job_id`
- 当 `search_scope` 允许跨库命中时，每个结果项都必须携带其来源 `library_id`，以支撑对象详情、预览与对象复用动作
- `preview` 承载可消费的预览引用语义，而不是要求直接暴露原始本地路径
- 同一 Source 下的多个 Asset 命中默认全部返回
- 结果折叠的默认稳定粒度是 Asset；agent-first 工作流可以进一步按显式策略折叠，但不得丢失每个 Source + Asset 位置
- `locations` 是结果位置数组，每项必须保留具体 `library_id`、`source_id`、`asset_id`、`source_uri`、locator 与 preview
- Unit 只作为匹配证据返回。`matched_units` 至少可以表达 `unit_id`、`unit_type`、`vector_space_id`、`rank`、`raw_score` 与可选的匹配摘要
- 同一 Asset 被多个 VectorSpace 或多个 Unit 命中时，应折叠为一个 Asset 结果；排序分值来自融合后的分数，`matched_units` 保留各空间命中证据
- 按 Source 或 Content 聚合只作为可选视图语义存在，不取代默认的 Asset 返回粒度
- 默认公开结果列表只返回最小结果卡片，不默认内联 `neighbor_context`
- 当前切片中，结果项上的 `cursor` 与响应级 `next_cursor` 复用同一续页令牌语义；客户端可以把最后一个结果项上的 `cursor` 作为下一页的 `cursor`
- 通过对象详情或展开路径返回的邻近上下文固定为 Source + Asset 语义：
  - 文档页返回前后页 Asset
  - 视频片段返回前后片段 Asset
  - 图片返回同源基础信息

## 调试与错误语义

- `debug=true` 时，稳定返回的调试信息至少包括：命中的 `content_type`、各内容类型原始分数摘要，以及 `backend`、`vector_type`、VectorSpace、Execution Input Types 等技术元信息
- 当前切片中，`debug=true` 的稳定最小实现至少应返回：参与本次查询的 `content_type` 列表、每个内容类型上的原始分数摘要、参与执行的 VectorSpace 摘要，以及当前后端 / 表征类型摘要
- `debug=true` 时可以返回预过滤摘要，表达是否使用 point allow-list 或 payload filter、候选点数量、下推字段与 fallback 原因；该摘要只用于诊断，不改变搜索结果契约
- 当前切片中，`debug.vector_type` 的正式公开值固定为 `multi_vector_late_interaction`
- 公开结果中的 `score` 与调试原始分数都只用于当前响应内的排序解释与诊断，不承诺跨内容类型或跨查询请求可直接比较
- 显式请求未启用内容类型时，应返回明确不可用状态
- 请求命中已启用但未就绪的内容类型时，应返回明确未就绪状态，而不是静默返回空结果
- 公开请求若携带多种查询输入，应返回明确“不支持”
- 当前阶段中，若端点不支持请求给出的 `search_scope.kind`，应返回明确“不支持”
- 当前阶段中，搜索端点若收到未知控制字段，应返回明确“不支持”或验证失败，而不是静默改变搜索语义
- 文档查询若携带非法、越界或不可解析的页范围，应返回明确失败，而不是隐式回退到整份文档
- 视频查询若携带非法、越界或不可解析的时间范围，应返回明确失败，而不是隐式回退到整段视频
- 视频查询超出允许的大小或时长上限时，应返回明确拒绝，而不是静默截断或自动降级

## 关联主题

- [000-foundation](../000-foundation/spec.md) 定义查询输入、检索对象与项目级上游定位
- [001-architecture](../001-architecture/spec.md) 定义搜索编排的系统边界、组件职责与交互路径
- [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义 Content、Source、Asset、Unit、UnitIndex、定位符与搜索相关状态边界的基础语义
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 定义 VectorSpace、UnitIndex、Source 级 active 与搜索可依赖的索引状态边界
- [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义已解析提供方选择、提供方能力与提供方绑定语义
- [008-ui-ux](../008-ui-ux/spec.md) 定义搜索工作区的应用壳层位置、导航关系与全应用体验边界
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义 `/search/*` 的请求 / 响应契约、公共错误载荷与分页编码
