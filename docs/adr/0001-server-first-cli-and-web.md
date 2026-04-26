---
name: 0001 Server-first CLI and Web
status: Accepted
date: 2026-04-26
---

## Context

FauniSearch 已经有一个 Rust HTTP 服务，承接库管理、来源根、任务、运行时健康、查询资产上传，以及 Text / Image / Video / Document 四类搜索。当前 Web 通过 Vite dev server 的 `/api` proxy 访问 Rust 服务，本地操作主要依赖 `scripts/local/*` 启动、停止、诊断和 smoke 验证。

下一步要补产品级 CLI 和正式 Web 入口。如果 CLI、Web 和本地脚本各自承载业务逻辑，搜索、任务、配置和运行时状态会很快分叉。这个项目需要一个明确的产品内核和协议边界，让不同入口共享同一套行为。

## Decision

FauniSearch 采用 server-first 形态：Rust HTTP server 是唯一产品运行时和协议边界。CLI 与 Web 都通过同一套公开 API 工作，不绕过 Rust 服务直接读写内部状态，也不复制搜索、导入、任务或配置逻辑。

公开 API 的长期方向采用 OpenAPI-first。OpenAPI contract 用来约束 CLI、Web 和后续 SDK，不让客户端靠手写请求形状长期漂移。现有规格仍然负责定义产品语义和协议约束，OpenAPI 负责把这些约束落到可消费的接口描述上。

新增产品 CLI 使用 `faus` 作为命令名。`faus` 是 workflow-first HTTP client，优先覆盖常用链路，例如启动或连接服务、打开 Web、查看状态、管理库、导入内容、执行四类搜索和观察任务。它不是 `scripts/local/*` 的替代品，也不是第二套业务实现。

正式 Web 由 Rust server 托管构建后的 UI 静态资产。Vite proxy 继续作为开发便利存在，但不再是正式 Web 入口的前提。`faus web` 负责启动或连接本地 server，并打开浏览器访问同一个 Rust server。

v1 默认只面向本机使用。服务默认绑定 localhost，不在第一阶段引入远程登录、mDNS、局域网发现或开放 CORS。远程访问可以在后续 ADR 中单独决策。

`scripts/local/*` 继续保留为本地运维自动化层，负责 bootstrap、run、stop、status、doctor、check 和 smoke。它们不扩张成产品 CLI 表面。

## Consequences

CLI 和 Web 的实现要先对齐 server contract，再补入口体验。任何需要被 CLI 和 Web 共同消费的能力，都应优先进入 Rust HTTP API 和 OpenAPI contract，而不是只放在某一个客户端里。

Rust API 类型、响应封套、错误载荷、multipart 上传、搜索输入和任务快照需要逐步整理成可生成、可校验、可复用的协议面。现有 `pub(crate)` 类型如果要作为 contract 事实源，需要按模块边界公开或映射到专门的 API schema。

Web 的生产交付会从开发代理模式转向 server-hosted 模式。开发时仍可用 Vite，但正式入口应能只通过 Rust server 打开并使用 UI。

本地脚本和产品 CLI 的职责要保持清楚。脚本处理机器和环境，`faus` 处理产品工作流。后续文档和帮助文本也应按这个边界组织。

## Alternatives Considered

### CLI-first

先把 `faus` 做成主要入口，Web 只维持现状。这条路短期能更快得到命令行体验，但容易让 CLI 先沉淀一套请求、状态和错误处理习惯，之后 Web 再追会产生漂移。

### Web-first

先把浏览器体验产品化，CLI 只保留启动和少量脚本命令。这条路适合以 UI 演示为主的阶段，但自动化、回归验证和可组合工作流会偏弱。

### Scripts as CLI

继续扩展 `scripts/local/*`，把它们当作产品 CLI。这会混淆本地运维和产品操作。脚本天然关注 env 文件、pid、日志、进程和 smoke；产品 CLI 应关注库、导入、搜索、任务和输出格式。

### Hosted Web Connects to Local Server

像 opencode 一样让 hosted Web 连接本地 server。这条路远期有价值，但第一阶段会提前引入 CORS、认证、局域网安全和连接管理。当前先保持 local-only，把单机闭环做稳。
