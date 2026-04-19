# 005 提供方配置与模型选择 (Provider Configuration and Model Selection)

定义 FauniSearch 的提供方与模型选择语义，明确哪些配置属于 provider、哪些配置属于 model，以及全局默认、库级覆盖与运行时已解析模型如何共同决定某条索引线是否可执行。

## 关键术语 (Terminology)

- 提供方配置（Provider Config）
- 模型目录项（Model Catalog Entry）
- 索引线模型选择（Index-Line Model Selection）
- 已解析模型选择（Resolved Model Selection）
- 运行时绑定模型（Runtime-Bound Model）
- 运行时修订（Runtime Revision）

## 范围

- 提供方配置与模型选择的稳定语义边界
- 全局默认与库级覆盖的最小解析顺序
- 当前正式索引线的模型配置粒度
- 运行时可见模型事实与显式失败语义

范围外：
- 检索后端产品私有配置、collection schema 与向量参数
- 第三方平台私有凭据字段与计费细节
- sidecar、DashScope 或其他远端平台的私有协议字段
- 模型懒加载、驻留、容量与缓存策略
- 搜索排序、结果语义与任务调度策略

## 设计原则

- Provider 薄化（Thin Provider）：provider 只承载平台与连接语义，不再隐藏实际模型身份
- 模型显式化（Model Is Explicit）：系统必须始终能够直接回答“当前实际用的是哪个 exact model_id”
- 按索引线选择（Index-Line Selection）：当前稳定配置粒度收敛到索引线，不再按 query kind 公开分别选模型
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

约束：
- 当前 provider 集合是内建固定集合；用户不能创建、删除或重命名 provider
- `local_sidecar` 可以展示但在当前切片中以 runtime-bound 方式提供模型事实，不要求支持 UI 中任意热切换模型
- `dashscope` 的 `base_url` 是正式配置维度；即使当前不可执行，也必须作为稳定语义保留

## 模型目录与模型选择

- 模型身份的稳定核心是 exact `model_id` 字符串
- `model_revision` 属于模型观察或运行时上下文的正式辅助维度
- 当前稳定模型选择粒度固定为“按索引线”
- 当前唯一正式 index line 仍是 `multivector`
- 文本、图片、视频与文档查询当前都共享 `multivector` 这条索引线的模型选择
- 当前不再公开“按 query kind 分别选模型”的稳定配置面

索引线模型选择的最小稳定字段包括：
- `provider_id`
- `model_id`

模型目录（Model Catalog）是可展示、可选择的模型清单，而不是 provider 本身：
- 当前目录至少应能表达“是否兼容当前 `multivector` 检索切片”
- 不兼容当前检索切片的文本专用 embedding 模型，例如 `text-embedding-v4`，不得进入当前 `multivector` 可选目录
- 当前 `local_sidecar` 的实际模型来自 sidecar 运行时绑定值；本切片中它应作为“可见但只读”的 runtime-bound 模型展示

## 解析顺序与已解析模型选择

- 当前正式生效的稳定解析顺序固定为：
  - 库级覆盖
  - 全局默认
  - 运行时事实补全
- `resolved model selection` 只有在以下条件同时满足时才成立：
  - 已解析出目标索引线的 provider 与模型选择
  - 所选模型与该索引线兼容
  - provider 当前已启用
  - provider 在当前切片中属于可执行路径，或能明确返回“当前仅配置、不可执行”
  - 运行时探测未将其判定为不可用

已解析模型选择（Resolved Model Selection）的稳定最小字段包括：
- `binding_source`
- `provider_id`
- `provider_kind`
- `model_id`
- 可选 `model_revision`
- `status`
- `message`
- 可选 `last_probed_at`

`binding_source` 当前至少包括：
- `global_default`
- `library_override`

## 当前切片的正式执行语义

- 当前唯一正式可执行模型 provider 仍是 `local_sidecar`
- `local_sidecar` 的真实模型事实来自 sidecar `/health` 与 `/capabilities` 返回的 `model_id` / `model_revision`
- 当前切片中，`local_sidecar` 的 `multivector` 选择应被视为 runtime-bound，而不是任意可写模型切换入口
- `dashscope` 当前只作为配置与未来兼容字段存在；解析到它时必须显式返回 `not_supported`
- `qdrant` 继续作为内部固定检索后端参与运行，但不再出现在 Settings 的模型配置面
- 搜索 debug、库摘要与 Settings 摘要都必须直接暴露 exact `model_id`
- Settings 与库摘要中的主编辑面只应承接 `provider_id` 与 `model_id`；`model_revision` 只作为只读运行时摘要返回

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

- [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义 provider config、model default、library override 与 resolved model 的状态承载位置
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 复用本专题的索引线模型选择与已解析模型语义
- [004-search](../004-search/spec.md) 复用本专题的已解析模型选择与搜索期调试摘要语义
- [007-storage-and-persistence](../007-storage-and-persistence/spec.md) 定义 provider configs 与 model defaults / overrides 的 durable 边界
- [008-ui-ux](../008-ui-ux/spec.md) 定义 Settings、库摘要与搜索工作区中如何呈现 resolved model
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义 provider/model 相关公开接口与 sidecar 暴露的运行时模型事实
