# 003 摄取与索引 (Ingestion and Indexing)

定义 FauniSearch 的摄取与索引语义，明确库内容如何通过正式接入边界进入系统，被收敛为 `Content`、`Source`、`Asset` 与 `Unit`，并通过 `UnitIndex` 与 `ContentE2eIndexState` 成为 active 可搜索事实。

## 关键术语 (Terminology)

- 来源类型（Source Kind）
- 摄取模式（Ingestion Mode）
- 库覆盖范围（Library Coverage）
- 降级来源根（Degraded Source Root）
- 摄取运行（Ingestion Run）
- 索引运行（Indexing Run）
- 向量空间（VectorSpace）
- 检查点（Checkpoint）
- 检索命名空间（Retrieval Namespace）
- 单元索引（UnitIndex）
- 内容端到端索引状态（ContentE2eIndexState）
- 资产（Asset）
- 单元（Unit）

## 范围

- 正式来源类型与正式接入边界
- 库来源、覆盖范围、重扫与失效的高层语义
- 摄取与索引的稳定阶段模型
- `Source Content -> Asset -> Unit` 的处理边界
- VectorSpace、UnitIndex、ContentE2eIndexState 与检索后端命名空间的生命周期边界
- 检查点、恢复、验证、切换与延迟清理窗口语义

范围外：

- 任务队列、worker 并发、重试策略与调度实现
- 提供方的能力、配置档与配置模型
- 检索后端的具体产品选择与底层机制
- 来源根规则 DSL、优先级表与存储 schema
- 对外搜索接口与请求响应形状
- 原始文件、Unit 物化载荷与临时资产存储区的物理布局

## 设计原则

- 时间顺序显式（Ingestion Before Indexing）：正式流程先定义内容如何进入库，再定义其如何变为可检索事实
- 覆盖范围显式（Coverage Is Explicit）：来源根、规则、显式输入与引用接入都必须收敛为清晰可追踪的库覆盖范围
- Source / Asset / Unit 分层（Layered Processing）：Source 负责来源位置，Asset 负责内容内部定位，Unit 负责模型执行
- VectorSpace 表示向量空间（VectorSpace As Representation Space）：VectorSpace 只表达某个模型的一种具体向量表示空间，不表达库级可见性或物理 collection
- 索引状态集中（Index State In UnitIndex）：active、向量引用和 job 归属由 UnitIndex 承接，不写入 Source 或 Asset
- 完成快路径显式（Explicit E2E Completion）：只有存在 ContentE2eIndexState 的 Source Content 才能复用已有 Asset、Unit 与 UnitIndex
- Source 级提交（Source-Level Activation）：一个 Source 的可见位置必须在其结构化位置与所需 UnitIndex 验证成功后才能成为 active
- 验证先于替换（Validation Before Replacement）：任何新 Source 版本都必须在该 Source 所需索引载荷验证通过后才能替换旧 active 版本
- 可重用但不混同（Reuse Without Conflation）：复用 Content 处理结果不得绕过库覆盖、内容类型启用、过滤器或搜索可见性规则
- 恢复优先（Recovery First）：摄取运行与索引运行都应通过稳定检查点支持恢复与重试，而不是依赖重新执行全部阶段作为唯一恢复路径

## 正式来源类型与接入边界

- 正式来源类型固定为 PDF、图片、视频
- 文档来源的正式原生类型固定为 PDF；Office 文档若被支持，只能先通过显式转换进入正式来源类型，再进入本专题定义的稳定摄取链路
- 来源类型定义的是可进入库的稳定原始内容边界，不等同于搜索结果粒度；搜索结果粒度由 Asset 决定
- 摄取模式至少包括：手动导入、来源根扫描、引用型接入
- 手动导入用于把显式选定的文件或对象纳入单个库
- 来源根扫描用于根据库来源根与其规则形成候选集合
- 引用型接入用于在不重写 Content 身份的前提下，把已有对象或已有位置关系纳入某个库的 Source 关系
- 临时查询资产只服务搜索输入，不构成正式来源类型或正式摄取模式
- 是否复制原始文件、如何摆放临时物化载荷，以及具体导入 UI 形状不在本专题固定

## 库来源、覆盖范围与失效语义

