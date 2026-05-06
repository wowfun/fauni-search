# 230 faus Find CLI

定义 `faus find` 的 agent-first 查找工作流：给定一个本地 folder 和一个查询输入时，CLI 通过已有 Rust server App API 自动准备该 folder；无 folder 时，CLI 在显式 scope 下直接搜索已有 active 索引。两种模式都返回可定位到具体文档页、图片或视频片段的 Asset 结果。

本专题承接 [030-cli](../030-cli/spec.md) 的产品 CLI 方向，复用 [210-faus-search-cli](../210-faus-search-cli/spec.md) 的连接、错误输出和查询资产上传规则；搜索请求 / 响应的公开字段由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接。

## 关键术语

- `faus find`
- Agent-first workflow
- Folder scope
- Scope-only search
- All-libraries scope
- 托管库（Managed Library）
- 来源根自动准备（Source Root Prepare）
- Asset 结果
- Unit 命中证据
- UnitIndex
- Active-only partial wait
- 结果位置（Result Location）

## 范围

- `faus find [<folder>]` 命令行为
- `--base-url`、`--json`、`--debug` 三个全局 flag 在 find 命令中的行为
- `--text` 与 `--image` 查询输入 flag
- folder 自动准备：创建或复用托管库、创建或复用来源根、触发 refresh / rescan
- 无 folder 时的显式 scope-only 搜索：`--all-libraries` 或 `--library-id`
- 显式 `partial` 模式下的 active-only 增量早返回
- 结果按 Asset 组织与 `locations` 定位输出
- 人类可读输出、JSON 输出、连接层错误与服务端错误映射

范围外：

- CLI 启动、停止或托管 runtime
- 数据库迁移、Qdrant collection 迁移或索引重建实现
- 任意自定义内容类型、近重复图片合并、perceptual hash 合并或 embedding 相似合并
- 文本 + 图片组合查询的服务端能力实现
- Qwen3-VL、ColQwen 或其他模型的专用调优流程
- Web UI、Playwright 场景或 Settings 页面实现

## 设计原则

- Client-only：`faus find` 只连接已有 Rust 主服务，不启动 runtime，不直接访问 SQLite、Qdrant、runtime 文件或 sidecar
- Folder-first：有 folder 时，用户只需要指定 folder；CLI 负责通过公开 API 建立或复用该 folder 对应的库和来源根
- Scope-explicit：无 folder 时必须显式指定 `--all-libraries` 或 `--library-id`，避免 agent 漏参数导致意外大范围检索
- Agent-first：输出优先服务自动化 agent 定位具体资料，而不是展示完整管理界面
- Asset-first：结果默认表示用户可操作的 Asset，例如文档页、图片、视频片段或文本块
- Unit-as-evidence：命中证据可以来自 Unit 和 UnitIndex，但输出不能把关键帧、页图或 OCR 文本误当成最终位置
- 默认稳定：默认等待本次 folder 准备任务完成后再搜索，返回已提交的 active 结果；需要尽早拿到已提交增量结果的调用方必须显式选择 `--wait-mode partial`
- 部分等待：`partial` 模式只轮询 active 搜索；一旦当前 folder 出现 active Asset 结果即可返回
- 精确复用：内容复用遵守两阶段精确身份判定；快速 fingerprint 只做候选筛选，SHA-256 等完整内容哈希才是合并依据
- 全局向量空间：服务端应通过全局 VectorSpace namespace 复用同一 Unit 的向量，并用 folder scope 解析出的 eligible UnitIndex point allow-list 做行级预过滤
- 位置保真：每个位置必须保留 Source + Asset 的 `source_uri`、locator、preview 与可选 `job_id`

## 命令入口

- 本切片登记 `faus find` 命令；最终实现必须遵守本专题行为
- Canonical 命令形态固定为：
  - `faus find <folder> --text <query>`
  - `faus find <folder> --image <path>`
  - `faus find --all-libraries --text <query>`
  - `faus find --library-id <library_id> --text <query>`
  - `faus find --library-id <library_id> --image <path>`
