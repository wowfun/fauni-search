# 排障

默认先运行：

```bash
bash scripts/local/doctor.sh
```

`doctor.sh` 是当前第一诊断入口。下面只收录当前最常见的问题。

## `.env` 缺失

症状：
- `doctor.sh` 提示 `.env is missing`
- `run-qdrant.sh` 或 `run.sh` 直接报 `.env is missing`
- 使用 `--dev` 时，脚本提示 `.env.dev is missing`

检查：

```bash
ls -l .env .env.example .env.dev .env.dev.example
```

处理：
- 运行 `bash scripts/local/bootstrap-linux.sh`
- 如果使用隔离开发配置，运行 `bash scripts/local/bootstrap-linux.sh --dev`
- 如果只是不小心删了 `.env`，也可以先从 `.env.example` 重新生成

## `cc` 或 `qdrant` 不在 `PATH`

症状：
- `doctor.sh` 提示 `cc is missing` 或 `qdrant is missing`
- `cargo` 链接失败
- `run-qdrant.sh` 提示 `qdrant is not installed or not on PATH`

检查：

```bash
command -v cc
command -v qdrant
```

处理：
- 优先使用 repo-local 工具安装：

```bash
bash scripts/local/install-tools.sh zig
bash scripts/local/install-tools.sh qdrant
```

- 如果两者都缺，直接运行：

```bash
bash scripts/local/install-tools.sh all
```

## `.venv` 缺少 sidecar 运行依赖

症状：
- `run.sh` 提示 `.venv/bin/python is missing sidecar runtime dependencies`
- `sidecar.log` 中出现 `No module named 'uvicorn'`、`fastapi` 或 `fauni_sidecar`

检查：

```bash
PYTHONPATH="$PWD/sidecar/src" .venv/bin/python -c "import fastapi, uvicorn, fauni_sidecar"
```

处理：
- 重新运行 `bash scripts/local/bootstrap-linux.sh`
- 如果只想补 sidecar 依赖，可执行：

```bash
uv pip install --python .venv/bin/python -e "sidecar[gpu]"
```

## `torch.cuda.is_available()` 在 Codex 沙箱里是假阴性

症状：
- `doctor.sh` 输出类似 `.venv CUDA probe is negative inside the current Codex sandbox`
- 但你在正常终端里实际能用 CUDA

检查：

```bash
.venv/bin/python -c "import torch; print(torch.__version__); print(torch.version.cuda); print(torch.cuda.is_available()); print(torch.cuda.device_count())"
```

处理：
- 优先相信正常终端里的结果，不要把 Codex 沙箱内的 CUDA 假阴性当成真实故障
- 只要正常终端里 `torch.cuda.is_available()` 为 `True`，当前环境就可继续使用

## 端口冲突

症状：
- `run.sh` 报 `port ... is already in use`
- `doctor.sh` 把某个服务端口标成 `already occupied`

检查：

```bash
lsof -nP -iTCP:53210 -sTCP:LISTEN
lsof -nP -iTCP:53211 -sTCP:LISTEN
lsof -nP -iTCP:55173 -sTCP:LISTEN
lsof -nP -iTCP:56333 -sTCP:LISTEN
```

处理：
- 如果占用者是当前仓库的本地服务，优先运行：

```bash
bash scripts/local/stop.sh --all --dry-run
bash scripts/local/stop.sh app sidecar ui
```

- 如果占用者不是当前仓库的本地服务，优先切到隔离开发配置：

```bash
bash scripts/local/bootstrap-linux.sh --dev
bash scripts/local/run.sh --dev
```

- 如果仍然冲突，再修改本次运行选中的 env 文件里的对应端口，然后重新运行启动脚本

## Qdrant 不可达

症状：
- `run-qdrant.sh` 报 `Qdrant did not become ready`
- `run.sh` 报 `Qdrant failed to start`
- `run.sh` 报 `Qdrant is not reachable ... after starting`

检查：

```bash
bash scripts/local/run-qdrant.sh
curl -s http://127.0.0.1:56333/collections
tail -n 50 data/runtime/logs/qdrant.log
```

如果你使用 `--dev`，对应检查命令是：

