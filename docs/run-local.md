# 本地运行

本文记录当前仓库的最小本地运行方式，目标是把 app、sidecar、UI 和 Qdrant 启起来，并确认当前工作台骨架可用。

## 当前状态

- 当前可用面包括：
  - 库创建与选择
  - 本地路径导入提交
  - 嵌入式任务面板
  - 三栏工作台中的对象详情与预览侧栏
  - 统一搜索入口中的 Text / Image / Video / Document 模式
  - 通过文件选择或粘贴图片进入临时查询资产链路，以及把库内 `image` / `document_page` 结果对象直接作为 query image
  - 通过本地视频上传或库内 `source_id` 复用进入视频查询链路，并可选指定时间范围
  - 通过 PDF 上传进入文档查询链路，并可选指定 `start_page/end_page` 页范围，或从库内 `document_page` 结果对象直接复用为查询文档
  - 显式 `not_ready` 反馈和真实搜索结果列表
- 当前 sidecar 已经具备真实的 ColQwen `query_embedding`、`image_query_embedding`、`video_query_embedding`、`document_embedding` 与 `document_query_embedding` 能力，并提供 `/health`、`/capabilities`、`/embed`。
- 当前 app 已经接通真实的 `app -> sidecar -> Qdrant` `vector_space` 搜索链，当前向量表征固定为 `multi_vector_late_interaction`，可实际命中 `image`、真实页级 `document_page` 与 `video_segment`。
- repo 基线现在默认让 `local_sidecar/athrael-soju/colqwen3.5-4.5B-v3` 承接 `image`、`document`、`video` 三类 content types；模型原生 `EmbeddingCapabilities` 仍只声明 `text,image`，`document/video` 通过 runtime execution inputs 与 adapter 链路执行。
- 当前仍然是早期工作台，不包含完整产品控制面，但已经具备文本、图片、视频与文档四种查询主链。
- 默认使用根 `.env` 作为本地运行时配置；传 `--dev` 时使用 `.env.dev`。同一次运行中，被选中的 env 文件是端口、URL、日志目录和运行时目录的单一事实源。

## 第一次使用：安装与初始化

这部分命令通常只在第一次准备仓库环境时执行。

1. 先确认宿主机前置已经装好：Rust、`cc`、Python `3.12`、`uv`、Node、`pnpm`、`qdrant`、`nvidia-smi`
2. 运行：

```bash
bash scripts/local/bootstrap-linux.sh
```

如果要使用隔离的开发配置，避免和默认本地服务抢端口：

```bash
bash scripts/local/bootstrap-linux.sh --dev
```

3. 再运行：

```bash
bash scripts/local/doctor.sh
```

`doctor.sh` 的职责是校验和诊断环境，不是启动服务。

如果你想先把当前文本搜索模型预下载到本地缓存，再额外执行：

```bash
bash scripts/local/download-model.sh
bash scripts/local/download-model.sh --hf-repo-id Qwen/Qwen3-VL-Embedding-2B
```

当前 `download-model.sh` 默认从仓库根 `fauni.config.json` 与 `${APP_RUNTIME_DIR}/runtime-config.json` 的合并结果中读取 `provider.local_sidecar.active_model` 和该 model 的 `version`。

## 安装完成后：日常启动

安装完成后，日常启动只需要运行：

```bash
bash scripts/local/run.sh
```

`run.sh` 会先解析 `fauni.config.json + ${APP_RUNTIME_DIR}/runtime-config.json`，再自动启动或复用 Qdrant，并启动 app、sidecar 和 UI。

如果当前环境仍有旧世代 runtime 数据，`run.sh` 会拒绝启动并提示先执行：

```bash
bash scripts/local/cutover-runtime.sh
```

`--dev` 环境需要单独执行：

```bash
bash scripts/local/cutover-runtime.sh --dev
```

`cutover-runtime.sh` 负责把旧世代 `app/` 与 `qdrant/` 归档到同环境下的 `legacy-*` 目录，并初始化新的 `${APP_RUNTIME_DIR}/runtime-config.json`。如果确认不再需要这些归档或旧世代 collection，可以再显式执行：

```bash
bash scripts/local/cleanup-legacy-runtime.sh --json
bash scripts/local/cleanup-legacy-runtime.sh --execute
```

`cleanup-legacy-runtime.sh` 只清理当前环境下的 `legacy-*` 归档和旧 `index_*` / `text_search_*` / 直接物理 `vector_space_*` collections；它不会删除 alias 当前 target，也不会把 `vector_space_stage_*` 当成 legacy 数据误删。

