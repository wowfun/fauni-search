# 140 库来源管理测试设计

本文件是 `140-library-source-management` 专题的正式配套文档，承接与库来源管理能力直接相关的测试设计、覆盖维度与当前阶段验证方案。长期能力定义见 [spec.md](./spec.md)，当前阶段实施范围见 [plan.md](./plan.md)。仓库级通用测试规则继续以 [AGENTS.md](../../AGENTS.md) 为准，本文件不重复改写这些通用约束。

## 角色与边界

- 本文件只承接 `140-library-source-management` 专题内的测试设计，不替代仓库级测试规则
- 本文件同时覆盖：
  - 长期能力层的测试原则与覆盖维度
  - 当前阶段 `API + 最小 UI` 闭环的详细测试设计
- 本文件不重新定义 `002`、`003`、`008`、`009` 等上游事实源

## 长期测试原则

- 来源管理测试长期上必须覆盖来源根、规则、执行动作、来源清单与失效语义，而不是只验证理想导入路径
- 成功路径、watcher 驱动路径、规则错误路径、失效路径与只读来源清单观察都必须进入覆盖
- 专题测试优先采用最贴近改动面的 Rust / API 窄测试，再用 UI 与本地 smoke 证明闭环
- 当前阶段来源清单是只读视图，因此测试重点应落在“是否正确观察到来源状态变化”，而不是 source repair

## 长期覆盖维度

### 来源根与规则

- 来源根 CRUD
- `enabled` / `disabled` 状态切换
- include / exclude / extension 规则匹配
- 规则变化后的覆盖范围重评估

### refresh / rescan / watcher

- 来源根级 `refresh`
- 来源根级 `rescan`
- 库级 `refresh`
- 库级 `rescan`
- watcher 变化经 debounce 后排队增量 `refresh`
- disabled 来源根不参与 watcher 与执行动作

### 来源清单与失效语义

- 库级聚合来源清单能够返回来源内容摘要
- 来源清单能够按来源根、来源类型与来源状态过滤
- 文件消失、不可达或脱离覆盖范围后，来源内容进入失效 / 脱离覆盖状态
- 失效来源内容在新一轮有效激活后退出新的搜索结果
- 结构化记录与历史在失效后仍保留

## 当前阶段默认测试入口

- Rust 主服务默认测试入口：`cargo test`
- 最小 UI 默认端到端入口：`Playwright`
- 无 GPU 快速检查入口：`bash scripts/local/check.sh`
- 本地 smoke 计划验证入口：`bash scripts/local/smoke-source-management.sh`

## 当前阶段测试分层

### Rust / API

- 使用 `cargo test` 覆盖当前阶段最贴近业务改动的逻辑
- 优先覆盖：
  - 来源根创建、更新、启用 / 停用、删除
  - 结构化规则匹配
  - 来源根级与库级 `refresh` / `rescan`
  - watcher 事件合并为增量 `refresh`
  - disabled 来源根不参与 watcher
  - 文件消失 / 脱离覆盖范围后的失效语义
  - 库级聚合来源清单与过滤

### 最小 UI

- 使用 `Playwright` 覆盖来源管理工作区中的最小闭环
- 当前阶段 Playwright 至少覆盖以下路径：
  - 创建来源根
  - 编辑规则
  - 启用 / 停用来源根
  - 触发库级与来源根级 `refresh` / `rescan`
  - 查看并筛选来源清单
  - 观察 watcher 驱动的新增 / 修改 / 删除后状态更新

### 本地 Smoke

- 使用本地临时目录与现有 committed fixture 做来源管理闭环验证
- 当前阶段 `smoke-source-management.sh` 需要真实验证：
  - 注册来源根
  - 初次 `refresh`
  - 来源内容进入来源清单与后续搜索链路
  - watcher 驱动的增量 `refresh`
  - 文件删除或规则变化导致的失效语义
  - 来源内容退出新的搜索结果

## 当前阶段场景矩阵

| 场景 | 优先验证层 | 说明 |
| --- | --- | --- |
| 来源根创建成功 | Rust / API + UI E2E | 验证最小来源根生命周期入口 |
| 来源根规则生效 | Rust / API + local smoke | 验证 include / exclude / extension |
| 来源根启用 / 停用 | Rust / API + UI E2E | 验证状态切换与 watcher 边界 |
| 来源根级 `refresh` | Rust / API + UI E2E | 验证单根增量重评估 |
| 来源根级 `rescan` | Rust / API + UI E2E | 验证单根全量重评估 |
| 库级 `refresh` | Rust / API + UI E2E | 验证多根聚合增量路径 |
| 库级 `rescan` | Rust / API + UI E2E | 验证多根聚合全量路径 |
| watcher 驱动刷新 | Rust / API + local smoke | 验证 debounce 后进入增量 `refresh` |
| 来源清单过滤 | Rust / API + UI E2E | 验证库级聚合只读视图 |
| 文件消失后标记失效 | Rust / API + local smoke | 验证结构化记录保留 |
| 失效内容退出新的搜索结果 | local smoke | 验证来源管理与搜索链的衔接 |

## Fixture 与验证材料

- 当前阶段优先复用现有 committed fixture，并在测试时复制到临时目录形成来源根样本
- watcher 相关测试优先使用测试过程内生成或复制的临时目录，不依赖长期存在的本地手工目录
- 本地 smoke 应避免修改 committed fixture 本身，只在临时目录内模拟新增 / 修改 / 删除

## Deferred Coverage

- source detail 单页覆盖
- source repair / 重新绑定动作覆盖
- 远端连接器与 URL 来源根覆盖
- 文件级人工确认、忽略队列与冲突修复覆盖
- watcher 高级配置与平台特定调优覆盖

## 关联文档

- [spec.md](./spec.md)
- [plan.md](./plan.md)
- [../002-state-and-data-model/spec.md](../002-state-and-data-model/spec.md)
- [../003-ingestion-and-indexing/spec.md](../003-ingestion-and-indexing/spec.md)
- [../008-ui-ux/spec.md](../008-ui-ux/spec.md)
- [AGENTS.md](../../AGENTS.md)
