# 160 faus Status CLI 测试设计

本文件承接 `160-faus-status-cli` 的测试设计。长期 CLI 规则见 [../030-cli/spec.md](../030-cli/spec.md)，CLI 启动入口见 [../150-faus-serve-cli/spec.md](../150-faus-serve-cli/spec.md)，当前阶段范围见 [spec.md](./spec.md) 与 [plan.md](./plan.md)。仓库级通用测试规则继续以 [AGENTS.md](../../AGENTS.md) 为准。

## 角色与边界

- 本文件只覆盖 `faus status` 行为
- 本文件不覆盖服务启动、Web 浏览器入口、库操作、导入、搜索或任务命令
- 本文件不覆盖服务端健康判定算法、Qdrant 行为或 provider 探测实现
- 本文件不要求启动 sidecar、Qdrant、Rust server 或 UI；HTTP 行为应通过测试 server 或 app test harness 验证

## 测试原则

- CLI 测试优先使用真实 binary 进程验证 stdout、stderr、退出码与环境变量行为
- `--json` 输出必须用 JSON parser 验证，不通过字符串片段猜测结构
- HTTP 行为可以通过本进程 test server 或仓库现有 app test harness 验证，不依赖用户本机服务
- 环境变量测试必须在测试结束后恢复或隔离，避免污染其他测试

## 默认测试入口

- Rust 编译检查：`cargo check --all-targets`
- Rust 测试入口：`cargo test`
- CLI 窄测试可以放在独立 integration test 中，通过 `Command` 运行 `faus` binary

## 场景矩阵

| 场景 | 预期 |
| --- | --- |
| `faus status` 无参数，测试 server 返回 health 与 runtime status | stdout 展示 base URL 与组件摘要，退出码为 0 |
| `FAUS_BASE_URL` 指向测试 server | 请求使用环境变量地址 |
| `FAUS_BASE_URL` 与 `--base-url` 同时存在 | 请求使用 `--base-url` |
| base URL 带尾随斜杠 | 请求路径仍为 `/health` 与 `/runtime/status` |
| `faus --json status` 或等价参数顺序 | stdout 是单个 JSON 对象，`status` 为 `ok` |
| `faus --debug --json status` | stdout 包含 CLI 侧 debug 信息 |
| runtime status 中 Qdrant 或 provider 不可用 | 命令退出码仍为 0，输出保留组件状态 |
| base URL 无效 | 退出码非 0，stderr 或 JSON 错误对象包含 `invalid_base_url` |
| server 不可达 | 退出码非 0，错误对象使用连接层错误码 |
| `/health` 或 `/runtime/status` 返回服务端错误载荷 | CLI 错误对象保留服务端错误语义 |
| 响应不是 JSON 或缺少必要字段 | 退出码非 0，错误对象表达响应契约不匹配 |
| 运行 `faus status` | 不启动 Qdrant、sidecar、Rust server 或 Vite UI |

## JSON 断言

成功 JSON 至少断言：

- `status == "ok"`
- `data.base_url` 等于规范化后的 base URL
- `data.health` 保留 `/health` 返回对象
- `data.runtime_status` 保留 `/runtime/status` 响应中的 `data` 对象
- 无多余非 JSON 输出

错误 JSON 至少断言：

- `status == "error"`
- `error.code` 存在且为字符串
- `error.message` 存在且为字符串
- 服务端错误载荷中的 `details` 与 `retryable` 在存在时被保留

## 环境隔离

- 每个涉及 `FAUS_BASE_URL` 的测试必须保存原值并在结束后恢复
- 测试不得依赖 `.env`、`.env.dev`、`APP_HOST` 或 `APP_PORT`
- 测试不得要求固定本地端口可用
- 测试 server 必须在测试结束后关闭

## Deferred Coverage

- 严格健康门禁行为
- 轮询等待或自动恢复
- Web 浏览器入口命令
- shell completion 与 help 文案快照
- package/install 相关验证

## 关联文档

- [spec.md](./spec.md)
- [plan.md](./plan.md)
- [../150-faus-serve-cli/spec.md](../150-faus-serve-cli/spec.md)
- [../030-cli/spec.md](../030-cli/spec.md)
- [../009-interfaces-and-protocol-contracts/spec.md](../009-interfaces-and-protocol-contracts/spec.md)
- [../010-local-operations-and-automation/spec.md](../010-local-operations-and-automation/spec.md)
