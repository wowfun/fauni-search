# 005 提供方配置与模型选择 (Provider Configuration and Model Selection)

定义 FauniSearch 的提供方与模型选择语义，明确哪些配置属于 provider、哪些配置属于 model，以及 provider 下嵌套的 model 配置、`local_sidecar.active_model` 与运行时已解析模型如何共同决定某个内容类型是否可执行。

## 关键术语 (Terminology)

- 提供方配置（Provider Config）
- 模型目录项（Model Catalog Entry）
- 向量能力（Embedding Capabilities）
- 执行输入能力（Execution Input Types）
- 活跃模型（Active Model）
- 模型版本（Model Version）
- 内容类型模型绑定（Content-Type Model Binding）
- 已解析内容模型（Resolved Content Model）
- 运行时绑定模型（Runtime-Bound Model）
- 运行时修订（Runtime Revision）

## 范围

- 提供方配置与模型选择的稳定语义边界
- provider 下嵌套 model 的稳定配置结构
- `local_sidecar.active_model` 与 model `version` 的稳定语义
- 全局默认与库级覆盖的最小解析顺序
- 当前正式内容类型的模型配置粒度
- 运行时可见模型事实、工程适配能力与显式失败语义

范围外：
- 检索后端产品私有配置、collection schema 与向量参数
- 第三方平台私有凭据字段与计费细节
- sidecar、DashScope 或其他远端平台的私有协议字段
- 模型懒加载、驻留、容量与缓存策略
- 搜索排序、结果语义与任务调度策略

## 设计原则

- Provider 薄化（Thin Provider）：provider 只承载平台与连接语义，不再隐藏实际模型身份
- 模型显式化（Model Is Explicit）：系统必须始终能够直接回答“当前实际用的是哪个 exact model_id”
- 原生事实优先（Embedding Facts First）：Embedding 模型只声明其原生输入与原生向量形态，不承担 query / index 角色语义
- 执行层独立（Execution Layer Is Separate）：模型原生能力与运行时可执行查询输入必须分离表达；工程适配输入不得反向污染 `EmbeddingCapabilities`
- 按内容类型选择（Content-Type Selection）：当前稳定配置粒度收敛到内容类型，而不是 query kind 或索引线
- 运行时事实优先（Runtime Truth Wins）：本地运行时已绑定模型时，应直接暴露运行时返回的 `model_id` / `model_revision`
- 显式失败（Explicit Failure）：模型不可执行、运行时不可达或当前切片未实现时，必须明确失败，不自动 fallback

## Provider Config

- Provider Config 是内建稳定资源，而不是用户任意创建 / 删除的 profile
- 当前固定 provider 集合为：
  - `local_sidecar`
  - `dashscope`
- `local_sidecar` 表示当前本地 Python sidecar 执行链路
- `dashscope` 表示面向未来百炼 / DashScope 兼容的 provider 语义位；本切片中允许配置、展示与持久化，但不要求进入真实执行路径
- `qdrant` 不再作为用户可配置 provider 暴露；它在当前切片中降为内部固定检索后端，只继续出现在运行时健康与调试摘要中

Provider Config 的最小稳定字段包括：
- `provider_id`
- `display_name`
- `provider_kind`
- `enabled`
- 可选 `base_url`
- 可选 `readonly_reason`
- 可选 `active_model`

约束：
- 当前 provider 集合是内建固定集合；用户不能创建、删除或重命名 provider
- `local_sidecar` 可以展示但在当前切片中以 runtime-bound 方式提供模型事实，不要求支持 UI 中任意热切换模型
- `dashscope` 的 `base_url` 是正式配置维度；即使当前不可执行，也必须作为稳定语义保留
- 配置文件中的 model 应嵌套在 provider 下，即 `provider.<provider_id>.models.<model_id>`
- `local_sidecar` 必须显式提供 `active_model`；其值必须指向该 provider 下某个已定义 model

provider 下嵌套 model 的最小稳定字段包括：
- `enabled`
- `version`
- `embedding_capabilities`

约束：
- `version` 是模型真实执行版本，不是展示标签
- `version` 允许自由字符串；默认值为 `main`
- 当前不支持同一 provider 下同一 `model_id` 的多版本并存
- provider/model 的项目级基线事实源是仓库根 `fauni.config.json`
- provider/model 的实例级覆盖事实源是 `${APP_RUNTIME_DIR}/runtime-config.json`

