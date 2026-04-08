# 本地运行

本文记录当前仓库的最小本地运行方式，目标是把 app、sidecar、UI 和 Qdrant 启起来，并确认当前工作台骨架可用。

## 当前状态

- 当前可用面包括：
  - 库创建与选择
  - 本地路径导入提交
  - 嵌入式任务面板
  - 三栏工作台中的对象详情与预览侧栏
  - 文本搜索输入、显式 `not_ready` 反馈和真实搜索结果列表
- 当前 sidecar 已经具备真实的 ColQwen `query_embedding` 和 `document_embedding` 能力，并提供 `/health`、`/capabilities`、`/embed`。
- 当前 app 已经接通真实的 `app -> sidecar -> Qdrant` multivector 搜索链，当前可实际命中 `image` 与真实页级 `document_page`。
- 当前仍然是早期工作台，不包含完整产品控制面，也还没有 `video_segment`、图片查询或视频查询。
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
```

## 安装完成后：日常启动

安装完成后，日常启动只需要运行：

```bash
bash scripts/local/run.sh
```

`run.sh` 会先自动启动或复用 Qdrant，再启动 app、sidecar 和 UI。

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

## 启动完成后：验证文本搜索链路

这一步是验证命令，不属于安装或启动流程。确认 app、sidecar 与 Qdrant 都已运行后，可以执行：

```bash
bash scripts/local/smoke-text-search.sh
```

如果 app、sidecar 和 Qdrant 是用 `--dev` 启动的，smoke 也要带 `--dev`。

自动化场景可以使用机器可读摘要：

```bash
bash scripts/local/smoke-text-search.sh --dev --json
```

该脚本会创建一个临时库，导入一张现有图片 fixture 和一个写入 `APP_RUNTIME_DIR/smoke-text-search/` 的多页 PDF，随后验证文本查询能够同时返回 `image` 与 `document_page`，并确认搜索后端是 `qdrant` / `multivector`。

## 快速检查

快速检查不启动长驻服务，也不加载真实 GPU 模型：

```bash
bash scripts/local/check.sh
```

该命令默认执行 Rust 测试、sidecar 窄测试和 UI 构建。隔离开发配置可以使用 `bash scripts/local/check.sh --dev`。

当前最小 UI happy-path smoke 入口：

```bash
pnpm --dir ui test:e2e
```

这条命令固定使用 `--dev` 配置。若 `--dev` 服务已在运行，它会直接复用；若未运行，它会自行拉起 `--dev` 的 app、sidecar、UI 和 Qdrant，并在结束后只清理由自己启动的 `--dev` 服务。当前覆盖最小 happy path、建库后直接搜索的 `not_ready`，以及无效导入路径的拒绝反馈。运行前仍需要先完成一次：

```bash
bash scripts/local/bootstrap-linux.sh --dev
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
- 中间列的文本搜索输入框、错误反馈区和真实结果列表
- 搜索结果卡片中的每条结果 score 展示；该值只用于当前响应内的相对排序参考
- 右侧详情栏中的图片/PDF 预览、locator、preview 链接和 neighbor context
- 从导入回执或搜索结果直接打开右侧详情的交互链

## 关键配置项

当前最常用的配置看本次运行选中的 env 文件：默认是根 `.env`，带 `--dev` 时是 `.env.dev`。

- `APP_HOST` / `APP_PORT`
- `SIDECAR_HOST` / `SIDECAR_PORT`
- `UI_HOST` / `UI_PORT`
- `QDRANT_HOST` / `QDRANT_PORT` / `QDRANT_URL`
- `DEV_LOG_DIR`
- `TEXT_SEARCH_MODEL_ID`
- `TEXT_SEARCH_MODEL_REVISION`
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
- `run.sh` 会自动启动或复用 Qdrant，检查 app / sidecar / UI 端口是否空闲，并在健康检查通过后才报告启动成功；加 `--detach` 时会后台启动并写入 pid 文件。
- sidecar 首次冷启动加载 ColQwen 模型可能需要数分钟；首次真实导入或搜索明显慢于后续热路径属于预期行为。
- `status.sh` 会报告 app、sidecar、UI 和 Qdrant 的 URL、ready 状态、pid、日志路径与配置来源；加 `--json` 时输出机器可读 JSON。
- `stop.sh` 会停止指定本地服务，支持 `--all` 停止 app、sidecar、UI 和 Qdrant，并会优先使用 pid 文件再回退到端口 / 命令发现。
- `smoke-text-search.sh` 是启动后的验证命令，用于跑真实 ColQwen + Qdrant 文本搜索 smoke；加 `--json` 时输出机器可读摘要。
- `check.sh` 是无 GPU 快速检查入口，不启动长驻服务。
- `pnpm --dir ui test:e2e` 是当前阶段最小 Playwright UI smoke，固定使用 `--dev` 配置；若 `--dev` 服务未运行则会自行启动并在结束后自清理。
- `download-model.sh` 会读取本次运行选中的 env 文件中的 `TEXT_SEARCH_MODEL_ID` / `TEXT_SEARCH_MODEL_REVISION`，并继承 `HF_ENDPOINT` / `HF_HUB_ENABLE_HF_TRANSFER` 来控制 Hugging Face 下载行为。
- 当 `HF_HUB_ENABLE_HF_TRANSFER=1` 时，下载会更激进，但重启后不会续传未完成的大文件；如果你更看重稳定续传，可以把它改成 `0`。

## 本地工作流状态

- `setup`：一次性安装与初始化，入口是 `bootstrap-linux.sh`，隔离开发配置使用 `bootstrap-linux.sh --dev`
- `diagnose`：环境诊断，入口是 `doctor.sh`
- `run`：启动 Qdrant、app、sidecar 和 UI，入口是 `run.sh`
- `status`：查看本地服务状态，入口是 `status.sh`
- `stop`：停止本地服务，入口是 `stop.sh`
- `test`：无 GPU 快速检查，入口是 `check.sh`
- `ui-smoke`：最小浏览器闭环验证，入口是 `pnpm --dir ui test:e2e`
- `smoke`：真实链路验证，入口是 `smoke-text-search.sh`

更多排障信息见 [排障](./troubleshooting.md)。
