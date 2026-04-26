# 200 faus Import CLI 当前阶段计划

本计划承接 [spec.md](./spec.md)，只规划 `faus import` 的基础本地路径导入提交能力。CLI binary、连接规则与错误输出复用既有 `faus` 基础，公开接口契约继续由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接。本切片不启动本地服务，不等待任务完成，不实现搜索或 source-root 管理。

## 概要

- 当前阶段只实现 `faus import --library-id <library_id> <path>...`
- 当前阶段复用全局 `--base-url`、`--json`、`--debug`
- 当前阶段只消费 `POST /libraries/{library_id}/imports`
- 当前阶段不新增 HTTP endpoint，不改变 OpenAPI contract

## 实现计划

### 1. CLI 入口

- 复用既有 `faus` binary
- 新增 `src/bin/faus/import.rs`，只承接 import 命令的参数、HTTP 调用和输出组织
- 在 `src/bin/faus/main.rs` 中接入 `Commands::Import(...)`
- 不提前抽象为 crate-level 模块

### 2. 参数解析

- 使用 `clap derive` 结构补充：
  - `import --library-id <library_id> <path>...`
- `<path>...` 至少要求一个路径
- 未识别参数、缺失 `--library-id` 或缺失路径使用 clap 默认错误输出和退出码

### 3. 路径处理

- 绝对路径原样发送
- 相对路径按当前 shell cwd 拼接为绝对路径后发送
- 不调用 canonicalize，不要求路径存在
- 不展开 quoted `~`

### 4. HTTP 调用

- 复用 `resolve_base_url`
- 复用 CLI 内部 POST JSON helper
- 请求 `POST /libraries/{library_id}/imports`
- body 为 `{"paths":[...]}`，路径顺序保持用户输入顺序
- 解析服务端 `SuccessEnvelope`，要求 `data.accepted` 与 `data.rejected` 存在且为数组
- 服务端 `ErrorEnvelope` 保持现有 CLI 错误映射

### 5. 输出

- 人类可读输出展示 accepted / rejected 数量和 job 摘要
- rejected 项逐行展示路径、reason_code 与 message
- `--json` 输出 `status: "ok"`、`data.base_url`、`data.import`
- `--debug --json` 附加 base URL 来源、请求 URL、HTTP status

### 6. 发布记录

- 代码实现落地后更新 `CHANGELOG.md`
- 规格创建本身不更新 `CHANGELOG.md`

## 当前阶段约束

- 不新增 HTTP endpoint
- 不改变 OpenAPI contract 的 schema 语义
- 不修改 `specs/README.md`
- 不启动本地服务，不调用 `faus serve`
- 不实现 `--wait`、轮询、watch、tail 或 job log
- 不实现 `faus import paths` 或 `faus library import`
- 不做文件存在性检查、文件类型筛选或递归扫描

## Deferred

- `--wait` / watch / tail 导入任务观察模式
- 上传式导入或远程导入输入变体
- source-root 导入管理
- import 与 jobs 的组合式 UX
- shell completion 与 man page

## 阶段验收摘要

- `faus import --help` 展示 `--library-id` 与路径参数
- `faus import --library-id demo file.pdf --json` 请求正确 HTTP method、path 和 JSON body
- `FAUS_BASE_URL` 能覆盖默认值
- `--base-url` 能覆盖环境变量
- 尾随斜杠不影响请求路径
- 相对路径按 cwd 转为绝对路径
- `--json` 输出稳定机器可读对象
- 连接失败、无效 base URL、服务端错误和响应契约不匹配返回稳定 CLI 错误

详细测试分层与场景矩阵见 [testing.md](./testing.md)。
