# 004 搜索 (Search)

定义 FauniSearch 的搜索语义，明确搜索查询 (Search Query) 如何在已启用内容类型上执行，并返回搜索结果 (Search Result)。

## 关键术语 (Terminology)

- 搜索查询（Search Query）
- 查询类型（Query Kind）
- 搜索结果（Search Result）
- 邻近上下文（Neighbor Context）
- 视觉单元（Visual Unit）
- 源内容（Source）

## 范围

- 四类正式查询输入与公开搜索能力边界
- 搜索目标选择、公共控制项、过滤与分页语义
- 结果粒度、混排、邻近上下文与调试输出语义
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
- 视觉单元优先（Visual Unit First）：搜索结果默认按视觉单元返回，而不是按源内容聚合后返回
- 分值上下文化（Contextual Score）：公开结果可以返回稳定 `score` 字段，但该值只在同一次搜索响应内表达当前后端返回的排序强弱，不承诺跨查询、跨索引线或跨后端可直接比较
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
- 视频查询若启用库内对象引用，长期上既可以通过库内视频源表达“整段视频或显式时间范围”，也可以通过库内 `video_segment` 直接表达“以该片段作为查询输入”
- 文档查询长期上允许携带一个可选页范围；缺失时按整份文档解释，存在时按指定文档片段解释
- 文档查询若启用库内对象引用，长期上通过库内文档源表达“整份文档或显式页范围”；能力专题可以额外允许通过库内 `document_page` 派生出单页查询，但公开请求仍应保持单一文档输入语义
- 视频查询必须受大小与时长上限约束；超限请求应被明确拒绝，但具体阈值不在本专题中固定

## 搜索目标与公共控制项

- 每次搜索请求都必须显式指向单个库
- 默认情况下，搜索会并行查询该库全部已启用内容类型，并按这些内容类型当前解析出的一个或多个 `vector_space` 并行执行后再融合排序
- 请求可以通过 `target_content_types` 显式限制本次查询的内容类型子集，但只能引用该库已启用内容类型
- 若请求命中未启用内容类型，应返回明确不可用状态
- 若请求显式命中，或默认会作用到任一已启用但尚未持有 active index 的内容类型，应返回明确未就绪状态，而不是静默忽略该内容类型、自动降级到其他内容类型或返回空结果
- 若某个已启用内容类型绑定的模型不支持当前查询输入，应跳过该内容类型，并在成功响应中的 `unsupported_content_types` 返回结构化原因；只有当全部目标内容类型都不可执行时，才允许显式失败
- 当前切片中，搜索是否支持 `text` / `image` / `document` / `video` 查询输入，必须以已解析 provider 的 Execution Input Types 判定，而不是以模型原生 `EmbeddingCapabilities.input_types` 判定
- 若多个目标内容类型解析到同一个 `vector_space`，系统应对该 `vector_space` 只生成一次查询 embedding 并复用到该空间承载的全部内容类型
- 若目标内容类型解析到多个不同 `vector_space`，系统应按空间分别生成查询 embedding、分别检索，再进行跨空间混排
- 搜索期提供方的能力、绑定与已解析提供方选择（Resolved Provider Selection）语义，由 [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义
- 公共控制项固定包括：`library_id`、`filters`、`top_k`、`cursor`、`debug`，以及可选的 `target_content_types`
- 正式公共过滤器固定包括：`visual_unit.kind`、`path_prefix`、`source_type`、`time_range`
- 分页采用 `cursor` 语义；本专题不固定 cursor 的内部编码方式
- 当前切片中，`path_prefix` 作用于结果对象的 `source_path` 前缀匹配；若提供多个前缀，则按“命中任一前缀即可保留”解释
- 当前切片中，`time_range` 作用于带时间定位符的结果对象；只有定位符中包含可解析 `start_ms` / `end_ms` 的时间型命中会参与该过滤，其他非时间型结果在启用 `time_range` 时应被排除
- 当前切片中的 `cursor` 用于在“同一查询输入与同一过滤条件”下继续读取已排序结果列表中的后续命中；它只承接续页语义，不承诺跨查询条件变化、跨索引重建或跨内容变更保持稳定

## 结果语义

- 搜索结果默认返回有序的视觉单元列表
- `document_page`、`image` 与 `video_segment` 允许在同一结果集中默认混排，并可按 `visual_unit.kind` 过滤
- 公共结果允许返回稳定 `score` 字段；若返回，该值只表达当前响应内的相对排序强弱，不改变“结果顺序优先于分值解释”的公开语义
- 公共结果卡片的稳定字段包括：`preview`、`source_path`、`source_type`、`kind`、`locator`、`cursor`，以及可选 `score`
- `preview` 承载可消费的预览引用语义，而不是要求直接暴露原始本地路径
- 同一源内容下的多个命中默认全部返回
- 按源内容聚合只作为可选视图语义存在，不取代默认的视觉单元返回粒度
- 默认公开结果列表只返回最小结果卡片，不默认内联 `neighbor_context`
- 当前切片中，结果项上的 `cursor` 与响应级 `next_cursor` 复用同一续页令牌语义；客户端可以把最后一个结果项上的 `cursor` 作为下一页的 `cursor`
- 通过对象详情或展开路径返回的邻近上下文固定为源内容级语义：
  - 文档页返回前后页
  - 视频片段返回前后片段
  - 图片返回同源基础信息

## 调试与错误语义

- `debug=true` 时，稳定返回的调试信息至少包括：命中的 `content_type`、各内容类型原始分数摘要，以及 `backend`、`repr_kind`、派生 `vector_space`、Execution Input Types 等技术元信息
- 当前切片中，`debug=true` 的稳定最小实现至少应返回：参与本次查询的 `content_type` 列表、每个内容类型上的原始分数摘要、参与执行的 `vector_space` 摘要，以及当前后端 / 表征类型摘要
- 公开结果中的 `score` 与调试原始分数都只用于当前响应内的排序解释与诊断，不承诺跨内容类型或跨查询请求可直接比较
- 显式请求未启用内容类型时，应返回明确不可用状态
- 请求命中已启用但未就绪的内容类型时，应返回明确未就绪状态，而不是静默返回空结果
- 公开请求若携带多种查询输入，应返回明确“不支持”
- 文档查询若携带非法、越界或不可解析的页范围，应返回明确失败，而不是隐式回退到整份文档
- 视频查询若携带非法、越界或不可解析的时间范围，应返回明确失败，而不是隐式回退到整段视频
- 视频查询超出允许的大小或时长上限时，应返回明确拒绝，而不是静默截断或自动降级

## 关联主题

- [000-foundation](../000-foundation/spec.md) 定义查询输入、检索对象与项目级上游定位
- [001-architecture](../001-architecture/spec.md) 定义搜索编排的系统边界、组件职责与交互路径
- [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义视觉单元、定位符、源内容与搜索相关状态边界的基础语义
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 定义已启用索引线、激活索引与搜索可依赖的索引状态边界
- [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义已解析提供方选择、提供方能力与提供方绑定语义
- [008-ui-ux](../008-ui-ux/spec.md) 定义搜索工作区的应用壳层位置、导航关系与全应用体验边界
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义 `/search/*` 的请求 / 响应契约、公共错误载荷与分页编码
