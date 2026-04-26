# 010 本地操作与自动化 (Local Operations and Automation)

定义 FauniSearch 仓库级本地操作与自动化约束，明确本地脚本如何选择配置、启动、停止、诊断、检查与验证运行面。

## 关键术语 (Terminology)

- 本地操作脚本 (Local Operation Script)
- 本地配置文件 (Local Env File)
- 开发隔离配置 (Development Isolated Env)
- E2E 有界运行面 (E2E Bounded Runtime)
- 代理测试运行面 (Agent Test Runtime)
- 前台运行 (Foreground Run)
- 分离运行 (Detached Run)
- 服务状态快照 (Service Status Snapshot)
- 快速检查 (Fast Check)
- Smoke 验证 (Smoke Verification)

## 范围

- 本地脚本入口与命令行约定
- `.env` / `.env.dev` 配置选择规则
- `fauni.config.json` / `${APP_RUNTIME_DIR}/runtime-config.json` 的本地选择与合并规则
- 启动、停止、状态查询、诊断、快速检查与 smoke 验证的本地自动化语义
- 本地日志、pid 文件与机器可读输出约定

范围外：
- 产品任务状态机、运行时健康判定算法与 sidecar 托管语义
- 应用公开 HTTP API 与 Rust / Python sidecar 协议
- 搜索、导入、索引、UI 体验与具体专题 fixture 语义
- CI、容器、随机端口分配或发布流水线

## 设计原则

- 选中配置即事实源：同一次本地脚本运行只应有一个 env 文件作为端口、URL、日志目录与运行时目录的事实源
- 用户默认优先：不传额外 flag 时默认使用 `.env`，服务当前使用者的日常本地运行
- 开发隔离显式：开发或代理会话需要避开默认服务时，必须通过显式 `--dev` 选择 `.env.dev`
- E2E / 代理数据有界：`.env.dev` 是 Playwright / E2E / 代理实测专用运行面，测试启动流程应在 Qdrant collections 过量时修剪旧测试索引，而不是每次清空全部数据
- 自动化输出稳定：面向脚本消费的状态与 smoke 摘要必须提供稳定 JSON 输出，不依赖人类日志解析
- 快速检查不启动长驻服务：默认快速检查不加载真实 GPU 模型、不启动 Qdrant / app / sidecar / UI 长驻进程

## 本地配置选择

- 本地脚本默认使用根 `.env` 作为用户 / 默认运行配置
- 本地脚本通过显式 `--dev` 选择开发隔离配置 `.env.dev`
- `.env.dev` 是 Playwright / E2E / 代理实测专用的有界运行面，Codex 等自动化代理可以用它执行真实服务启动、HTTP smoke 与 CLI 验证；它不承载长期人工调试数据，需要长期保留状态的人工调试应使用默认 `.env` 或另行复制 env 文件
- provider/model 的项目级基线事实源是仓库根 `fauni.config.json`
- provider/model 的实例级覆盖事实源是 `${APP_RUNTIME_DIR}/runtime-config.json`
- 本地脚本应先选中 `.env` 或 `.env.dev`，再由其中的 `${APP_RUNTIME_DIR}` 解析实例级覆盖配置路径
- `.env.example` 是默认配置模板；`.env.dev.example` 是开发隔离配置模板
- `.env*.example` 只作为模板，不承接本机实际运行状态
- 脚本不得通过逐项端口 flag、脚本常量或 UI 常量维护与选中 env 文件并行的配置真值
- 开发隔离配置应与默认配置使用不同端口、日志目录与运行时数据目录，避免两个本地运行面互相抢占服务或 Qdrant storage
- `.env` / `.env.dev` 不再承接 `local_sidecar` 的正式模型事实；本地模型选择应从合并后的配置文件读取

## 本地操作入口

