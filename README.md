# FauniSearch

FauniSearch 是一个本地优先的多模态检索系统。

当前阶段已经具备真实链路和最小 UI 闭环，但仍然以研发和验证为主。

## Features

- 以文搜索：以文本作为查询，在同一库中检索图片、文档页和视频片段相关内容
- 以图搜索：支持上传单张查询图片，也支持把库内 `image` / `document_page` 结果对象直接复用为新的 query image
- 以视频搜索：支持上传本地查询视频、指定时间范围的视频片段查询，以及把库内 `video_segment` 直接复用为新的查询视频片段
- 以文档搜索：支持上传 PDF、整份文档查询、页范围查询，以及把库内 `document_page` 结果对象直接复用为新的查询文档

## Quick Start

1. 初始化仓库环境

```bash
bash scripts/local/bootstrap-linux.sh
```

2. 启动本地服务

```bash
bash scripts/local/run.sh
```

## Docs

- [本地运行](./docs/run-local.md)：最小安装、启动、验证和运行期说明
- [排障](./docs/troubleshooting.md)：常见问题入口

## 项目结构

主要目录：
- `src/`：Rust 主服务
- `sidecar/`：Python sidecar，ML 与媒体处理
- `ui/`：应用界面
- `tests/`：共享测试与集成验证

## Development Baseline

- 当前标准开发环境：`Linux/WSL2 + NVIDIA GPU`
- 当前主链：Rust 主服务 + Python sidecar + Qdrant + 最小 UI
- 当前模型策略：`ColQwen3.5-4.5B-v3` 权重懒加载
- 当前阶段不使用 Docker、devcontainer 或 Nix

如果你准备修改能力边界或实现语义，先读对应 `specs/` 下的事实源；如果你只是想跑起来或排障，优先从 [docs/run-local.md](./docs/run-local.md) 和 [docs/troubleshooting.md](./docs/troubleshooting.md) 开始。
