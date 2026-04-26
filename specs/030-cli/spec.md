# 030 产品 CLI (CLI)

定义 FauniSearch 产品级命令行入口 `faus` 的稳定职责、运行边界、连接规则、输出约定与基础命令面。`faus` 以 Rust 主服务为产品核心：`serve` 负责启动本机产品 runtime，其余工作流命令默认作为 HTTP client 消费同一公开契约。

## 关键术语 (Terminology)

- 产品 CLI (Product CLI)
- 产品 runtime 入口 (Product Runtime Entry)
- 工作流优先命令 (Workflow-first Command)
- 运行时连接 (Runtime Connection)
- 基础服务地址 (Base URL)
- 人类可读输出 (Human-readable Output)
- 机器可读输出 (Machine-readable Output)
- 一键上传搜索 (Upload-and-search)

## 范围

- `faus` 命令名、全局 flag 与基础连接规则
- `faus serve` 的产品 runtime 入口职责
- `faus` 的产品工作流命令面
- CLI 默认输出、`--json` 输出与错误映射约定
- CLI 与 Rust HTTP server、`scripts/local/*`、Web 入口之间的职责分界
- CLI 搜索命令的输入方式与服务端能力边界

范围外：

- HTTP endpoint、请求 / 响应 payload、错误载荷与 OpenAPI schema 的具体形状
- Web 产品体验、Web 信息架构、前端实现拆分与静态资源托管细节
- bootstrap、doctor、reset、smoke、日志归档、pid 管理、后台守护与发布安装等 operator 自动化
- shell completion、man page、包分发、安装器与发布渠道

## 设计原则

- Server-first：产品业务能力只通过 Rust 主服务公开 API 暴露；除 `faus serve` 的本机 runtime 启动职责外，CLI 不直接读写内部状态、SQLite、Qdrant、runtime 文件或 sidecar
- 层级清晰：`serve` 是 headless runtime 入口，`status` 是只读观察命令，`web` 基于启动或连接后的 Rust server 进入浏览器体验，library / import / search / jobs 是公开 HTTP API 的工作流 client
- 工作流优先：命令按用户目标组织，优先覆盖启动、状态查看、库操作、导入、搜索、任务观察和打开 Web，不机械镜像全部 HTTP API
- 可脚本化输出：默认输出服务人类阅读；显式 `--json` 时必须提供稳定机器可读结构，不依赖人类文案解析
- 运维分层：`faus serve` 是产品 runtime 启动入口；bootstrap、doctor、reset、stop、detach、smoke、Qdrant 诊断和运行面清理继续归 `scripts/local/*` 与 [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md)
- 契约复用：CLI 消费的 HTTP 契约由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 与 OpenAPI contract 承接，`030` 不重复定义 payload 细节
- 本机优先：CLI 默认面向本机运行面，不引入远程登录、mDNS、开放 CORS 或局域网发现
- 实现边界：`faus` 内部可以拆分为 binary-local modules 以复用连接、错误、serve 与工作流实现，但公共接口仍以本规格定义的命令、flags 与输出契约为准

## 全局入口与连接规则

- 产品 CLI 的命令名固定为 `faus`
- `faus` 至少支持以下全局 flag：
  - `--base-url <url>`：指定 Rust 主服务基础地址，适用于 client 型命令
  - `--json`：输出机器可读 JSON；长运行命令的具体语义由对应专题定义
  - `--debug`：请求或展示调试信息
- client 型命令的 base URL 解析优先级固定为：
  - 显式 `--base-url`
  - 环境变量 `FAUS_BASE_URL`
  - 默认值 `http://127.0.0.1:53210`
- `--base-url` 与 `FAUS_BASE_URL` 的尾随斜杠不得影响最终请求路径
- `faus serve` 不通过 `--base-url` 决定监听地址；它使用 `--host`、`--port` 与运行配置启动 Rust server
- `faus status`、library、import、search、jobs 等 client 型命令默认不读取 `.env`、`.env.dev` 或 `FAUNI_ENV_FILE`
- 当 Rust 主服务不可达时，client 型命令必须明确报告连接失败；是否启动本机 runtime 只属于 `faus serve` 与基于它的 `faus web` 语义
- client 型命令连接 Rust 主服务 App API 时不得使用 ambient `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY` 等代理环境变量；本地产品 API 默认应直连目标 base URL

## 输出与错误