- `bootstrap-linux.sh` 承接一次性安装与初始化；带 `--dev` 时应初始化 `.env.dev`
- `doctor.sh` 承接环境诊断；允许在选中 env 文件缺失时回退到对应 example 进行诊断
- `download-model.sh` 承接 Hugging Face 模型预下载；默认读取合并后配置中的 `provider.local_sidecar.active_model` 与对应 model 的 `version`，并允许通过显式 `hf_repo_id` 参数临时指定其他 Hugging Face 仓库而不改写配置
- `run-qdrant.sh` 承接 Qdrant 本地进程启动或复用；产品级后端启动默认由 `faus serve` 复用其运行约定，而不是由 `run.sh` 直接调用该脚本
- `run.sh` 承接 full-stack 本地开发启动；它作为 `faus serve` + Vite UI 的 wrapper，默认前台运行，`--detach` 时进入分离运行
- `cutover-runtime.sh` 承接 runtime 世代切换；它按当前环境单独归档旧 `app/` 与 `qdrant/`，并初始化新的 `${APP_RUNTIME_DIR}/runtime-config.json`
- `cleanup-legacy-runtime.sh` 承接旧世代 runtime 清理；默认只扫描并报告当前环境的 legacy 归档与旧 Qdrant collections，只有显式 `--execute` 才执行删除
- `prune-dev-qdrant-collections.sh` 承接 E2E Qdrant collections 上限控制；它只允许显式 `--dev`，在 Qdrant 启动前扫描 `${QDRANT_STORAGE_DIR}/collections`，超过阈值时按时间顺序只保留最新的 Playwright stage collections
- `reset-dev-runtime.sh` 可以作为人工兜底重置工具；它只允许显式 `--dev`，先停止 `.env.dev` 下的 app、sidecar、UI 与 Qdrant，再清空并重建 `${APP_RUNTIME_DIR}` 与 `${QDRANT_STORAGE_DIR}`
- `run.sh` 启动后端 runtime 时必须调用同一配置上下文下的 `faus serve`；Qdrant、Python sidecar、Rust server、provider/model 解析与 runtime 世代检查由 `faus serve` 承接
- `run.sh` 不得再直接启动 `cargo run`、`.venv/bin/python -m fauni_sidecar` 或调用 `run-qdrant.sh` 来形成第二套后端编排路径
- `run.sh` 必须复用选中 env 下同一个 `APP_RUNTIME_DIR`；重启 app 不得隐式改写持久状态路径，也不得在启动时自动清理旧的 durable store
- 若选中 env 下的 `${APP_RUNTIME_DIR}/state.sqlite` 仍是旧的单行 snapshot store，`run.sh` / `faus serve` 必须让 App 以清晰错误拒绝启动；本地脚本不得隐式迁移、归档或清空该 store
- 当 `run.sh` 等待 app ready 失败或发现 app 进程提前退出时，应从 `app.log` 暴露明确启动失败原因；遇到旧 snapshot store 时，`.env.dev` 应提示 `reset-dev-runtime.sh --dev`，默认 `.env` 应提示显式 `cutover-runtime.sh`，不得只输出泛化超时信息
- `run.sh` 必须先等待后端 runtime 的 app、sidecar 与 Qdrant ready，再启动 Vite UI，避免开发服务器在后端未就绪时代理 `/api/*` 请求并写入误导性的连接失败日志
- `stop.sh` 承接 app、sidecar、UI 与 Qdrant 的停止；必须支持选中配置下的服务发现
- `stop.sh --all` 只承接停进程语义，不承接数据清空、runtime wipe 或旧 collection 自动清理
- E2E Qdrant 数据修剪必须通过 `prune-dev-qdrant-collections.sh --dev` 这类显式 prune 入口完成；全量清空必须通过 `reset-dev-runtime.sh --dev` 这类显式 reset 入口完成，不得隐式塞进 `stop.sh --all`
- `status.sh` 承接服务状态查询；必须支持 `--json` 输出机器可读状态快照
- `check.sh` 承接无 GPU 快速检查
- `smoke-text-search.sh` 承接真实 ColQwen + Qdrant 文本搜索 smoke；必须支持 `--json` 输出机器可读验证摘要
- 各能力专题可以提供自己的主题 smoke 脚本；若存在，例如 `smoke-image-search.sh`，则也必须支持 `--json` 输出机器可读验证摘要

## 日志、pid 与状态

- app、sidecar、UI 与 Qdrant 的日志位置由选中 env 文件中的 `DEV_LOG_DIR` 决定
- `${APP_RUNTIME_DIR}` 是当前 restart-persistence 事实源的一部分；停止脚本不得把它当成临时日志目录一起清掉
- 分离运行时，app 与 UI 应将 pid 写入 `DEV_LOG_DIR` 下的稳定 pid 文件；当 app 由 `faus serve` 承接时，`app.pid` 记录 `faus serve` 进程
- Qdrant 与 sidecar 由 `faus serve` 启动或复用时，不要求 `run.sh` 写入新的 `qdrant.pid` 或 `sidecar.pid`；`stop.sh` 的端口 / 命令发现仍作为兜底
- `status.sh` 应报告每个服务的 URL、ready 状态、pid、日志路径与配置来源；app pid 识别必须覆盖旧 Rust server binary、`cargo run` 与 `faus serve`
- `stop.sh` 应优先复用 pid 文件，并保留端口 / 命令发现兜底，避免 pid 文件缺失时无法停止本仓库服务
- 旧 runtime-token Qdrant collections 的清理属于 operator/manual concern，不属于 `run.sh` / `stop.sh` 的自动职责
- 本次 alias cutover 后，旧 `text_search_*` collection、旧“直接物理 `index_*` collection”与旧“直接物理 `vector_space_*` collection”的清理同样属于 operator/manual concern；`run.sh` / `stop.sh` 不负责自动迁移或自动删除这些 collections
- `cleanup-legacy-runtime.sh` 应只作用于选中 env 对应的 runtime root，并支持：
  - 默认 scan-only 输出
  - `--execute` 显式删除
  - `--json` 机器可读摘要
