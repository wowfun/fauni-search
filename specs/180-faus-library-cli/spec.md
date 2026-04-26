# 180 faus Library CLI

定义 `faus library` 的具体行为：通过 Rust server 公开 App API 承接基础库工作流，包括列出、创建、查看、重命名、归档与恢复库。本专题承接 [030-cli](../030-cli/spec.md) 的长期 CLI 方向，复用 [160-faus-status-cli](../160-faus-status-cli/spec.md) 的连接与错误输出规则，不承接导入、搜索、任务或 source-root 管理。

## 关键术语

- `faus`
- Library CLI
- 基础服务地址
- 库快照
- 生命周期状态
- 人类可读输出
- 机器可读输出

## 范围

- `faus library` 命令组行为
- `--base-url`、`--json`、`--debug` 三个全局 flag 在 library 命令中的行为
- `FAUS_BASE_URL` 与默认 base URL 的解析优先级
- 库列表、创建、查看、重命名、归档与恢复的 CLI 消费方式
- 人类可读输出、JSON 输出、连接层错误与服务端错误映射

范围外：

- 库物理删除命令
- source roots、content types、resolved models、vector-space diagnostics 或 maintenance 命令
- 导入、搜索、任务与 Web 浏览器入口命令
- HTTP endpoint、OpenAPI schema 或服务端 payload 细节的定义
- 服务启动、停止、诊断、日志、pid 与 Qdrant 管理

## 设计原则

- 工作流优先：命令名使用产品动作，保持 `list/create/show/rename/archive/restore`
- 公开 API 优先：CLI 只消费 Rust server App API，不直接访问 SQLite、runtime 文件、Qdrant 或 sidecar
- 输出可脚本化：`--json` 输出固定为单个 JSON 对象，不混入人类文案、ANSI 控制字符或日志行
- 错误语义保留：服务端 `ErrorEnvelope` 映射到 CLI 错误对象，不改写 `code/message/details/retryable`
- 契约复用：公开 HTTP 契约由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接，180 不复制 payload schema 细节

## 命令入口

- 本切片要求 `faus library` 可用
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
- `faus library` 默认不读取 `.env`、`.env.dev`、`FAUNI_ENV_FILE`、`APP_HOST` 或 `APP_PORT`
- `faus library` 连接 App API 时不使用 ambient `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY` 等代理环境变量
- `--base-url` 与 `FAUS_BASE_URL` 的尾随斜杠不得影响最终请求路径
- base URL 必须是可解析的 HTTP 或 HTTPS URL
- 无效 base URL 必须返回 CLI 层错误，不尝试修正为其他地址

## `faus library`

- `faus library list` 请求 `GET /libraries`
- `faus library create --display-name <name> [--library-id <id>]` 请求 `POST /libraries`
- `faus library show <library_id>` 请求 `GET /libraries/{library_id}`
- `faus library rename <library_id> --display-name <name>` 请求 `PATCH /libraries/{library_id}`
- `faus library archive <library_id>` 请求 `POST /libraries/{library_id}/archive`
- `faus library restore <library_id>` 请求 `POST /libraries/{library_id}/restore`
- 请求路径应基于规范化后的 base URL 拼接，避免双斜杠或遗漏路径分隔符
- 本切片不实现 delete 命令；物理删除不属于基础 library CLI 命令面

## 人类可读输出

- `list` 默认按服务端返回顺序逐行展示库摘要
- 每行至少展示：
  - `id`
  - `lifecycle_state`
  - `display_name`
  - `accepted_items`
  - `pending_jobs`
- `create/show/rename/archive/restore` 默认输出单个库摘要
- 空列表应明确展示没有库，而不是静默成功

## JSON 输出

`faus library list --json` 成功输出必须是单个 JSON 对象：

```json
{
  "status": "ok",
  "data": {
    "base_url": "http://127.0.0.1:53210",
    "libraries": []
  }
}
```

`faus library create/show/rename/archive/restore --json` 成功输出必须是单个 JSON 对象：

```json
{
  "status": "ok",
  "data": {
    "base_url": "http://127.0.0.1:53210",
    "library": {}
  }
}
```

- `data.base_url` 使用规范化后的 URL
- `data.libraries` 保留 `GET /libraries` 响应中的库数组
- `data.library` 保留对应单库响应中的库快照对象
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

- `faus library --help` 应说明库命令只消费公开 App API，不启动本地进程
- `list/create/show/rename/archive/restore` 每个子命令应提供简短用途说明
- `create` 与 `rename` 的 help 必须解释 `--display-name`，`create` 还应解释可选 `--library-id`

## 与本地脚本的分界

- `faus library` 只操作产品库资源，不启动本地进程
- `faus serve` 负责产品 runtime 启动，见 [150-faus-serve-cli](../150-faus-serve-cli/spec.md)
- `scripts/local/*` 继续负责服务 stop、状态脚本、doctor、smoke 与本地运行面管理
- `faus library` 不替代 source-root、导入、搜索、任务或维护脚本

## 当前阶段承接

- 当前阶段实施计划见 [plan.md](./plan.md)
- 当前阶段测试设计见 [testing.md](./testing.md)
- 状态查询能力见 [160-faus-status-cli](../160-faus-status-cli/spec.md)

## 验收标准

- `faus library` 暴露 `list/create/show/rename/archive/restore`
- `--base-url`、`FAUS_BASE_URL` 与默认值的优先级符合本专题规则
- 尾随斜杠不会影响最终请求路径
- `--json` 成功输出是稳定 JSON 对象，并保留服务端库快照结构
- 连接失败、无效 base URL 或响应契约不匹配返回非零退出码
- 服务端错误载荷在 CLI 错误对象中保留
- 本切片不启动本地进程，不新增 HTTP endpoint，不改变 Web 前端实现

## 关联主题

- [030-cli](../030-cli/spec.md)
- [150-faus-serve-cli](../150-faus-serve-cli/spec.md)
- [160-faus-status-cli](../160-faus-status-cli/spec.md)
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md)
- [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md)
