# 170 faus Web CLI 当前阶段计划

本计划承接 [spec.md](./spec.md)，只规划 `faus web`。Runtime 启动能力由 [150-faus-serve-cli](../150-faus-serve-cli/spec.md) 建立，状态连接经验由 [160-faus-status-cli](../160-faus-status-cli/spec.md) 建立，CLI-hosted Web 入口由 [020-frontend-architecture](../020-frontend-architecture/spec.md) 约束，长期 CLI 命令面继续由 [030-cli](../030-cli/spec.md) 承接。本切片不改变 Web 前端实现，不新增 App API。

## 概要

- 当前阶段只实现 `faus web`
- 当前阶段复用全局 `--base-url`、`--json`、`--debug` 的基础解析
- 当前阶段连接已有 server，或在没有显式目标 server 时启动默认本机 runtime
- 当前阶段通过 `webbrowser` 打开浏览器；打开失败时打印 URL
- 当前阶段不实现状态查询、库操作、导入、搜索或任务命令

## 实现计划

### 1. CLI 入口

- 复用既有 `faus` binary
- 保持现有 Rust server binary 不变
- 复用 `src/bin/faus/` 下已有的 binary-local modules；新增 `web` 时优先复用同目录 client/error/serve helper，不把 CLI 细节抽到 crate-level 模块
- 新增 `src/bin/faus/web.rs`，只承接 `faus web` 的连接、打开浏览器和输出组织

### 2. 参数解析

- 使用既有 `clap derive` 结构补充子命令 `web`
- `faus web` 是当前阶段新增的唯一子命令
- 未识别命令、缺失子命令或非法参数使用 clap 默认错误输出和退出码

### 3. base URL 与连接

- 解析顺序固定为：
  - 命令行 `--base-url`
  - `FAUS_BASE_URL`
  - `http://127.0.0.1:53210`
- 使用 URL parser 校验 base URL
- 只接受 `http` 与 `https`
- 规范化输出时移除根路径尾随斜杠
- 显式目标 server 不可达时返回连接错误

### 4. 启动或连接 server

- 显式 `--base-url` 或 `FAUS_BASE_URL` 存在时，只连接该 server
- 没有显式目标 server 时，先探测默认地址
- 默认地址不可达时，复用 `faus serve` 的启动能力启动本机 runtime
- `faus serve` 启动流程应抽出 ready hook，供 `faus web` 在 runtime ready 后检查 Web 根路径并打开浏览器；不得复制 Qdrant、sidecar 或 Rust server 启动逻辑
- `faus web --json` 触发 runtime 启动时，serve 进度日志不得写入 stdout，以免破坏 JSON 输出
- 如果本命令启动了 runtime，打开浏览器后继续前台运行，直到用户中断
- 如果只是连接已有 server，打开或打印 URL 后可以退出

### 5. 浏览器打开与输出

- Web URL 默认等于 `UI_HOST` / `UI_PORT` 对应的本地 Web server 根路径
- 本地 Web server 应返回 `ui/dist/index.html`，并把 App API 路径代理到规范化后的 base URL
- 若 `ui/dist/index.html` 缺失，命令应报告可诊断失败，而不是打开 `/routes`
- 使用 `webbrowser` crate 打开 Web URL；CLI 内部保留可替换 opener，以便测试不真实打开用户浏览器
- 浏览器打开失败时打印 Web URL，允许用户手动访问
- `--json` 输出 `status: "ok"` 与 `data.base_url`、`data.web_url`、`data.opened`、`data.server_started`
- `--debug --json` 可以附加 `debug.base_url_source`、连接结果和启动路径

### 6. 错误处理

- 无效 base URL 返回非零退出码
- 显式目标 server 不可达返回非零退出码
- 默认本机 runtime 启动失败返回非零退出码
- 浏览器打开失败但 URL 已打印时不作为 server 失败处理
- 人类可读错误写入 stderr

### 7. 发布记录

- 代码实现落地后更新 `CHANGELOG.md`
- 规格创建本身不更新 `CHANGELOG.md`

## 当前阶段约束

- 不新增 HTTP endpoint
- 不改变 OpenAPI contract
- 不修改 `specs/README.md`
- 不启动 Vite UI
- 不实现后台守护、stop、doctor、reset 或 smoke
- 不实现除 `web` 之外的新产品 CLI 子命令

## Deferred

- shell completion 与 man page
- 包分发、安装器与发布渠道
- library、import、search、jobs 等产品工作流命令

## 阶段验收摘要

- `faus web --base-url <url>` 连接已有 server 并打开浏览器
- `FAUS_BASE_URL` 能指定已有 server
- 没有显式目标 server 时，命令能连接或启动默认本机 runtime
- 尾随斜杠被规范化
- 浏览器打开失败时打印 URL
- `--json` 输出稳定机器可读对象
- 无效 base URL、显式 server 不可达或启动失败返回稳定 CLI 层错误

详细测试分层与场景矩阵见 [testing.md](./testing.md)。
