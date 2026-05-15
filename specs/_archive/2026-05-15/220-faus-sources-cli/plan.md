# 220 faus Sources CLI 当前阶段计划

本计划承接 [spec.md](./spec.md)，只规划 `faus sources` 的首个来源管理 CLI 切片。CLI binary、连接规则与错误输出复用既有 `faus` 基础，公开接口契约继续由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接。

## 概要

- 当前阶段实现 top-level `faus sources`
- 当前阶段覆盖 roots CRUD、sources list、refresh/rescan
- 当前阶段复用全局 `--base-url`、`--json`、`--debug`
- 当前阶段不新增 HTTP endpoint，不改变 OpenAPI contract

## 实现计划

1. 新增 `src/bin/faus/sources.rs`，在 `main.rs` 接入 `Commands::Sources(SourcesArgs)`。
2. 在 client helper 中补齐 `delete_json`；必要时增加 URL query 组装 helper，避免手写 query 拼接。
3. 实现 roots 命令：
   - list/show 使用 GET
   - create 使用 POST
   - update 使用 PATCH
   - delete 使用 DELETE
4. 实现 sources list：按可选过滤参数组装 URL-encoded query string。
5. 实现 refresh/rescan：根据是否存在 `--source-root-id` 选择库级或来源根级 action endpoint。
6. 输出沿用既有 CLI 风格：human 短摘要，`--json` 单对象，`--debug --json` 附加 request metadata。
7. 代码落地后更新 `CHANGELOG.md`。

## 当前阶段约束

- 不启动本地服务，不调用 `faus serve`
- 不读取 `.env`、`.env.dev` 或本地 runtime 文件
- 不实现 maintenance、rebuild、settings、source repair、远端来源连接器
- 不修改 `specs/README.md`

## 阶段验收摘要

- help 暴露 `sources`、`roots`、`list/create/show/update/delete/refresh/rescan`
- 每个命令使用正确 HTTP method、path 和 body
- 相对 root path 转绝对路径
- rules flags 序列化正确，update 时整体替换 rules
- filters query URL-encoded
- `--json` 输出稳定机器可读对象
- 服务端错误和响应契约不匹配返回稳定 CLI 错误

详细测试分层与场景矩阵见 [testing.md](./testing.md)。
