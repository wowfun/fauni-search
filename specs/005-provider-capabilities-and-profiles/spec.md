# 005 提供方能力与配置档 (Provider Capabilities and Profiles)

定义 FauniSearch 的提供方语义边界，明确提供方家族 (Provider Family)、提供方配置档 (Provider Profile)、提供方能力 (Provider Capability)、提供方绑定 (Provider Binding) 与运行时探测 (Runtime Probe) 如何共同决定某项索引或搜索能力是否可用。

## 关键术语 (Terminology)

- 提供方家族（Provider Family）
- 模型提供方（Model Provider）
- 检索后端提供方（Retrieval Backend Provider）
- 提供方配置档（Provider Profile）
- 提供方能力（Provider Capability）
- 提供方绑定（Provider Binding）
- 运行时探测（Runtime Probe）
- 已解析提供方选择（Resolved Provider Selection）

## 范围

- 提供方家族、配置档与能力的稳定语义
- 最小配置、绑定粒度与优先级解析顺序
- 声明能力与运行时探测的关系
- 提供方不可用或能力不满足时的显式失败语义

范围外：
- 具体模型名、默认模型线与权重来源
- 检索后端产品细节与具体协议字段
- HTTP wire shape、sidecar 或远端提供方的私有协议字段、凭据字段与数据库 schema
- 模型懒加载、驻留、容量与缓存策略
- 任务调度、融合排序与搜索结果语义

## 设计原则

- 能力先声明（Declared Capability First）：提供方的稳定能力边界必须先被显式声明，不能仅依赖配置档名称或实现细节推断
- 探测只校验不扩权（Probe Does Not Expand）：运行时探测只能确认、裁剪或拒绝声明能力，不能隐式扩宽能力集合
- 绑定先于执行（Binding Before Execution）：索引与搜索在执行前必须先解析出合法提供方绑定，而不是在运行时临时猜测可用提供方
- 显式失败（Explicit Failure）：缺少可用绑定、能力不满足或运行时不可用时，应明确失败，不自动 fallback 到其他提供方或能力线
- 抽象后端边界（Abstract Backend Boundary）：检索后端提供方在本专题中只定义抽象边界与绑定语义，不把当前后端产品细节写成稳定事实

## 提供方家族与配置档

- 提供方家族是上层分类边界，用于区分不同类型提供方的职责与绑定面
- 模型提供方负责模型侧能力，例如向量编码或查询理解，可同时服务 `indexing` 与 `search`，也可以只服务其中一个阶段
- 检索后端提供方负责检索索引承载与查询执行的后端抽象
- 正式模型提供方配置档固定为：
  - `local_python`
  - `remote_http`
- `local_python` 表示本地 Python sidecar 协作路径；该配置档可以声明设备偏好，但设备偏好只代表可声明偏好，不代表最终可用性承诺
- `remote_http` 表示远端 HTTP 服务协作路径；其能力仍需显式声明并接受运行时探测校验
- 检索后端提供方保持抽象家族语义；`QdrantProvider` 只可作为当前实现实例被提及，不构成产品级稳定承诺

## 提供方能力

- 提供方能力是提供方的稳定能力声明，至少覆盖三类维度：
  - 支持的查询输入：`text`、`image`、`video`
  - 支持的索引线：`single-vector`、`multivector`
  - 支持的使用阶段：`indexing`、`search`
- 提供方可以只支持其中一部分输入类型、索引线或使用阶段
- 对检索后端提供方而言，上述维度表达的是兼容边界与可承载范围，不意味着其直接承担模型推理职责
- 支持某类查询输入，不自动意味着支持所有索引线
- 支持某条索引线，不自动意味着同时支持 `indexing` 与 `search` 两个阶段
- 提供方能力是该提供方的声明上限；运行时只能把可用集合缩小或判定不可用，不能把声明中不存在的能力补出来

## 提供方绑定与解析顺序

- 提供方绑定是把提供方选择规则绑定到具体使用场景的稳定语义
- 稳定绑定粒度至少包括：
  - 按索引线绑定，用于解析某条索引线的索引期提供方选择
  - 按查询用途或查询类型（query kind）绑定，用于解析某类搜索请求的提供方选择
- 同一提供方家族不要求所有绑定面完全对称，但都必须服从统一的解析顺序
- 稳定解析顺序固定为：
  - 显式绑定
  - 库级覆盖
  - 全局默认
- 已解析提供方选择（Resolved Provider Selection）只有在以下条件同时满足时才成立：
  - 已解析出某个提供方选择
  - 该提供方的声明能力覆盖目标输入、索引线与使用阶段
  - 运行时探测未将其判定为不可用
- 若缺少可用绑定，或解析出的提供方无法满足能力 / 探测约束，应显式失败

## 运行时探测与失败语义

- 运行时探测用于在运行时校验提供方是否真实可用，以及声明能力是否需要被裁剪
- 运行时探测可以验证例如：
  - 配置档是否可达
  - 运行环境是否满足声明能力
  - 可声明的设备偏好是否在当前环境中可兑现
- 运行时探测可以产生三类稳定结果：
  - 确认可用
  - 裁剪可用能力
  - 判定不可用并返回失败
- 对 `local_python` 这类配置档，设备偏好不可满足时，应按探测结果显式失败或裁剪，而不是把设备偏好当作自动可兑现承诺
- 提供方解析失败、能力不匹配或探测不通过时，系统不得静默切换到其他提供方、其他索引线或其他查询路径
- 本地运行时托管、模型驻留、容量逐出与维护执行语义由 [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 定义，不在本专题重写

## 关联主题

- [000-foundation](../000-foundation/spec.md) 定义提供方驱动架构、多向量能力与项目级基础约束
- [001-architecture](../001-architecture/spec.md) 定义提供方所处的系统边界、组件职责与交互路径
- [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义提供方绑定状态的承载位置、作用域与事实源归属
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 定义正式来源边界与索引线生命周期，并复用本专题的提供方能力与提供方绑定语义
- [004-search](../004-search/spec.md) 定义搜索语义与结果语义，并复用本专题的已解析提供方选择与提供方能力语义
- [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 定义本地运行时托管、模型驻留、容量逐出与后台维护执行语义
- [007-storage-and-persistence](../007-storage-and-persistence/spec.md) 定义提供方绑定记录、持久队列记录与检索命名空间的物理持久化边界
- [008-ui-ux](../008-ui-ux/spec.md) 定义提供方绑定与相关设置的管理体验、应用入口与非搜索控制面接口族
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义搜索与控制面公开接口的编码契约，以及 Rust / Python sidecar 的稳定协议载荷