- `<folder>` 可以是相对路径或绝对路径；CLI 必须按当前 shell cwd 规范化为绝对路径后提交给服务端
- `<folder>` 是可选 positional；缺失时必须显式传入 `--all-libraries` 或 `--library-id <library_id>`
- `<folder>` 与 `--all-libraries` 同时出现是 `validation_failed`
- `faus find` 至少支持以下全局 flag：
  - `--base-url <url>`
  - `--json`
  - `--debug`
- `faus find` 支持以下 query input flag：
  - `--text <query>`
  - `--image <path>`
- 当前稳定能力要求恰好一个 query input；同时给出 `--text` 与 `--image` 时，CLI 必须返回 `not_supported`
- `faus find` 可规划以下局部 flag：
  - `--top-k <n>`
  - `--target-content-type <type>`，可重复
  - `--wait-mode <partial|complete>`
  - `--wait-timeout-ms <ms>`
  - `--poll-interval-ms <ms>`
  - `--library-id <library_id>`，用于覆盖默认托管库选择
  - `--all-libraries`，用于无 folder 时搜索所有已有 active 库
  - `--rescan`
- `--library-id` 的语义按模式区分：有 folder 时表示承载该 folder 的库覆盖；无 folder 时表示搜索 scope
- 无 folder 的 scope-only 模式不创建库、不创建来源根、不触发 refresh / rescan、不等待 prepare job
- scope-only 模式下传入 `--rescan`、显式 `--wait-mode`、显式 `--wait-timeout-ms` 或显式 `--poll-interval-ms` 是 `validation_failed`
- `faus find --all-libraries --image <path>` 必须使用全局 QueryAsset 上传入口；`faus find --library-id <id> --image <path>` 使用库级 QueryAsset 上传入口
- `--wait-mode complete` 是默认模式，表示等待本次准备任务结束后再搜索；若超时，应返回当前可解释状态和统一错误，不得静默降级为无结果
- `--wait-mode partial` 表示允许在准备任务仍运行时轮询 active 搜索；一旦已有 Source 级 active 结果即可返回
- 默认 `--wait-timeout-ms` 固定为 `300000`，默认 `--poll-interval-ms` 固定为 `1000`
- `--rescan` 表示准备阶段强制触发来源根级 `rescan`；未传入时默认触发来源根级 `refresh`

## base URL 规则

- base URL 解析优先级固定为：
  - 显式 `--base-url`
  - 环境变量 `FAUS_BASE_URL`
  - 默认值 `http://127.0.0.1:53210`
- `faus find` 默认不读取 `.env`、`.env.dev`、`FAUNI_ENV_FILE`、`APP_HOST` 或 `APP_PORT`
- `faus find` 连接 App API 时不使用 ambient `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY` 等代理环境变量
- 当 Rust 主服务不可达时，`faus find` 必须报告连接失败，并提示需要已有 `faus serve` 或等价 runtime

## Folder 自动准备

- CLI 必须先确认 `<folder>` 是可解析的本地目录；不是目录、不可读或路径不存在时返回 CLI 层错误
- 默认托管库选择必须稳定、可复用，避免每次调用创建新库
- 默认 `library_id` 固定为 `faus-find-<16 hex>`，其中 `<16 hex>` 是规范化绝对 folder path 的 SHA-256 摘要前 16 个小写十六进制字符
- 默认 `display_name` 固定为 `faus find: <folder basename>`；若 basename 为空，则使用规范化绝对 folder path
- 若显式传入 `--library-id`，CLI 应使用该库作为 folder 的承载库；库不存在时应通过服务端错误返回，不得在未授权语义下隐式创建不同库
- 未显式传入 `--library-id` 时，CLI 应先查询默认托管库；若不存在，应通过 `POST /libraries` 创建该库并传入派生出的 `library_id` 与 `display_name`
- CLI 应通过公开 API 查询或创建来源根，使规范化 folder 成为目标库的来源根
- CLI 应通过 `GET /libraries/{library_id}/source-roots` 查找同一规范化 folder 的来源根；若已存在多个匹配项，复用列表中的第一个
- 若来源根已存在且指向同一规范化 folder，CLI 应复用它，而不是创建重复来源根；若不存在，应通过 `POST /libraries/{library_id}/source-roots` 创建
- 准备阶段默认触发来源根级 `refresh`；传入 `--rescan` 时触发来源根级 `rescan`
- 准备输出必须表达实际触发动作、`job_id`、等待模式、当前状态与是否复用了已有库 / 来源根
- `faus find` 只编排公开 API；它不直接枚举目录、不直接生成 embedding、不直接写入检索后端
- `faus find` 不要求也不依赖专用 server prepare endpoint；后续如新增 server 侧 prepare API，必须保持当前公开 API 编排语义兼容

