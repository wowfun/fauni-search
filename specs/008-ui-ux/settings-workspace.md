# 008 设置工作区 (Settings Workspace)

定义 FauniSearch 设置工作区的章节导航、主编辑面、模型测试与诊断分层，明确设置工作区如何承接 provider 配置、内容类型配置与当前库覆盖。

## 关键术语 (Terminology)

- 设置工作区（Settings Workspace）
- 章节导航（Settings Nav Rail）
- 活动工作面（Active Settings Surface）
- 模型提供方（Provider Models）
- 内容类型（Content Types）
- 当前库覆盖（Library Overrides）
- 模型测试（Model Tests）
- 诊断（Diagnostics）

## 范围

- 设置工作区内部的信息架构与共享布局
- 各设置章节的职责边界与主次关系
- provider/model 配置、全局内容类型、当前库覆盖、模型测试与诊断在设置工作区中的承载方式
- 当前库上下文、当前生效模型与 `vector_space` 诊断摘要在设置中的展示层级

范围外：
- provider 配置、`content_types`、resolved model、模型测试与诊断接口的请求 / 响应编码
- provider 解析、运行时 adapter、任务推进与持久化实现
- 像素级布局、设计 token 与具体组件实现

## 设计原则

- 配置先于诊断（Configuration Before Diagnostics）：主设置流应优先承接真实配置动作，诊断后置
- 内容类型优先（Content-Type First）：设置主入口应围绕内容类型配置与当前库覆盖组织，而不是围绕 provider 工程字段平铺
- 当前库差异显式（Visible Library Difference）：当前库覆盖必须清楚表达“沿用全局默认”与“覆盖当前库”的差异
- 生效结果可见（Effective Result Must Be Visible）：用户调整配置时，必须能直接看到当前生效模型或等价摘要
- 原生能力与工程增强分离（Native Facts Separate From Runtime Adapters）：模型原生能力、执行输入类型与 runtime adapters 必须分开展示
- 模型测试独立于保存（Testing Separate From Saving）：模型测试固定面向当前草稿，不应与正式保存流混杂

## 工作区骨架

- 设置工作区采用左侧章节导航 + 右侧活动工作面的结构
- 左侧章节导航只负责选择章节；章节职责应通过稳定命名和必要状态标签表达，不再为每个入口常驻解释性长句
- 章节导航顺序固定为：
  - 模型提供方
  - 内容类型
  - 当前库覆盖
  - 诊断
- 设置工作区中的主入口固定为：
  - 模型提供方
  - 内容类型
  - 当前库覆盖
- 诊断明确后置，不应与主设置流同等竞争
- 右侧活动工作面必须直接进入当前章节的正式编辑面、对照面或诊断面；章节标题可以保留，但不再常驻独立概览 hero、指标卡、解释性摘要或与章节同义的二级标题

## 内容类型

- `内容类型` 章节承接全局 `content_types` 的正式编辑入口
- 工作区必须允许用户按内容类型进入具体编辑，而不是一次铺开所有内容类型字段
- 内容类型编辑至少应承接：
  - `enabled`
  - `model`
  - `vector_type`
- `enabled` 是主开关
- 每类内容类型可以独立保存
- 内容类型摘要应优先表达当前选择和保存结果，不应堆叠技术事实或重复显示内容类型 id 与名称
- 当前阶段 `内容类型` 章节优先展示内容类型切换与当前表单；全局启用数等摘要只在确实影响当前操作时显示

## 当前库覆盖

- `当前库覆盖` 章节承接库级 `content_types` 覆盖与对应的生效结果摘要
- 用户必须能够清楚表达：
  - 继承全局默认
  - 覆盖当前库
- `恢复默认` 或等价动作必须是明确次主动作，而不是隐藏入口
- 当前库覆盖编辑面必须直接展示当前生效结果，包括：
  - 当前 resolved model 摘要
  - 当前 `model_id`
  - 当前 `model_version`
  - 当前 `vector_type`
  - 当前绑定来源摘要
- 当前阶段 `当前库覆盖` 章节直接把继承 / 覆盖控制、生效结果和编辑表单放在同一个工作面内，不再额外增加章节概览层

## 模型提供方

- `模型提供方` 章节承接 runtime overlay 中 provider 与 provider models 的正式编辑入口
- `模型提供方` 章节可以同时承接 provider 状态摘要、provider 字段、模型字段与测试面，但不应演化成新的运维控制台
- 连接地址只是 provider 的一个字段，不应成为独立用户任务或章节名称
- 当前 exact `model_id` 与 `model_version` 必须直接可见；用户不应需要理解内部执行线字段才能知道实际模型
- `model_revision` 应作为只读运行时事实展示
- 当前阶段 `模型提供方` 章节直接呈现 provider 列表、当前 provider 编辑器、当前 model 编辑器与测试台；健康状态仅作为状态标签和只读运行时事实出现
- Provider 与 model 的保存只写 `${APP_RUNTIME_DIR}/runtime-config.json` 覆盖层；删除覆盖表示回落 repo 基线

