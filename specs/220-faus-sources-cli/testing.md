# 220 faus Sources CLI 测试设计

本文件承接 `220-faus-sources-cli` 的测试设计。长期 CLI 规则见 [../030-cli/spec.md](../030-cli/spec.md)，来源管理语义见 [../140-library-source-management/spec.md](../140-library-source-management/spec.md)，当前阶段范围见 [spec.md](./spec.md) 与 [plan.md](./plan.md)。

## 默认测试入口

- Rust 编译检查：`cargo check --all-targets`
- CLI 二进制单测：`cargo test --bin faus`
- CLI sources 窄测试：`cargo test --test faus_cli sources`

## 场景矩阵

| 场景 | 预期 |
| --- | --- |
| `faus sources --help` | 展示 roots、list、refresh、rescan |
| `faus sources roots --help` | 展示 list/create/show/update/delete |
| roots list | 请求 `GET /libraries/{library_id}/source-roots`，输出 `data.source_roots` |
| roots create | 请求 `POST /libraries/{library_id}/source-roots`，root path 转绝对路径 |
| roots update | 请求 `PATCH /libraries/{library_id}/source-roots/{source_root_id}` |
| update rules flags | 发送完整 `rules` 对象，未传列表为空数组 |
| roots delete | 请求 `DELETE /libraries/{library_id}/source-roots/{source_root_id}` |
| sources list filters | 请求 `GET /libraries/{library_id}/sources?...`，query URL-encoded |
| refresh/rescan library | 请求 `/libraries/{library_id}/refresh` 或 `/rescan` |
| refresh/rescan root | 请求 `/libraries/{library_id}/source-roots/{source_root_id}/refresh` 或 `/rescan` |
| `--base-url` 覆盖 env | 使用 flag 地址且尾随斜杠不影响路径 |
| 服务端 `ErrorEnvelope` | CLI 错误对象保留服务端语义 |
| 非 JSON 或缺必要字段 | 返回 `invalid_response` |

## JSON 断言

- roots list: `status == "ok"`，`data.base_url`，`data.source_roots` 为数组
- root 单对象: `data.source_root` 为对象
- sources list: `data.sources` 为数组
- actions: `data.action.accepted` 与 `data.action.rejected` 为数组
- `--debug --json` 包含 `base_url_source`、`request_url`、`http_status`

## 真实 Dev 验证

可选真实 dev 验证使用 `.env.dev`：

- `bash scripts/local/run.sh --dev --detach`
- 创建临时库
- `target/debug/faus --base-url http://127.0.0.1:54210 --json sources roots create --library-id <id> --root-path <temp-dir>`
- `target/debug/faus --base-url http://127.0.0.1:54210 --json sources roots list --library-id <id>`
- `target/debug/faus --base-url http://127.0.0.1:54210 --json sources refresh --library-id <id>`
- 用 `faus jobs` 查看返回 job
- `bash scripts/local/stop.sh --dev --all`

## Deferred Coverage

- maintenance / rebuild
- source repair
- settings / model tests
- 远端来源连接器
- watch/tail job log
