# 140 库来源管理 (Library Source Management)

定义 FauniSearch 的库来源管理能力域，明确单个库如何通过来源根、来源规则、来源清单、`refresh` / `rescan` 与 watcher 管理正式来源内容，并把这些变化推进到后续可搜索状态。

## 关键术语 (Terminology)

- 库来源管理（Library Source Management）
- 来源根（Source Root）
- 来源规则（Source Rule）
- 来源清单（Source Inventory）
- 增量刷新（Refresh）
- 全量重扫（Rescan）
- 来源监听（Watcher）

## 范围

- 单个库作用域下的来源根生命周期管理
- 来源规则与覆盖范围的长期语义
- 库级来源清单与来源状态观察
- `refresh` / `rescan` / watcher 在来源管理中的长期角色
- 来源失效、脱离覆盖范围与后续可搜索性的关系

范围外：
- 具体请求 / 响应字段、过滤参数与动作回执编码
- 文件系统 watcher 的产品实现、底层库选择与平台差异
- 去重算法、索引算法、检索后端实现与模型选择
- 来源内容修复动作、来源级纠错工作流与人工确认队列
- UI 组件布局、前端框架、表单细节与视觉样式

## 设计原则

- 库级作用域（Library-Scoped Sources）：来源根与来源清单都只在单个库作用域下解释，不发生跨库来源共享
- 来源根先于来源内容（Roots Before Inventory）：来源管理先定义内容从哪里进入库，再定义库级来源清单如何被观察
- 规则显式（Explicit Coverage Rules）：来源覆盖范围必须通过结构化规则显式表达，而不是依赖隐式约定
- 增量优先（Incremental Before Full）：日常变化优先通过 `refresh` 消化，`rescan` 承担显式全量重评估
- 失效保留（Invalidate Without Hard Delete）：文件消失、不可达或脱离覆盖范围时，系统先保留结构化记录与历史，再通过后续激活把它们从新搜索结果中移出
- 事实源复用（Fact-Source Reuse）：来源管理复用上游状态模型、摄取 / 索引、接口与 UI 专题，不在本专题重写这些稳定事实

## 当前阶段承接

- 当前阶段实施计划见 [plan.md](./plan.md)
- 当前阶段测试设计见 [testing.md](./testing.md)
- 本专题是 library 下来源管理的父专题；当前阶段先落来源根闭环与库级来源清单，只读不做 source repair
- 当前阶段只支持本地目录来源根

## 能力边界

- 库来源管理始终作用于单个目标库
- 单个库可以拥有多个来源根；单个来源根只能属于一个库
- 来源根长期上至少承接：
  - 根定位
  - `enabled` 状态
  - 健康 / 降级状态
  - watcher 运行状态
  - 覆盖范围摘要
  - 结构化规则
- 来源规则长期上至少支持：
  - 包含规则（include）
  - 排除规则（exclude）
  - 来源类型或等价扩展名过滤
- 库级来源清单是正式能力边界；当前阶段只承接聚合列表与状态观察，不承接 source repair
- watcher 长期上属于来源管理能力的一部分，而不是额外独立专题

## 来源根、规则与来源清单状态

- 来源根是库级扫描入口与覆盖边界对象；停用来源根后，它不再参与 watcher、`refresh` 或 `rescan`
- 来源规则只参与候选集合与覆盖范围收敛，不决定源内容身份
- 库级来源清单是对当前库结构化 `Source` 集合的聚合观察面，而不是独立身份层
- 库级来源清单可以附带代表性视觉对象摘要与预览引用，用于支持来源浏览工作区中的预览优先详情；这类附加摘要仍从属于聚合来源观察面，不构成独立 source detail 工作流
- 来源内容可以因文件消失、不可达、权限变化、来源根停用或规则变化而进入失效 / 脱离覆盖状态
- 失效或脱离覆盖的来源内容仍保留结构化记录、历史与身份，不应立即硬删除
- 当新的有效索引激活后，失效或脱离覆盖的来源内容必须退出新的搜索结果

## 功能需求

### 来源根与规则

- `FR-1` 系统必须允许用户在单个目标库中创建、查看、编辑、启用、停用与删除来源根。
- `FR-2` 每个来源根必须具备结构化规则输入，长期上至少支持 include / exclude 与来源类型或等价扩展名过滤。
- `FR-3` 规则变化必须影响后续覆盖范围评估，但不得改写既有源内容身份。

### refresh / rescan / watcher

- `FR-4` 系统必须同时支持库级与来源根级 `refresh` / `rescan`。
- `FR-5` `refresh` 必须表达增量重评估语义；`rescan` 必须表达全量重评估语义。
- `FR-6` 对已启用的本地目录来源根，watcher 变化必须能够在 debounce 后进入增量 `refresh` 路径，而不是直接触发全量 `rescan`。
- `FR-7` 当来源根或来源内容不可达、降级或失效时，系统必须向用户表达明确状态，而不是把这些变化静默吞掉。

### 来源清单与可搜索性

- `FR-8` 系统必须提供库级聚合来源清单，允许用户观察当前库中的来源内容。
- `FR-9` 来源清单至少必须支持按来源根、来源类型与来源状态进行过滤或等价筛选。
- `FR-10` 当前阶段来源清单只承接只读观察语义，不承接 source repair 或人工纠错动作。
- `FR-11` 当来源内容因文件消失或脱离覆盖范围而失效时，系统必须保留其结构化记录，并在后续有效激活后让其退出新的搜索结果。
- `FR-12` 当前阶段来源清单可以返回代表性视觉对象摘要与预览引用，以支撑来源浏览工作区的预览优先详情；缺少可预览对象时系统必须明确返回空摘要，而不是伪造预览。

## 公开能力与复用边界

- 本专题复用以下既有基础能力，而不重新定义其事实源：
  - [002-state-and-data-model](../002-state-and-data-model/spec.md) 中的 `Library Source Root`、`Library Source Root Rule` 与 `Source` 状态边界
  - [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 中的覆盖范围、`refresh` / `rescan`、失效与激活语义
  - [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 中的任务、后台执行与 watcher 触发后的任务承接
  - [008-ui-ux](../008-ui-ux/spec.md) 中的来源根与规则工作区、库级来源清单视图与管理入口
  - [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 中的来源管理公开接口、动作回执与过滤编码
- 本专题不新增 sidecar 私有协议，也不定义搜索结果、对象详情或检索 payload 编码

## 验收标准

- `AC-1` 用户能够在单个库中创建并保存来源根与结构化规则。
- `AC-2` 用户能够对单个来源根与整个库分别触发 `refresh` 与 `rescan`，并看到动作进入后台执行系统。
- `AC-3` watcher 检测到已启用来源根下的文件变化后，能够在 debounce 后推动一次增量 `refresh`。
- `AC-4` 用户能够在库级来源清单中看到聚合后的来源内容，并按来源根、来源类型与状态进行筛选。
- `AC-5` 当文件消失、不可达或脱离覆盖范围后，相关来源内容退出新的搜索结果，但结构化记录与历史仍保留。
- `AC-6` 当前阶段即使不提供 source repair，用户仍能通过来源根管理与来源清单完成最小来源管理闭环。

## 关联主题

- [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义来源根、来源规则与来源内容的状态模型
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 定义覆盖范围、`refresh` / `rescan` 与失效 / 激活语义
- [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 定义后台任务、watcher 事件承接与运行时观察语义
- [008-ui-ux](../008-ui-ux/spec.md) 定义来源管理工作区、库级来源清单视图与应用级入口
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义来源管理接口、动作回执与过滤编码
