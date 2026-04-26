# 200 faus Import CLI 测试设计

本文件承接 `200-faus-import-cli` 的测试设计。长期 CLI 规则见 [../030-cli/spec.md](../030-cli/spec.md)，状态命令连接经验见 [../160-faus-status-cli/spec.md](../160-faus-status-cli/spec.md)，任务观察能力见 [../190-faus-jobs-cli/spec.md](../190-faus-jobs-cli/spec.md)，当前阶段范围见 [spec.md](./spec.md) 与 [plan.md](./plan.md)。仓库级通用测试规则继续以 [AGENTS.md](../../AGENTS.md) 为准。

## 角色与边界

- 本文件只覆盖 `faus import` 行为
- 本文件不覆盖服务启动、Web 浏览器入口、搜索、source-root 或 maintenance 命令
- 本文件不覆盖服务端导入分类、文件类型识别或索引执行细节
- 本文件不要求启动 sidecar、Qdrant、Rust server 或 UI；HTTP 行为应通过测试 server 或 app test harness 验证

## 测试原则

- CLI 测试优先使用真实 binary 进程验证 stdout、stderr、退出码与环境变量行为
- `--json` 输出必须用 JSON parser 验证，不通过字符串片段猜测结构
- HTTP 行为可以通过本进程 test server 或仓库现有 app test harness 验证，不依赖用户本机服务
- 环境变量测试必须通过子进程环境隔离，避免污染其他测试
- 路径测试不得要求真实文件存在，除非验证真实 dev 运行面

## 默认测试入口

- Rust 编译检查：`cargo check --all-targets`
- CLI 二进制单测：`cargo test --bin faus`
- CLI import 窄测试：`cargo test --test faus_cli import`

## 场景矩阵

| 场景 | 预期 |
| --- | --- |
| `faus import --help` | 展示 `--library-id` 与 `<path>...` |
| `FAUS_BASE_URL=<test-server> faus import --library-id demo file.pdf --json` | 请求 `POST /libraries/demo/imports`，输出 `data.import` |
| 相对路径输入 | body 中路径按当前 cwd 转为绝对路径 |
| 绝对路径输入 | body 中路径原样发送 |
| 多路径输入 | body 中路径顺序与用户输入一致 |
| `--base-url <test-server>/` 与 `FAUS_BASE_URL` 同时存在 | 请求使用 flag 地址，且无双斜杠 |
| 服务端返回 `ErrorEnvelope` | CLI 错误对象保留服务端错误语义 |
| 响应不是 JSON 或缺少 `data.accepted` / `data.rejected` | 退出码非 0，错误对象表达响应契约不匹配 |
| 运行 `faus import ...` | 不启动 Qdrant、sidecar、Rust server 或 Vite UI |

## JSON 断言

成功 JSON 至少断言：

- `status == "ok"`
- `data.base_url` 等于规范化后的 base URL
- `data.import.accepted` 存在且为数组
- `data.import.rejected` 存在且为数组
- `data.import.job_handle` 与 `data.import.job` 在服务端返回时被保留
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

## 真实 Dev 验证

可选真实 dev 验证使用 `.env.dev`：

- `bash scripts/local/run.sh --dev --detach`
- 创建或复用测试库
- `target/debug/faus --base-url http://127.0.0.1:54210 --json import --library-id <id> <file>`
- 使用 `target/debug/faus --base-url http://127.0.0.1:54210 --json jobs list` 或 `jobs show` 验证返回 job 可观察
- `bash scripts/local/stop.sh --dev --all`

## Deferred Coverage

- `--wait` / watch / tail 导入任务观察模式
- 上传式导入或远程导入输入变体
- 文件存在性检查、文件类型筛选或递归扫描
- source-root 导入管理
- shell completion 与 help 文案快照
- package/install 相关验证

## 关联文档

- [spec.md](./spec.md)
- [plan.md](./plan.md)
- [../030-cli/spec.md](../030-cli/spec.md)
- [../009-interfaces-and-protocol-contracts/spec.md](../009-interfaces-and-protocol-contracts/spec.md)
- [../160-faus-status-cli/spec.md](../160-faus-status-cli/spec.md)
- [../190-faus-jobs-cli/spec.md](../190-faus-jobs-cli/spec.md)
- [../010-local-operations-and-automation/spec.md](../010-local-operations-and-automation/spec.md)
