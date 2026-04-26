# 150 faus Serve CLI 当前阶段计划

本计划承接 [spec.md](./spec.md)，只规划 `faus` 产品 CLI 的最小入口和 `faus serve`。长期 CLI 命令面继续由 [030-cli](../030-cli/spec.md) 承接，公开接口契约继续由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接。本切片不实现状态查询、Web 浏览器入口或其他产品工作流命令。

## 概要

- 当前阶段交付形态固定为一个可运行的 `faus` binary
- 当前阶段只实现 `faus serve`
- 当前阶段启动 Qdrant、Python sidecar 与 Rust server
- 当前阶段支持 `--host`、`--port`、`--dev`
- 当前阶段不启动 Vite UI，不实现后台守护

## 实现计划

### 1. CLI 依赖与 binary

- 在 Rust 依赖中新增 `clap`，启用 derive
- 新增 `faus` binary；实现可以从单文件演进为 `src/bin/faus/` 下的 binary-local modules
- 保持现有 Rust server binary 不变
- 早期实现可以保持单文件；当 `status`、`web` 等后续切片加入后，应按 binary-local modules 拆分，避免形成新的 crate-level CLI 公共面

### 2. 参数解析

- 使用 `clap derive` 定义：
  - 子命令 `serve`
  - `serve --host <host>`
  - `serve --port <port>`
  - `serve --dev`
  - 全局 `--debug`
- `faus serve` 是当前阶段唯一可用子命令
- 未识别命令、缺失子命令或非法参数使用 clap 默认错误输出和退出码

### 3. Runtime 启动

- 复用或抽取现有 Rust server 启动逻辑，避免产生第二套 server 初始化路径
- 复用本地运行配置中已有的 Qdrant 与 sidecar 启动约定
- 按顺序启动或确认：
  - Qdrant 可用
  - Python sidecar 可用
  - Rust server ready
- ready 判定以 Rust server 可提供公开 App API 为准
- 启动失败时清理本次命令启动的子进程

### 4. 地址与开发配置

- `--host` 覆盖 Rust server 监听 host
- `--port` 覆盖 Rust server 监听 port
- 默认监听 `127.0.0.1:53210`
- `--dev` 使用本地开发运行配置，具体 env 文件和 runtime 目录语义与 [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md) 对齐
- 不用 `--base-url` 配置监听地址

### 5. 输出与信号

- 人类可读输出展示启动进度、server base URL 与 OpenAPI URL
- `--debug` 展示配置来源、子进程命令和 readiness 细节
- 捕获中断信号并关闭本次命令启动的子进程
- 前台运行期间不混入不稳定机器输出

### 6. 本地脚本对齐

- 保留 `scripts/local/*` 的 bootstrap、doctor、stop、smoke、reset 和 detach 能力
- `scripts/local/run.sh` 作为 `faus serve` wrapper 启动 headless 后端，并额外启动 Vite UI
- wrapper 负责 `--detach`、日志文件、pid、UI readiness 和自动化环境隔离

### 7. 发布记录

- 代码实现落地后更新 `CHANGELOG.md`
- 规格创建本身不更新 `CHANGELOG.md`

## 当前阶段约束

- 不新增 HTTP endpoint
- 不改变 OpenAPI contract
- 不修改 `specs/README.md`
- 不启动 Vite UI
- 不把后台守护、stop、doctor、reset 或 smoke 能力并入 `faus serve` 命令；本地脚本可以作为验收入口覆盖这些边界
- 不实现 `status`、`web`、library、import、search 或 jobs 子命令

## Deferred

- 状态查询命令，由 [160-faus-status-cli](../160-faus-status-cli/spec.md) 承接
- Web 浏览器入口，由 [170-faus-web-cli](../170-faus-web-cli/spec.md) 承接
- shell completion 与 man page
- 包分发、安装器与发布渠道

## 阶段验收摘要

- `faus serve` 默认以前台方式启动本机 runtime
- `--host` 与 `--port` 能改变 Rust server 监听地址
- `--dev` 能选择开发运行配置
- 命令启动 Qdrant、Python sidecar 与 Rust server，但不启动 Vite UI
- 启动失败返回稳定非零退出码
- 中断退出会清理本次命令启动的子进程

详细测试分层与场景矩阵见 [testing.md](./testing.md)。
