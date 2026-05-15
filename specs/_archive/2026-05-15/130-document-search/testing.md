# 130 文档搜索测试设计

本文件是 `130-document-search` 专题的正式配套文档，承接与文档搜索能力直接相关的测试设计、覆盖维度与当前阶段验证方案。长期能力定义见 [spec.md](./spec.md)，当前阶段实施范围见 [plan.md](./plan.md)。仓库级通用测试规则继续以 [AGENTS.md](../../AGENTS.md) 为准，本文件不重复改写这些通用约束。

## 角色与边界

- 本文件只承接 `130-document-search` 专题内的测试设计，不替代仓库级测试规则
- 本文件同时覆盖：
  - 长期能力层的测试原则与覆盖维度
  - 当前阶段 `API + 最小 UI` 闭环的详细测试设计
- 本文件可以写当前阶段默认测试入口、fixture 策略与验证材料，但不重新定义 `004`、`009`、`100`、`110`、`120` 等基础事实源

## 长期测试原则

- 文档搜索测试长期上必须覆盖查询输入、结果与可搜索状态三大维度，而不是只验证理想命中路径
- 文档搜索长期结果对象覆盖 `document_page`、`image` 与 `video_segment`
- 成功命中、无效查询输入、非法页范围、`not_ready` 与明确失败 / 不可搜索反馈都必须进入测试覆盖
- 文档搜索默认返回最小结果卡片；对象详情、页范围与预览应通过详情 / 展开路径另行验证
- 专题测试优先采用最贴近改动面的窄测试，再用更高层集成或端到端测试证明闭环

## 长期覆盖维度

### 查询输入

- 单库文档查询能够消费有效查询文档
- 查询文档缺失、过期、损坏或不可读取时返回明确失败
- 查询页范围缺失时默认整份文档
- 查询页范围非法、越界或不可解析时返回明确失败
- 库内对象引用若被启用，只有当前阶段支持的对象粒度能够作为查询文档；未启用对象类型返回明确拒绝
- 当前阶段支持的库内对象引用粒度是 `source_id`；`document_page` 复用应被解释为 `source_id + 单页范围`
- 单次文档搜索请求只承接一种文档输入，不伪装为文档 + 文本组合查询

### 结果

- 文档搜索能够命中 `document_page`
- 文档搜索能够命中 `image`
- 文档搜索能够命中 `video_segment`
- 多类 Asset 结果允许在同一结果集中按实际分数混排
- 结果卡片稳定返回最小字段，并能继续打开对象详情

### 可搜索状态

- 目标内容只有在正式接入与索引完成后才进入可搜索状态
- 请求命中已启用但未 ready 的索引线时返回明确 `not_ready`
- 导入 / 索引失败会产生明确失败或不可搜索反馈
- 无效查询输入、非法页范围、无结果、未 ready 与失败五种外显结果彼此可区分

## 当前阶段默认测试入口

- Rust 主服务默认测试入口：`cargo test`
- Python sidecar 默认测试入口：`.venv-test/bin/python -m pytest sidecar/tests`
- 最小 UI 默认端到端入口：`Playwright`
- 无 GPU 快速检查入口：`bash scripts/local/check.sh`
- GPU smoke 计划验证入口：`bash scripts/local/smoke-document-search.sh`；隔离开发配置使用 `bash scripts/local/smoke-document-search.sh --dev`
- GPU smoke 默认验证路径：真实本地视觉模型链路 + Qdrant

## 当前阶段测试分层

### Rust 主服务

- 使用 `cargo test` 覆盖当前阶段最贴近业务改动的逻辑
- 优先覆盖：
  - 查询文档上传成功创建临时查询资产
  - 非 PDF 上传被拒绝
  - 查询页范围缺失时默认整份文档
  - 非法、越界或缺失字段的页范围返回稳定失败
  - 库内 `source_id` 引用能够作为查询文档
  - `document_page` 结果复用路径映射为 `source_id + 单页范围`
  - 文档搜索命中 `document_page` / `image`
  - 若当前阶段 fixture 与实现已经具备稳定跨文档到视频时刻的召回，再额外覆盖 `video_segment`
  - `not_ready` 与失败反馈映射

### Python sidecar

- 使用 `.venv-test/bin/python -m pytest sidecar/tests` 覆盖当前阶段模型与文档处理链路
- 优先覆盖：
  - `document_query_embedding` 的整份 PDF 成功路径
  - `document_query_embedding` 的指定页范围成功路径
  - 越界页范围、损坏 PDF 或非法输入的稳定错误返回

