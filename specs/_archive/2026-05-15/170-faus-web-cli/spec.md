# 170 faus Web CLI

定义 `faus web` 的具体行为：连接已有 Rust server，或在没有显式目标 server 时复用 `faus serve` 能力启动默认本机 runtime，然后打开浏览器进入 Web 体验。本专题承接 [030-cli](../030-cli/spec.md) 的长期 CLI 方向，并复用 [150-faus-serve-cli](../150-faus-serve-cli/spec.md) 与 [160-faus-status-cli](../160-faus-status-cli/spec.md) 的基础，不承接库操作、导入、搜索或任务命令。

## 关键术语

- `faus`
- Web CLI
- 基础服务地址
- Web 入口
- 浏览器打开
- 连接已有 server
- 启动本机 runtime

## 范围

- `faus web` 命令行为
- `--base-url`、`--json`、`--debug` 三个全局 flag 在 `web` 命令中的行为
- `FAUS_BASE_URL` 与默认 base URL 在 `web` 命令中的解析优先级
- 启动或连接 server 后打开浏览器的行为
- 浏览器打开失败时打印 URL 的回退行为

范围外：

- 状态查询命令、库命令、导入命令、搜索命令与任务命令的实现
- HTTP endpoint、OpenAPI schema 或服务端 payload 变更
- Web 静态资源托管的实现细节、Vite 代理、前端路由或 UI 构建
- bootstrap、doctor、reset、smoke、后台守护、pid 文件与日志归档
- shell completion、安装器、发布渠道与 man page

## 设计原则

- 浏览器入口：`faus web` 面向进入 Web 体验，不只是打印 URL
- 先连接后启动：显式 `--base-url` 或 `FAUS_BASE_URL` 表示用户已有目标 server；没有显式目标时，命令可以启动默认本机 runtime
- 复用 serve：本机 runtime 启动语义复用 [150-faus-serve-cli](../150-faus-serve-cli/spec.md)，不形成第二套启动实现
- 本地 Web server：`faus web` 托管 `ui/dist`，并把同源 API 路径代理到 Rust server App API
- 不启动 Vite：`faus web` 使用构建产物，不启动 Vite UI
- 可回退：浏览器打开失败不应吞掉入口信息，必须打印可访问 URL
- 边界复用：Web 产品体验由 [008-ui-ux](../008-ui-ux/spec.md) 承接，前端实现由 [020-frontend-architecture](../020-frontend-architecture/spec.md) 承接，公开 HTTP 契约由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接

## 命令入口

- 本切片要求 `faus web` 可用
- `faus` binary 与 runtime 启动基础由 [150-faus-serve-cli](../150-faus-serve-cli/spec.md) 承接
- 状态探测和 base URL 连接经验复用 [160-faus-status-cli](../160-faus-status-cli/spec.md)
- `faus` 至少支持以下全局 flag：
  - `--base-url <url>`
  - `--json`
  - `--debug`
- 未支持的命令或参数应由 CLI 参数解析层返回非零退出码，并展示清晰错误

## base URL 与启动规则

- base URL 解析优先级固定为：
  - 显式 `--base-url`
  - 环境变量 `FAUS_BASE_URL`
  - 默认值 `http://127.0.0.1:53210`
- `--base-url` 与 `FAUS_BASE_URL` 的尾随斜杠不得影响 App API base URL
- base URL 必须是可解析的 HTTP 或 HTTPS URL
- 显式 `--base-url` 或 `FAUS_BASE_URL` 存在时，`faus web` 只连接该 server，不自动启动本机 runtime
- 没有显式目标 server 时，`faus web` 应优先连接默认地址；默认地址探测可以对连接失败、空响应或非 JSON `/health` 等启动期探测失败做短暂重试，重试后仍失败时可以复用 `faus serve` 能力启动默认本机 runtime
- Web URL 默认使用当前运行配置中的 `UI_HOST` 与 `UI_PORT`
- `faus web` 启动的本地 Web server 若端口被占用，应返回清晰错误
- Web 产物缺失属于 `faus web` 本地 Web server 错误，不触发启动本机 runtime，应作为可诊断错误返回
- `faus web` 应保持前台运行，直到用户中断

## `faus web`