## 模型测试

- 模型测试固定面向 `模型提供方` 中当前 provider + model 的未保存草稿，而不是已保存配置
- 模型测试区必须根据当前模型的 `EmbeddingCapabilities.input_types` 动态渲染输入控件：
  - `text` 显示文本输入
  - `image` 显示单文件输入
- 模型测试区除主输入外，还必须支持一个可选的第二输入区域；第二输入的模态选择应与主输入独立，但同样必须受当前模型原生 `input_types` 约束
- 模型测试结果至少应展示：
  - 当前 resolved model 摘要
  - `operation_kind`
  - embedding `shape`
  - 向量结果
- 当用户提供第二输入时，模型测试结果还必须展示：
  - 第二输入的 `operation_kind`
  - 第二输入的 embedding `shape`
  - 第二输入的向量结果
  - 第二输入与主输入之间的相似度
- 当当前 provider / model 不支持测试时，设置工作区必须明确展示 `not_supported` 或等价原因，而不是静默禁用
- 当前阶段不再保留独立 `模型测试` 章节；测试目标和支持模态只保留在 `模型提供方` 当前编辑面内部

## 诊断

- `诊断` 章节承接运行时健康摘要、`vector_space` 生命周期摘要与其他技术性事实
- `诊断` 章节当前也是 Jobs 的唯一正式观察面；Jobs 应作为默认折叠区与运行时事实并列，而不是另起壳层 drawer
- `vector_space diagnostics` 仅在当前库上下文下显示，不应伪装成全局主设置字段
- `vector_space_id` 只允许出现在诊断 / 调试摘要中，不应成为主编辑面字段
- “查看任务”类深链接必须统一打开 `设置 > 诊断` 并自动展开 Jobs 区，而不是先经过中间抽屉
- 当前阶段 `诊断` 章节直接呈现运行时健康、默认折叠的 Jobs 区与执行空间诊断；不再保留“打开工具”式维护桥接卡，也不在设置页重复堆叠当前库维护按钮墙

## 设置工作区中的运行时与持久化事实

- Settings 工作区中的 provider / model settings 保存语义固定为：
  - 读取 repo 基线 `fauni.config.json` 与 `${APP_RUNTIME_DIR}/runtime-config.json` 的 merged 结果
  - 用户侧写入只落 `${APP_RUNTIME_DIR}/runtime-config.json`
  - Settings 不得把 provider config、全局 `content_types` 配置或库级内容类型覆盖重新写回 `state.sqlite`
- `内容类型` 章节编辑 `${APP_RUNTIME_DIR}/runtime-config.json.content_types` 的固定四类覆盖，删除单项覆盖表示恢复 repo 基线
- `当前库覆盖` 章节编辑 `${APP_RUNTIME_DIR}/runtime-config.json.libraries.<library_id>.content_types` 的固定四类覆盖，删除单项覆盖表示恢复继承
- `libraries` 在 Settings 中只表示库级内容类型覆盖，不承接库创建、删除、归档或来源管理
- Settings 与库摘要都必须直接展示当前 exact `model_id`
- Settings 与库摘要都必须直接展示当前配置 `model_version`
- Settings、model-catalog 与 resolved model 摘要只应展示模型原生向量能力；`document` / `video` 这类工程增强输入不得作为模型原生能力或原生测试模态呈现
- 工程增强能力若需暴露，只允许出现在运行时健康 / 诊断或调试面中，并且必须与模型原生能力分开展示
- 设置工作区若承载运行时诊断摘要，应明确把“模型原生能力”“Execution Input Types”和“runtime adapters”分成三个字段区块，不得把 adapter 列表或执行输入列表混入 `EmbeddingCapabilities`

## 关联主题

- [spec.md](./spec.md) 定义设置工作区在全应用壳层中的位置与总体边界
- [app-shell-and-navigation.md](./app-shell-and-navigation.md) 定义设置工作区的主导航与次级入口关系
- [shared-product-language.md](./shared-product-language.md) 定义设置工作区应复用的产品级视觉与文案原则
- [current-targets.md](./current-targets.md) 记录当前阶段设置页左导航、右编辑面与章节顺序目标
- [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义 provider config、模型选择与解析语义
- [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 定义运行时健康、任务、维护与 probe 的底层语义
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义设置、模型测试、resolved model 与诊断接口的请求 / 响应契约
