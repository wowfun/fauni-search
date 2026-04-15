# 140 库来源管理当前阶段计划

本计划承接 [spec.md](./spec.md) 的长期能力定义，收敛库来源管理当前阶段的首个可演示闭环。相关公开接口契约统一复用 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md)，来源根与来源内容的状态模型统一复用 [002-state-and-data-model](../002-state-and-data-model/spec.md)，应用级工作区与控制面入口统一复用 [../008-ui-ux/spec.md](../008-ui-ux/spec.md)。详细测试设计见 [testing.md](./testing.md)。

## 概要

- 当前阶段交付形态固定为 `API + 最小 UI` 的可演示闭环，而不是纯后端原型
- 当前阶段专题名虽然是 `library-source-management`，但首个实现切片只落：
  - 来源根生命周期管理
  - 结构化规则
  - 库级与来源根级 `refresh` / `rescan`
  - watcher 驱动的增量 `refresh`
  - 库级来源清单只读视图
- 当前阶段来源根固定为本地目录根，不做 URL、对象存储或远端连接器
- 当前阶段来源清单只读，不做 source repair 或人工纠错动作

## 当前阶段工作流

### 1. 库级来源根管理

- 当前阶段用户必须先创建库或选择已有库，再进入来源管理工作区
- 当前阶段用户能够在单个库内查看、创建、编辑、启用、停用与删除来源根
- 当前阶段来源根输入至少包括：
  - `root_path`
  - `enabled`
  - 结构化 `rules`
- 当前阶段每个来源根都必须显式展示：
  - `status`
  - `watch_state`
  - `coverage_summary`
  - 最近一次 `refresh` / `rescan` 摘要

### 2. 结构化规则

- 当前阶段规则编辑固定为结构化表单，而不是 DSL
- 当前阶段规则字段固定为：
  - `include_globs`
  - `exclude_globs`
  - `include_extensions`
- 当前阶段 glob 语义固定为相对 `root_path` 的路径 glob
- 当前阶段空 `include_globs` 表示“该来源根下所有候选内容都可进入规则评估”
- 当前阶段 `exclude_globs` 优先于 `include_globs`
- 当前阶段 `include_extensions` 是 allowlist；留空表示当前正式来源类型全部允许

### 3. refresh / rescan / watcher

- 当前阶段必须同时存在：
  - 库级 `refresh`
  - 库级 `rescan`
  - 来源根级 `refresh`
  - 来源根级 `rescan`
- 当前阶段 `refresh` 表示增量重评估：
  - 优先基于 watcher 事件、已知路径变化或已有覆盖快照
  - 只推进受影响候选集合
- 当前阶段 `rescan` 表示全量重评估：
  - 重新枚举来源根或整个库的来源覆盖范围
  - 再统一应用规则
- 当前阶段 watcher 固定默认开启，但只作用于 `enabled=true` 的本地目录来源根
- watcher 变化事件必须经过 debounce 后排队进入增量 `refresh`，不直接触发 `rescan`

### 4. 库级来源清单与失效语义

- 当前阶段来源清单固定为库级聚合列表，不按单个来源根单独开页面
- 当前阶段来源清单至少支持按以下条件筛选：
  - `source_root_id`
  - 来源类型
  - 来源状态
- 当前阶段来源清单只读，主要承接：
  - 当前来源路径 / 归属摘要
  - 来源类型
  - 来源状态
  - 所属来源根摘要
- 当前阶段当文件消失、不可达、来源根停用或规则变化导致来源内容脱离覆盖范围时：
  - 相关 `Source` 先标记为失效 / 脱离覆盖
  - 结构化记录与历史保留
  - 在新一轮有效索引激活后退出新的搜索结果
- 当前阶段不提供 source 级 repair、重新绑定或人工确认入口

## 当前阶段约束

- 当前阶段只支持本地目录来源根
- 当前阶段只支持结构化 include / exclude / extension 规则，不实现 DSL
- 当前阶段 watcher 不提供单独配置界面，也不支持按来源根关闭 debounce 策略
- 当前阶段来源清单只读，不做 source detail / repair / override
- 当前阶段不实现远端连接器、对象存储、URL 来源根或混合协议来源
- 当前阶段不实现跨库来源视图；来源清单始终以单库聚合视图进入

## Deferred

- source detail 单页与 source repair 动作
- 文件级人工确认、忽略队列与冲突修复流
- 远端连接器、URL 来源根、对象存储与网络盘来源
- watcher 的更细粒度配置与平台特定调优
- 库间来源汇总视图
- 更复杂的规则 DSL、优先级调试与规则模拟器

## 阶段验收摘要

- 用户能够在单个库中创建、编辑、启用、停用与删除来源根
- 用户能够通过结构化规则表达 include / exclude / extension 过滤
- 用户能够分别触发库级与来源根级 `refresh` / `rescan`
- watcher 检测到已启用来源根下的变化后，能够在 debounce 后推动一次增量 `refresh`
- 用户能够在库级来源清单中看到聚合的来源内容，并按来源根 / 来源类型 / 状态筛选
- 当文件消失或脱离覆盖范围时，相关来源内容退出新的搜索结果，但结构化记录与历史仍保留

详细测试分层、场景矩阵与本地 smoke 设计见 [testing.md](./testing.md)。
