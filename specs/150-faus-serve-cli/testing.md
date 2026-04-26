# 150 faus Serve CLI 测试设计

本文件承接 `150-faus-serve-cli` 的测试设计。长期 CLI 规则见 [../030-cli/spec.md](../030-cli/spec.md)，当前阶段范围见 [spec.md](./spec.md) 与 [plan.md](./plan.md)。仓库级通用测试规则继续以 [AGENTS.md](../../AGENTS.md) 为准。

## 角色与边界

- 本文件只覆盖 `faus serve` 与最小 CLI 入口行为
- 本文件不覆盖状态查询、Web 浏览器入口、库操作、导入、搜索或任务命令
- 本文件不覆盖 Web 前端渲染、静态资源托管或 Vite 开发代理
- 本文件不要求把 bootstrap、doctor、reset、smoke 或后台守护并入 CLI 测试

## 测试原则

- CLI 测试优先使用真实 binary 进程验证 stdout、stderr、退出码与信号处理
- 涉及端口的测试必须使用随机可用端口，避免依赖固定本机状态
- 涉及环境变量的测试必须在结束后恢复或隔离
- 子进程测试必须确保结束后清理 Qdrant、sidecar 与 Rust server 进程
- 能用测试替身覆盖的启动失败场景，不依赖真实外部服务随机失败

## 默认测试入口

- Rust 编译检查：`cargo check --all-targets`
- Rust 测试入口：`cargo test`
- CLI 窄测试可以放在独立 integration test 中，通过 `Command` 运行 `faus` binary
- 长运行启动测试应有超时控制，失败时输出进程日志摘要
- 真实 `faus serve --dev` 验收入口固定为显式 smoke：`bash scripts/local/smoke-faus-serve.sh --dev`
- `smoke-faus-serve.sh` 必须显式传入 `--dev`；不传 `--dev` 应直接失败，避免误触默认 `.env` 运行面

## 场景矩阵

| 场景 | 预期 |
| --- | --- |
| `faus serve --host 127.0.0.1 --port <free-port>` | Rust server 在指定地址 ready，命令保持前台运行 |
| `faus serve --dev --port <free-port>` | 使用开发运行配置启动 runtime |
| `scripts/local/smoke-faus-serve.sh --dev` | 构建并启动 `faus serve --dev`，探测 App / sidecar / Qdrant，确认不启动 Vite，停止后端口释放 |
| `scripts/local/run.sh --dev --detach` | 通过 `faus serve` 启动后端，并额外启动 Vite UI |
| 端口已被占用 | 命令返回非零退出码，并清理本次启动的子进程 |
| Qdrant 启动失败 | 命令返回非零退出码，stderr 展示明确原因 |
| sidecar 启动失败 | 命令返回非零退出码，stderr 展示明确原因 |
| Rust server ready 超时 | 命令返回非零退出码，清理本次启动的子进程 |
| 用户发送中断信号 | 命令退出，并关闭本次启动的子进程 |
| 启动成功 | 输出包含 server base URL 与 OpenAPI URL，不包含 Web URL |
| 启动成功 | 不启动 Vite UI，不要求 Vite 端口可用 |

## 断言重点

- Rust server ready 后，`GET /health` 可访问
- `GET /openapi.json` 可访问，证明公开 App API 已启动
- 命令不会创建或依赖 Vite 开发服务器
- 命令退出后不会遗留本次启动的子进程
- `--host`、`--port` 与 `--dev` 的行为可被测试观察
- `smoke-faus-serve.sh --dev --json` 输出单个机器可读 JSON 摘要，包含 HTTP 探测、Vite 未启动、端口释放与关键输出行检查结果

## 环境隔离

- 测试不得依赖固定本地端口可用
- 测试不得污染用户本机 Qdrant 数据目录
- 测试不得复用用户长期运行的 sidecar 进程
- 每个测试都应使用独立临时 runtime 目录或测试配置
- 测试结束必须恢复修改过的环境变量
- 显式 smoke 例外使用 `.env.dev` 固定端口和 runtime 目录；启动前必须确认 dev app、sidecar、Qdrant 与 UI 端口均未被占用
- 显式 smoke 只停止本次启动的 `faus serve --dev` 及其子进程，不停止默认 `.env` 运行面，不清理长期数据

## Deferred Coverage

- stop、doctor、reset 与 `smoke-faus-serve.sh` 之外的 smoke 脚本
- Web 浏览器打开行为
- shell completion 与 help 文案快照
- package/install 相关验证

## 关联文档

- [spec.md](./spec.md)
- [plan.md](./plan.md)
- [../030-cli/spec.md](../030-cli/spec.md)
- [../009-interfaces-and-protocol-contracts/spec.md](../009-interfaces-and-protocol-contracts/spec.md)
- [../010-local-operations-and-automation/spec.md](../010-local-operations-and-automation/spec.md)
