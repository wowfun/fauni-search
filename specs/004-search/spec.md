# 004 搜索 (Search)

定义 FauniSearch 的搜索语义，明确搜索查询 (Search Query) 如何在已启用索引线上执行，并返回搜索结果 (Search Result)。

## 关键术语 (Terminology)

- 搜索查询（Search Query）
- 查询类型（Query Kind）
- 搜索结果（Search Result）
- 邻近上下文（Neighbor Context）
- 视觉单元（Visual Unit）
- 源内容（Source）

## 范围

- 三类正式查询输入与公开搜索能力边界
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
- 启用线约束（Enabled-Line Constraint）：搜索只能以库内已启用索引线为目标，不能绕过索引配置直接访问未启用索引线
- 视觉单元优先（Visual Unit First）：搜索结果默认按视觉单元返回，而不是按源内容聚合后返回
- 排序优先于分值解释（Order Before Score）：公开结果顺序是稳定排序表达；公共结果不承诺统一分数字段
- 显式拒绝（Explicit Rejection）：不支持或不可用的搜索请求应明确拒绝，而不是静默降级、忽略或改写请求含义

## 查询类型与公开搜索入口

- 文本（text）、图片（image）与视频（video）是三类正式查询输入
- 对外稳定入口固定为：
  - `POST /search/text`
  - `POST /search/image`
  - `POST /search/video`
- 这些端点的请求 / 响应字段形状由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接；本专题继续定义其行为语义
- 对外入口按查询类型分路，但内部可以收敛到统一查询编排模型
- 公开接口只接受单一输入；若请求同时携带多种查询输入，应返回明确“不支持”
- 图片与视频查询都支持两类输入来源：临时上传资产与库内对象引用
- 视频查询必须受大小与时长上限约束；超限请求应被明确拒绝，但具体阈值不在本专题中固定

## 搜索目标与公共控制项

- 每次搜索请求都必须显式指向单个库
- 默认情况下，搜索会并行查询该库全部已启用索引线，并以融合后的排序结果返回
- 请求可以通过 `target_index_lines` 显式限制本次查询的索引线子集，但只能引用该库已启用的索引线
- 若请求命中未启用索引线，应返回明确不可用状态
- 若请求显式命中，或默认会作用到任一已启用但尚未持有 active index 的索引线，应返回明确未就绪状态，而不是静默忽略该索引线、自动降级到其他索引线或返回空结果
- 搜索期提供方的能力、绑定与已解析提供方选择（Resolved Provider Selection）语义，由 [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义
- 公共控制项固定包括：`library_id`、`filters`、`top_k`、`cursor`、`debug`，以及可选的 `target_index_lines`
- 正式公共过滤器固定包括：`visual_unit.kind`、`path_prefix`、`source_type`、`time_range`
- 分页采用 `cursor` 语义；本专题不固定 cursor 的内部编码方式

## 结果语义

- 搜索结果默认返回有序的视觉单元列表
- `document_page`、`image` 与 `video_segment` 允许在同一结果集中默认混排，并可按 `visual_unit.kind` 过滤
- 公共结果不返回统一 `score` 字段；结果顺序本身承载稳定排序语义
- 公共结果卡片的稳定字段包括：`preview`、`source_path`、`source_type`、`kind`、`locator`、`cursor`
- `preview` 承载可消费的预览引用语义，而不是要求直接暴露原始本地路径
- 同一源内容下的多个命中默认全部返回
- 按源内容聚合只作为可选视图语义存在，不取代默认的视觉单元返回粒度
- 默认公开结果列表只返回最小结果卡片，不默认内联 `neighbor_context`
- 通过对象详情或展开路径返回的邻近上下文固定为源内容级语义：
  - 文档页返回前后页
  - 视频片段返回前后片段
  - 图片返回同源基础信息

## 调试与错误语义

- `debug=true` 时，稳定返回的调试信息至少包括：命中的 `index_line`、各索引线原始分数，以及 `provider`、`backend`、`repr_kind` 等技术元信息
- 调试原始分数只用于诊断，不承诺跨索引线或跨查询请求可直接比较
- 显式请求未启用索引线时，应返回明确不可用状态
- 请求命中已启用但未就绪的索引线时，应返回明确未就绪状态，而不是静默返回空结果
- 公开请求若携带多种查询输入，应返回明确“不支持”
- 视频查询超出允许的大小或时长上限时，应返回明确拒绝，而不是静默截断或自动降级

## 关联主题

- [000-foundation](../000-foundation/spec.md) 定义三类查询输入、三类检索对象与项目级上游定位
- [001-architecture](../001-architecture/spec.md) 定义搜索编排的系统边界、组件职责与交互路径
- [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义视觉单元、定位符、源内容与搜索相关状态边界的基础语义
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 定义已启用索引线、激活索引与搜索可依赖的索引状态边界
- [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义已解析提供方选择、提供方能力与提供方绑定语义
- [008-ui-ux](../008-ui-ux/spec.md) 定义搜索工作区的应用壳层位置、导航关系与全应用体验边界
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义 `/search/*` 的请求 / 响应契约、公共错误载荷与分页编码
