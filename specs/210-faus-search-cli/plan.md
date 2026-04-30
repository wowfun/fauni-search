# 210 faus Search CLI 当前阶段计划

本计划承接 [spec.md](./spec.md)，只规划 `faus search` 的基础显式范围与本地文件查询能力。CLI binary、连接规则与错误输出复用既有 `faus` 基础，公开接口契约继续由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接。本切片不启动本地服务，不实现 filters 或库内对象复用输入。

## 概要

- 当前阶段实现 flag-based `faus search`
- 当前阶段复用全局 `--base-url`、`--json`、`--debug`
- 当前阶段只消费现有 query asset upload 与 search endpoints
- 当前阶段不新增 HTTP endpoint，不改变 OpenAPI contract
- 当前阶段只允许一个 query input；多个输入返回 `not_supported`

## 实现计划

### 1. CLI 入口

- 复用既有 `faus` binary
- 新增 `src/bin/faus/search.rs`，只承接 search 命令的参数、HTTP 调用和输出组织
- 在 `src/bin/faus/main.rs` 中接入 `Commands::Search(SearchArgs)`
- 不提前抽象为 crate-level 模块

### 2. 参数解析

- 使用 `clap derive` 结构补充：
  - `--library-id <library_id>`
  - `--all-libraries`
  - `--text <text>`
  - `--image <path>`
  - `--video <path>`
  - `--document <path>`
  - `--top-k <n>`
  - `--cursor <cursor>`
  - `--target-content-type <type>`，可重复
  - `--video-start-ms <ms>` / `--video-end-ms <ms>`
  - `--document-start-page <n>` / `--document-end-page <n>`
- 运行期校验 scope：必须且只能给出 `--library-id` 或 `--all-libraries`
- 运行期校验 query input：必须至少一个；多个返回 `not_supported`
- 非文本输入与 `--all-libraries` 返回 `not_supported`
- `--top-k 0` 与 locator 起止只给一端返回 CLI 层错误

### 3. HTTP 调用

- 复用 `resolve_base_url` 和 JSON helper
- 为 query asset upload 增加 CLI 内部 multipart helper
- text 直接 `POST /search/text`
- image/video/document 先上传本地文件，再用返回的 `temp_asset_id` 调用对应 search endpoint
- 上传成功要求响应包含 `data.temp_asset_id`
- 搜索成功要求响应包含 `data.results` 数组
- 服务端 `ErrorEnvelope` 保持现有 CLI 错误映射

### 4. 请求体

- search scope 使用结构化对象：
  - 单库：`{"kind":"library","library_id":"..."}`
  - 所有库：`{"kind":"all_libraries"}`
- 文本搜索体包含 `text` 与 `search_scope`
- 非文本搜索体包含 `library_id`、`search_scope.kind=library` 与对应 `*_input.kind=temp_asset`
- `--debug` 使 search 请求体包含 `debug: true`
- `--top-k`、`--cursor`、`--target-content-type` 只在用户传入时写入
- 视频和文档 locator flags 只在 start/end 成对出现时写入

### 5. 输出

- 人类可读输出展示结果数量、`next_cursor` 与逐条结果摘要
- `--json` 输出 `status: "ok"`、`data.base_url`、`data.search`
- 非文本搜索 `--json` 额外输出 `data.query_asset`
- `--debug --json` 附加 base URL 来源、上传请求 URL、搜索请求 URL 与 HTTP status

### 6. 发布记录

- 代码实现落地后更新 `CHANGELOG.md`
- 规格创建本身不更新 `CHANGELOG.md`

## 当前阶段约束

- 不新增 HTTP endpoint
- 不改变 OpenAPI contract 的 schema 语义
- 不修改 `specs/README.md`
- 不启动本地服务，不调用 `faus serve`
- 不实现组合查询 API、filters JSON、复杂过滤 flags、库内对象复用输入或分页自动拉取
- 不实现 `faus search jobs` 或 `--wait`

## Deferred

- 组合查询服务端能力
- filters JSON 与常用过滤 flags
- 库内 Asset / Source 复用查询
- 自动分页、watch、tail 或 search history
- shell completion 与 man page
- search 命令真实 dev smoke 脚本

## 阶段验收摘要

- `faus search --help` 展示 scope、query input flags 与示例
- `faus search --library-id demo --text "query" --json` 请求正确 body
- `faus search --all-libraries --text "query" --json` 请求所有库范围
- `faus search --library-id demo --image|--video|--document <path> --json` 先上传再搜索
- 多 query input 返回 `not_supported`
- 非文本 + `--all-libraries` 返回 `not_supported`
- `FAUS_BASE_URL` 能覆盖默认值
- `--base-url` 能覆盖环境变量
- 尾随斜杠不影响请求路径
- `--json` 输出稳定机器可读对象
- 连接失败、无效 base URL、服务端错误和响应契约不匹配返回稳定 CLI 错误

详细测试分层与场景矩阵见 [testing.md](./testing.md)。