## Scope-only 搜索

- Scope-only 模式只包装已有 active 索引结果；它不准备 folder，不触发 source-root action，也不轮询 job
- `faus find --all-libraries --text <query>` 必须调用 `POST /search/text`，并发送 `search_scope.kind=all_libraries`
- `faus find --library-id <library_id> --text <query>` 必须调用 `POST /search/text`，并发送 `search_scope.kind=library` 与对应 `library_id`
- `faus find --library-id <library_id> --image <path>` 必须先调用 `/libraries/{library_id}/query-assets/images` 上传 query image，再调用 `POST /search/image`
- `faus find --all-libraries --image <path>` 必须先调用 `/query-assets/images` 上传全局 query image，再调用 `POST /search/image` 并发送 `search_scope.kind=all_libraries`
- Scope-only 搜索不得发送 `filters.path_prefix`；它只表达显式 library 或 all-libraries scope
- Scope-only JSON 输出必须表达 `scope`，并将 `prepare` 标记为 skipped：`status=skipped`、`action=none`、`job_id=null`、`wait_mode=none`

## 搜索执行

- 文本查询使用 `POST /search/text`
- 图片查询应先通过 query asset upload API 上传本地图片，再使用 `POST /search/image`
- 搜索范围必须限制在目标 folder 对应的库与 URI 前缀；最低稳定方式是 `search_scope.kind=library` 加 `filters.path_prefix=<folder file URI>`
- 服务端应按该 folder scope 从 Source、SourceAssetLocation、Asset、Unit 与 active UnitIndex 解析 eligible UnitIndex；`filters.path_prefix=<folder file URI>` 必须在 eligible UnitIndex 生成前生效，并优先使用过滤后的 point allow-list 预过滤减少无关向量相似度计算
- CLI 不直接访问 SQLite、Qdrant 或检索后端 namespace，也不自行构造 point allow-list；预过滤计划与执行属于服务端搜索编排职责
- `faus find` 默认 `--wait-mode complete` 下等待本次 prepare job 终态后搜索；搜索由服务端按 active 结果解释
- `--wait-mode partial` 只在 prepare job 运行期间反复执行 active 搜索；它不暴露未提交索引结果
- 搜索请求应传递 `top_k`、`target_content_types` 与 `debug` 等可映射公共控制项
- 服务端若暂不支持某类搜索输入或范围，必须通过稳定错误载荷返回 `not_supported` 或 `not_ready`；CLI 不得把它改写为空结果
- 若 `partial` 模式下本次 prepare job 仍在运行且 active 搜索已有结果，CLI 可以返回 `prepare.status=running` 与 active `results`
- 若 `complete` 模式等待超时，CLI 必须返回非零错误，并在 JSON 错误的 `details` 中保留 `library_id`、`source_root_id`、prepare `action`、`job_id` 与最后观察到的 job 状态

## 结果语义

