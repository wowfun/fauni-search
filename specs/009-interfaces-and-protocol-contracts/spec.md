# 009 接口与协议契约 (Interfaces and Protocol Contracts)

定义 FauniSearch 的接口与协议契约，明确应用公开接口如何编码，以及 Rust 主服务与 Python sidecar 之间的稳定 HTTP/JSON 协议如何构成统一事实源。

## 关键术语 (Terminology)

- 请求封套（Request Envelope）
- 响应封套（Response Envelope）
- 错误载荷（Error Payload）
- 资源引用对象（Resource Reference）
- 游标令牌（Cursor Token）
- 搜索请求载荷（Search Request Payload）
- 搜索响应载荷（Search Response Payload）
- 任务动作载荷（Task Action Payload）
- 任务快照载荷（Task Snapshot Payload）
- 健康快照载荷（Health Snapshot Payload）
- sidecar HTTP 契约（Sidecar HTTP Contract）

## 范围

- 应用公开接口的稳定请求 / 响应契约
- 搜索接口与非搜索控制面接口的请求 / 响应形状
- 公共错误载荷、分页 / 游标、资源句柄、时间戳与诊断字段的最小编码约定
- Rust 主服务与 Python sidecar 的稳定 HTTP/JSON 协议边界

范围外：
- 搜索排序、过滤规则、结果语义与默认搜索行为
- 任务状态机、恢复算法、调度策略与健康判定算法
- 提供方私有远端 API、凭据字段与第三方产品协议
- SQLite/Qdrant schema、目录布局、前端路由与视觉样式

## 设计原则

- 语义与编码分离（Semantics Before Encoding）：领域语义由各自专题承接，本专题只定义这些语义如何被稳定编码
- 公开契约统一（Unified Public Contract）：公开接口应复用统一的请求封套、响应封套、错误载荷与分页约定，而不是每个接口族各自发明编码模型
- 窄协议边界（Narrow Protocol Boundary）：跨进程协议只传递稳定输入、输出与诊断信息，不泄露内部对象或实现细节
- 显式错误（Explicit Errors）：验证失败、不可用、超时与不支持场景必须通过稳定错误载荷表达，而不是依赖隐式约定
- 游标不透明（Opaque Cursor）：游标令牌只作为后续分页输入使用，不承诺客户端可解析其内部编码

## 统一公开契约

- 应用公开接口固定以结构化 JSON 对象承载请求与响应；若通过 HTTP 暴露，其 body 应直接复用本专题定义的请求 / 响应载荷
- 请求封套是顶层请求对象；操作字段直接位于顶层，并可按需携带 `request_id`、`debug`、`cursor` 等通用字段
- 响应封套是顶层响应对象；成功响应必须包含 `data`，失败响应必须包含 `error`，两者不得同时出现
- 列表或分页响应可以在 `data` 内承载结果数组，并在顶层响应中返回 `next_cursor`
- 调试或诊断字段只应在显式请求调试信息、或协议约定必须返回诊断摘要时出现
- 公开时间戳应采用 RFC 3339 / ISO 8601 字符串表达
- 公开资源标识、任务句柄、运行时句柄与引用句柄应采用稳定字符串表示，不暴露底层数据库主键或后端私有命名细节

### 错误载荷与错误码族

- 错误载荷至少包含：`code`、`message`
- 错误载荷可按需附带：`details`、`retryable`、`resource`、`diagnostics`
- 公共稳定错误码族至少包括：
  - `invalid_request`
  - `validation_failed`
  - `not_supported`
  - `not_enabled`
  - `not_ready`
  - `not_found`
  - `conflict`
  - `runtime_unavailable`
  - `timeout`
  - `internal_error`
- 领域专题可以定义何时触发这些错误，但其公开编码与错误载荷形状由本专题统一承接

## 搜索公开接口契约

- 正式公开搜索端点固定为：
  - `POST /search/text`
  - `POST /search/image`
  - `POST /search/video`
- 这三类端点共享同一搜索请求封套，至少包含：`library_id`、`filters`、`top_k`、`cursor`、`debug`，以及可选的 `target_index_lines`
- `/search/text` 的请求载荷必须携带 `text`
- `/search/image` 的请求载荷必须携带 `image_input`
- `/search/video` 的请求载荷必须携带 `video_input`
- `image_input` 与 `video_input` 都必须编码为显式区分输入来源的结构化对象
- 图片与视频查询长期上允许两类稳定编码形式：临时查询资产引用、库内对象引用
- 图片查询的稳定输入对象至少应支持以下两种显式编码：
  - 临时查询资产引用：`{ "kind": "temp_asset", "temp_asset_id": "..." }`
  - 库内对象引用：`{ "kind": "library_object", "visual_unit_id": "..." }`
