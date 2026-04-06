# 008 应用界面与体验 (UI/UX)

定义 FauniSearch 的全应用界面与体验语义，明确应用壳层、全局导航、工作区切换、非搜索管理流，以及支撑这些体验的非搜索控制面接口族如何构成统一事实源。

## 关键术语 (Terminology)

- 应用壳层（App Shell）
- 工作区（Workspace）
- 全局导航（Global Navigation）
- 库管理流（Library Management Flow）
- 来源根管理流（Source Root Management Flow）
- 设置界面（Settings Surface）
- 任务中心（Task Center）
- 运行时健康界面（Runtime Health Surface）
- 收藏管理（Favorite Management）
- 搜索历史管理（Search History Management）

## 范围

- 全应用信息架构、壳层布局与工作区语义
- 非搜索管理体验与对应的非搜索控制面接口族
- 库、来源根、配置、任务、健康、收藏与搜索历史的应用入口与操作流
- 搜索工作区在应用中的位置、导航关系与壳层承载方式

范围外：
- 搜索请求、搜索结果、过滤分页与搜索结果交互语义
- 非搜索控制面接口的请求 / 响应形状、错误载荷与分页编码
- 任务状态机、取消/恢复内部语义与健康判定算法
- 物理 schema、目录布局、迁移版本与存储实现
- 视觉样式、设计 token、像素级布局与组件实现细节
- 前端框架、状态管理库与具体路由实现

接口契约承接：
- 非搜索控制面接口族的具体请求 / 响应形状、动作回执、任务快照与健康快照编码由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义

## 设计原则

- 应用优先于页面（Application Before Screens）：先定义应用级工作区与导航结构，再定义单页或单表单体验
- 搜索与管理分治（Search and Management Separation）：搜索工作台属于应用体验的一部分，但搜索行为规则继续由搜索专题承接
- 入口统一（Unified Entry Points）：所有非搜索管理动作都应通过统一的应用入口和控制面接口进入系统，而不是散落为隐式旁路
- 观察语义复用（Reuse Runtime Semantics）：任务、取消、恢复与健康的底层语义复用运行时专题；本专题只定义它们如何进入应用体验
- 最小管理闭环（Minimum Management Loop）：凡是进入应用长期状态的库、来源根、收藏或搜索历史，都应至少具备最小查看与管理入口

## 应用壳层与工作区

- 应用壳层是全局导航、当前库上下文、工作区切换与全局反馈的统一承载层
- 应用至少应存在以下正式工作区：
  - 搜索工作区
  - 库管理工作区
  - 来源根与规则工作区
  - 设置工作区
  - 任务中心
  - 运行时健康与诊断工作区
  - 收藏与搜索历史工作区
- 搜索工作区可以是默认主工作区，但其搜索语义、结果语义、分页规则与调试输出继续由 [004-search](../004-search/spec.md) 定义；具体请求 / 响应契约由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义
- 工作区切换必须保持当前库上下文可见，并允许用户明确知道当前操作作用于哪个库
- 应用壳层可以承载全局反馈，例如后台任务状态、运行时降级提醒与维护动作结果，但这些反馈不应改写对应专题中的底层语义

## 非搜索管理体验

- 库管理流至少应覆盖：创建、删除、重命名、归档或等价生命周期操作
- 来源根管理流至少应覆盖：查看、创建、编辑、启用、停用、删除，以及规则的最小管理能力
- 设置界面至少应覆盖：库配置、启用索引线、提供方绑定、刷新策略与相关默认值的管理流
- 应用必须提供导入、刷新、重扫、重建、清理与维护动作的明确用户入口，并能向用户表达动作已进入后台执行系统
- 任务中心至少应支持：查看任务列表、查看阶段进度、取消、重试与恢复入口
- 运行时健康界面至少应支持：查看本地运行时与远端提供方的健康摘要、诊断摘要与必要维护入口
- 收藏管理与搜索历史管理至少应覆盖：查看与最小清理 / 删除能力
- `favorites` 与 `search_history` 虽属于辅助状态，但一旦进入正式应用状态，就不应只存在于持久层而缺少管理入口

## 非搜索控制面接口族

- 本专题中的控制面接口族是支撑应用体验的稳定公开入口，不是对页面行为的内部实现细节描述
- `008` 至少固定以下非搜索接口族的存在与职责：
  - 库管理接口族
  - 来源根与规则管理接口族
  - 配置与绑定管理接口族
  - 任务动作接口族
  - 收藏 / 搜索历史管理接口族
  - 运行时维护入口
- 这些接口族应覆盖应用壳层与各工作区中的正式管理动作，但不承接搜索端点
- 搜索端点的行为语义继续由 [004-search](../004-search/spec.md) 承接，不并入本专题
- 任务与运行时健康的公开入口可以出现在本专题中，但其状态、取消、恢复与健康语义本身继续复用 [006-runtime-and-execution](../006-runtime-and-execution/spec.md)
- 控制面接口族可以是 HTTP、IPC 或等价公开管理边界；其具体请求 / 响应形状、错误载荷与分页编码由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义

## 关联主题

- [001-architecture](../001-architecture/spec.md) 定义应用壳层所依赖的系统边界、编排中心与组件交互路径
- [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义库、配置、来源根、任务、收藏与搜索历史的状态模型与事实源归属
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 定义刷新、重扫、重建与启用索引线切换所依赖的摄取 / 索引语义
- [004-search](../004-search/spec.md) 定义搜索工作区中的搜索语义、结果语义、过滤分页规则与搜索交互
- [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义提供方绑定、能力与解析语义
- [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 定义任务、取消、恢复、健康摘要与维护执行的底层语义
- [007-storage-and-persistence](../007-storage-and-persistence/spec.md) 定义收藏、搜索历史、任务记录与应用工作区所依赖的物理持久化边界
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义非搜索控制面接口族的请求 / 响应契约、动作回执与公开快照编码
