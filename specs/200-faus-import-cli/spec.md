# 200 faus Import CLI

定义 `faus import` 的具体行为：通过 Rust server 公开 App API 把本地路径列表提交给指定库的导入接口，并返回服务端的 accepted / rejected / job 信息。本专题承接 [030-cli](../030-cli/spec.md) 的长期 CLI 方向，复用 [160-faus-status-cli](../160-faus-status-cli/spec.md) 的连接与错误输出规则，并把任务观察交给 [190-faus-jobs-cli](../190-faus-jobs-cli/spec.md)。

## 关键术语

- `faus`
- Import CLI
- 基础服务地址
- 本地路径列表
- 导入回执
- 任务摘要
- 人类可读输出
- 机器可读输出

## 范围

- `faus import` 命令行为
- `--base-url`、`--json`、`--debug` 三个全局 flag 在 import 命令中的行为
- `FAUS_BASE_URL` 与默认 base URL 的解析优先级
- 本地路径参数到导入请求 payload 的 CLI 侧处理
- 人类可读输出、JSON 输出、连接层错误与服务端错误映射

范围外：

- `--wait`、轮询、watch、tail 或 job log
- source-root、refresh、rescan、maintenance 或 search 命令
- 文件类型筛选、文件存在性检查、递归扫描或上传式导入
- HTTP endpoint、OpenAPI schema 或服务端 payload 细节的定义
- 服务启动、停止、诊断、日志、pid 与 Qdrant 管理

## 设计原则

- 提交即返回：`faus import` 只提交导入请求并展示服务端回执，不等待索引完成
- 公开 API 优先：CLI 只消费 Rust server App API，不直接访问 SQLite、runtime 文件、Qdrant 或 sidecar
- 路径语义清晰：相对路径按当前 shell cwd 转成绝对路径发送，避免 server cwd 差异造成误导
- 输出可脚本化：`--json` 输出固定为单个 JSON 对象，不混入人类文案、ANSI 控制字符或日志行
- 错误语义保留：服务端 `ErrorEnvelope` 映射到 CLI 错误对象，不改写 `code/message/details/retryable`
- 契约复用：公开 HTTP 契约由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接，200 不复制 payload schema 细节

## 命令入口

- 本切片要求 `faus import` 可用
- 命令形态固定为 `faus import --library-id <library_id> <path>...`
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
- `faus import` 默认不读取 `.env`、`.env.dev`、`FAUNI_ENV_FILE`、`APP_HOST` 或 `APP_PORT`
- `faus import` 连接 App API 时不使用 ambient `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY` 等代理环境变量
- `--base-url` 与 `FAUS_BASE_URL` 的尾随斜杠不得影响最终请求路径
- base URL 必须是可解析的 HTTP 或 HTTPS URL
- 无效 base URL 必须返回 CLI 层错误，不尝试修正为其他地址

## `faus import`

- `faus import --library-id <library_id> <path>...` 请求 `POST /libraries/{library_id}/imports`
- 请求 body 使用本地路径列表：`{"paths":["/absolute/path"]}`
- 相对路径必须按当前 shell cwd 转为绝对路径后发送
- 绝对路径应原样发送
- CLI 不检查路径是否存在，不筛选文件类型，不递归展开目录，也不展开 quoted `~`
- 服务端负责决定每个路径进入 `accepted` 还是 `rejected`
- 请求路径应基于规范化后的 base URL 拼接，避免双斜杠或遗漏路径分隔符
- 本切片不实现 `faus import paths`、`faus library import` 或 `--wait`

## 人类可读输出

- 默认输出导入提交摘要
- 摘要至少展示：
  - accepted 数量
  - rejected 数量
  - job id / status / phase（当服务端返回 job 时）
- rejected 项应逐行展示原始路径、`reason_code` 与 `message`
- 当服务端没有返回 job handle 时，输出应明确表达没有排队任务

## JSON 输出

`faus import --library-id demo file.pdf --json` 成功输出必须是单个 JSON 对象：

```json
{
  "status": "ok",
  "data": {
    "base_url": "http://127.0.0.1:53210",
    "import": {}
  }
}
```

- `data.base_url` 使用规范化后的 URL
- `data.import` 保留服务端 `SuccessEnvelope.data` 原结构，包括 `accepted`、`rejected`、`job_handle` 与 `job`
- 当 `--debug` 与 `--json` 同时出现时，可以附加 `debug` 对象，用于展示 base URL 来源、请求路径或响应状态码等 CLI 侧信息
- `--json` 输出不得包含 ANSI 控制字符、进度文案或日志行

## 错误输出

- 无效 base URL 是 CLI 层错误，不是服务端 `ErrorEnvelope`
- 连接失败、请求超时、非 JSON 响应或响应契约不匹配属于 CLI 层错误
- 服务端统一错误载荷必须映射到 CLI 错误对象中，不得改写服务端错误语义
- CLI 层错误可以附带 `hint`，用于提示用户启动 `faus serve`、检查显式 base URL、等待服务 ready 或确认目标是否为 FauniSearch server
- `--json` 下的错误输出必须是单个 JSON 对象，并可在 `error.hint` 与 `error.details` 中提供诊断上下文
- 人类可读错误应写入 stderr，并返回非零退出码

## Help 文案

- `faus import --help` 应说明 import 只提交本地路径到公开 App API，不启动本地进程
- help 必须解释 `--library-id`
- help 必须展示 `<path>...` 是一个或多个本地路径

## 与本地脚本的分界

- `faus import` 只提交产品导入请求，不启动本地进程
- `faus serve` 负责产品 runtime 启动，见 [150-faus-serve-cli](../150-faus-serve-cli/spec.md)
- `faus jobs` 负责观察或处理后台任务，见 [190-faus-jobs-cli](../190-faus-jobs-cli/spec.md)
- `scripts/local/*` 继续负责服务 stop、状态脚本、doctor、smoke 与本地运行面管理

## 当前阶段承接

- 当前阶段实施计划见 [plan.md](./plan.md)
- 当前阶段测试设计见 [testing.md](./testing.md)
- 状态查询能力见 [160-faus-status-cli](../160-faus-status-cli/spec.md)
- 任务观察能力见 [190-faus-jobs-cli](../190-faus-jobs-cli/spec.md)

## 验收标准

- `faus import --library-id <library_id> <path>...` 可用
- `--base-url`、`FAUS_BASE_URL` 与默认值的优先级符合本专题规则
- 尾随斜杠不会影响最终请求路径
- 相对路径按当前 cwd 转为绝对路径发送
- `--json` 成功输出是稳定 JSON 对象，并保留服务端导入回执结构
- 连接失败、无效 base URL 或响应契约不匹配返回非零退出码
- 服务端错误载荷在 CLI 错误对象中保留
- 本切片不启动本地进程，不新增 HTTP endpoint，不改变 Web 前端实现

## 关联主题

- [030-cli](../030-cli/spec.md)
- [150-faus-serve-cli](../150-faus-serve-cli/spec.md)
- [160-faus-status-cli](../160-faus-status-cli/spec.md)
- [190-faus-jobs-cli](../190-faus-jobs-cli/spec.md)
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md)
- [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md)