- 默认人类可读输出应提供短摘要，优先展示用户下一步需要的信息，例如监听地址、健康状态、库 ID、任务 ID、搜索结果数量、命中类型与 Web URL
- `--json` 输出必须是单个 JSON 对象，不应混入进度文案、ANSI 控制字符或日志行
- `--json` 成功输出至少应能表达：
  - `status`
  - `data`
  - 可选 `warnings`
  - 可选 `debug`
- `--json` 失败输出至少应能表达：
  - `status: "error"`
  - `error.code`
  - `error.message`
  - 可选 `error.hint`
  - 可选 `error.details`
  - 可选 `error.retryable`
- 服务端统一错误载荷必须原样映射到 CLI 错误对象中；CLI 可以补充连接层错误码，但不得改写服务端错误语义
- 默认人类可读输出中的错误文案应保留服务端错误消息，并在连接失败、空响应、not_ready、not_supported 这类常见场景中给出明确下一步；CLI 层错误可以附带 `hint`
- CLI 层连接和响应错误的 JSON `details` 应优先包含 `base_url`、`base_url_source`、`request_url` 与必要的 HTTP 状态，避免只暴露底层 parser 或 socket 文案
- `--debug` 可以透传 HTTP 请求中的 `debug` 字段，或展示响应中的调试摘要；调试信息不得默认进入人类可读输出的主层级

## Help 与示例

- 顶层 `faus --help` 应说明产品 CLI 的用途，并展示 `serve`、`status`、`library`、`web` 等常用入口
- 全局 flags 的 help 文案必须说明 `--base-url`、`--json`、`--debug` 的用途
- 子命令 help 应用产品语义描述职责边界，例如 `status` 只读连接已有 server，`library` 操作库资源，`web` 启动或连接 Web 体验
- 基础工作流命令应提供足够短的示例，使用户能直接尝试 `faus serve`、`faus status`、`faus library list` 与 `faus web`

## 基础命令面

`faus` 固定采用工作流优先命令面，至少包括：

- `faus serve`
- `faus status`
- `faus library ...`
- `faus import ...`
- `faus search text|image|video|document ...`
- `faus jobs ...`
- `faus web`

### `faus serve`

- `faus serve` 是产品级 headless runtime 入口
- `faus serve` 启动 runtime 三件套：Qdrant、Python sidecar 与 Rust server
- `faus serve` 不启动 Vite UI，也不托管 `ui/dist`；它只提供 headless App API runtime
- 默认前台运行，进程生命周期由当前终端控制
- 后台化、pid、日志文件、stop 与运行面清理由 `scripts/local/run.sh --detach` 等 wrapper 和本地脚本承接
- `faus serve` 至少支持：
  - `--host <host>`
  - `--port <port>`
  - `--dev`
- `--host` 与 `--port` 决定 Rust server 监听地址；默认仍面向 `127.0.0.1:53210`
- `--dev` 选择本地开发运行配置；具体 env 文件语义与本地自动化边界由 [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md) 承接
- `faus serve` 不引入远程 auth、开放 CORS、mDNS 或局域网发现

### `faus status`

- `faus status` 用于查看目标 Rust 主服务和产品运行面的可用状态
- `faus status` 只连接已有 server，不启动本地进程
- 至少应消费 `/health` 与 `/runtime/status`
- 人类可读输出应区分 app、Qdrant 与 provider / sidecar 状态
- `--json` 输出应保留服务端 runtime status 的结构化信息

### `faus library`

- `faus library` 承接库级常用工作流
- 至少应规划以下能力：
  - 列出库
  - 创建库
  - 查看单个库
  - 重命名库
  - 归档与恢复库
- 物理删除、批量迁移与复杂生命周期操作不作为基础命令面的必选能力
- 库命令使用公开库管理 API，不直接访问本地持久化文件

### `faus import`

- `faus import` 用于把本地路径提交给指定库的导入接口
- 基础命令形态固定为 `faus import --library-id <library_id> <path>...`
- 输入路径可以是相对路径或绝对路径；相对路径由 CLI 按当前 shell cwd 转为绝对路径后提交
- `faus import` 只负责提交导入请求并返回任务摘要，不负责直接扫描文件、生成 embedding 或等待全量索引完成
- 等待、轮询、watch、tail 或 job log 属于扩展能力；基础命令不默认阻塞到索引完成，任务观察交给 `faus jobs`

### `faus search`

