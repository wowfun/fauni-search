# 007 存储与持久化 (Storage and Persistence)

定义 FauniSearch 的物理持久化边界，明确逻辑状态如何落到主结构化存储、检索后端命名空间、派生资产存储区、临时资产存储区与应用数据根，并约束迁移版本与可恢复边界。

## 关键术语 (Terminology)

- 应用数据根（Application Data Root）
- 主结构化存储（Structured Store）
- 检索命名空间（Retrieval Namespace）
- 派生资产存储区（Derived Asset Store）
- 临时资产存储区（Temporary Asset Store）
- 迁移版本（Migration Version）
- 持久队列记录（Durable Queue Record）

## 范围

- 物理持久化分层与各类状态的唯一物理落点
- 主结构化存储中的持久记录族边界
- 检索命名空间、派生资产存储区、临时资产存储区与运行时工作区的分层语义
- 结构化记录、检索载荷与文件载荷之间的稳定引用边界
- 主结构化存储、应用数据根与检索命名空间的迁移版本与兼容语义

范围外：
- 搜索 API 形状、请求响应字段与分页编码
- 提供方能力、提供方绑定解析与运行时探测规则
- 摄取、索引、搜索或执行流程本身
- 前端管理页面、控制面 API 与具体操作界面
- ORM 选型、具体 SQL 方言优化与后端产品私有协议细节

应用体验承接：
- 前端管理页面与工作区体验由 [008-ui-ux](../008-ui-ux/spec.md) 定义；控制面 API 的请求 / 响应契约由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义

## 设计原则

- 唯一物理归宿（One Physical Home per Truth）：每类逻辑真相只能有一个稳定物理落点；其他位置只能是引用、缓存、索引或可重建副本
- 结构化 / 检索 / 文件分层（Structured / Retrieval / File Separation）：结构化业务真相、检索载荷与文件载荷必须分层持久化，避免边界漂移
- 身份稳定先于迁移（Identity Stability Before Migration）：迁移与布局升级不得改写 `library_id`、`source_id`、`content_version_id` 等稳定身份
- 可重建载荷与不可替代真相区分（Rebuildable Payload vs Durable Truth）：可重建的派生载荷与检索载荷可以被回收或重建；结构化真相与持久队列记录不得被静默丢弃
- 逻辑隔离优先于物理复制（Logical Isolation Before Duplication）：库之间默认通过 `library_id` 等稳定标识隔离，而不是通过复制整套基础设施存储实现隔离

## 物理持久化分层

- 结构化真相默认进入单一主结构化存储；当前默认实现为单一主 SQLite，路径固定为 `${APP_RUNTIME_DIR}/state.sqlite`
- provider / model settings 的稳定事实源不再进入主结构化存储；当前固定采用双层 JSON 配置：
  - repo 基线：`fauni.config.json`
  - runtime 覆盖：`${APP_RUNTIME_DIR}/runtime-config.json`
- 检索载荷进入检索后端命名空间；检索后端只承载索引、向量与最小检索载荷，不承载结构化业务真相
- 派生资产载荷进入应用数据根下的派生资产存储区，并通过结构化记录引用
- 临时查询资产进入应用数据根下的临时资产存储区，按短生命周期管理，不作为长期迁移保护对象
- Settings 模型测试所需的临时文件也进入应用数据根下的独立短生命周期临时工作区；它们不进入 durable truth
- 运行时 scratch、导出中间文件与诊断文件进入应用数据根下的独立运行时工作区；它们不是稳定事实源
- 原始文件默认按外部引用管理，不复制入应用数据根；应用内稳定承载的是其结构化引用、覆盖边界与派生状态
- 运行时驻留状态可以跨请求存在于进程内，但不进入长期持久化真相层

## 主结构化存储与持久记录族

- 主结构化存储是结构化业务真相与持久队列记录的唯一长期事实源
- 默认采用单一主结构化存储文件承载全应用结构化真相，库级隔离通过 `library_id` 等稳定标识实现
- 当前 v1 主结构化存储承载最小 restart-durable 记录族：
  - `libraries`
  - `library_configs`
  - `library_source_roots`
  - `library_source_root_rules`
  - `sources`
  - `visual_units`
  - `active_index_references`
  - `retired_vector_space_inventory`
