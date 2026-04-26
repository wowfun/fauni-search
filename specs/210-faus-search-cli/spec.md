# 210 faus Search CLI

定义 `faus search` 的具体行为：通过 Rust server 公开 App API 执行文本、图片、视频与文档搜索。命令采用 flag-based 查询输入，而不是 `text|image|video|document` 子命令；首版只执行单一输入搜索，但命令形态为未来组合查询保留空间。

本专题承接 [030-cli](../030-cli/spec.md) 的长期 CLI 方向，复用 [160-faus-status-cli](../160-faus-status-cli/spec.md) 的连接与错误输出规则。

## 关键术语

- `faus`
- Search CLI
- Flag-based 查询输入
- 搜索范围
- Query asset
- 本地查询文件
- 组合查询
- 搜索结果
- 人类可读输出
- 机器可读输出

## 范围

- `faus search` 命令行为
- `--base-url`、`--json`、`--debug` 三个全局 flag 在 search 命令中的行为
- `FAUS_BASE_URL` 与默认 base URL 的解析优先级
- `--text`、`--image`、`--video`、`--document` 查询输入 flag
- 本地图片、视频与文档文件到 query asset upload 的 CLI 侧处理
- 人类可读输出、JSON 输出、连接层错误与服务端错误映射

范围外：

- 组合查询的服务端能力实现
- 库内对象复用输入
- filters JSON 或复杂过滤 flag
- `--wait`、watch、分页自动拉取、tail 或 job log
- HTTP endpoint、OpenAPI schema 或服务端搜索语义的定义
- 服务启动、停止、诊断、日志、pid 与 Qdrant 管理

## 设计原则

- 命令形态稳定：`faus search` 通过输入 flag 表达查询类型，未来组合查询不需要新增主子命令
- 显式范围：搜索命令不得隐式猜测用户要搜哪个库或所有库
- 单输入先行：首版只允许一个 query input；多个输入返回 CLI 层 `not_supported`
- 本地文件优先：非文本搜索首版只承接本地文件上传查询，库内对象复用留给后续切片
- 公开 API 优先：CLI 只消费 Rust server App API，不直接访问 SQLite、runtime 文件、Qdrant 或 sidecar
- 输出可脚本化：`--json` 输出固定为单个 JSON 对象，不混入人类文案、ANSI 控制字符或日志行
- 搜索载荷保真：JSON 输出必须保留服务端 search `data` 原结构，包括 `results`、`next_cursor`、`unsupported_content_types` 与 `debug`
- 契约复用：公开 HTTP 契约由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接，210 不复制 search schema 细节

## 命令入口

- 本切片要求 `faus search` 可用
- Canonical 命令形态固定为 `faus search [scope] [query input flags]`
- `faus` 至少支持以下全局 flag：
  - `--base-url <url>`
  - `--json`
  - `--debug`
- `faus search` 支持以下搜索范围 flag，二选一：
  - `--library-id <library_id>`
  - `--all-libraries`
- `faus search` 支持以下 query input flag：
  - `--text <text>`
  - `--image <path>`
  - `--video <path>`
  - `--document <path>`
- `faus search` 共享以下局部 flag：
  - `--top-k <n>`
  - `--cursor <cursor>`
  - `--target-content-type <type>`，可重复
  - `--video-start-ms <ms>`
  - `--video-end-ms <ms>`
  - `--document-start-page <n>`
  - `--document-end-page <n>`
- 参数解析层应允许多个 query input flag 同时出现；执行层在首版返回 `not_supported`，以保留未来组合查询命令形态

## base URL 规则

- base URL 解析优先级固定为：
  - 显式 `--base-url`
  - 环境变量 `FAUS_BASE_URL`
  - 默认值 `http://127.0.0.1:53210`
- `faus search` 默认不读取 `.env`、`.env.dev`、`FAUNI_ENV_FILE`、`APP_HOST` 或 `APP_PORT`
- `faus search` 连接 App API 时不使用 ambient `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY` 等代理环境变量
- `--base-url` 与 `FAUS_BASE_URL` 的尾随斜杠不得影响最终请求路径
- base URL 必须是可解析的 HTTP 或 HTTPS URL
- 无效 base URL 必须返回 CLI 层错误，不尝试修正为其他地址

## 搜索执行规则

- 搜索范围必须显式给出，且 `--library-id` 与 `--all-libraries` 不得同时出现
- 查询输入必须至少给出一个
- 首版只允许一个 query input；多个输入返回 `not_supported`，hint 说明组合查询尚未启用
- `--text` 支持 `--library-id` 与 `--all-libraries`
- `--image`、`--video`、`--document` 只支持 `--library-id`
- 非文本输入与 `--all-libraries` 组合时返回 `not_supported`
- `--top-k` 必须大于 0
- `--debug` 出现时，search 请求体必须携带 `debug: true`
- `--top-k`、`--cursor`、`--target-content-type` 进入对应 search 请求体
- 视频 `--video-start-ms` / `--video-end-ms` 同时出现时写入 `video_input.locator`；只出现一端是 CLI 层错误
- 文档 `--document-start-page` / `--document-end-page` 同时出现时写入 `document_input.locator`；只出现一端是 CLI 层错误
- multipart 上传字段固定为 `file`，文件名取本地路径 basename
- CLI 读取本地查询文件用于上传；路径不存在或无法读取是 CLI 层错误

## HTTP 调用

