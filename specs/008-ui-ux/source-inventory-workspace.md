# 008 来源清单工作区 (Source Inventory Workspace)

定义 FauniSearch 来源清单工作区的共享界面与体验约束，明确库级来源观察如何作为正式管理工作区存在，并与搜索工作区形成清晰分工。

## 关键术语 (Terminology)

- 来源清单工作区（Source Inventory Workspace）
- 来源摘要条（Inventory Summary Bar）
- 来源过滤区（Inventory Filter Dock）
- 来源行（Inventory Source Row）
- 工作区切换器（Workspace Switcher）

## 范围

- 来源清单工作区内部的共享布局与交互骨架
- 来源摘要、过滤与库级来源列表的共享观察语义
- 来源清单工作区与搜索工作区之间的切换关系

范围外：
- 来源根生命周期、`refresh` / `rescan` / watcher 的底层语义
- `/libraries/:id/sources` 等接口的请求 / 响应编码
- 来源修复、人工纠错或 source-level detail 的独立工作流
- 像素级布局、视觉样式与具体组件实现

## 设计原则

- 管理面独立（Management Surface Is Separate）：来源清单属于管理 / 观察面，不应继续占据搜索入口上方的主流位置
- 当前库显式（Visible Library Scope）：来源清单工作区必须始终保持当前库上下文可见
- 摘要先于列表（Summary Before Rows）：用户进入来源清单工作区后，应先看到总量与状态摘要，再进入过滤与逐条核对
- 过滤可持续（Persistent Filtering）：来源根、来源类型与来源状态过滤应在工作区内保持稳定，不因切换搜索模式而被重写
- 搜索草稿不受扰（Search Drafts Stay Intact）：用户在搜索工作区中的查询草稿、结果与详情选中态，不得因为切换到来源清单工作区而丢失

## 工作区骨架

- 来源清单工作区与搜索工作区共享同一应用壳层、当前库上下文与全局反馈区域
- 来源清单工作区至少应同时呈现：
  - 当前库上下文与来源根管理入口
  - 库级来源摘要条
  - 可持续的来源过滤区
  - 库级来源列表
- 当前阶段来源清单工作区不要求提供来源详情侧栏；主要任务是观察、筛选与核对来源状态

## 摘要、过滤与列表

- 来源摘要条至少应覆盖：总来源数、`active`、`invalidated` 与 `out_of_scope` 的状态计数或等价摘要
- 来源过滤区至少应支持：按来源根、来源类型与来源状态进行过滤
- 来源过滤区可以采用粘性区块或等价承载方式，但不应遮挡列表内容
- 来源列表应优先呈现紧凑行式信息，至少覆盖：
  - `source_path`
  - `source_root_label`
  - `source_type`
  - `kind`
  - `status`
  - `visual_unit_count`
  - `status_reason`（若存在）
- 当当前筛选条件下无来源时，工作区必须显示明确空状态，而不是退化为无反馈空白

## 与搜索工作区的关系

- 工作区切换器必须允许用户在 `Search` 与 `Inventory` 之间显式切换，而无需离开当前应用壳层
- 搜索工作区可以保留来源管理摘要与进入来源清单工作区的入口，但不应继续内嵌完整来源清单列表
- 从搜索工作区切换到来源清单工作区，再切回搜索工作区时，查询草稿、结果列表与详情选中态必须保持

## 关联主题

- [spec.md](./spec.md) 定义来源清单工作区在全应用壳层中的位置与管理入口边界
- [search-workspace.md](./search-workspace.md) 定义搜索工作区与来源清单工作区的分工与切换关系
- [140-library-source-management](../140-library-source-management/spec.md) 定义来源根、来源清单与 `refresh` / `rescan` 的能力边界
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义来源清单接口与过滤编码
