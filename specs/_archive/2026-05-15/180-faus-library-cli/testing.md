# 180 faus Library CLI 测试设计

本文件承接 `180-faus-library-cli` 的测试设计。长期 CLI 规则见 [../030-cli/spec.md](../030-cli/spec.md)，状态命令连接经验见 [../160-faus-status-cli/spec.md](../160-faus-status-cli/spec.md)，当前阶段范围见 [spec.md](./spec.md) 与 [plan.md](./plan.md)。仓库级通用测试规则继续以 [AGENTS.md](../../AGENTS.md) 为准。

## 角色与边界

- 本文件只覆盖 `faus library` 行为
- 本文件不覆盖服务启动、Web 浏览器入口、导入、搜索、任务、source-root 或物理删除命令
- 本文件不覆盖服务端库状态机实现细节
- 本文件不要求启动 sidecar、Qdrant、Rust server 或 UI；HTTP 行为应通过测试 server 或 app test harness 验证

## 测试原则

- CLI 测试优先使用真实 binary 进程验证 stdout、stderr、退出码与环境变量行为
- `--json` 输出必须用 JSON parser 验证，不通过字符串片段猜测结构
- HTTP 行为可以通过本进程 test server 或仓库现有 app test harness 验证，不依赖用户本机服务
- 环境变量测试必须在测试结束后恢复或隔离，避免污染其他测试

## 默认测试入口

- Rust 编译检查：`cargo check --all-targets`
- CLI 二进制单测：`cargo test --bin faus`
- CLI library 窄测试：`cargo test --test faus_cli library`

## 场景矩阵

| 场景 | 预期 |
| --- | --- |
| `faus library --help` | 展示 list/create/show/rename/archive/restore |
| `FAUS_BASE_URL=<test-server> faus library list --json` | 请求 `GET /libraries`，输出 `data.libraries` |
| `--base-url <test-server>/` 与 `FAUS_BASE_URL` 同时存在 | 请求使用 flag 地址，且无双斜杠 |
| `faus library create --display-name Demo --library-id demo --json` | 请求 `POST /libraries`，body 包含 `display_name` 与 `library_id` |
| `faus library show demo --json` | 请求 `GET /libraries/demo`，输出 `data.library` |
| `faus library rename demo --display-name Renamed --json` | 请求 `PATCH /libraries/demo`，body 只包含 `display_name` |
| `faus library archive demo --json` | 请求 `POST /libraries/demo/archive`，输出 `data.library.lifecycle_state` |
| `faus library restore demo --json` | 请求 `POST /libraries/demo/restore`，输出 `data.library.lifecycle_state` |
| 服务端返回 `ErrorEnvelope` | CLI 错误对象保留服务端错误语义 |
| 响应不是 JSON 或缺少必要字段 | 退出码非 0，错误对象表达响应契约不匹配 |
| 运行 `faus library ...` | 不启动 Qdrant、sidecar、Rust server 或 Vite UI |

## JSON 断言

成功 JSON 至少断言：

- `status == "ok"`
- `data.base_url` 等于规范化后的 base URL
- list 输出包含 `data.libraries`
- 单库操作输出包含 `data.library`
- 无多余非 JSON 输出

错误 JSON 至少断言：

- `status == "error"`
- `error.code` 存在且为字符串
- `error.message` 存在且为字符串
- 服务端错误载荷中的 `details` 与 `retryable` 在存在时被保留

## 环境隔离

- 每个涉及 `FAUS_BASE_URL` 的测试必须通过子进程环境隔离
- 测试不得依赖 `.env`、`.env.dev`、`APP_HOST` 或 `APP_PORT`
- 测试不得要求固定本地端口可用
- 测试 server 必须在测试结束后关闭

## Deferred Coverage

- delete 命令
- source-root 管理
- content type / provider override 管理
- import、search、jobs 命令
- shell completion 与 help 文案快照
- package/install 相关验证

## 关联文档

- [spec.md](./spec.md)
- [plan.md](./plan.md)
- [../030-cli/spec.md](../030-cli/spec.md)
- [../009-interfaces-and-protocol-contracts/spec.md](../009-interfaces-and-protocol-contracts/spec.md)
- [../160-faus-status-cli/spec.md](../160-faus-status-cli/spec.md)
- [../010-local-operations-and-automation/spec.md](../010-local-operations-and-automation/spec.md)