- `faus search text` 用于文本搜索，必须支持单库和所有库两种搜索范围
- `faus search image`、`faus search video` 与 `faus search document` 优先支持一键上传搜索：CLI 先通过 query asset upload API 上传本地文件，再用返回的 `temp_asset_id` 调用对应搜索端点
- 非文本搜索遵守服务端能力边界，默认只要求单库搜索；当用户请求服务端尚不支持的范围或输入形态时，应返回 `not_supported` 或等价服务端错误
- 搜索命令应支持 `top_k`、`cursor`、`debug` 等与公开搜索契约对应的常用参数
- 人类可读搜索输出应优先展示结果数量、来源路径、结果类型、库 ID、score 与可取用预览 URL
- `--json` 搜索输出应保留服务端搜索响应结构，不丢弃 `next_cursor`、`unsupported_content_types` 或 `debug`

### `faus jobs`

- `faus jobs` 用于观察和处理后台任务
- `faus jobs` 是 top-level runtime resource group，不作为 `library`、`import` 或 `search` 的参数或子资源命令
- 至少应规划以下能力：
  - 列出任务
  - 查看单个任务
  - 取消任务
  - 恢复任务
  - 重试任务
- 任务状态机、恢复规则与重试约束由运行时专题和服务端 API 承接，CLI 只表达结果和错误

### `faus web`

- `faus web` 用于进入当前 Rust server 对应的 Web 体验
- 当显式 `--base-url` 或 `FAUS_BASE_URL` 存在时，`faus web` 优先连接该 server 并打开对应 Web URL
- 当没有显式目标 server 时，`faus web` 可以复用 `faus serve` 能力启动默认本机 runtime，再启动本地 Web server 并打开浏览器
- 如果浏览器打开失败，命令应打印可访问 URL
- `faus web` 托管 `ui/dist` 并把同源 API 路径代理到 Rust server App API；Web URL 默认使用本地运行配置的 `UI_HOST` / `UI_PORT`
- `faus web` 应保持前台运行并由用户中断退出
- CLI-hosted Web 的具体路由、静态资源托管和 Vite 开发模式分界由 [008-ui-ux](../008-ui-ux/spec.md) 与 [020-frontend-architecture](../020-frontend-architecture/spec.md) 承接

## 与本地脚本的分界

- `faus serve` 是产品 runtime 的最薄启动入口，不是完整 operator 工具
- `scripts/local/*` 继续负责 bootstrap、run wrapper、stop、status、doctor、check、smoke、runtime reset、日志、pid 与 Qdrant 诊断
- `scripts/local/run.sh` 可以作为 `faus serve` 的 wrapper 承接 `--detach`、日志文件和开发自动化
- `faus status` 是产品运行面状态，不是 `scripts/local/status.sh` 或 `scripts/local/doctor.sh` 的完整替代
- `faus` 可以在错误提示中引用本地脚本作为启动、停止或诊断建议，但不得把 doctor、reset、smoke 等脚本行为内联为普通产品命令的默认执行路径
- 自动化场景如果需要启动、停止或隔离 `.env.dev` 运行面，应继续使用本地脚本；如果需要操作产品资源，应使用 `faus`

## 与 Web 和接口契约的分界

- Web 产品体验、应用壳层、工作区语义与可见交互由 [008-ui-ux](../008-ui-ux/spec.md) 承接
- Web 前端代码组织、构建入口、Vite 开发模式与静态资源实现由 [020-frontend-architecture](../020-frontend-architecture/spec.md) 承接
- CLI 消费的公开 HTTP API、错误载荷、搜索封套、任务快照和 OpenAPI contract 由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接
- 系统级 server-first 架构边界由 [001-architecture](../001-architecture/spec.md) 承接

## 验收标准

- 仓库存在 `faus` 产品 CLI 入口规划，且其命令面不与 `scripts/local/*` 职责混淆
- 基础命令面覆盖 serve、status、library、import、search、jobs 与 web
- `serve -> status -> web` 的职责层级清晰：启动 runtime、观察 runtime、进入浏览器体验
- `faus` 的 base URL 解析、`--json` 输出、错误映射和运行边界有明确规格约束
- 搜索命令覆盖文本单库 / 所有库，以及图片、视频、文档的一键上传搜索
- 规格没有新增 HTTP endpoint，也没有复制 009 中的 payload 细节

## 关联主题

- [001-architecture](../001-architecture/spec.md) 定义 Rust 主服务作为唯一系统级编排中心
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义公开 HTTP/API 契约与 OpenAPI 承接方向
- [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md) 定义本地运维脚本、env 选择、服务启停与 smoke 验证
- [008-ui-ux](../008-ui-ux/spec.md) 定义 Web 产品体验与应用壳层语义
- [020-frontend-architecture](../020-frontend-architecture/spec.md) 定义前端实现层组织边界
