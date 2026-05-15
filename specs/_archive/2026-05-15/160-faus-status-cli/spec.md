# 160 faus Status CLI

定义 `faus status` 的具体行为：只读连接已有 Rust server，消费 `/health` 与 `/runtime/status`，展示产品运行面状态。本专题承接 [030-cli](../030-cli/spec.md) 的长期 CLI 方向，并复用 [150-faus-serve-cli](../150-faus-serve-cli/spec.md) 建立的 CLI 入口基础，不承接服务启动、库操作、导入、搜索、任务或 Web 浏览器入口命令。

## 关键术语

- `faus`
- Status CLI
- 基础服务地址
- 轻量 liveness
- 运行时状态
- 人类可读输出
- 机器可读输出

## 范围

- `faus status` 命令行为
- `--base-url`、`--json`、`--debug` 三个全局 flag 在 `status` 命令中的行为
- `FAUS_BASE_URL` 与默认 base URL 的解析优先级
- `/health` 与 `/runtime/status` 的 CLI 消费方式
- `faus status` 的人类可读输出、JSON 输出、连接层错误与服务端错误映射

范围外：

- 服务启动、停止、诊断、日志、pid 与 Qdrant 管理
- 库命令、导入命令、搜索命令、任务命令与 Web 浏览器入口命令
- HTTP endpoint、OpenAPI schema 或服务端 payload 细节的定义
- 严格健康门禁、shell completion、安装器、发布渠道与 man page

## 设计原则

- 只读观察：`faus status` 只读取 Rust server 公开 App API，不修改服务端状态
- 不启动进程：`faus status` 不启动 app、sidecar、Qdrant、Rust server 或 UI，也不调用 `faus serve`
- 双层状态：`/health` 用于确认 app 轻量可达，`/runtime/status` 用于展示 app、Qdrant 与 providers 的运行面状态
- 观察命令退出语义：只要成功取得状态快照，即使某些组件不可用，也以退出码 `0` 返回
- 输出可脚本化：`--json` 输出固定为单个 JSON 对象，不混入人类文案、ANSI 控制字符或日志行
- 契约复用：公开 HTTP 契约由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接，160 不复制 payload schema 细节

## 命令入口

- 本切片要求 `faus status` 可用
- `faus` binary 与 runtime 启动基础由 [150-faus-serve-cli](../150-faus-serve-cli/spec.md) 承接
- `faus` 至少支持以下全局 flag：
  - `--base-url <url>`
  - `--json`
  - `--debug`
- 未支持的命令或参数应由 CLI 参数解析层返回非零退出码，并展示清晰错误

## base URL 规则

- base URL 解析优先级固定为：
  - 显式 `--base-url`
  - 环境变量 `FAUS_BASE_URL`
  - 默认值 `http://127.0.0.1:53210`
- `faus status` 默认不读取 `.env`、`.env.dev`、`FAUNI_ENV_FILE`、`APP_HOST` 或 `APP_PORT`
- `--base-url` 与 `FAUS_BASE_URL` 的尾随斜杠不得影响最终请求路径
- base URL 必须是可解析的 HTTP 或 HTTPS URL
- 无效 base URL 必须返回 CLI 层错误，不尝试修正为其他地址

## `faus status`

- `faus status` 必须请求当前 base URL 下的 `GET /health` 与 `GET /runtime/status`
- 请求路径应基于规范化后的 base URL 拼接，避免双斜杠或遗漏路径分隔符
- `GET /health` 返回轻量 liveness JSON，不要求套 `SuccessEnvelope<T>`
- `GET /runtime/status` 返回运行时状态快照，至少包含 `app`、`qdrant` 与 `providers`
- `faus status` 不直接访问 sidecar、Qdrant、SQLite、runtime 文件或本地脚本
- 本切片不定义轮询、等待、自动恢复或 strict gate 行为

## 人类可读输出

- 默认输出采用“摘要 + 组件”粒度
- 输出应至少展示：
  - 当前 base URL
  - app liveness 状态
  - app runtime 状态
  - Qdrant runtime 状态
  - provider / sidecar 状态概览
- 组件不可用时应展示服务端返回的状态与消息，但不把该状态转成 CLI 执行失败
- 连接失败时应提示目标 base URL 不可达，并提示用户通过 `faus serve` 或本地脚本启动服务
- 连接失败、空响应或非 JSON 响应等 CLI 层错误可以附带 `hint`，帮助用户区分服务未启动、服务仍在启动、端口被占用或目标不是 FauniSearch server

## JSON 输出

`faus status --json` 成功输出必须是单个 JSON 对象：

```json
{
  "status": "ok",
  "data": {
    "base_url": "http://127.0.0.1:53210",
    "health": {},
    "runtime_status": {}
  }
}
```

- `data.base_url` 使用规范化后的 URL
- `data.health` 保留 `/health` 返回的结构化信息
- `data.runtime_status` 保留 `/runtime/status` 响应中的 `data` 对象
- 当 `--debug` 与 `--json` 同时出现时，可以附加 `debug` 对象，用于展示 base URL 来源、请求路径或响应状态码等 CLI 侧信息
- `--json` 输出不得包含 ANSI 控制字符、进度文案或日志行

## 错误输出

- 无效 base URL 是 CLI 层错误，不是服务端 `ErrorEnvelope`
- 连接失败、请求超时、非 JSON 响应或响应契约不匹配属于 CLI 层错误
- 服务端统一错误载荷必须映射到 CLI 错误对象中，不得改写服务端错误语义
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

- `faus status --help` 应说明该命令只连接已有 Rust server，不启动任何本地进程
- help 中应描述 `--base-url`、`--json`、`--debug` 对状态查询的影响
- help 示例应覆盖默认运行面和显式 base URL 的常见用法

## 与本地脚本的分界

- `faus status` 只连接已有 server，不启动本地进程
- `faus serve` 负责产品 runtime 启动，见 [150-faus-serve-cli](../150-faus-serve-cli/spec.md)
- `scripts/local/*` 继续负责服务 stop、状态脚本、doctor、smoke 与本地运行面管理
- `faus status` 是产品运行面观察命令，不替代 `scripts/local/status.sh` 或 `scripts/local/doctor.sh`

## 当前阶段承接

- 当前阶段实施计划见 [plan.md](./plan.md)
- 当前阶段测试设计见 [testing.md](./testing.md)
- Web 浏览器入口命令由 [170-faus-web-cli](../170-faus-web-cli/spec.md) 承接

## 验收标准

- `faus status` 能基于默认 base URL 请求 `/health` 与 `/runtime/status`
- `--base-url`、`FAUS_BASE_URL` 与默认值的优先级符合本专题规则
- 尾随斜杠不会影响最终请求路径
- `--json` 成功输出是稳定 JSON 对象，并保留 health 与 runtime status 结构
- server 可达且状态成功取得时退出码为 `0`，即使 Qdrant 或 provider 显示不可用
- 连接失败、无效 base URL 或响应契约不匹配返回非零退出码
- 本切片不启动本地进程，不新增 HTTP endpoint，不改变 Web 前端实现

## 关联主题

- [030-cli](../030-cli/spec.md)
- [150-faus-serve-cli](../150-faus-serve-cli/spec.md)
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md)
- [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md)
- [170-faus-web-cli](../170-faus-web-cli/spec.md)