- 当前结构化存储使用 SQLite 多表记录承载 durable truth，schema version 由 `state_meta` 单行记录
- 当前写入策略仍采用单事务整表重写结构化表，而不是 row-by-row live sync；这只是写入实现策略，不再把全部 durable truth 放入单个 JSON snapshot
- 旧的 `durable_state_snapshots.payload_json` 单行 snapshot store 不自动迁移；启动时遇到旧 store 必须拒绝启动并提示 operator 通过 reset / cutover 显式处理
- 复杂叶子字段可以用 JSON 文本列保存，例如 source-root rules、visual-unit locator 与 neighbor context；高基数实体与实体顺序必须行化
- 为承接 [002-state-and-data-model](../002-state-and-data-model/spec.md) 中的稳定关系，可以存在必要的关联记录族；但这些记录不得改变上游定义的事实源归属
- `jobs`、`job_attempts`、`search_history` 与 `favorites` 不属于当前 v1 的 restart-durable subset；它们在重启后清空或缺失，不构成持久恢复失败
- 主结构化存储中的记录可以引用文件载荷与检索命名空间，但文件载荷与检索后端不得反向承担结构化真相职责

## 检索命名空间与文件载荷

- 检索命名空间是检索后端内承载某次索引构建结果的稳定命名与映射边界
- 对同一库中的某个 `vector_space`，至少应区分 active 与 staging 两类命名空间角色
- 检索命名空间的稳定映射边界至少包含：`library_id`、`vector_space_id`、`content_version` 与命名空间角色
- 当前 v1 stable active logical namespace naming 固定为 `vector_space_{library_id}_{vector_space_id}`
- 当前实现中，`vector_space_{library_id}_{vector_space_id}` 由 active alias 或等价逻辑命名承接，而不是要求对应单一固定物理 collection；staging 命名空间在验证通过前不承担公开检索真相
- 新建的物理检索 collection 必须使用 disk-backed vector 配置；当前 v1 要求其 named vectors 使用 `on_disk: true`
- 失败或中断留下的 staging 命名空间，以及因替换而退役的旧 active 命名空间，在延迟清理窗口内保留，用于诊断、恢复与安全回收
- 当内容类型配置改绑到新的执行签名时，主结构化存储中的 active vector space reference 必须先移除旧 `vector_space`，随后旧命名空间才作为 retired 命名空间进入延迟清理窗口
- retired `vector_space` inventory 属于主结构化存储中的 durable truth；在后台清理实际成功前，不得只因为进程重启或一次清理失败就丢失这些待清理记录
- 检索命名空间的具体产品实现可以是 collection、alias 或等价机制，但这些后端私有机制不作为本专题中的稳定事实
- 旧 runtime-token collection、更早的 `text_search_*` collection，以及把旧 `index_*` 或新 `vector_space_*` stable logical name 直接占成物理 collection 的实现，都与当前 stable naming 不兼容；本专题只要求 operator/manual cleanup，不要求应用启动或 `run.sh` 自动迁移或自动清理
- 派生资产存储区承载预览图、视频分段、关键帧拼图等可重建载荷；其长期身份由结构化存储中的 `derived_assets` 记录与载荷引用承接
- 临时资产存储区承载图片 / 视频查询的上传输入、裁剪结果与短期中间载荷；这些载荷过期后可以直接删除，不承诺迁移保留
- 运行时工作区承载 scratch 文件、临时导出和诊断中间文件；即使被清空，也不得破坏结构化真相与激活索引引用
- Settings 模型测试的临时输入文件属于运行时工作区或等价临时工作区的一部分；调用结束后可以立即删除，不要求跨请求或跨重启保留

## 逻辑状态到物理落点的映射