如果这套服务需要和默认 `.env` 服务同时存在，给日常命令统一加 `--dev`：

```bash
bash scripts/local/run.sh --dev
```

自动化场景可以把 app、sidecar 和 UI 以分离模式启动：

```bash
bash scripts/local/run.sh --dev --detach
bash scripts/local/status.sh --dev --json
```

## 日常停止

停止命令和启动命令分开；如果只想停止部分服务，直接写服务名：

```bash
bash scripts/local/stop.sh app sidecar
```

停止全部本地服务：

```bash
bash scripts/local/stop.sh --all
```

如果只想确认会停止哪些进程，不实际停止：

```bash
bash scripts/local/stop.sh --all --dry-run
```

停止 `--dev` 配置启动的服务时也要带同一个 flag：

```bash
bash scripts/local/stop.sh --dev --all
```

当前支持的服务名是 `app`、`sidecar`、`ui`、`qdrant`。

## 启动完成后：验证真实检索链路

这一步是验证命令，不属于安装或启动流程。确认 app、sidecar 与 Qdrant 都已运行后，可以执行：

```bash
bash scripts/local/smoke-text-search.sh
```

如果 app、sidecar 和 Qdrant 是用 `--dev` 启动的，smoke 也要带 `--dev`。

自动化场景可以使用机器可读摘要：

```bash
bash scripts/local/smoke-text-search.sh --dev --json
```

该脚本会创建一个临时库，导入一张现有图片 fixture 和一个写入 `APP_RUNTIME_DIR/smoke-text-search/` 的多页 PDF，随后验证文本查询能够同时返回 `image` 与 `document_page`，并确认搜索后端是 `qdrant`，表征类型是 `multi_vector_late_interaction`。

图片查询对应 smoke：

```bash
bash scripts/local/smoke-image-search.sh --dev --json
```

该脚本会创建一个临时库，导入同一组图片 / PDF 内容，上传一张临时查询图片，然后验证 `/search/image` 能够同时返回 `image` 与 `document_page`，并确认搜索后端是 `qdrant`，表征类型是 `multi_vector_late_interaction`。
随后它还会复用库内返回的 `image` 结果对象，以 `library_object` 形式再次发起图片搜索，并验证这条路径也能命中 `image` 与 `document_page`。
最后它还会复用库内返回的 `document_page` 对象，以 `library_object` 形式再次发起图片搜索，并验证文档页 query image 链路也能命中 `image` 与 `document_page`。

视频查询对应 smoke：

```bash
bash scripts/local/smoke-video-search.sh --dev --json
```

该脚本会读取 local-only 视频 manifest，自动派生关键截图与 clip，把原视频、派生截图和派生 PDF 一起导入同一临时库，然后验证：
- 临时上传视频 + 指定时间范围的 `/search/video`
- 临时上传 clip 的整段视频查询
- 库内 `source_id + 指定时间范围` 的 `/search/video`
- 库内 `video_segment` 直接作为查询视频片段再次发起 `/search/video`
- 统一结果列表能同时返回 `video_segment`、`image` 与 `document_page`
- 搜索后端是 `qdrant`，表征类型是 `multi_vector_late_interaction`

文档查询对应 smoke：

```bash
bash scripts/local/smoke-document-search.sh --dev --json
```

该脚本会创建一个临时库，导入一张图片和一个两页 PDF，随后验证：
- 临时上传查询 PDF 的 `/search/document`
- 以 `source_id` 发起的整份文档查询
- 指定单页范围的文档查询
- 把库内返回的 `document_page` 结果对象直接复用为查询文档
- 统一结果列表能稳定返回 `document_page` 与 `image`
- 搜索后端是 `qdrant`，表征类型是 `multi_vector_late_interaction`

运行时健康与 `vector_space` 诊断对应 smoke：

```bash
bash scripts/local/smoke-runtime-health.sh --dev --json
```

该脚本会导入一组真实 `image / document / video` 样本，然后验证：
- `GET /runtime-health` 中的 app / qdrant / local_sidecar 状态
- local sidecar 的 exact model、原生 `EmbeddingCapabilities`、`execution_input_types`
- `GET /libraries/{library_id}/resolved-content-models`
- `GET /libraries/{library_id}/vector-space-diagnostics`
- 默认 `image/document/video` content types 共享同一个 active `vector_space`

## 快速检查

快速检查不启动长驻服务，也不加载真实 GPU 模型：

```bash
bash scripts/local/check.sh
```

