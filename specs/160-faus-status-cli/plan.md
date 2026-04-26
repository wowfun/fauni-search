# 160 faus Status CLI 当前阶段计划

本计划承接 [spec.md](./spec.md)，只规划 `faus status`。CLI binary 与 `serve` 入口由 [150-faus-serve-cli](../150-faus-serve-cli/spec.md) 建立，长期 CLI 命令面继续由 [030-cli](../030-cli/spec.md) 承接，公开接口契约继续由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接。本切片不启动本地服务，不修改服务端状态，也不改变 Web 前端实现。

## 概要

- 当前阶段只实现 `faus status`
- 当前阶段复用全局 `--base-url`、`--json`、`--debug` 的基础解析
- 当前阶段通过 `/health` 与 `/runtime/status` 展示运行面状态
- 当前阶段不实现服务启动、Web 浏览器入口、库操作、导入、搜索或任务命令

## 实现计划

### 1. CLI 入口

- 复用既有 `faus` binary
- 保持现有 Rust server binary 不变
- `status` 可以放在 `src/bin/faus/` 下的 binary-local module 中，复用同目录的 client/error helper；不提前抽象为 crate-level 模块

### 2. 参数解析

- 使用既有 `clap derive` 结构补充：
  - 全局 `--base-url <url>`
  - 全局 `--json`
  - 全局 `--debug`
  - 子命令 `status`
- `faus status` 是当前阶段新增的唯一子命令
- 未识别命令、缺失子命令或非法参数使用 clap 默认错误输出和退出码

### 3. base URL 解析

- 解析顺序固定为：
  - 命令行 `--base-url`
  - `FAUS_BASE_URL`
  - `http://127.0.0.1:53210`
- 使用 URL parser 校验 base URL
- 只接受 `http` 与 `https`
- 规范化请求路径时移除根路径尾随斜杠
- 不读取 `.env`、`.env.dev`、`FAUNI_ENV_FILE`、`APP_HOST` 或 `APP_PORT`

### 4. HTTP 状态查询

- `faus status` 请求 `GET /health`
- `faus status` 请求 `GET /runtime/status`
- 两个请求都通过当前 base URL 拼接，不直接访问 sidecar、Qdrant、SQLite 或 runtime 文件
- server 可达且两个状态响应成功解析时，CLI 退出码为 `0`
- Qdrant 或 provider 在响应中显示不可用时，CLI 仍退出 `0`

### 5. 输出

- 人类可读输出展示 base URL、app liveness、app runtime、Qdrant 与 providers 概览
- `--json` 输出 `status: "ok"` 与 `data.base_url`、`data.health`、`data.runtime_status`
- `--debug --json` 可以附加 `debug.base_url_source`、请求路径和 HTTP 状态码
- `--debug` 不改变人类可读主输出

### 6. 错误处理

- 无效 base URL 返回非零退出码
- 连接失败、请求超时、非 JSON 响应或响应契约不匹配返回非零退出码
- 服务端 `ErrorEnvelope` 映射为 CLI 错误对象，保留服务端 `code`、`message`、`details` 与 `retryable`
- 人类可读错误写入 stderr
- `--json` 错误输出单个 JSON 对象，包含 `status: "error"` 与 `error.code`

### 7. 发布记录

- 代码实现落地后更新 `CHANGELOG.md`
- 规格创建本身不更新 `CHANGELOG.md`

## 当前阶段约束

- 不新增 HTTP endpoint
- 不改变 OpenAPI contract 的 schema 语义
- 不修改 `specs/README.md`
- 不启动本地服务，不调用 `faus serve`
- 不实现 strict gate、轮询等待或自动恢复
- 不实现除 `status` 之外的产品 CLI 子命令

## Deferred

- Web 浏览器入口命令，由 [170-faus-web-cli](../170-faus-web-cli/spec.md) 承接
- 严格健康门禁或 CI 友好的 `--strict` 行为
- shell completion 与 man page
- 包分发、安装器与发布渠道
- library、import、search、jobs 等产品工作流命令

## 阶段验收摘要

- `faus status` 默认连接 `http://127.0.0.1:53210`
- `FAUS_BASE_URL` 能覆盖默认值
- `--base-url` 能覆盖环境变量
- 尾随斜杠不影响 `/health` 与 `/runtime/status` 请求路径
- `--json` 输出稳定机器可读对象
- 连接失败和无效 base URL 返回稳定 CLI 层错误
- 组件不可用但状态成功取得时，命令仍以退出码 `0` 返回
- 执行 `faus status` 不启动任何本地进程

详细测试分层与场景矩阵见 [testing.md](./testing.md)。