| 逻辑状态族 | 主要物理落点 | 说明 |
| --- | --- | --- |
| 库、库配置、库来源根、库来源根规则 | 主结构化存储 | 作为全应用共享的结构化真相长期保留 |
| 源内容、视觉单元、内容版本 | 主结构化存储 | 承载稳定身份、归属与处理边界 |
| 派生资产记录 | 主结构化存储 | 记录资产类型、规格、引用关系与载荷引用 |
| 派生资产载荷 | 派生资产存储区 | 可重建文件载荷，不替代结构化真相 |
| 激活索引引用 | 主结构化存储 | 指向当前 active / staging 检索命名空间 |
| 索引与向量载荷 | 检索命名空间 | 仅承载检索事实，不承载业务元数据真相 |
| provider configs | 配置文件事实源 | repo 基线 `fauni.config.json` 与 runtime 覆盖 `${APP_RUNTIME_DIR}/runtime-config.json` 深合并后的结果 |
| 全局 `content_types` 与库级内容类型覆盖 | 配置文件事实源 | 当前仍由 settings 接口公开，但其 durable truth 不再进入 `state.sqlite` |
| 任务状态、任务尝试、检查点引用 | 运行时进程自身 | 当前 v1 只作为进程内执行状态；重启后清空 |
| 搜索历史记录、收藏记录 | 非当前 v1 durable subset | 当前切片不要求跨 restart 恢复 |
| 临时查询资产 | 临时资产存储区 | 纯临时输入，不构成长期共享事实源 |
| 运行时驻留状态 | 运行时进程自身 | 可观察但不长期持久化 |

## 迁移版本与兼容语义

- 迁移版本至少分为三类：
  - 主结构化存储的 schema version
  - 应用数据根的 layout version
  - 检索命名空间的兼容代际或命名代际
- 主结构化存储的 schema version 必须持久记录，并作为结构化迁移的唯一基准
- 当前结构化 SQLite store 的 schema version 为 `3`；低于该版本且仍使用单行 snapshot 的 store 不自动迁移
- 应用数据根的 layout version 必须能表达派生资产存储区、临时资产存储区与运行时工作区的布局兼容性
- 当检索命名空间的物理命名或后端兼容要求发生变化时，应通过显式兼容代际或重建路径处理，而不是直接改写结构化真相含义
- 当前 active index reference 在应用启动时必须重新对照 stable active namespace naming 探测可用性；若 active alias 缺失、alias target 缺失，或只剩同名旧物理 collection，该引用应失活并让搜索返回 `not_ready`
- provider configs、全局 `content_types` 与库级内容类型覆盖不再以 `state.sqlite` 为事实源；旧 `state.sqlite` 中即使残留这些字段，也不得覆盖 merged config 的解析结果
- Settings 写入 provider configs、provider models、全局 `content_types` 与库级内容类型覆盖时，只写 `${APP_RUNTIME_DIR}/runtime-config.json`
- 删除 runtime config 中的 provider/model/content-type 覆盖只表示回落到 repo 基线或上层继承；当前不通过 tombstone 遮蔽 repo 基线
- `${APP_RUNTIME_DIR}/runtime-config.json.libraries` 在 Settings 中只承接库级配置覆盖，不承接库生命周期、来源根或来源清单的事实源
- 升级过程中，`library_id`、`source_id`、`visual_unit_id`、`content_version_id` 等稳定身份不得被重写
- 派生资产载荷、临时资产与检索命名空间若与新版本不兼容，可以按规则重建；主结构化存储中的稳定记录与持久队列记录不得依赖“删掉重建”作为默认升级手段
- 若派生资产载荷或检索命名空间被判定为需要重建，主结构化存储中的引用、检查点与激活关系应继续作为恢复入口，而不是被隐式清空

## 关联主题

- [000-foundation](../000-foundation/spec.md) 定义本地优先、单一事实源与默认技术栈基线
- [001-architecture](../001-architecture/spec.md) 定义结构化存储、检索后端与 Rust 主服务之间的系统边界
- [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义逻辑实体、状态族、事实源归属与可恢复边界
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 定义来源边界、内容版本、active / staging 语义与延迟清理窗口
- [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义 provider config、模型选择与运行时探测语义
- [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 定义任务恢复、检查点推进、运行时工作区清理与后台维护执行语义
- [008-ui-ux](../008-ui-ux/spec.md) 定义依赖这些持久记录与工作区数据的应用壳层、管理体验与控制面接口族
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义搜索与控制面公开接口的请求 / 响应契约，以及任务 / 健康快照的公开编码
