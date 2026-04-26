# 150 faus Serve CLI

定义 `faus serve` 作为产品级 headless runtime 入口，并建立后续 CLI 切片复用的最小命令行基础。本专题承接 [030-cli](../030-cli/spec.md) 的长期 CLI 方向，只固定本机 runtime 启动边界，不承接状态查询、库操作、导入、搜索、任务或 Web 浏览器入口命令。

## 关键术语

- `faus`
- Serve CLI
- 产品 runtime
- Runtime 三件套
- 前台运行
- 开发运行配置
- 本地脚本 wrapper

## 范围

- `faus` binary 的最小可运行入口
- `faus serve` 命令行为
- `--host`、`--port`、`--dev` 在 `serve` 命令中的行为
- Qdrant、Python sidecar 与 Rust server 的启动顺序和生命周期边界
- `faus serve` 与 `scripts/local/*` 的职责分界

范围外：

- `faus status`、Web 入口命令、库命令、导入命令、搜索命令与任务命令
- HTTP endpoint、OpenAPI schema 或服务端 payload 细节的定义
- Vite UI、浏览器打开、Web 静态资源托管或前端构建
- bootstrap、doctor、reset、smoke、后台守护、pid 文件、日志归档与 stop 命令
- shell completion、安装器、发布渠道与 man page

## 设计原则

- Headless runtime：`faus serve` 启动产品运行所需的后端 runtime，不启动 Vite UI
- 产品入口优先：`faus serve` 是用户可直接使用的产品 runtime 入口，不要求用户先理解 `scripts/local/*`
- 前台默认：命令默认占用当前终端运行，日志输出和退出信号都围绕前台进程设计
- Wrapper 分层：后台化、pid、日志文件、stop、reset、doctor 和 smoke 继续由本地脚本承接
- 本机安全边界：默认监听本机地址，不引入远程 auth、开放 CORS、mDNS 或局域网发现
- 规格复用：本地运行环境、env 文件和自动化脚本的完整语义由 [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md) 承接

## 命令入口

- 本切片新增产品 CLI binary：`faus`
- 本切片只要求 `faus serve` 可用
- `faus serve` 至少支持：
  - `--host <host>`
  - `--port <port>`
  - `--dev`
- 未支持的命令或参数应由 CLI 参数解析层返回非零退出码，并展示清晰错误

## `faus serve`

- `faus serve` 启动 runtime 三件套：
  - Qdrant
  - Python sidecar
  - Rust server
- `faus serve` 不启动 Vite UI，也不要求前端开发服务器存在
- `faus serve` 默认前台运行，直到用户中断或进程收到终止信号
- 启动失败必须返回非零退出码，并清理本次命令启动的子进程
- 正常中断时应尽量关闭本次命令启动的 sidecar 与 Qdrant 子进程，避免遗留孤儿进程
- `faus serve` 可以复用既有 runtime 目录、Qdrant 数据目录与本地配置，但不得改变公开 HTTP API 契约

## 地址与运行配置

- `--host` 决定 Rust server 监听 host
- `--port` 决定 Rust server 监听 port
- 默认监听地址仍面向 `127.0.0.1:53210`
- `--dev` 选择本地开发运行配置；具体 env 文件、端口、日志和 runtime 目录规则由 [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md) 承接
- `faus serve` 不使用 `--base-url` 作为监听地址输入
- `APP_HOST`、`APP_PORT`、`FAUNI_ENV_FILE` 等运行环境变量属于运行配置层；`faus serve` 可以按本地运行规格读取它们，但 client 型命令不得用它们推导 base URL

## 输出与错误

- 默认人类可读输出应展示启动进度、监听地址和关键 runtime 组件状态
- Rust server ready 后应展示至少以下信息：
  - server base URL
  - Web URL
  - OpenAPI URL
- `--debug` 可以展示子进程命令、端口选择、配置来源与 readiness 探测细节
- `--json` 的长运行输出语义不在本切片强制固定；如果实现支持机器输出，必须避免混入人类日志
- 启动失败、端口占用、配置无效、Qdrant 启动失败、sidecar 启动失败或 Rust server ready 超时都必须返回非零退出码

## 与本地脚本的分界

- `faus serve` 是产品 runtime 的最薄前台入口
- `scripts/local/run.sh` 作为 full-stack wrapper 调用 `faus serve` 启动 headless 后端，并额外启动 Vite UI；`run.sh --detach` 提供后台化、pid、日志文件和本地开发自动化
- `scripts/local/stop.sh`、`status.sh`、`doctor.sh`、`check-e2e.sh`、smoke 脚本和 runtime reset 仍由 [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md) 承接
- `faus serve` 不替代 bootstrap、doctor、reset 或 smoke
- `faus serve` 不直接承接 library、import、search、jobs 等产品资源操作

## 当前阶段承接

- 当前阶段实施计划见 [plan.md](./plan.md)
- 当前阶段测试设计见 [testing.md](./testing.md)
- 状态查询能力由 [160-faus-status-cli](../160-faus-status-cli/spec.md) 承接
- Web 浏览器入口由 [170-faus-web-cli](../170-faus-web-cli/spec.md) 承接

## 验收标准

- `faus serve` 能以前台命令形式启动 Qdrant、Python sidecar 与 Rust server
- `--host`、`--port` 与 `--dev` 的行为有明确实现和测试覆盖
- `faus serve` 不启动 Vite UI，也不依赖 Vite 开发服务器
- 启动失败返回非零退出码，并清理本次启动的子进程
- 本切片不新增 HTTP endpoint，不改变 OpenAPI，不修改 Web 前端实现
- 本切片不把 bootstrap、doctor、reset、smoke 或后台守护内联为产品 CLI 默认行为

## 关联主题

- [030-cli](../030-cli/spec.md)
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md)
- [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md)
- [160-faus-status-cli](../160-faus-status-cli/spec.md)
- [170-faus-web-cli](../170-faus-web-cli/spec.md)