- 库来源根提供库级扫描入口与覆盖边界；同一库可以拥有多个来源根
- 库覆盖范围由所有可用库来源根的覆盖边界，以及各自包含 / 排除规则的生效结果共同决定
- 包含 / 排除规则只决定候选集合与覆盖范围，不决定 Content、Source、Asset 或 Unit 身份
- 降级来源根是正式状态：来源根可因不可达、权限变化或局部异常进入降级状态
- 来源根降级会缩小或不稳定化本次实际覆盖范围，但不改变已确认 Source 的身份规则
- 被停用的来源根不会参与 watcher、增量 `refresh` 或全量 `rescan`
- 手动导入与引用型接入可以绕过目录遍历式扫描，但仍必须进入统一的覆盖与来源语义
- `refresh` 与 `rescan` 都可以重新评估覆盖范围、可达性与来源状态，并推动后续摄取或索引
- 当前阶段 `refresh` 表示增量重评估：优先基于 watcher 事件、已知路径变化或已有覆盖快照，只推进受影响的候选集合
- 当前阶段 `rescan` 表示全量重评估：重新枚举目标来源根或整个库的来源覆盖范围，再统一应用规则并收敛候选集合
- 对已启用的本地目录来源根，watcher 的变化事件应经过 debounce 后排队进入增量 `refresh`，而不是直接触发全量 `rescan`
- 原始文件失效、路径失联、来源根停用、来源根移除或规则变化导致脱离覆盖范围时，会影响 Source 的可用性与后续摄取候选，但不应把同一 Content 误判为新的身份
- 当前阶段当 Source 因文件消失或脱离覆盖范围而失效时，应先保留结构化记录，并在新的有效索引激活后退出新的搜索结果，而不是立即硬删除

## 摄取运行、索引运行与 VectorSpace