### 最小 UI

- 使用 `Playwright` 覆盖共享搜索工作区中的文档搜索闭环
- 当前阶段 Playwright 至少覆盖以下路径：
  - 创建启用 `multi_vector_late_interaction` content types 的库
  - 先让 PDF、图片或视频内容进入可搜索状态
  - 切换到 `Document` 模式
  - 通过临时上传 PDF 提供查询文档
  - 通过数字输入控件指定页范围，或保持留空以表示整份文档
  - 在结果列表或详情里将库内 `document_page` 直接复用为查询文档
  - 执行文档搜索
  - 看到真实结果、`score` 与详情 / 预览
- 当前阶段应至少覆盖：
  - 整份文档查询 happy path
  - 指定页范围查询 happy path
  - `document_page` 结果复用查询
  - `not_ready`
  - 非 PDF 上传拒绝

### GPU Smoke

- 使用真实本地模型链路 + Qdrant 环境做至少一次 GPU smoke
- 当前阶段 `smoke-document-search.sh` 需要真实验证：
  - PDF 导入
  - PDF 查询上传
  - 整份文档查询
  - 指定页范围查询
  - API 层 `source_id` 整份文档 / 显式页范围查询
  - `document_page` 结果复用查询
  - 同一结果集中至少能看到 `document_page` 与 `image`
  - 若当前阶段本地 fixture 与实现已经具备稳定跨文档到视频时刻的召回，再额外验证 `video_segment`
  - 后端为 `qdrant`，表征为 `multi_vector_late_interaction`

## 当前阶段场景矩阵

| 场景 | 优先验证层 | 说明 |
| --- | --- | --- |
| 查询文档上传成功创建临时资产 | Rust 主服务 + UI E2E | 验证临时查询资产链路 |
| 非 PDF 上传被拒绝 | Rust 主服务 + UI E2E | 验证输入错误不被伪装成无结果 |
| 页范围缺失时按整份文档查询 | Rust 主服务 + UI E2E + GPU smoke | 验证默认整份文档语义 |
| 指定页范围时按文档片段查询 | Rust 主服务 + UI E2E + GPU smoke | 验证文档片段作为实际查询输入 |
| 非法页范围被拒绝 | Rust 主服务 | 验证非法范围不被隐式回退 |
| 库内 `source_id` 作为查询文档 | Rust 主服务 + GPU smoke | 验证整份文档或显式页范围的库内对象复用路径；当前阶段 UI 不单独暴露 `source_id` 选择器 |
| `document_page` 结果复用为查询文档 | Rust 主服务 + UI E2E + GPU smoke | 验证工作区派生复用路径 |
| 文档搜索命中 `document_page` | Rust 主服务 + GPU smoke | 证明主路径 |
| 文档搜索命中 `image` | Rust 主服务 + GPU smoke | 证明跨对象混排中的静态画面命中 |
| 文档搜索命中 `video_segment` | GPU smoke（可选扩展） | 证明跨对象混排中的视频片段命中；不是当前阶段 gate |
| 统一结果集按实际分数混排 | Rust 主服务 + UI E2E + GPU smoke | 证明不做人为对象分桶 |
| `not_ready` 反馈 | Rust 主服务 + UI E2E | 证明未就绪不被伪装成空结果 |

## Fixture 与验证材料

- 当前阶段 committed fixture 继续优先复用现有小型 PDF 与图片样本
- 若后续需要更贴近真实使用场景的文档样本，可补充 local-only fixture，而不是先把第三方文档直接提交进仓库
- fixture 设计应优先支持窄测试、GPU smoke 与 UI E2E 分层复用，而不是只服务手工演示

## Deferred Coverage

- docx / pptx / xlsx 查询文档覆盖
- 多区间页范围查询覆盖
- 文档 + 文本组合查询覆盖
- 文档 URL 引用接入覆盖
- 查询文档长期保存、历史管理与复用覆盖
- 来源根扫描、规则、刷新与重扫覆盖
- 完整任务中心与任务动作覆盖
- 独立运行时健康页覆盖
- Qdrant 托管模式覆盖
- `single-vector` 可运行实现覆盖

## 关联文档

- [spec.md](./spec.md)
- [plan.md](./plan.md)
- [../008-ui-ux/search-workspace.md](../008-ui-ux/search-workspace.md)
- [AGENTS.md](../../AGENTS.md)