- 视频查询的稳定输入对象至少应支持以下两种显式编码：
  - 临时查询资产引用：`{ "kind": "temp_asset", "temp_asset_id": "...", "locator": { "start_ms": ..., "end_ms": ... }? }`
  - 库内对象引用：`{ "kind": "library_object", "source_id": "...", "visual_unit_id": "..."?, "locator": { "start_ms": ..., "end_ms": ... }? }`
- 视频查询的 `locator` 若出现，必须同时包含 `start_ms` 与 `end_ms`；缺失 `locator` 时表示整段视频查询
- 当视频查询使用库内对象引用时，能力专题可以选择：
  - 通过 `source_id` 表示“整段视频或显式时间范围”
  - 通过 `visual_unit_id` 表示“直接复用某个库内 `video_segment` 作为查询输入”
- 各能力专题可以在当前阶段先只启用其中一个正式输入变体；未启用变体应通过统一错误载荷返回 `not_supported`
- 搜索响应载荷通过响应封套中的 `data` 返回，至少包含：
  - 有序 `results`
  - 可选 `next_cursor`
  - 可选 `debug`
- 每个搜索结果项的稳定字段至少包括：`preview`、`source_path`、`source_type`、`kind`、`locator`、`cursor`，以及可选 `score`
- `preview` 必须编码为结构化资源引用对象；该对象至少包含一个可直接取用、对客户端保持不透明的 `url`，并可按需附带 `handle`
- `neighbor_context` 不在搜索结果列表中默认内联返回；对象详情与展开接口负责承接该信息
- 若搜索结果返回 `score`，该字段必须编码为数值，并且只表达当前查询 / 当前后端返回的相对排序强弱；客户端不得将其视为跨查询、跨索引线或跨后端可直接比较的统一分值
- `debug` 载荷继续承接更细粒度的后端原始分数与技术诊断；公开 `score` 不能替代这些调试字段
- 搜索请求若同时携带多种查询输入、显式请求未启用索引线，或命中已启用但未就绪的目标索引线，应通过统一错误载荷返回失败
- `not_ready` 错误的 `details` 至少应支持按索引线返回结构化条目；每个条目至少可表达 `index_line`，并可附带相关 `job` / `phase` 摘要
- 搜索结果字段的含义、过滤 / 排序规则、邻近上下文语义与显式拒绝规则由 [004-search](../004-search/spec.md) 定义

## 非搜索控制面接口契约

- 非搜索控制面接口族由 [008-ui-ux](../008-ui-ux/spec.md) 定义其存在与职责；本专题固定这些接口族的公开编码
- 若通过 HTTP 暴露，库创建接口的稳定入口应包括 `POST /libraries`
- `POST /libraries` 的请求载荷至少包含：`name` 与 `config`
- `config` 作为稳定扩展对象，承接例如 `enabled_index_lines` 一类库级配置；本专题不固定其完整字段集
- 库管理、来源根与规则管理、配置与绑定管理、收藏管理、搜索历史管理等资源型接口，应采用统一的资源快照载荷与列表 / 详情 / 变更响应形状
- 若通过 HTTP 暴露，视觉对象详情接口的稳定入口应包括 `GET /libraries/{library_id}/visual-units/{visual_unit_id}`
- 若通过 HTTP 暴露，视觉对象预览资源的稳定入口应包括 `GET /libraries/{library_id}/visual-units/{visual_unit_id}/preview`
- 视觉对象详情响应至少应返回目标对象的稳定详情快照、稳定 `preview` 资源引用对象，并可附带 `neighbor_context`
- 导入、刷新、重扫、重建、清理、维护，以及任务取消 / 重试 / 恢复等动作型接口，应采用显式动作载荷，而不是通过隐式读写触发后台执行
- 若通过 HTTP 暴露，当前正式导入动作入口应包括 `POST /libraries/{library_id}/imports`
- `POST /libraries/{library_id}/imports` 的首个稳定输入变体是本地路径列表；本专题不阻止未来新增上传或其他输入变体
- 若通过 HTTP 暴露，临时查询图片上传入口的稳定入口应包括 `POST /libraries/{library_id}/query-assets/images`
- 若通过 HTTP 暴露，临时查询视频上传入口的稳定入口应包括 `POST /libraries/{library_id}/query-assets/videos`
- `POST /libraries/{library_id}/query-assets/images` 的首个稳定输入变体是单图片上传；本专题不阻止未来新增批量上传或其他查询资产输入变体
- `POST /libraries/{library_id}/query-assets/videos` 的首个稳定输入变体是单视频上传；本专题不阻止未来新增批量上传或其他查询资产输入变体
- 临时查询图片上传成功响应至少应返回：`temp_asset_id`、稳定 `preview` 资源引用对象，以及最小输入摘要
- 临时查询视频上传成功响应至少应返回：`temp_asset_id`、稳定 `preview` 资源引用对象，以及最小输入摘要
- 动作型成功响应应在 `data` 中返回至少以下信息：
  - `accepted`
  - `rejected`
  - `job_handle`
  - 初始任务快照或等价任务引用