## 模型目录与模型选择

- 模型身份的稳定核心是 exact `model_id` 字符串
- `model_revision` 属于模型观察或运行时上下文的正式辅助维度
- model 的正式配置字段名是 `version`；运行时摘要中的 `model_revision` 只是对当前真实执行版本的只读观测
- 当前稳定模型选择粒度固定为“按内容类型”
- 当前正式内容类型至少包括：
  - `image`
  - `document`
  - `video`
  - `text`
- 当前不再公开“按 query kind 分别选模型”的稳定配置面

内容类型模型绑定的最小稳定字段包括：
- `enabled`
- `model`
- `vector_type`

模型目录（Model Catalog）是可展示、可选择的模型清单，而不是 provider 本身：
- 当前目录至少应能表达该模型的原生向量能力，而不是工程增强后的输入适配能力
- 不兼容当前执行切片的文本专用 embedding 模型，例如 `text-embedding-v4`，不得进入当前可执行内容类型的可选目录
- 当前 `local_sidecar` 的实际模型来自配置文件中的 `active_model + version`，并由 sidecar 运行时回传其真实 `model_id / model_revision`；本切片中它应作为“可见但只读”的 runtime-bound 模型展示

Embedding Capabilities 是模型原生事实的稳定最小承载，不引入 query / index 角色语义。

Embedding Capabilities 的正式字段包括：
- `input_types`
- `vector_types`
- `supports_mixed_inputs`

约束：
- `input_types` 只表达模型真正原生支持的输入类型；当前切片只要求承接 `text` 与 `image`
- `document` 与 `video` 不得进入 Embedding Capabilities；它们属于运行时工程适配输入
- `vector_types` 当前至少支持：
  - `single_vector`
  - `independent_vectors`
  - `multi_vector_late_interaction`
- `supports_mixed_inputs` 表达单个逻辑输入样本中是否允许混合多种原生输入类型

Execution Input Types 用于表达当前 provider / model / runtime adapter 组合在正式搜索执行面上可承接的查询输入；它不属于模型原生事实。

Execution Input Types 的约束：
- 它们必须由运行时 `/capabilities` 中的 `operations + runtime_adapters` 派生，而不是从 `EmbeddingCapabilities` 反推
- 它们当前至少允许表达：
  - `text`
  - `image`
  - `document`
  - `video`
- 它们只允许出现在运行时健康、诊断与调试摘要中
- Settings、model-catalog 与 resolved content models 不得把 Execution Input Types 混入 `EmbeddingCapabilities`

## 解析顺序与已解析内容模型

- 当前正式生效的稳定解析顺序固定为：
  - 库级覆盖
  - 全局默认
  - 运行时事实补全
- `resolved content model` 只有在以下条件同时满足时才成立：
  - 已解析出目标内容类型的 provider、模型与 `vector_type`
  - 所选模型与该内容类型声明的 `vector_type` 兼容
  - provider 当前已启用
  - provider 在当前切片中属于可执行路径，或能明确返回“当前仅配置、不可执行”
  - 运行时探测未将其判定为不可用

已解析内容模型（Resolved Content Model）的稳定最小字段包括：
- `binding_source`
- `content_type`
- `provider_id`
- `provider_kind`
- `model_id`
- `model_version`
- 可选 `model_revision`
- `vector_type`
- 可选 `vector_space_id`
- `embedding_capabilities`
- `status`
- `message`
- 可选 `last_probed_at`

`binding_source` 当前至少包括：
- `global_default`
- `library_override`

## 当前切片的正式执行语义

- 当前唯一正式可执行模型 provider 仍是 `local_sidecar`
- `local_sidecar` 的真实模型事实来自配置文件中的 `active_model + version`，并由 sidecar `/health` 与 `/capabilities` 回传其真实 `model_id` / `model_revision`
- 当前切片中，`local_sidecar` 的正式执行选择应被视为 runtime-bound，而不是任意可写模型切换入口
- 当前 `local_sidecar` / ColQwen 运行时的 Embedding Capabilities 固定为：
  - `input_types = ["text", "image"]`
  - `vector_types = ["multi_vector_late_interaction"]`
  - `supports_mixed_inputs = false`
- 当前 `local_sidecar` / ColQwen 运行时的正式 Execution Input Types 固定为：
  - `text`
  - `image`
  - `document`
  - `video`