```bash
bash scripts/local/run-qdrant.sh --dev
curl -s http://127.0.0.1:57333/collections
tail -n 50 data/runtime/dev/logs/qdrant.log
```

如果你改过选中的 env 文件里的端口，`curl` 地址也要按该文件改。

处理：
- 先确认选中的 env 文件中 `QDRANT_PORT` 和 `QDRANT_URL` 一致
- 确认 `qdrant` 命令可执行
- 再检查 `DEV_LOG_DIR` 下的 `qdrant.log`

## `run.sh` 启动失败后看哪里

症状：
- `run.sh` 在 app、sidecar 或 UI 的健康检查阶段失败

检查：

```bash
bash scripts/local/status.sh --json
tail -n 50 data/runtime/logs/app.log
tail -n 50 data/runtime/logs/sidecar.log
tail -n 50 data/runtime/logs/ui.log
```

处理：
- app 启不来：先看 `app.log`，常见是端口冲突或 Rust 运行失败
- sidecar 启不来：先看 `sidecar.log`，常见是 `.venv` 缺依赖或环境变量不对
- UI 启不来：先看 `ui.log`，常见是端口冲突或 `ui/node_modules` 没准备好

如果以上都不明显，回到第一步重新跑：

```bash
bash scripts/local/doctor.sh
```

如果使用 `run.sh --detach` 启动，先用 `status.sh` 查看 pid 与 ready 状态：

```bash
bash scripts/local/status.sh
bash scripts/local/status.sh --json
```

如果使用 `--dev`，上述 status 命令也要带 `--dev`。

## 右侧预览打不开或为空白

症状：
- 搜索结果能点开详情，但右侧图片或 PDF 预览为空白
- “打开预览”链接返回 404 或浏览器报加载失败

检查：

```bash
bash scripts/local/status.sh --json
tail -n 50 data/runtime/logs/app.log
```

处理：
- 当前预览由 app 提供稳定的 preview 资源入口，而不是 UI 直接读取本地文件；所以先确认 app 仍然处于 ready 状态
- 如果是 `--dev` 配置，确认你访问的是对应 `--dev` UI，而不是默认 `.env` 的 UI
- 如果 app 已重启过但浏览器页面是旧的，直接刷新 UI，或重新执行一次 `bash scripts/local/run.sh` / `bash scripts/local/run.sh --dev`
- 如果问题只出现在 PDF 预览，优先检查导入对象的 detail 接口是否还能正常返回 `preview.url`

## `smoke-text-search.sh` 失败

症状：
- `smoke-text-search.sh` 报 app、sidecar 或 Qdrant 不可达
- `smoke-text-search.sh` 报导入没有进入 `completed` / `activated`
- `smoke-text-search.sh` 报结果中缺少 `image` 或 `document_page`
- 首次冷启动时，`smoke-text-search.sh` 在一段时间内没有输出，看起来像卡住

检查：

```bash
bash scripts/local/status.sh --json
curl -s http://127.0.0.1:53210/health
curl -s http://127.0.0.1:53211/capabilities
curl -s http://127.0.0.1:56333/collections
tail -n 80 data/runtime/logs/app.log
tail -n 120 data/runtime/logs/sidecar.log
tail -n 80 data/runtime/logs/qdrant.log
```

如果你使用 `--dev`，对应端口来自 `.env.dev`，默认是 `54210`、`54211`、`57333`。如果你改过选中的 env 文件里的端口，`curl` 地址也要按该文件改。

处理：
- 先确认 `bash scripts/local/run.sh` 已经成功启动；它会自动启动或复用 Qdrant。如果使用 `--dev`，这里也要带 `--dev`
- 如果你只想单独诊断 Qdrant，再运行 `bash scripts/local/run-qdrant.sh`；如果使用 `--dev`，这里也要带 `--dev`
- 前台模式下 `run.sh` 退出后 app、sidecar 和 UI 会被清理；Qdrant 由 `run-qdrant.sh` 管理，不会被 `run.sh` 的清理流程隐式停止
- 如果使用分离模式，确认 `bash scripts/local/run.sh --detach` 已成功返回，并用 `status.sh` 检查 pid 文件和 ready 状态
- sidecar 首次冷启动加载 ColQwen 模型可能需要数分钟；在这一阶段，首次真实导入或 smoke 明显慢于后续热路径属于预期行为
- 如果失败摘要里出现 `runtime_unavailable` 或 `Sidecar ...`，优先看 `data/runtime/logs/sidecar.log`
- 如果失败摘要里出现 `Qdrant ...`，优先看 `data/runtime/logs/qdrant.log`

