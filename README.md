# FauniSearch

FauniSearch 是一个本地优先（Local-First）的视觉检索系统。

当前最小本地运行与排障文档见 [`docs/`](./docs/) 目录。

## 开发环境准备

当前标准开发环境固定为：
- `Linux/WSL2 + NVIDIA GPU`
- Rust 主服务 + Python sidecar + 最小 UI
- `Qdrant` 以本机进程方式运行
- `ColQwen3.5-4.5B-v3` 权重采用懒加载

当前阶段不使用 Docker、devcontainer 或 Nix。

### 系统前置

在运行仓库脚本前，宿主机需要先具备：
- Rust stable，且 `cargo` / `rustc` 在 `PATH`
- Linux C toolchain，且 `cc` 在 `PATH`
- Python `3.12`
- `uv`
- Node `22+`
- `pnpm 10+`
- `Qdrant` 可执行文件，且 `qdrant` 在 `PATH`
- `nvidia-smi` 可用

推荐先确保以下命令各自可执行：
- `cargo --version`
- `rustc --version`
- `cc --version`
- `python3 --version`
- `uv --version`
- `node --version`
- `pnpm --version`
- `qdrant --version`
- `nvidia-smi`

如果宿主机缺少 `cc` 或 `qdrant`，仓库提供本地安装脚本：
- `scripts/local/install-tools.sh zig`
- `scripts/local/install-tools.sh qdrant`
- `scripts/local/install-tools.sh all`

这些工具会被安装到 `tools/local/bin/`，现有本地脚本会自动优先使用该目录。

### 一次性安装与初始化

这组命令主要在第一次拉起仓库时执行；它们不是日常启动命令。

1. 安装系统前置：Rust、Qdrant、Node/pnpm、Python/uv
2. 如果宿主机缺 `cc` 或 `qdrant`，先运行：
   - `scripts/local/install-tools.sh zig`
   - `scripts/local/install-tools.sh qdrant`
   - 或 `scripts/local/install-tools.sh all`
3. 运行 `scripts/local/bootstrap-linux.sh`
4. 运行 `scripts/local/doctor.sh` 验证环境

如果需要和默认本地服务隔离端口与运行目录，可给本地脚本加 `--dev`，例如：
- `scripts/local/bootstrap-linux.sh --dev`
- `scripts/local/doctor.sh --dev`

如果你想预热当前文本搜索模型缓存，再额外运行：
- `scripts/local/download-model.sh`

### 安装完成后的启动命令

这组命令用于安装完成后的日常启动：

1. 运行 `scripts/local/run.sh`

隔离开发配置的对应命令是：
- `scripts/local/run.sh --dev`

自动化场景可使用分离运行：
- `scripts/local/run.sh --dev --detach`
- `scripts/local/status.sh --dev --json`

### 安装完成后的停止命令

这组命令用于停止本地服务：

- 停止指定服务：`scripts/local/stop.sh app sidecar`
- 停止单个服务：`scripts/local/stop.sh qdrant`
- 停止全部服务：`scripts/local/stop.sh --all`
- 停止隔离开发配置的全部服务：`scripts/local/stop.sh --dev --all`
- 只查看会停止哪些进程：`scripts/local/stop.sh --all --dry-run`

`scripts/local/doctor.sh` 是诊断和校验命令，不属于服务启动命令；在改动依赖、端口或 `.env` 后，可以随时重跑。

### 安装完成后的验证命令

这组命令用于服务启动后验证当前文本搜索主链；它们不是安装命令，也不是启动命令：

1. 确认 `scripts/local/run.sh` 已经在运行
2. 运行 `scripts/local/smoke-text-search.sh`

如果服务使用 `--dev` 启动，验证命令也使用 `scripts/local/smoke-text-search.sh --dev`。
自动化场景可使用 `scripts/local/smoke-text-search.sh --dev --json` 获取机器可读摘要。

快速无 GPU 检查入口：
- `scripts/local/check.sh`

### 仓库内环境资产

- `.env.example`
  - 本地默认端口、运行目录和模型下载相关配置模板
- `.env`
  - 默认本地运行时配置
- `.env.dev.example`
  - 隔离开发配置模板，默认端口与运行目录和 `.env.example` 分开
- `.env.dev`
  - 通过 `--dev` 选择的本地运行时配置
  - 本地端口、目录与 URL 应只在当前选中的 env 文件里调整，不在代码或脚本里重复改常量
- `rust-toolchain.toml`
  - 固定 Rust `stable`，并启用 `rustfmt`、`clippy`
- `.python-version`
  - 固定 Python `3.12`
- `scripts/local/bootstrap-linux.sh`
  - 初始化 `.env`；带 `--dev` 时初始化 `.env.dev`
  - 创建运行目录
  - 创建 `.venv-test` 与 `.venv`
  - 安装 sidecar 与 UI 依赖
  - 安装 Playwright 浏览器
- `scripts/local/install-tools.sh`
  - 安装 repo-local `zig` / `cc` / `c++` / `qdrant`
  - 将工具放到 `tools/local/bin/`