- 文本搜索请求 `POST /search/text`
- 图片搜索先请求 `POST /libraries/{library_id}/query-assets/images`，再请求 `POST /search/image`
- 视频搜索先请求 `POST /libraries/{library_id}/query-assets/videos`，再请求 `POST /search/video`
- 文档搜索先请求 `POST /libraries/{library_id}/query-assets/documents`，再请求 `POST /search/document`
- 非文本搜索的 search 请求体使用上传返回的 `temp_asset_id`
- 上传成功要求响应包含 `data.temp_asset_id`
- 搜索成功要求响应包含 `data.results` 数组
- 服务端统一错误载荷必须映射到 CLI 错误对象中，不得改写服务端错误语义

## 示例

- `faus search --library-id demo --text "terminal screen"`
- `faus search --all-libraries --text "quarterly report"`
- `faus search --library-id demo --image ./query.png`
- `faus search --library-id demo --video ./clip.mp4 --video-start-ms 42000 --video-end-ms 50000`
- `faus search --library-id demo --document ./report.pdf --document-start-page 1 --document-end-page 3`
- `faus search --library-id demo --text "terminal" --image ./query.png` 当前返回 `not_supported`

## 人类可读输出

- 默认输出搜索结果摘要
- 摘要至少展示：
  - result 数量
  - `next_cursor`（存在时）
  - 每条结果的 `library_id`、`kind`、score、locator、source path 与 preview URL
- 没有结果时应输出 `No results.`
- 非文本搜索可以额外展示 query asset id

## JSON 输出

`faus search --library-id demo --text "query" --json` 成功输出必须是单个 JSON 对象：

```json
{
  "status": "ok",
  "data": {
    "base_url": "http://127.0.0.1:53210",
    "search": {}
  }
}
```

- `data.base_url` 使用规范化后的 URL
- `data.search` 保留服务端 `SuccessEnvelope.data` 原结构
- 非文本搜索成功时，`data.query_asset` 保留上传端点返回的 `data` 原结构
- 当 `--debug` 与 `--json` 同时出现时，可以附加 `debug` 对象，用于展示 base URL 来源、上传请求、搜索请求与响应状态码等 CLI 侧信息
- `--json` 输出不得包含 ANSI 控制字符、进度文案或日志行

## 错误输出

- 缺失显式搜索范围、缺失查询输入、`--top-k 0` 或 locator flag 只出现一端是 CLI 层错误，不发起 HTTP 请求
- 多个 query input 或非文本 `--all-libraries` 返回 CLI 层 `not_supported`
- 无效 base URL、本地查询文件无法读取、连接失败、请求超时、非 JSON 响应或响应契约不匹配属于 CLI 层错误
- 服务端统一错误载荷必须映射到 CLI 错误对象中，不得改写服务端错误语义
- CLI 层错误可以附带 `hint`，用于提示用户启动 `faus serve`、检查显式 base URL、等待服务 ready、确认目标是否为 FauniSearch server，或说明组合查询尚未启用
- `--json` 下的错误输出必须是单个 JSON 对象，并可在 `error.hint` 与 `error.details` 中提供诊断上下文
- 人类可读错误应写入 stderr，并返回非零退出码

## Help 文案

- `faus search --help` 应展示 scope、query input flags、常用参数与示例
- help 必须说明 search 命令只消费公开 App API，不启动本地进程
- help 必须说明当前组合查询尚未启用，但命令形态已经预留

## 与其他命令的分界

- `faus search` 只执行产品搜索请求，不启动本地进程
- `faus import` 负责把本地路径提交为库内容，见 [200-faus-import-cli](../200-faus-import-cli/spec.md)
- `faus jobs` 负责观察或处理后台任务，见 [190-faus-jobs-cli](../190-faus-jobs-cli/spec.md)
- `scripts/local/*` 继续负责服务 stop、状态脚本、doctor、smoke 与本地运行面管理

## 当前阶段承接

- 当前阶段实施计划见 [plan.md](./plan.md)
- 当前阶段测试设计见 [testing.md](./testing.md)
- 状态查询能力见 [160-faus-status-cli](../160-faus-status-cli/spec.md)
- 任务观察能力见 [190-faus-jobs-cli](../190-faus-jobs-cli/spec.md)

## 验收标准

- `faus search` 可用，且不提供 `text|image|video|document` 子命令作为 canonical 形态
- 搜索必须显式指定单库或所有库范围
- text 搜索支持单库和所有库
- 图片、视频、文档搜索必须显式指定 `--library-id`
- 图片、视频、文档搜索先上传本地查询文件，再调用对应搜索端点
- 多输入组合当前返回 `not_supported`
- `--base-url`、`FAUS_BASE_URL` 与默认值的优先级符合本专题规则
- `--json` 成功输出是稳定 JSON 对象，并保留服务端搜索结构
- 连接失败、无效 base URL、本地文件无法读取或响应契约不匹配返回非零退出码
- 服务端错误载荷在 CLI 错误对象中保留
- 本切片不启动本地进程，不新增 HTTP endpoint，不改变 Web 前端实现

## 关联主题

- [030-cli](../030-cli/spec.md)
- [150-faus-serve-cli](../150-faus-serve-cli/spec.md)
- [160-faus-status-cli](../160-faus-status-cli/spec.md)
- [190-faus-jobs-cli](../190-faus-jobs-cli/spec.md)
- [200-faus-import-cli](../200-faus-import-cli/spec.md)
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md)
- [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md)
