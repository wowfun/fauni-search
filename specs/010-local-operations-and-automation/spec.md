# 010 本地操作与自动化 (Local Operations and Automation)

定义 FauniSearch 仓库级本地操作与自动化约束，明确本地脚本如何选择配置、启动、停止、诊断、检查与验证运行面。

## 关键术语 (Terminology)

- 本地操作脚本 (Local Operation Script)
- 本地配置文件 (Local Env File)
- 开发隔离配置 (Development Isolated Env)
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
- 自动化输出稳定：面向脚本消费的状态与 smoke 摘要必须提供稳定 JSON 输出，不依赖人类日志解析
- 快速检查不启动长驻服务：默认快速检查不加载真实 GPU 模型、不启动 Qdrant / app / sidecar / UI 长驻进程

## 本地配置选择

- 本地脚本默认使用根 `.env` 作为用户 / 默认运行配置
- 本地脚本通过显式 `--dev` 选择开发隔离配置 `.env.dev`
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
- `run-qdrant.sh` 承接 Qdrant 本地进程启动或复用
- `run.sh` 承接 app、sidecar 与 UI 的启动；默认前台运行，`--detach` 时进入分离运行
- `cutover-runtime.sh` 承接 runtime 世代切换；它按当前环境单独归档旧 `app/` 与 `qdrant/`，并初始化新的 `${APP_RUNTIME_DIR}/runtime-config.json`
- `run.sh` 启动 app、sidecar 与 UI 前应先确认 Qdrant 可访问；若不可访问，应自动调用同一配置上下文下的 `run-qdrant.sh` 启动或复用 Qdrant，并在 Qdrant 仍不可访问时失败
- `run.sh` 启动前必须解析合并后的 provider/model 配置，并为本地 sidecar 与本地 app 导出所需的运行时模型环境变量
- `run.sh` 若检测到当前环境下仍存在未 cutover 的旧世代 runtime 数据，必须拒绝启动并明确提示先执行 `cutover-runtime.sh`
- `run.sh` 必须复用选中 env 下同一个 `APP_RUNTIME_DIR`；重启 app 不得隐式改写持久状态路径，也不得在启动时自动清理旧的 durable store
- `stop.sh` 承接 app、sidecar、UI 与 Qdrant 的停止；必须支持选中配置下的服务发现
- `stop.sh --all` 只承接停进程语义，不承接数据清空、runtime wipe 或旧 collection 自动清理
- `status.sh` 承接服务状态查询；必须支持 `--json` 输出机器可读状态快照
- `check.sh` 承接无 GPU 快速检查
- `smoke-text-search.sh` 承接真实 ColQwen + Qdrant 文本搜索 smoke；必须支持 `--json` 输出机器可读验证摘要
- 各能力专题可以提供自己的主题 smoke 脚本；若存在，例如 `smoke-image-search.sh`，则也必须支持 `--json` 输出机器可读验证摘要

## 日志、pid 与状态

- app、sidecar、UI 与 Qdrant 的日志位置由选中 env 文件中的 `DEV_LOG_DIR` 决定
- `${APP_RUNTIME_DIR}` 是当前 restart-persistence 事实源的一部分；停止脚本不得把它当成临时日志目录一起清掉
- 分离运行时，app、sidecar 与 UI 应将 pid 写入 `DEV_LOG_DIR` 下的稳定 pid 文件
- Qdrant 由 `run-qdrant.sh` 启动或复用，其 pid 文件继续由 Qdrant 启动入口维护；`run.sh` 可以作为依赖前置步骤调用 `run-qdrant.sh`，但不应在自身清理流程中隐式停止 Qdrant
- `status.sh` 应报告每个服务的 URL、ready 状态、pid、日志路径与配置来源
- `stop.sh` 应优先复用 pid 文件，并保留端口 / 命令发现兜底，避免 pid 文件缺失时无法停止本仓库服务
- 旧 runtime-token Qdrant collections 的清理属于 operator/manual concern，不属于 `run.sh` / `stop.sh` 的自动职责
- 本次 alias cutover 后，旧 `text_search_*` collection 与旧“直接物理 `index_*` collection”的清理同样属于 operator/manual concern；`run.sh` / `stop.sh` 不负责自动迁移或自动删除这些旧 collections
- 如需兼容切换，操作员应先在选中配置下手工清理旧物理 `index_*` collections，再让新机制重建 active alias 与其物理 target
- `cutover-runtime.sh` 只归档旧 `${APP_RUNTIME_DIR}/state.sqlite` 所在 `app/` 目录与 `QDRANT_STORAGE_DIR` 所在 `qdrant/` 目录；它不归档下载缓存、模型缓存、日志或其他工具缓存

## 快速检查与 smoke

- `check.sh` 默认执行 Rust 主服务窄测试、sidecar 窄测试、UI TypeScript typecheck 与 UI 构建检查
- `check.sh` 不应默认执行 GPU smoke，也不应要求 app、sidecar、UI 或 Qdrant 已经启动
- `smoke-text-search.sh` 用于真实模型与真实 Qdrant 链路验证，应在 app、sidecar 与 Qdrant 已可访问后运行
- `smoke-image-search.sh` 若存在，用于真实图片查询链路验证，并应复用与 `smoke-text-search.sh` 相同的本地配置选择、服务前置与 JSON 输出约定
- `smoke-runtime-health.sh` 若存在，用于真实运行时健康与 `vector_space` 诊断链路验证，并应复用与其他 smoke 相同的本地配置选择、服务前置与 JSON 输出约定
- `check-e2e.sh` 若存在，应作为 Playwright UI smoke 与本地 smoke 的统一聚合入口，并默认面向 `--dev` 隔离运行面
- smoke 的机器可读摘要至少包含：`status`、`library_id`、`job_id`、`result_kinds`、`backend`、`repr_kind`
- `smoke-runtime-health.sh` 的机器可读摘要至少还应包含：
  - `runtime_health`
  - `vector_space_ids`
  - 可选 `unsupported_content_types`
- Playwright UI smoke 默认使用 `--dev` 隔离配置，优先复用现有 `--dev` 服务；仅在 `--dev` 服务未运行时才自行启动，并且只清理由自身启动的服务
- Playwright UI smoke 不应依赖默认 `.env` profile，也不应在结束时误停默认 profile 的服务
- UI 侧 Vite、Playwright 配置与 `tests/e2e` 源文件应统一使用 TypeScript；本地 `typecheck` 必须覆盖这些 Node / Playwright 入口，而不只覆盖浏览器端 `src/**/*.ts`

## 关联主题

- [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 定义产品运行期执行、任务、运行时进程与健康语义
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义应用公开接口与 sidecar HTTP/JSON 协议
- [100-text-search/testing.md](../100-text-search/testing.md) 定义文本搜索专题的测试覆盖、fixture 与 GPU smoke 场景