## `pnpm --dir ui test:e2e` 失败

症状：
- Playwright 启动前直接报 `.env.dev is missing`
- Playwright 报 `--dev runtime is partially running`
- Playwright 在浏览器启动阶段报类似 `error while loading shared libraries: libatk-1.0.so.0`
- UI smoke 跑到一半没有结果，或任务长期停在非终态

检查：

```bash
ls -l .env.dev .env.dev.example
bash scripts/local/status.sh --dev --json
tail -n 80 data/runtime/dev/logs/app.log
tail -n 120 data/runtime/dev/logs/sidecar.log
tail -n 80 data/runtime/dev/logs/ui.log
tail -n 80 data/runtime/dev/logs/qdrant.log
```

处理：
- 先确认你至少执行过一次 `bash scripts/local/bootstrap-linux.sh --dev`
- 如果 `status.sh --dev --json` 显示 app、sidecar、UI 只有部分在跑，先执行 `bash scripts/local/stop.sh --dev --all` 清干净，再重新运行 `pnpm --dir ui test:e2e`
- 如果错误发生在 Chromium 启动前，而且日志里出现 `libatk-1.0.so.0`、`libgtk-3.so.0` 之类缺库信息，问题在宿主机的 Playwright 浏览器运行库，不在仓库测试代码；先按宿主机方式补齐这些系统库，再重跑
- 这条 Playwright 命令固定只操作 `--dev` 配置；它不会复用默认 `.env` profile，也不应该去停止默认 profile 的服务
- 如果失败发生在导入或搜索阶段，优先按 `smoke-text-search.sh` 的排障路径继续看 app / sidecar / Qdrant 日志

## 模型下载失败

症状：
- `download-model.sh` 报 `.env is missing`
- 使用 `--dev` 时，`download-model.sh` 报 `.env.dev is missing`
- `download-model.sh` 报 `huggingface_hub is missing`
- `download-model.sh` 报 `HF_HUB_ENABLE_HF_TRANSFER=1 but hf_transfer is not installed`
- 下载几乎没速度，或每次重试都像从头开始
- `Ctrl-C` 后下载进程迟迟不停
- 下载过程报网络、认证或远端仓库访问失败

检查：

```bash
grep '^TEXT_SEARCH_MODEL_' .env
grep '^HF_' .env
.venv/bin/python -c "import huggingface_hub; print(huggingface_hub.__version__)"
.venv/bin/python -c "import hf_transfer; print(hf_transfer.__file__)"
bash scripts/local/download-model.sh --help
```

处理：
- 先确认本次运行选中的 env 文件里的 `TEXT_SEARCH_MODEL_ID` / `TEXT_SEARCH_MODEL_REVISION` 是你要的值
- 如果 `.venv` 缺依赖，重新运行 `bash scripts/local/bootstrap-linux.sh`
- 如果启用了 `HF_HUB_ENABLE_HF_TRANSFER=1`，确保 `.venv` 里已经装了 `hf_transfer`
- `huggingface_hub 0.36.2` 在 `HF_HUB_ENABLE_HF_TRANSFER=1` 时不会续传未完成的大文件；如果网络不稳、镜像表现差，或你更需要稳定中断/重试，直接把选中的 env 文件里的 `HF_HUB_ENABLE_HF_TRANSFER` 改成 `0`
- 当前脚本已经会在 `Ctrl-C` 后主动向下载子进程发送 `SIGTERM`，若仍卡住会升级到 `SIGKILL`
- 如果是网络或鉴权问题，先在正常终端里解决访问 Hugging Face 的问题，再重试 `bash scripts/local/download-model.sh`
