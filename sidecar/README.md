# FauniSearch Sidecar

本目录承载 Python sidecar 的本地开发入口。

当前阶段已经提供：

- `GET /health`
- `GET /capabilities`
- `POST /embed`

当前真实接通的操作包括：

- `operation_kind=query_embedding`
- `operation_kind=image_query_embedding`
- `operation_kind=video_query_embedding`
- `operation_kind=document_embedding`

这些操作都由本地 ColQwen3.5 模型在 GPU 环境中懒加载执行；`document_embedding` 当前承接图片与 PDF 页图编码，`image_query_embedding` 承接图片查询输入，`video_query_embedding` 承接整段视频或指定时间范围的视频查询输入。

补充约束：

- `POST /embed` 的成功响应 shape 保持稳定，不因当前批处理实现而改变
- `document_embedding` 当前允许批量输入，但实现可以基于运行时批大小上限拒绝过大的 `inputs.documents`
- 当 `document_embedding` 因批大小上限拒绝请求时，应返回统一的 `validation_failed`