该命令默认执行 Rust 测试、sidecar 窄测试、UI TypeScript typecheck 和 UI 构建。隔离开发配置可以使用 `bash scripts/local/check.sh --dev`。

当前最小 UI smoke 入口：

```bash
pnpm --dir ui test:e2e
```

这条命令固定使用 `--dev` 配置。若 `--dev` 服务已在运行，它会直接复用；若未运行，它会自行拉起 `--dev` 的 app、sidecar、UI 和 Qdrant，并在结束后只清理由自己启动的 `--dev` 服务。当前覆盖文本 happy path、图片查询 happy path、粘贴图片查询、库内 `image` / `document_page` 对象作为 query image、视频查询 happy path、库内 `video_segment` 作为 query video、文档查询 happy path、页范围查询、库内 `document_page` 作为查询文档、建库后直接搜索的 `not_ready`，以及无效导入路径、非图片/视频/PDF 查询上传的拒绝反馈。运行前仍需要先完成一次：

```bash
bash scripts/local/bootstrap-linux.sh --dev
```

统一入口：

```bash
bash scripts/local/check-e2e.sh --all
```

该脚本默认面向 `--dev` 运行面，先确保 `--dev` 服务可用，再按需运行：
- Playwright 分域 E2E
- `smoke-runtime-health.sh`
- `smoke-text-search.sh`
- `smoke-image-search.sh`
- `smoke-video-search.sh`
- `smoke-document-search.sh`
- `smoke-source-management.sh`

也可以只跑其中一部分：

```bash
bash scripts/local/check-e2e.sh --ui
bash scripts/local/check-e2e.sh --smoke
```

## 当前访问入口

以下默认值来自当前 `.env.example`；如果你修改了根 `.env`，请以 `.env` 为准。`--dev` 使用 `.env.dev`，默认模板来自 `.env.dev.example`。

- UI: `http://127.0.0.1:55173/`
- app health: `http://127.0.0.1:53210/health`
- sidecar health: `http://127.0.0.1:53211/health`
- Qdrant collections: `http://127.0.0.1:56333/collections`

`--dev` 的默认入口是：

- UI: `http://127.0.0.1:56173/`
- app health: `http://127.0.0.1:54210/health`
- sidecar health: `http://127.0.0.1:54211/health`
- Qdrant collections: `http://127.0.0.1:57333/collections`

UI 当前包含：

- 库创建表单
- 当前库选择器
- 路径导入表单与回执区
- TATDQA demo fixture 的填入与“导入并搜索”快捷动作
- 最近任务列表
- 中间列的统一搜索入口、Text / Image / Video 模式切换、错误反馈区和真实结果列表
- 中间列的统一搜索入口、Text / Image / Video / Document 模式切换、错误反馈区和真实结果列表
- `Image` 模式下的查询图片卡片；当前支持文件选择、粘贴图片，也支持从结果列表把库内 `image` / `document_page` 对象直接设为 query image
- `Video` 模式下的查询视频卡片；当前支持本地视频上传、库内 `source_id` 复用、库内 `video_segment` 复用、时间范围滑块，以及对当前查询视频的即时预览
- `Document` 模式下的查询文档卡片；当前支持 PDF 上传、整份文档默认查询、`start_page/end_page` 数字输入，以及从结果列表把库内 `document_page` 对象直接设为查询文档
- 临时上传查询图片的有效窗口是临时性的；运行期会自动回收过期查询图片及其预览文件，过期后需重新上传
- 临时上传查询文档与查询视频同样属于运行期资产；视频查询 smoke 会基于 local-only manifest 自动派生 clip、截图与辅助 PDF，不把这些派生文件提交进仓库
- 搜索结果卡片中的每条结果 score 展示；该值只用于当前响应内的相对排序参考
- 右侧详情栏中的图片/PDF/视频预览、locator、preview 链接和 neighbor context
- 从导入回执或搜索结果直接打开右侧详情的交互链

## 关键配置项

当前最常用的配置看本次运行选中的 env 文件：默认是根 `.env`，带 `--dev` 时是 `.env.dev`。

- `APP_HOST` / `APP_PORT`
- `SIDECAR_HOST` / `SIDECAR_PORT`
- `UI_HOST` / `UI_PORT`
- `QDRANT_HOST` / `QDRANT_PORT` / `QDRANT_URL`
- `DEV_LOG_DIR`
- `HF_ENDPOINT`
- `HF_HUB_ENABLE_HF_TRANSFER`

