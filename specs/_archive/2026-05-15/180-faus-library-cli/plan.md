# 180 faus Library CLI 当前阶段计划

本计划承接 [spec.md](./spec.md)，只规划 `faus library` 的基础库工作流。CLI binary、连接规则与错误输出复用既有 `faus` 基础，公开接口契约继续由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接。本切片不启动本地服务，不实现导入、搜索、任务、source-root 或物理删除命令。

## 概要

- 当前阶段只实现 `faus library`
- 当前阶段复用全局 `--base-url`、`--json`、`--debug`
- 当前阶段覆盖 list/create/show/rename/archive/restore
- 当前阶段不新增 HTTP endpoint，不改变 OpenAPI contract

## 实现计划

### 1. CLI 入口

- 复用既有 `faus` binary
- 新增 `src/bin/faus/library.rs`，只承接 library 命令的参数、HTTP 调用和输出组织
- 在 `src/bin/faus/main.rs` 中接入 `Commands::Library(...)`
- 不提前抽象为 crate-level 模块

### 2. 参数解析

- 使用 `clap derive` 结构补充：
  - `library list`
  - `library create --display-name <name> [--library-id <id>]`
  - `library show <library_id>`
  - `library rename <library_id> --display-name <name>`
  - `library archive <library_id>`
  - `library restore <library_id>`
- 未识别命令、缺失参数或非法参数使用 clap 默认错误输出和退出码

### 3. HTTP 调用

- 复用 `resolve_base_url`
- 补齐 CLI 内部 JSON request helper，用于 GET、POST JSON、PATCH JSON 与空 POST
- 解析服务端 `SuccessEnvelope`，list 读取 `data.libraries`，单库操作读取 `data`
- 服务端 `ErrorEnvelope` 保持现有 CLI 错误映射

### 4. 输出

- 人类可读 list 逐行展示库摘要，空列表显示 `No libraries.`
- 人类可读单库操作输出一行库摘要
- `--json` list 输出 `status: "ok"`、`data.base_url`、`data.libraries`
- `--json` 单库操作输出 `status: "ok"`、`data.base_url`、`data.library`
- `--debug --json` 附加 base URL 来源、请求 URL、HTTP status

### 5. 发布记录

- 代码实现落地后更新 `CHANGELOG.md`
- 规格创建本身不更新 `CHANGELOG.md`

## 当前阶段约束

- 不新增 HTTP endpoint
- 不改变 OpenAPI contract 的 schema 语义
- 不修改 `specs/README.md`
- 不启动本地服务，不调用 `faus serve`
- 不实现 delete、source roots、content types、import、search 或 jobs

## Deferred

- 库物理删除命令
- source-root 管理命令
- content type / provider override 命令
- import、search、jobs 等产品工作流命令
- shell completion 与 man page

## 阶段验收摘要

- `faus library --help` 展示基础子命令
- `faus library list/create/show/rename/archive/restore` 请求正确 HTTP method 和 path
- `FAUS_BASE_URL` 能覆盖默认值
- `--base-url` 能覆盖环境变量
- 尾随斜杠不影响请求路径
- `--json` 输出稳定机器可读对象
- 连接失败、无效 base URL、服务端错误和响应契约不匹配返回稳定 CLI 错误

详细测试分层与场景矩阵见 [testing.md](./testing.md)。