- `faus find` 结果面向定位任务，默认按 Asset 组织
- 每个结果表示一个用户可操作位置，例如一个 PDF 页、一张图片、一个视频片段或一个文本块
- 命中证据可以来自一个或多个 Unit；Unit 摘要只用于解释匹配，不替代 Asset 位置
- CLI 应输出 find 专用结果结构；普通 `/search/*` 返回的单个 Asset 结果应被包装为一个包含单个 `locations[]` 的 find 结果
- `locations` 中的每个位置至少应保留：
  - `library_id`
  - `source_id`
  - `asset_id`
  - 可选 `source_root_id`
  - `source_uri`
  - `source_type`
  - `asset_type`
  - `locator`
  - `preview`
- 文档页结果必须能通过 `locator` 定位到真实页序；视频片段结果必须能通过 `locator.start_ms` / `locator.end_ms` 定位到真实时间范围

## JSON 输出

`faus find ./notes --text "quarterly revenue" --json` 成功输出必须是单个 JSON 对象：

```json
{
  "status": "ok",
  "data": {
    "base_url": "http://127.0.0.1:53210",
    "scope": {
      "kind": "folder",
      "library_id": "faus-find-8f14e45fceea167a",
      "path_prefix": "file:///abs/path/notes"
    },
    "folder": {
      "input": "./notes",
      "path": "/abs/path/notes"
    },
    "library": {
      "library_id": "faus-find-8f14e45fceea167a",
      "source_root_id": "root-1",
      "reused_library": true,
      "reused_source_root": true
    },
    "prepare": {
      "status": "ready",
      "action": "refresh",
      "job_id": "job-1",
      "wait_mode": "complete"
    },
    "results": [
      {
        "asset_id": "asset-1",
        "asset_type": "document_page",
        "score": 12.34,
        "locator": { "page": 1, "page_label": "1" },
        "preview": { "url": "/libraries/faus-find-8f14e45fceea167a/assets/asset-1/preview" },
        "matched_units": [],
        "locations": [
          {
            "library_id": "faus-find-8f14e45fceea167a",
            "source_root_id": "root-1",
            "source_id": "src-1",
            "asset_id": "asset-1",
            "source_uri": "file:///abs/path/notes/report.pdf",
            "source_type": "pdf",
            "asset_type": "document_page",
            "locator": { "page": 1, "page_label": "1" },
            "preview": { "url": "/libraries/faus-find-8f14e45fceea167a/assets/asset-1/preview" }
          }
        ]
      }
    ]
  }
}
```

- `data.base_url` 使用规范化后的 URL
- `data.scope` 必须表达 `folder`、`library` 或 `all_libraries`
- `data.folder.input` 保留用户输入，`data.folder.path` 保留规范化绝对路径
- `data.library` 至少返回 `library_id`、`source_root_id`、`reused_library` 与 `reused_source_root`
- `data.prepare.status` 至少能表达 `ready`、`running`、`not_ready`、`failed`
- `data.prepare.job_id` 在触发或复用准备任务时必须返回
- `data.prepare.wait_mode` 必须表达本次等待语义，默认是 `complete`
- `data.results` 中每个结果至少应支持 `asset_id`、`asset_type`、`score`、`locator`、`preview`、可选 `job_id`、可选 `matched_units` 与 `locations`
- 当 `--debug` 与 `--json` 同时出现时，可以附加 `debug` 对象，用于展示 base URL 来源、prepare 请求、上传请求、搜索请求与响应状态码等 CLI 侧信息
- `--json` 输出不得包含 ANSI 控制字符、进度文案或日志行

Scope-only 文本搜索的 JSON 输出必须保持同一外层形状，但不包含 folder 准备语义：

```json
{
  "status": "ok",
  "data": {
    "base_url": "http://127.0.0.1:53210",
    "scope": { "kind": "all_libraries" },
    "folder": null,
    "prepare": {
      "status": "skipped",
      "action": "none",
      "job_id": null,
      "wait_mode": "none"
    },
    "results": []
  }
}
```

- `faus find --library-id <library_id> ...` 的 scope-only 输出应包含 `data.library.library_id`
- `faus find --all-libraries ...` 的 scope-only 输出不得伪造单一 `data.library`

