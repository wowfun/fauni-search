# 220 faus Sources CLI

定义 `faus sources` 的具体行为：通过 Rust server 公开 App API 管理库级来源根、查看来源清单，并触发库级或来源根级 `refresh` / `rescan`。本专题承接 [030-cli](../030-cli/spec.md) 的长期 CLI 方向，复用 [140-library-source-management](../140-library-source-management/spec.md) 的来源管理语义与 [160-faus-status-cli](../160-faus-status-cli/spec.md) 的连接和错误输出规则。

## 范围

- `faus sources` 命令组行为
- `--base-url`、`--json`、`--debug` 三个全局 flag 在 sources 命令中的行为
- `FAUS_BASE_URL` 与默认 base URL 的解析优先级
- source-root 生命周期、库级来源清单、库级 / 来源根级 `refresh` 与 `rescan`
- 人类可读输出、JSON 输出、连接层错误与服务端错误映射

范围外：

- settings、model tests、maintenance、rebuild、source repair、远端来源连接器
- HTTP endpoint、OpenAPI schema 或服务端 payload 细节的定义
- 服务启动、停止、诊断、日志、pid 与 Qdrant 管理

## 设计原则

- 来源管理独立成组：`sources` 是 top-level resource group，不塞进 `library` 子命令
- 显式库作用域：所有命令必须显式传 `--library-id <library_id>`
- 配置与观察分离：`sources roots ...` 管长期来源根配置，`sources list` 管只读来源清单
- 动作即返回：`refresh` / `rescan` 只提交动作并返回回执与任务摘要，任务观察交给 `faus jobs`
- 公开 API 优先：CLI 只消费 Rust server App API，不直接访问 SQLite、runtime 文件、Qdrant 或 sidecar
- 契约复用：公开 HTTP 契约由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接，220 不复制 payload schema 细节

## 命令入口

- 本切片要求 `faus sources` 可用
- `faus` 至少支持以下全局 flag：
  - `--base-url <url>`
  - `--json`
  - `--debug`
- 命令面固定为：
  - `faus sources roots list --library-id <library_id>`
  - `faus sources roots create --library-id <library_id> --root-path <path> [--disabled] [--include-glob <glob>]... [--exclude-glob <glob>]... [--include-extension <ext>]...`
  - `faus sources roots show --library-id <library_id> <source_root_id>`
  - `faus sources roots update --library-id <library_id> <source_root_id> [--root-path <path>] [--enable|--disable] [rules flags...]`
  - `faus sources roots delete --library-id <library_id> <source_root_id>`
  - `faus sources list --library-id <library_id> [--source-root-id <id>] [--source-type <type>] [--status <status>]`
  - `faus sources refresh --library-id <library_id> [--source-root-id <id>]`
  - `faus sources rescan --library-id <library_id> [--source-root-id <id>]`

## base URL 规则

- base URL 解析优先级固定为：显式 `--base-url`、`FAUS_BASE_URL`、默认 `http://127.0.0.1:53210`
- `faus sources` 默认不读取 `.env`、`.env.dev`、`FAUNI_ENV_FILE`、`APP_HOST` 或 `APP_PORT`
- `faus sources` 连接 App API 时不使用 ambient `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY` 等代理环境变量
- `--base-url` 与 `FAUS_BASE_URL` 的尾随斜杠不得影响最终请求路径
- base URL 必须是可解析的 HTTP 或 HTTPS URL

## 行为规则

- `--root-path` 相对路径按当前 shell cwd 转为绝对路径后发送；绝对路径原样发送
- create 的 `--disabled` 表示请求 `enabled=false`；默认不传时由服务端默认启用
- rules flags 映射为 `rules.include_globs`、`rules.exclude_globs`、`rules.include_extensions`
- `roots update` 中只要传入任一 rules flag，就发送完整 `rules` 对象；未传的列表为空数组
- `roots update` 中 `--enable` 与 `--disable` 互斥
- `sources list` query 参数必须 URL-encoded
- `refresh/rescan` 没有 `--source-root-id` 时调用库级 endpoint；有 `--source-root-id` 时调用来源根级 endpoint
- 本切片不实现 `rebuild`、maintenance、settings 或 source repair

## 输出

- roots list JSON：`status: "ok"`、`data.base_url`、`data.source_roots`
- root create/show/update/delete JSON：`status: "ok"`、`data.base_url`、`data.source_root`
- sources list JSON：`status: "ok"`、`data.base_url`、`data.sources`
- refresh/rescan JSON：`status: "ok"`、`data.base_url`、`data.action`
- `--debug --json` 可附加 base URL 来源、请求 URL 与 HTTP status
- 人类输出使用短摘要：root id/path/enabled/status/watch/counts；source id/type/status/path/root；action accepted/rejected 数与 job id/status
- 服务端 `ErrorEnvelope` 必须原样映射；非 JSON 或缺少必要 `data` 字段返回 `invalid_response`

## 验收标准

- `faus sources roots list/create/show/update/delete` 可用
- `faus sources list` 可按 source root、source type、status 过滤
- `faus sources refresh/rescan` 可提交库级与来源根级动作
- `--base-url`、`FAUS_BASE_URL` 与默认值的优先级符合本专题规则
- `--json` 成功输出是稳定 JSON 对象，并保留服务端来源结构
- 连接失败、无效 base URL 或响应契约不匹配返回非零退出码
- 本切片不启动本地进程，不新增 HTTP endpoint，不改变 Web 前端实现

## 关联主题

- [030-cli](../030-cli/spec.md)
- [140-library-source-management](../140-library-source-management/spec.md)
- [160-faus-status-cli](../160-faus-status-cli/spec.md)
- [190-faus-jobs-cli](../190-faus-jobs-cli/spec.md)
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md)
- [010-local-operations-and-automation](../010-local-operations-and-automation/spec.md)
