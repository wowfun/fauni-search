# 170 faus Web CLI 测试设计

本文件承接 `170-faus-web-cli` 的测试设计。长期 CLI 规则见 [../030-cli/spec.md](../030-cli/spec.md)，runtime 启动能力见 [../150-faus-serve-cli/spec.md](../150-faus-serve-cli/spec.md)，状态连接能力见 [../160-faus-status-cli/spec.md](../160-faus-status-cli/spec.md)，当前阶段范围见 [spec.md](./spec.md) 与 [plan.md](./plan.md)。仓库级通用测试规则继续以 [AGENTS.md](../../AGENTS.md) 为准。

## 角色与边界

- 本文件只覆盖 `faus web` 行为
- 本文件不覆盖状态查询、库操作、导入、搜索或任务命令
- 本文件不覆盖 Web 前端渲染或 Vite 开发代理；`faus web` 的本地 Web server 静态托管和 API proxy 需要窄测试覆盖
- 本文件不要求把 bootstrap、doctor、reset、smoke 或后台守护并入 CLI 测试

## 测试原则

- CLI 测试优先使用真实 binary 进程验证 stdout、stderr、退出码与环境变量行为
- `--json` 输出必须用 JSON parser 验证，不通过字符串片段猜测结构
- 浏览器打开应通过可替换 opener 或测试替身验证，避免在测试中真实打开用户浏览器
- CLI integration test 可使用内部测试环境变量驱动 opener 成功或失败；该变量不属于公开 CLI 接口
- 环境变量测试必须在测试结束后恢复或隔离，避免污染其他测试
- 启动默认本机 runtime 的测试必须有超时控制，并清理本次启动的子进程

## 默认测试入口

- Rust 编译检查：`cargo check --all-targets`
- Rust 测试入口：`cargo test`
- CLI 窄测试可以放在独立 integration test 中，通过 `Command` 运行 `faus` binary

## 场景矩阵

| 场景 | 预期 |
| --- | --- |
| `faus web --base-url <test-server>` | 启动本地 Web server，打开 Web URL，退出码为 0 |
| `FAUS_BASE_URL=<test-server> faus web` | 使用环境变量地址 |
| `FAUS_BASE_URL` 与 `--base-url` 同时存在 | 使用 `--base-url` |
| base URL 带尾随斜杠 | App API base URL 使用规范化后的根 URL |
| 连接已有 server | 请求 `/health` 与根路径 `/`，不请求 `/runtime/status` |
| `faus --json web --base-url <test-server>` | stdout 是单个 JSON 对象，`status` 为 `ok` |
| `faus --debug --json web --base-url <test-server>` | stdout 包含 CLI 侧 debug 信息 |
| 浏览器 opener 失败 | stdout 或 stderr 打印 Web URL，server 成功时命令仍可成功退出 |
| `ui/dist/index.html` 缺失 | 退出码非 0，并报告可诊断失败，不改开 `/routes` |
| 显式 base URL 无效 | 退出码非 0，错误对象包含 `invalid_base_url` |
| 显式目标 server 不可达 | 退出码非 0，错误对象使用连接层错误码 |
| 未提供显式目标且默认地址不可达 | 命令复用 `faus serve` 启动默认本机 runtime |
| `faus web` 启动 runtime 后收到中断信号 | 命令退出并清理本次启动的子进程 |
| 运行 `faus web` | 不启动 Vite UI |

## JSON 断言

成功 JSON 至少断言：

- `status == "ok"`
- `data.base_url` 等于规范化后的 base URL
- `data.web_url` 等于本地 Web server URL
- `data.opened` 为布尔值
- `data.server_started` 为布尔值
- 无多余非 JSON 输出

错误 JSON 至少断言：

- `status == "error"`
- `error.code` 存在且为字符串
- `error.message` 存在且为字符串

## 环境隔离

- 每个涉及 `FAUS_BASE_URL` 的测试必须保存原值并在结束后恢复
- 测试不得依赖 `.env`、`.env.dev`、`APP_HOST` 或 `APP_PORT`
- 测试不得要求固定本地端口可用
- 浏览器 opener 测试不得真实修改用户桌面状态
- `faus web` 的连接型测试应使用测试 HTTP server 返回 `/health` 与 HTML 根路径，不启动真实 runtime
- 启动 runtime 的测试必须使用隔离 runtime 目录，并在结束后清理

## Deferred Coverage

- Web 前端真实页面渲染
- shell completion 与 help 文案快照
- package/install 相关验证

## 关联文档

- [spec.md](./spec.md)
- [plan.md](./plan.md)
- [../150-faus-serve-cli/spec.md](../150-faus-serve-cli/spec.md)
- [../160-faus-status-cli/spec.md](../160-faus-status-cli/spec.md)
- [../030-cli/spec.md](../030-cli/spec.md)
- [../009-interfaces-and-protocol-contracts/spec.md](../009-interfaces-and-protocol-contracts/spec.md)
- [../010-local-operations-and-automation/spec.md](../010-local-operations-and-automation/spec.md)
- [../008-ui-ux/spec.md](../008-ui-ux/spec.md)
- [../020-frontend-architecture/spec.md](../020-frontend-architecture/spec.md)