- 摄取运行面向单个库发起，是一次把显式输入或来源覆盖范围收敛为 Source Content、Source、Asset 与 Unit 的稳定处理尝试
- 索引运行可以面向单个库、来源根或明确候选集合发起，并按当前已解析 VectorSpace 拆分为一个或多个子构建
- VectorSpace 是向量表示空间，至少由 `provider_id`、`model_id`、`model_version`、可选 `model_revision`、`vector_type` 与必要 adapter / preprocessing signature 决定
- `vector_space_id` 是 VectorSpace 的稳定标识；同一个 `vector_space_id` 下的向量可以直接比较
- 内容类型若解析到同一个 VectorSpace，可以共享 Unit 编码和 UnitIndex
- `UnitIndex` 以 `unit_id + vector_space_id` 为 key；它决定该 Unit 在对应 VectorSpace 下是否 active、failed 或 not_ready，并通过 `vector_ref` 指向检索后端向量载荷
- `ContentE2eIndexState` 以 `content_id + pipe_signature + vector_space_id` 为 key；它决定某个 Source Content 是否可以跳过 Asset / Unit / UnitIndex 的重复创建
- 检索后端中的向量载荷可以按实现选择物理集合、命名空间或 alias；这些后端命名空间是存储实现细节，不是核心对象，也不等于 VectorSpace
- 全局复用不需要独立的向量缓存对象；同一 Unit 在同一 VectorSpace 下的 UnitIndex 已经是向量复用边界
- active lifecycle 只写入 UnitIndex 和 SourceAssetLocation，不写入 VectorSpace 本身；后端命名空间不承载业务真相
- 搜索期如何选择已启用内容类型，以及针对未启用内容类型请求的拒绝语义，由 [004-search](../004-search/spec.md) 定义
- 摄取与索引在需要模型能力的阶段所依赖的提供方能力、提供方绑定与运行时探测语义，由 [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义

## 稳定阶段模型与检查点

- 摄取与索引的稳定阶段顺序为：接入确认（intake） -> 扫描 / 候选收集（scan） -> Source Content 判定（dedup source content） -> Source 收敛（settle source） -> 完成快路径检查（check e2e state） -> Asset / Unit 收敛（settle assets and units） -> Unit 物化（materialize units） -> 编码（encode） -> 向量写入（vector write） -> 验证（validate） -> Source 激活（activate source）
- 接入确认阶段负责确认显式输入、来源根候选或引用关系的接入边界
- 对手动导入或引用型接入，扫描阶段表示对显式输入集的枚举与确认，而不要求目录遍历
- Source Content 判定阶段沿用 [002-state-and-data-model](../002-state-and-data-model/spec.md) 的身份规则：`size_bytes + fast_fingerprint` 只生成候选，SHA-256 懒计算且只用于候选确认；没有候选时不强制计算 SHA-256
- Source 收敛阶段负责创建或更新库内 Source，并引用原始 Content
- 完成快路径检查阶段查询 `ContentE2eIndexState(content_id, pipe_signature, vector_space_id)`；若完成标记存在，系统复用已有 Asset、Unit 与 UnitIndex，只提交 Source 和 SourceAssetLocation
- 若完成标记缺失，系统必须执行完整端到端索引流程，不得因为 Content 已存在就跳过处理
- Asset / Unit 收敛阶段负责从 Source Content 中形成全局 Asset 与 Unit；已存在的 Asset / Unit 应复用，缺失项才创建
- PDF 一类文档来源必须按真实页序展开出多个 `document_page` Asset，而不是把整份文档压缩为单个占位页对象
- 文档来源生成的 `document_page` Asset 必须携带真实页定位，并形成可用于详情展开的前后页邻近关系
- 视频来源必须展开出多个 `video_segment` Asset，而不是只把整段视频作为单个占位结果对象
- 视频来源生成的 `video_segment` Asset 必须携带来自同一源视频时间轴的真实 `start_ms` / `end_ms` 定位范围
- 当模型不能直接 embed 某个 Asset 表示的内容时，Unit 生成阶段应通过明确处理策略生成可嵌入 Unit，例如 PDF page image unit、video keyframe image unit 或 OCR text unit
- Unit 物化阶段负责生成模型输入字节，例如 PDF 页图、视频关键帧或文本片段；物化结果是临时文件、内存字节或可清理缓存，不进入核心状态模型
- 编码阶段负责生成目标 VectorSpace 所需的向量表示，不在本专题定义模型选择或提供方协议
- 编码阶段应先查询 `UnitIndex(unit_id, vector_space_id)`；active 命中可直接复用，缺失时才调用模型编码并写入检索后端
- 向量写入阶段负责把待激活向量载荷写入检索后端；只有该 Source 所有适用 VectorSpace 写入与验证成功后，才能写入对应 active UnitIndex
- 在完整端到端索引成功后，系统必须在一个 SQLite transaction 中提交 Source、SourceAssetLocation、Asset、Unit、UnitIndex ready 与 ContentE2eIndexState
- 显式可恢复检查点至少包括：接入确认完成、扫描完成、Source Content 判定完成、Source 收敛完成、完成快路径检查完成、Asset / Unit 收敛完成、Unit 物化完成、编码完成、向量写入完成、Source 激活完成
- 恢复语义基于已落盘检查点推进；本专题不定义内部批次切分、并发模型或阶段内重试细节

## Source 级激活与复用快路径

- 每次索引运行按 Source 形成可提交单元；一个 Source 的新版本只有在其 SourceAssetLocation 与所需 UnitIndex 验证成功后才能成为 active
- active UnitIndex 代表某个 Unit 在某个 VectorSpace 下的当前生效索引事实，是对外承担默认可检索事实的最小结构化边界
- SourceAssetLocation 代表某个 Source 中可返回的位置。SourceAssetLocation 的 active 可见性不得替代 UnitIndex 的向量可见性
- 新 Source 命中已有 Source Content 且 ContentE2eIndexState 存在时，必须走复用快路径：提交 Source、SourceAssetLocation，并让 Source active；不得重新物化 Unit 输入或重复编码
- 新 Source 版本需要完整处理时，必须原子写入该 Source、其 SourceAssetLocation、缺失的 Asset、Unit、active UnitIndex 与 ContentE2eIndexState
- 新 Source 版本失败、任务失败或无法恢复时，旧 active Source 版本必须保持可搜索；已经写入检索后端但未对应 active UnitIndex 的向量点只能作为 orphan 载荷存在，搜索不得返回它们
- 当前切片中的 import、`refresh` 与 `rescan` 都采用 Source 级提交语义：一个 Source 成功后即可对普通 active 搜索可见，同一 job 中其他 Source 不阻塞它
- 对同一 `source_uri` 的手动 import，当前切片应把它视为对既有 manual-import Source 的一次更新，而不是重复生成第二份结构化 Source；新版本成功提交前旧版本继续承担 active 搜索
- 对来源变更驱动的增量 `refresh` / `rescan`，未变化对象不要求重新编码；失效、删除或脱离覆盖范围的 Source 必须先移除或隐藏其 active SourceAssetLocation，再 best-effort 清理旧向量点
- 对同一个 Source 内的多个 VectorSpace，激活与失败语义按 Source 原子成立：任一适用 VectorSpace 写入失败时，该 Source 的新版本不得部分 active
- 当单次 import、`refresh` 或 `rescan` 同时推进多个 Source 时，若其中部分 Source 失败，运行级任务可以进入失败终态，但已成功激活的 Source 不得因此被回滚

## 重启恢复与重新判定

- 当前切片中，active UnitIndex 与 ContentE2eIndexState 会随结构化 durable truth 跨 restart 保留
- 应用启动后，必须把已保留的 active UnitIndex 的 `vector_ref` 与检索后端中的 stable active namespace 重新对照判定
- 若结构化记录存在，但 `vector_ref` 指向的向量点缺失、active namespace 缺失、alias target 缺失，或只剩同名旧物理 collection，则对应 VectorSpace 必须转为 inactive；随后搜索应返回 `not_ready`
- `ContentE2eIndexState` 只能在对应 Asset、Unit 和 UnitIndex 仍满足结构化一致性时作为完成快路径依据；若一致性检查失败，系统必须清除或忽略该完成标记并重新执行端到端索引
- 如果进程在向量写入后、SQLite transaction 提交前终止，检索后端可能留下 orphan vector；由于没有 ContentE2eIndexState，后续运行不得把该 Content 视为完成
- 如果进程在创建 provisional Content 后终止，后续运行可以复用该 Content 作为候选，但仍必须检查 ContentE2eIndexState；`Content exists` 不能作为处理完成的判断条件
- 应用启动只恢复 durable truth，不自动触发 `refresh`、`rescan`、自动重建索引或补做停机期间的 filesystem drift reconciliation
- 来源根 watcher 在启动后只重新播种运行时观察状态；是否需要推进新的 `refresh` / `rescan` 仍由后续显式触发或新的 watcher 事件决定

## 验证、切换与保留窗口

- 激活前的稳定验证契约至少包括：结构化对象与索引载荷的一致性、关键 Unit 结果完整性、SourceAssetLocation 与覆盖范围一致性，以及可激活性检查
- 具体验证实现可以包含抽样或 smoke test，但本专题不固定其算法、阈值或调用形状
- 任一 Source 级阶段失败、中断或验证未通过时，该 Source 的当前 active 位置必须保持不变；已经写入但未提交的检索载荷必须通过 active UnitIndex 与 SourceAssetLocation 过滤隐藏
- 来源根降级、重扫发现变化或原始文件失效会驱动后续替换流程；对失效 Source，应先移除或隐藏其 active SourceAssetLocation，再清理旧检索载荷，避免 orphan point 继续出现在结果中
- 因配置移除而失活的旧 active 检索后端命名空间，以及失败或中断留下的 orphan 检索载荷，都进入延迟清理窗口，用于诊断、恢复与安全回收；这些载荷可由命名约定、后端枚举和 UnitIndex 对照发现，不要求单独结构化表记录
- 延迟清理窗口只定义“暂不立即删除”的稳定语义，不构成正式回滚承诺
- 清理时机、TTL、容量配额与具体 GC 策略不在本专题固定

## 关联主题

- [000-foundation](../000-foundation/spec.md) 定义本地优先、多向量能力与项目级上游基础约束
- [001-architecture](../001-architecture/spec.md) 定义系统编排边界、编排中心与组件交互路径
- [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义库、库来源根、Content、Source、SourceAssetLocation、Asset、Unit、UnitIndex 与相关状态边界的基础语义
- [004-search](../004-search/spec.md) 定义搜索语义、搜索期 VectorSpace 选择与显式拒绝规则
- [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 定义提供方能力、提供方绑定、解析顺序与运行时探测语义
- [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 定义摄取与索引任务的执行系统、恢复、取消、进度与后台清理语义
- [007-storage-and-persistence](../007-storage-and-persistence/spec.md) 定义主结构化存储、检索命名空间、临时资产存储区与物理持久化边界
- [008-ui-ux](../008-ui-ux/spec.md) 定义导入、刷新、重扫、重建与相关配置管理在应用中的入口与操作流
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义导入、刷新、重扫、重建等公开接口的请求 / 响应契约，以及相关 sidecar 协议载荷
- [140-library-source-management](../140-library-source-management/spec.md) 定义来源根生命周期、结构化规则、库级来源清单，以及当前阶段 `refresh` / `rescan` / watcher 的功能边界