- `faus web` 的 App API base URL 等于规范化后的 base URL
- `faus web` 的 Web URL 等于本地 Web server 根路径，默认来自 `UI_HOST` / `UI_PORT`
- 本地 Web server `GET /` 返回 `ui/dist/index.html`，`/assets/*` 返回构建产物，SPA fallback 返回 `index.html`
- 本地 Web server 必须把 `/health`、`/openapi.json`、`/routes`、`/runtime/*`、`/settings/*`、`/libraries/*`、`/jobs/*`、`/search/*` 代理到 App API base URL
- 命令应在打开浏览器前确认目标 Rust server 与本地 Web server 都达到可用状态
- 浏览器打开成功时，人类可读输出应保持简短，至少能让用户看到 URL
- 浏览器打开失败时，命令必须打印可访问 URL，并说明用户可以手动打开
- `faus web` 不启动 Vite UI，不执行前端构建

## JSON 输出

`faus web --json` 成功输出必须是单个 JSON 对象：

```json
{
  "status": "ok",
  "data": {
    "base_url": "http://127.0.0.1:53210",
    "web_url": "http://127.0.0.1:55173",
    "opened": true,
    "server_started": false
  }
}
```

- `data.base_url` 是 App API URL，`data.web_url` 是本地 Web server URL
- `data.opened` 表示浏览器打开请求是否成功
- `data.server_started` 表示本次命令是否启动了本机 runtime
- 当 `--debug` 与 `--json` 同时出现时，可以附加 `debug` 对象，用于展示 base URL 来源、连接结果或启动路径等 CLI 侧信息
- `--json` 输出不得包含 ANSI 控制字符、进度文案或日志行

## 错误输出

- 无效 base URL 是 CLI 层错误，不是服务端 `ErrorEnvelope`
- 显式目标 server 不可达、返回空响应或返回无效响应时，命令返回对应错误，不自动改为启动默认 runtime
- 本机 runtime 启动失败时，命令返回启动层错误，并保留关键失败原因
- 浏览器打开失败但 URL 已打印时，不应被视为 server 失败；命令可以用成功退出表达“已提供可访问 URL”
- CLI 层错误可以附带 `hint`；默认地址启动期探测失败应提示命令会尝试启动 runtime，显式目标失败应提示检查 `--base-url` 或 `FAUS_BASE_URL`
- `--json` 下的错误输出必须是单个 JSON 对象：

```json
{
  "status": "error",
  "error": {
    "code": "connection_failed",
    "message": "...",
    "hint": "..."
  }
}
```

- 人类可读错误应写入 stderr，并返回非零退出码
- JSON 错误对象可在 `error.details` 中包含 `base_url`、`base_url_source`、`request_url` 与必要的 HTTP 状态

## Help 文案

- `faus web --help` 应说明该命令负责启动或连接 App API、托管本地 Web server 并打开浏览器
- help 中应明确 `faus web` 不启动 Vite UI、不执行前端构建
- help 示例应覆盖默认打开 Web 和连接显式 App API base URL 的常见用法

## 与本地脚本的分界

- `faus web` 可以启动或连接产品 runtime，但不替代 bootstrap、doctor、reset、smoke、stop 或后台守护
- 需要后台运行、pid、日志文件或 stop 行为时，仍由 `scripts/local/*` wrapper 承接
- `faus web` 不启动 Vite UI；CLI-hosted Web、开发期 Vite proxy 和前端构建由 [020-frontend-architecture](../020-frontend-architecture/spec.md) 承接

## 当前阶段承接

- 当前阶段实施计划见 [plan.md](./plan.md)
- 当前阶段测试设计见 [testing.md](./testing.md)
- Runtime 启动能力见 [150-faus-serve-cli](../150-faus-serve-cli/spec.md)
- 状态查询能力见 [160-faus-status-cli](../160-faus-status-cli/spec.md)

## 验收标准

- `faus web` 能连接已有 server 并打开对应 Web URL
- 没有显式目标 server 时，`faus web` 能复用 `faus serve` 能力启动默认本机 runtime
- 浏览器打开失败时，命令打印可访问 URL
- `--base-url`、`FAUS_BASE_URL` 与默认值的优先级符合本专题规则
- 尾随斜杠不会改变 App API base URL
- `--json` 成功输出是稳定 JSON 对象
- 本切片不新增 HTTP endpoint，不改变 OpenAPI，不修改 Web 前端实现，不启动 Vite UI
- 本切片依赖 `ui/dist/index.html` 已存在；若 Web assets 未构建，`faus web` 应返回可诊断失败

## 关联主题

- [030-cli](../030-cli/spec.md)
- [150-faus-serve-cli](../150-faus-serve-cli/spec.md)
- [160-faus-status-cli](../160-faus-status-cli/spec.md)
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md)
- [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md)
- [008-ui-ux](../008-ui-ux/spec.md)
- [020-frontend-architecture](../020-frontend-architecture/spec.md)