如果只是改端口或目录，只改本次运行选中的 env 文件，不要在代码、脚本或文档里同步维护第二套常量。

## 日志位置

默认日志目录由 `DEV_LOG_DIR` 控制；当前 `.env.example` 默认值是 `data/runtime/logs`，`.env.dev.example` 默认值是 `data/runtime/dev/logs`。

常用日志文件：

- `data/runtime/logs/app.log`
- `data/runtime/logs/sidecar.log`
- `data/runtime/logs/ui.log`
- `data/runtime/logs/qdrant.log`

## 运行时说明

- `bootstrap-linux.sh` 会准备 `.env`、运行目录、`.venv-test`、`.venv`、UI 依赖和 Playwright；加 `--dev` 时会准备 `.env.dev` 和对应运行目录。
- `doctor.sh` 是第一诊断入口，用于检查工具、目录、端口、虚拟环境和 CUDA 可用性；它不是启动命令。
- `run-qdrant.sh` 会启动或复用本地 Qdrant，也可由 `run.sh` 自动调用。
- `run.sh` 会自动启动或复用 Qdrant，检查 app / sidecar / UI 端口是否空闲，解析 `local_sidecar.active_model + version`，并在健康检查通过后才报告启动成功；加 `--detach` 时会后台启动并写入 pid 文件。
- sidecar 首次冷启动加载 ColQwen 模型可能需要数分钟；首次真实导入或搜索明显慢于后续热路径属于预期行为。
- `status.sh` 会报告 app、sidecar、UI 和 Qdrant 的 URL、ready 状态、pid、日志路径与配置来源；加 `--json` 时输出机器可读 JSON。
- `stop.sh` 会停止指定本地服务，支持 `--all` 停止 app、sidecar、UI 和 Qdrant，并会优先使用 pid 文件再回退到端口 / 命令发现。
- `smoke-text-search.sh` 是启动后的验证命令，用于跑真实 ColQwen + Qdrant 文本搜索 smoke；加 `--json` 时输出机器可读摘要。
- `smoke-image-search.sh` 是启动后的验证命令，用于跑真实 ColQwen + Qdrant 图片搜索 smoke；加 `--json` 时输出机器可读摘要。
- `smoke-video-search.sh` 是启动后的验证命令，用于跑真实 ColQwen + Qdrant 视频搜索 smoke；它会基于 local-only manifest 自动派生截图与 clip，并验证查询视频上传、可选时间范围、`source_id` 复用和三类对象混排结果。
- `smoke-document-search.sh` 是启动后的验证命令，用于跑真实 ColQwen + Qdrant 文档搜索 smoke；它会验证查询 PDF 上传、整份文档查询、页范围查询和 `document_page` 复用路径。
- `check.sh` 是无 GPU 快速检查入口，不启动长驻服务。
- `pnpm --dir ui test:e2e` 是当前阶段最小 Playwright UI smoke，固定使用 `--dev` 配置；若 `--dev` 服务未运行则会自行启动并在结束后自清理。
- `download-model.sh` 会读取合并后配置中的 `provider.local_sidecar.active_model` 与对应 model 的 `version`，并继承选中 env 文件中的 `HF_ENDPOINT` / `HF_HUB_ENABLE_HF_TRANSFER` 来控制 Hugging Face 下载行为。
- 需要临时下载其他 Hugging Face 仓库时，可额外传 `--hf-repo-id <repo_id>`，例如 `Qwen/Qwen3-VL-Embedding-2B`；这不会改写当前 env 文件。
- 当 `HF_HUB_ENABLE_HF_TRANSFER=1` 时，下载会更激进，但重启后不会续传未完成的大文件；如果你更看重稳定续传，可以把它改成 `0`。

## 本地工作流状态

- `setup`：一次性安装与初始化，入口是 `bootstrap-linux.sh`，隔离开发配置使用 `bootstrap-linux.sh --dev`
- `diagnose`：环境诊断，入口是 `doctor.sh`
- `run`：启动 Qdrant、app、sidecar 和 UI，入口是 `run.sh`
- `status`：查看本地服务状态，入口是 `status.sh`
- `stop`：停止本地服务，入口是 `stop.sh`
- `test`：无 GPU 快速检查，入口是 `check.sh`
- `ui-smoke`：最小浏览器闭环验证，入口是 `pnpm --dir ui test:e2e`
- `smoke`：真实链路验证，入口是 `smoke-text-search.sh` / `smoke-image-search.sh` / `smoke-video-search.sh`

更多排障信息见 [排障](./troubleshooting.md)。
