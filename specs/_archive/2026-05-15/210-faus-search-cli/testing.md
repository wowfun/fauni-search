# 210 faus Search CLI 测试设计

本文件承接 `210-faus-search-cli` 的测试设计。长期 CLI 规则见 [../030-cli/spec.md](../030-cli/spec.md)，状态命令连接经验见 [../160-faus-status-cli/spec.md](../160-faus-status-cli/spec.md)，任务观察能力见 [../190-faus-jobs-cli/spec.md](../190-faus-jobs-cli/spec.md)，当前阶段范围见 [spec.md](./spec.md) 与 [plan.md](./plan.md)。仓库级通用测试规则继续以 [AGENTS.md](../../AGENTS.md) 为准。

## 角色与边界

- 本文件只覆盖 `faus search` 行为
- 本文件不覆盖服务启动、Web 浏览器入口、import、jobs、source-root 或 maintenance 命令
- 本文件不覆盖服务端搜索排序、embedding、Qdrant 查询或结果语义细节
- 本文件不要求启动 sidecar、Qdrant、Rust server 或 UI；HTTP 行为应通过测试 server 或 app test harness 验证

## 测试原则

- CLI 测试优先使用真实 binary 进程验证 stdout、stderr、退出码与环境变量行为
- `--json` 输出必须用 JSON parser 验证，不通过字符串片段猜测结构
- HTTP 行为可以通过本进程 test server 或仓库现有 app test harness 验证，不依赖用户本机服务
- 环境变量测试必须通过子进程环境隔离，避免污染其他测试
- 本地查询文件测试使用临时目录和小型测试文件
- 多输入组合必须测试为 CLI 层 `not_supported`，不发起 HTTP 请求

## 默认测试入口

- Rust 编译检查：`cargo check --all-targets`
- CLI 二进制单测：`cargo test --bin faus`
- CLI search 窄测试：`cargo test --test faus_cli search`

## 场景矩阵

| 场景 | 预期 |
| --- | --- |
| `faus search --help` | 展示 scope、query input flags、常用参数与 examples |
| `faus search --library-id demo --text "query" --json` | 请求 `POST /search/text`，body 中 scope 为单库 |
| `faus search --all-libraries --text "query" --json` | 请求 `POST /search/text`，body 中 scope 为所有库 |
| 文本搜索缺少 scope | 非零退出，不发起 HTTP 请求 |
| `faus search --library-id demo --image image.png --json` | 先上传 image query asset，再请求 `/search/image` |
| `faus search --library-id demo --video clip.mp4 --video-start-ms 1000 --video-end-ms 2000 --json` | 请求体包含 video locator |
| `faus search --library-id demo --document report.pdf --document-start-page 1 --document-end-page 2 --json` | 请求体包含 document locator |
| `faus search --library-id demo --text q --image image.png --json` | 返回 `not_supported`，不发起 HTTP 请求 |
| 非文本输入 + `--all-libraries` | 返回 `not_supported`，不发起 HTTP 请求 |
| `--top-k`、`--cursor`、`--target-content-type` | 进入 search 请求体 |
| `--debug --json` | 输出 CLI debug metadata，search 请求体包含 `debug: true` |
| `--base-url <test-server>/` 与 `FAUS_BASE_URL` 同时存在 | 请求使用 flag 地址，且无双斜杠 |
| 服务端返回 `ErrorEnvelope` | CLI 错误对象保留服务端错误语义 |
| 上传或搜索响应不是 JSON 或缺必要字段 | 退出码非 0，错误对象表达响应契约不匹配 |
| 运行 `faus search ...` | 不启动 Qdrant、sidecar、Rust server 或 Vite UI |

## JSON 断言

成功 JSON 至少断言：

- `status == "ok"`
- `data.base_url` 等于规范化后的 base URL
- `data.search.results` 存在且为数组
- `data.search.next_cursor`、`data.search.unsupported_content_types`、`data.search.debug` 在服务端返回时被保留
- 非文本搜索存在 `data.query_asset.temp_asset_id`
- 无多余非 JSON 输出

错误 JSON 至少断言：

- `status == "error"`
- `error.code` 存在且为字符串
- `error.message` 存在且为字符串
- CLI 层 `not_supported` 包含解释性 `error.hint`
- 服务端错误载荷中的 `details` 与 `retryable` 在存在时被保留

## 环境隔离

- 每个涉及 `FAUS_BASE_URL` 的测试必须通过子进程环境隔离
- 测试不得依赖 `.env`、`.env.dev`、`APP_HOST` 或 `APP_PORT`
- 测试不得要求固定本地端口可用
- 测试 server 必须在测试结束后关闭

## 真实 Dev 验证

可选真实 dev 验证使用 `.env.dev`：

- `bash scripts/local/run.sh --dev --detach`
- 创建或复用测试库
- `target/debug/faus --base-url http://127.0.0.1:54210 --json search --library-id <id> --text "query"`
- 对已有图片、视频、文档文件分别运行本地文件搜索
- `bash scripts/local/stop.sh --dev --all`

## Deferred Coverage

- 组合查询服务端能力
- filters JSON 与常用过滤 flags
- 库内 Asset / Source 复用查询
- 自动分页、watch、tail 或 search history
- shell completion 与 help 文案快照
- package/install 相关验证

## 关联文档

- [spec.md](./spec.md)
- [plan.md](./plan.md)
- [../030-cli/spec.md](../030-cli/spec.md)
- [../009-interfaces-and-protocol-contracts/spec.md](../009-interfaces-and-protocol-contracts/spec.md)
- [../160-faus-status-cli/spec.md](../160-faus-status-cli/spec.md)
- [../190-faus-jobs-cli/spec.md](../190-faus-jobs-cli/spec.md)
- [../200-faus-import-cli/spec.md](../200-faus-import-cli/spec.md)
- [../010-local-operations-and-automation/spec.md](../010-local-operations-and-automation/spec.md)