- 导入动作的 `accepted` / `rejected` 逐项回执至少应包含：
  - 原始路径
  - 规范化路径（若可得）
  - `reason_code`
  - `message`
- 任务快照载荷至少应包含：`job_id`、`kind`、`status`、`phase`、`progress`、`cancelable`、`current_attempt`
- 运行时健康快照载荷至少应包含：`runtime_kind`、`status`、`last_probe_at`、`diagnostics`
- 管理列表型响应与搜索分页一样复用 `next_cursor` 语义，但不要求与搜索结果使用相同的列表字段名称
- 本专题不强制控制面接口必须采用 HTTP 路由；若经由 IPC 或其他公开边界暴露，其请求 / 响应 payload 仍应复用本专题定义的稳定形状

## Rust 主服务与 Python sidecar 的稳定协议

- Rust 主服务与 Python sidecar 之间的稳定跨进程协议固定为 HTTP/JSON
- sidecar HTTP 契约至少覆盖以下协议面：
  - 健康探测
  - 能力 / 可用性探测
  - 推理 / 编码请求
  - 失败、超时与不可用返回
- 当前阶段 sidecar 的稳定入口至少包括：
  - `GET /health`
  - `GET /capabilities`
  - `POST /embed`
- 健康探测响应必须返回健康快照载荷，至少表达：可用 / 降级 / 不可用状态、最近一次探测结果与诊断摘要
- 能力 / 可用性探测响应必须能表达：声明能力、当前可用能力裁剪结果，以及是否可服务目标操作
- 推理 / 编码请求至少应显式携带：
  - `operation_kind`
  - 输入引用或临时资产引用
  - 已解析提供方选择摘要或等价执行上下文
  - 与目标索引线或目标输出相关的最小上下文
  - 可选 `debug`
- 当前阶段 `POST /embed` 至少应支持以下 `operation_kind`：
  - `query_embedding`，用于承接文本查询编码；其输入至少应能表达一个或多个查询文本，以及与目标索引线相关的最小上下文
  - `image_query_embedding`，用于承接图片查询编码；其输入至少应能表达一个或多个本地图片引用，且在需要时能够携带视觉单元级 `locator`
  - `video_query_embedding`，用于承接视频查询编码；其输入至少应能表达一个或多个本地视频引用，并在需要时能够携带视频时间范围 `locator`
  - `document_embedding`，用于承接当前阶段 PDF 页图或图片对象的编码；其输入至少应能表达一个或多个本地文件引用，以及与目标索引线相关的最小上下文
- `image_query_embedding` 的单项输入在需要时必须能够携带视觉单元级 `locator`；当前阶段至少应支持用页定位符表达库内 `document_page` 作为查询图片的路径
- `video_query_embedding` 的单项输入在需要时必须能够携带视频时间范围 `locator`；当前阶段至少应支持用 `start_ms` / `end_ms` 表达整段视频中的目标片段
- `document_embedding` 的单项输入在需要时必须能够携带视觉单元级 `locator`；当前阶段至少应支持用页定位符表达 PDF 的目标页
- `document_embedding` 的成功响应应按请求输入逐项返回结构化结果；若输入携带了视觉单元级 `locator`，响应应能返回与该输入对应的定位摘要
- 推理 / 编码成功响应必须返回与 `operation_kind` 对应的结构化 `data`，例如向量输出、派生结果描述或媒体处理摘要；不得依赖未文档化的 sidecar 私有字段
- sidecar 的超时、不可达、能力不满足与内部失败，必须复用稳定错误载荷与错误码族表达，而不是只依赖传输层异常
- sidecar 协议只承接公开输入 / 输出 / 诊断契约；模型加载、驻留、容量逐出与运行时托管语义由 [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 定义

## 关联主题

- [000-foundation](../000-foundation/spec.md) 定义项目级基础约束、HTTP/JSON 边界基线与上游能力定位
- [001-architecture](../001-architecture/spec.md) 定义 Rust 主服务、Python sidecar 与其他一级组件之间的系统边界
- [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义资源标识、任务状态、健康状态与辅助状态的逻辑模型
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 定义导入、刷新、重扫、索引线切换与内容版本的上游行为语义
- [004-search](../004-search/spec.md) 定义搜索查询、结果语义、过滤分页规则与显式拒绝条件
- [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义提供方能力、绑定、解析顺序与运行时探测判定语义
- [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 定义任务执行、任务恢复、运行时健康与 sidecar 托管语义
- [007-storage-and-persistence](../007-storage-and-persistence/spec.md) 定义结构化记录、任务记录、检索命名空间与文件载荷的物理落点
- [008-ui-ux](../008-ui-ux/spec.md) 定义搜索工作区、管理工作区、控制面入口与应用级体验边界