- `dashscope` 当前只作为配置与未来兼容字段存在；解析到它时必须显式返回 `not_supported`
- `qdrant` 继续作为内部固定检索后端参与运行，但不再出现在 Settings 的模型配置面
- 搜索 debug、库摘要与 Settings 摘要都必须直接暴露 exact `model_id`
- 搜索 debug、库摘要与 Settings 摘要都必须直接暴露当前配置中的 `model_version`
- Settings 与库摘要中的主编辑面应以内容类型为入口，承接：
  - `enabled`
  - `model`
  - `vector_type`
- `model_revision` 只作为只读运行时摘要返回
- `vector_space_id` 只作为派生执行诊断事实返回；它不得成为用户主配置输入
- 本地 sidecar 加载、本地主服务 fallback 摘要与本地下载脚本都应默认以 `local_sidecar.active_model` 对应 model 的 `version` 作为执行版本
- Settings 必须提供“测试当前 Provider + 模型配置”的诊断入口，用于在保存前直接验证当前草稿是否能返回 embedding
- Settings 模型测试固定使用当前未保存草稿，而不是已持久化的 defaults / overrides
- Settings 模型测试只应回传当前 provider 的可编辑草稿字段；`local_sidecar` 这类 runtime-bound provider 的连接信息只可展示、不可作为测试草稿重新提交
- Settings 模型测试的输入模态必须由当前模型目录项或等价运行时能力快照中的 `Embedding Capabilities.input_types` 驱动
- 当前切片中，Settings 模型测试只要求承接 `text` 与 `image`
- Settings 模型测试除主输入外，还应允许一个可选的第二输入；第二输入的模态应独立选择，但同样必须受 `Embedding Capabilities.input_types` 约束
- 当提供第二输入时，Settings 模型测试除返回第二输入对应的向量结果外，还必须返回第二输入与主输入之间的相似度
- Settings 模型测试中的跨模态相似度是模型诊断能力；它不改变正式索引、正式搜索或 `multivector` 的执行语义
- Settings 模型测试是纯诊断能力，不创建 job，不写 durable state，不改变全局默认、库级覆盖或已解析模型选择
- 当前切片中，`dashscope` 仍只作为配置与未来兼容字段存在；在 Settings 模型测试中选择它时，必须显式返回 `not_supported`
- 文档与视频查询能力属于 runtime adapter，不属于模型原生能力；它们只允许在运行时诊断或调试面中以命名 adapter 列表呈现
- 文档与视频查询能力虽然不属于模型原生能力，但在当前切片中属于 `local_sidecar` 的正式执行能力；搜索是否可执行必须以 Execution Input Types 判定，而不是以 `EmbeddingCapabilities.input_types` 判定
- 已启用内容类型可以解析到多个不同的可执行 `model/vector_type` 组合；系统必须按组合派生多个 `vector_space`，而不是要求全库收敛到单一执行绑定

## 运行时探测与失败语义

- provider 的可达性与可执行性应通过运行时探测表达，但探测结果不改变模型身份语义
- `local_sidecar` 的探测至少应覆盖：
  - sidecar 可达性
  - `can_service`
  - 当前绑定的 `model_id`
  - 当前绑定的 `model_revision`
- `dashscope` 当前不要求进入真实 probe 或真实执行路径；若被选中，应稳定返回 `not_supported`
- 解析失败、provider 禁用、运行时不可达或模型不兼容时，系统不得自动切换到其他 provider 或其他模型

## 兼容语义

- 当前 005 公开配置面不承诺向后兼容旧的 profile/binding 抽象
- 本专题的稳定配置面只承接 `provider_id` 与 `model_id`

## 关联主题

- [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义 provider config、content type bindings 与 resolved content models 的状态承载位置
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 复用本专题的内容类型模型绑定与已解析内容模型语义
- [004-search](../004-search/spec.md) 复用本专题的已解析内容模型与搜索期调试摘要语义
- [007-storage-and-persistence](../007-storage-and-persistence/spec.md) 定义 provider configs 与 content type bindings 的 durable 边界
- [008-ui-ux](../008-ui-ux/spec.md) 定义 Settings、库摘要与搜索工作区中如何呈现 resolved content models
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义 provider/model 相关公开接口与 sidecar 暴露的运行时模型事实