- `scripts/local/doctor.sh`
  - 检查工具、端口、目录、虚拟环境和关键依赖
  - 优先检查 `.venv` 的 GPU 可用性，并在受限的 Codex 沙箱里避免把 CUDA 假阴性当成本地真实失败
  - 当选中的 env 文件缺失时回退读取对应 example，避免在脚本里重复维护硬编码默认值
- `scripts/local/run-qdrant.sh`
  - 启动或复用本机 Qdrant 进程
  - 以完全分离的后台方式启动，避免父 shell 退出时进程被带走
- `scripts/local/download-model.sh`
  - 按选中的 env 文件中的 `TEXT_SEARCH_MODEL_ID` / `TEXT_SEARCH_MODEL_REVISION` 预下载当前文本搜索模型
  - 继承选中 env 文件中的 `HF_ENDPOINT` / `HF_HUB_ENABLE_HF_TRANSFER`
  - 使用 Hugging Face 默认用户缓存
- `scripts/local/run.sh`
  - 自动启动或复用 Qdrant，并统一启动 Rust app、Python sidecar 和 UI
  - 要求选中 env 文件中声明的 app / sidecar / UI 端口空闲，并在健康检查通过后才报告成功
  - 支持 `--detach`，用于后台启动 app、sidecar 和 UI，并写入 pid 文件
- `scripts/local/status.sh`
  - 查询 app、sidecar、UI 和 Qdrant 的 URL、ready 状态、pid、日志路径与配置来源
  - 支持 `--json` 输出机器可读状态
- `scripts/local/stop.sh`
  - 停止指定本地服务，支持 `app`、`sidecar`、`ui`、`qdrant`
  - 支持 `--all` 停止全部服务，支持 `--dry-run` 预览将停止的进程
- `scripts/local/smoke-text-search.sh`
  - 在 app、sidecar 和 Qdrant 已启动后运行真实 GPU smoke
  - 验证图片和多页 PDF 页图能够进入 Qdrant-backed multivector 文本搜索链
  - 支持 `--json` 输出机器可读验证摘要
- `scripts/local/check.sh`
  - 运行无 GPU 快速检查，不启动长驻服务
- `tools/python/`
  - 放置本地脚本复用的 Python 工具源码
  - 当前承接 HTTP / 端口 / CUDA 探针、模型下载和文本搜索 smoke 的 Python 逻辑

### 默认端口与目录

默认 `.env.example`：

- App API：`127.0.0.1:53210`
- Python sidecar：`127.0.0.1:53211`
- UI dev server：`127.0.0.1:55173`
- Qdrant：`127.0.0.1:56333`
- 本地运行数据：`data/runtime/`

隔离开发 `.env.dev.example`：

- App API：`127.0.0.1:54210`
- Python sidecar：`127.0.0.1:54211`
- UI dev server：`127.0.0.1:56173`
- Qdrant：`127.0.0.1:57333`
- 本地运行数据：`data/runtime/dev/`

### 本地工作流状态

- `setup`：运行 `scripts/local/bootstrap-linux.sh`，准备 `.env`、依赖与运行目录；隔离开发配置使用 `scripts/local/bootstrap-linux.sh --dev`
- `diagnose`：运行 `scripts/local/doctor.sh`，检查工具、端口、虚拟环境和 CUDA
- `run`：运行 `scripts/local/run.sh`；它会自动启动或复用 Qdrant
- `status`：运行 `scripts/local/status.sh`，查看本地服务状态；自动化可加 `--json`
- `stop`：运行 `scripts/local/stop.sh`
- `test`：运行 `scripts/local/check.sh` 或更窄的 Rust / sidecar / UI 测试入口
- `smoke`：运行 `scripts/local/smoke-text-search.sh`，验证真实模型与 Qdrant 链路

### Python 环境策略

- `.venv-test`
  - 轻量 sidecar 测试环境
  - 默认用于 `.venv-test/bin/python -m pytest sidecar/tests`
- `.venv`
  - 唯一 GPU/runtime sidecar 环境
  - `bootstrap-linux.sh` 会安装 sidecar 运行依赖和与参考环境对齐的 `torch 2.10.0+cu130` 栈
  - `doctor.sh`、`run.sh`、`download-model.sh` 与 `smoke-text-search.sh` 都只使用它
  - sidecar 首次冷启动加载 ColQwen 模型可能需要数分钟；首次真实导入或 smoke 会显著慢于后续热路径

### 当前实现切片

- `src/`
  - Rust HTTP 服务，当前已提供库管理、路径导入、任务查询、视觉对象详情与 Qdrant 驱动的文本搜索
- `sidecar/`
  - Python sidecar，当前已提供 `/health`、`/capabilities` 与 `/embed`
  - 当前已接通 `query_embedding` 与 `document_embedding`
- `ui/`
  - 最小 Vite 工作台，当前已接通三栏工作流：左侧建库/导入/任务，中间搜索与结果，右侧预览与详情
  - 搜索结果与对象详情都使用 app 提供的稳定 preview 资源引用，而不是直接暴露本地文件路径

## 项目结构

主要目录：
- `src/`：Rust 主服务
- `sidecar/`：Python sidecar，ML 或媒体处理
- `ui/`：应用界面
- `tests/`：共享测试