## 人类可读输出

- 默认输出应展示短摘要：folder、prepare 状态、命中数量与前若干结果
- Scope-only 模式应展示显式 scope 与 `prepare=skipped`
- 每条结果应优先展示来源 URI、结果类型、locator、score 与 preview URL
- 当 partial 模式在 prepare job 仍 running 时返回，应在人类可读输出中展示 prepare 仍在运行；结果本身仍为 active
- 没有结果时应输出 `No results.`

## 错误输出

- folder 不存在、不是目录、不可读、图片查询文件不可读、缺失查询输入、多个查询输入或 `--top-k 0` 是 CLI 层错误，不发起搜索请求
- 无 folder 且未传 `--all-libraries` 或 `--library-id` 是 CLI 层 `validation_failed`
- 无 folder 时传入 prepare-only flag 是 CLI 层 `validation_failed`
- 连接失败、请求超时、非 JSON 响应或响应契约不匹配属于 CLI 层错误
- 服务端统一错误载荷必须映射到 CLI 错误对象中，不得改写服务端错误语义
- `--json` 下的错误输出必须是单个 JSON 对象，并可在 `error.hint` 与 `error.details` 中提供诊断上下文
- 人类可读错误应写入 stderr，并返回非零退出码

## 与其他命令的分界

- `faus find` 面向“指定 folder 或显式现有 scope 中快速找资料”的 agent workflow
- `faus find` 的 scope-only 模式面向 agent-friendly 包装：它不准备 folder，但仍按 find JSON 输出组织 `locations[]` 与 `matched_units[]`
- `faus search` 继续承接基础搜索 API 形态：显式库范围、active-only 默认、不自动准备 folder，见 [210-faus-search-cli](../210-faus-search-cli/spec.md)
- `faus import` 负责把路径显式提交给指定库，不默认等待搜索可见，见 [200-faus-import-cli](../200-faus-import-cli/spec.md)
- `faus sources` 负责显式来源根管理；`faus find` 只在自动准备 folder 时复用对应公开能力
- `faus jobs` 负责观察或处理后台任务，见 [190-faus-jobs-cli](../190-faus-jobs-cli/spec.md)
- `scripts/local/*` 继续负责服务 stop、状态脚本、doctor、smoke 与本地运行面管理

## 验收标准

- specs 中存在 `faus find [<folder>]` 的独立专题
- `faus find` 被定义为 client 型命令，不启动 runtime
- 文本与图片查询入口明确，且当前只允许单一查询输入
- 无 folder 时必须显式给出 `--all-libraries` 或 `--library-id`
- Scope-only 模式明确不 prepare folder，且 `--all-libraries --image` 使用全局 QueryAsset，不得创建库、来源根或 source-root job
- 默认托管库由规范化 folder path 的 SHA-256 摘要派生为 `faus-find-<16 hex>`
- folder 自动准备明确通过公开 API 创建或复用库与来源根，并默认触发 source-root `refresh`
- `--rescan` 明确强制触发 source-root `rescan`
- 默认等待模式为 `complete`，默认搜索可见性为 `active`
- `partial` 模式使用 active-only 早返回；它不暴露未提交结果
- 结果按 Asset 组织，并通过 `locations` 保留具体文档页、图片或视频片段位置
- JSON 输出包含 scope、prepare 状态、results、locations 与可选 `job_id`
- 本专题不实现 CLI、不新增 server endpoint、不定义数据迁移

## 关联主题

- [002-state-and-data-model](../002-state-and-data-model/spec.md)
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md)
- [004-search](../004-search/spec.md)
- [007-storage-and-persistence](../007-storage-and-persistence/spec.md)
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md)
- [030-cli](../030-cli/spec.md)
- [190-faus-jobs-cli](../190-faus-jobs-cli/spec.md)
- [200-faus-import-cli](../200-faus-import-cli/spec.md)
- [210-faus-search-cli](../210-faus-search-cli/spec.md)