- `cleanup-legacy-runtime.sh` 在执行删除前必须确认选中 env 下的 app、sidecar、UI 与 Qdrant 均未运行；若仍有任一服务运行，脚本必须拒绝执行删除
- `cleanup-legacy-runtime.sh --execute` 应删除该 env root 下全部 `legacy-*` 归档目录，以及 Qdrant storage 中旧世代 `index_*`、`text_search_*` 与直接物理 `vector_space_*` collections
- `cleanup-legacy-runtime.sh` 不得删除 alias 当前 target，也不得把当前命名方案中的 `vector_space_stage_*` collections 当成 legacy collection 误删
- 如需兼容切换，操作员应先执行 `cutover-runtime.sh` 归档旧世代 runtime，再按需执行 `cleanup-legacy-runtime.sh --execute` 清理旧归档与旧 collections
- `cutover-runtime.sh` 只归档旧 `${APP_RUNTIME_DIR}/state.sqlite` 所在 `app/` 目录与 `QDRANT_STORAGE_DIR` 所在 `qdrant/` 目录；它不归档下载缓存、模型缓存、日志或其他工具缓存
- `prune-dev-qdrant-collections.sh --dev` 默认在 collections 总数大于 500 时触发，按 collection 名称中的 Playwright 时间戳或文件时间排序，只保留最新 100 个 `vector_space_stage_playwright-*` collection，并同步删除指向被删 collection 的 alias
- `prune-dev-qdrant-collections.sh` 不得在 Qdrant 运行时直接删除 storage 文件；如需删除，调用方必须先停止 `.env.dev` 下的相关服务
- `reset-dev-runtime.sh --dev` 删除并重建 `.env.dev` 指向的 `${APP_RUNTIME_DIR}` 与 `${QDRANT_STORAGE_DIR}`，保留 `${DEV_LOG_DIR}` 目录但清理 stale pid 文件，并重新初始化 `${APP_RUNTIME_DIR}/runtime-config.json`
- `.env.dev` 遇到旧 snapshot `state.sqlite` 时，推荐使用 `reset-dev-runtime.sh --dev` 明确重建 dev 运行面；默认 `.env` 运行面应由用户手动 cutover 或 reset，不自动处理
- `reset-dev-runtime.sh` 不得作用于默认 `.env`，也不得把被清理数据归档为 legacy；它的语义是 E2E 手动全量 reset，不是 operator cutover

## 快速检查与 smoke

- `check.sh` 默认执行 Rust 主服务窄测试、sidecar 窄测试、UI TypeScript typecheck 与 UI 构建检查
- `check.sh` 不应默认执行 GPU smoke，也不应要求 app、sidecar、UI 或 Qdrant 已经启动
- `smoke-text-search.sh` 用于真实模型与真实 Qdrant 链路验证，应在 app、sidecar 与 Qdrant 已可访问后运行
- `smoke-image-search.sh` 若存在，用于真实图片查询链路验证，并应复用与 `smoke-text-search.sh` 相同的本地配置选择、服务前置与 JSON 输出约定
- `smoke-runtime-status.sh` 若存在，用于真实运行时状态与 `vector_space` 诊断链路验证，并应复用与其他 smoke 相同的本地配置选择、服务前置与 JSON 输出约定
- `check-e2e.sh` 若存在，应作为 Playwright UI smoke 与本地 smoke 的统一聚合入口，并默认面向 `--dev` 隔离运行面
- smoke 的机器可读摘要至少包含：`status`、`library_id`、`job_id`、`result_kinds`、`backend`、`vector_type`
- `smoke-runtime-status.sh` 的机器可读摘要至少还应包含：
  - `runtime_status`
  - `vector_space_ids`
  - 可选 `unsupported_content_types`
- Playwright UI smoke 默认使用 `--dev` 隔离配置；启动前应先执行 `.env.dev` Qdrant collection prune，避免历史 `playwright-*` stage collection 与 alias 无限增长并拖慢 Qdrant 冷启动
- Playwright UI smoke 结束时只停止由自身启动的 `.env.dev` 服务，不在 teardown 阶段删除数据，以便失败后保留现场；下一次 E2E 启动前只在超过阈值时修剪旧 collection
- Playwright UI smoke 不应依赖默认 `.env` profile，也不应在结束时误停默认 profile 的服务
- UI 侧 Vite、Playwright 配置与 `tests/e2e` 源文件应统一使用 TypeScript；本地 `typecheck` 必须覆盖这些 Node / Playwright 入口，而不只覆盖浏览器端 `src/**/*.ts`

## 关联主题

- [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 定义产品运行期执行、任务、运行时进程与健康语义
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义应用公开接口与 sidecar HTTP/JSON 协议
- [100-text-search/testing.md](../100-text-search/testing.md) 定义文本搜索专题的测试覆盖、fixture 与 GPU smoke 场景
