# FauniSearch Sidecar

本目录承载 Python sidecar 的本地开发入口。

当前阶段已经提供：

- `GET /health`
- `GET /capabilities`
- `POST /embed`

当前真实接通的操作包括：

- `operation_kind=query_embedding`
- `operation_kind=document_embedding`

两者都由本地 ColQwen3.5 模型在 GPU 环境中懒加载执行；`document_embedding` 当前承接图片和 PDF 首页图的编码。
