# 120 视频搜索测试设计

本文件是 `120-video-search` 专题的正式配套文档，承接与视频搜索能力直接相关的测试设计、覆盖维度与当前阶段验证方案。长期能力定义见 [spec.md](./spec.md)，当前阶段实施范围见 [plan.md](./plan.md)。仓库级通用测试规则继续以 [AGENTS.md](../../AGENTS.md) 为准，本文件不重复改写这些通用约束。

## 角色与边界

- 本文件只承接 `120-video-search` 专题内的测试设计，不替代仓库级测试规则
- 本文件同时覆盖：
  - 长期能力层的测试原则与覆盖维度
  - 当前阶段 `API + 最小 UI` 闭环的详细测试设计
- 本文件可以写当前阶段默认测试入口、fixture 策略与验证材料，但不重新定义 `004`、`009`、`100`、`110` 等基础事实源

## 长期测试原则

- 视频搜索测试长期上必须覆盖查询输入、结果与可搜索状态三大维度，而不是只验证理想命中路径
- 视频搜索长期结果对象覆盖 `video_segment`、`image` 与 `document_page`
- 成功命中、无效查询输入、非法时间范围、`not_ready` 与明确失败 / 不可搜索反馈都必须进入测试覆盖
- 视频搜索默认返回最小结果卡片；对象详情、时间范围与预览应通过详情 / 展开路径另行验证
- 专题测试优先采用最贴近改动面的窄测试，再用更高层集成或端到端测试证明闭环

## 长期覆盖维度

### 查询输入

- 单库视频查询能够消费有效查询视频
- 查询视频缺失、过期、损坏或不可读取时返回明确失败
- 查询时间范围缺失时默认整段视频
- 查询时间范围非法、越界或不可解析时返回明确失败
- 库内对象引用若被启用，只有当前阶段支持的对象粒度能够作为查询视频；未启用对象类型返回明确拒绝
- 当前阶段支持的库内对象引用粒度包括 `source_id` 与库内 `video_segment`
- 单次视频搜索请求只承接一种视频输入，不伪装为视频 + 文本组合查询

### 结果

- 视频搜索能够命中 `video_segment`
- 视频搜索能够命中 `image`
- 视频搜索能够命中 `document_page`
- 多类 Asset 结果允许在同一结果集中按实际分数混排
- 结果卡片稳定返回最小字段，并能继续打开对象详情

### 可搜索状态

- 目标内容只有在正式接入与索引完成后才进入可搜索状态
- 请求命中已启用但未 ready 的索引线时返回明确 `not_ready`
- 导入 / 索引失败会产生明确失败或不可搜索反馈
- 无效查询输入、非法时间范围、无结果、未 ready 与失败五种外显结果彼此可区分

## 当前阶段默认测试入口

- Rust 主服务默认测试入口：`cargo test`
- Python sidecar 默认测试入口：`.venv-test/bin/python -m pytest sidecar/tests`
- 最小 UI 默认端到端入口：`Playwright`
- 无 GPU 快速检查入口：`bash scripts/local/check.sh`
- 本地视频样本派生入口：`python3 tools/python/extract_video_artifacts.py --manifest <local-manifest> --all`
- GPU smoke 当前验证入口：`bash scripts/local/smoke-video-search.sh`；隔离开发配置使用 `bash scripts/local/smoke-video-search.sh --dev`
- GPU smoke 默认验证路径：真实本地视觉模型链路 + Qdrant

这些入口是 `120-video-search` 当前阶段的实现假设，用于收敛测试设计；并不自动构成整个仓库的全局工具链事实源。

## 当前阶段测试分层

### Rust 主服务

- 使用 `cargo test` 覆盖当前阶段最贴近业务改动的逻辑
- 优先覆盖：
  - 查询视频上传成功创建临时查询资产
  - 非视频上传被拒绝
  - 查询时间范围缺失时默认整段视频
  - 非法、越界或缺失字段的时间范围返回稳定失败
  - 库内 `source_id` 引用能够作为查询视频
  - 库内 `video_segment` 引用能够作为查询视频
  - 不受支持的库内对象引用返回稳定 `not_supported`
  - 视频搜索命中 `video_segment` / `image` / `document_page`
  - `not_ready` 与失败反馈映射

### Python sidecar

- 使用 `.venv-test/bin/python -m pytest sidecar/tests` 覆盖当前阶段模型与视频处理链路
- 优先覆盖：
  - `video_query_embedding` 的整段视频成功路径
  - `video_query_embedding` 的指定时间范围成功路径
  - 非法视频路径、损坏视频或非法范围的稳定错误返回
  - sidecar 在不可用、失败或输入异常时的稳定返回

### 最小 UI

- 使用 `Playwright` 覆盖共享搜索工作区中的视频搜索闭环
- 当前阶段 Playwright 至少覆盖以下路径：
  - 创建启用 `multi_vector_late_interaction` content types 的库
  - 先让视频、图片或 PDF 内容进入可搜索状态
  - 切换到 `Video` 模式
  - 通过临时上传视频或库内视频对象引用提供查询视频
  - 在结果列表或详情里将库内 `video_segment` 直接复用为查询视频片段
  - 在需要时通过时间轴拖选指定时间范围
  - 执行视频搜索
  - 看到真实结果、`score` 与详情 / 预览
- 当前阶段应至少覆盖：
  - 整段视频查询 happy path
  - 指定时间范围查询 happy path
  - `not_ready`
  - 非视频上传拒绝
  - 库内 `source_id` 作为查询视频的 happy path
  - 库内 `video_segment` 作为查询视频的 happy path
- 非法时间范围反馈在当前阶段优先由 Rust 主服务与 API 层验证；共享工作区的时间范围滑块应尽量阻止用户形成非法范围，而不是依赖 UI E2E 反复构造无效输入

### GPU Smoke

- 使用真实本地模型链路 + Qdrant 环境做至少一次 GPU smoke
- GPU smoke 只证明当前阶段主链在目标环境中可运行，不替代窄测试或 UI E2E
- 当前阶段 `smoke-video-search.sh` 需要真实验证：
  - local-only manifest 可读取
  - 对应本地视频样本存在
  - 截图与 clip 可从样本自动派生
  - 视频、派生截图与派生 PDF 能被导入到同一目标库
  - 临时上传视频 + 可选时间范围能够触发真实 `/search/video`
  - 库内 `source_id + 可选时间范围` 能再次触发真实 `/search/video`
  - 同一结果集中能看到 `video_segment`、`image` 与 `document_page`
  - 后端为 `qdrant`，表征为 `multi_vector_late_interaction`
- GPU smoke 与手工验收可以先通过 local-only manifest 自动派生所需截图和 clip，而不是手工维护第二套视频子样本

## 当前阶段场景矩阵

| 场景 | 优先验证层 | 说明 |
| --- | --- | --- |
| 创建或选择启用 `multi_vector_late_interaction` content types 的目标库 | UI E2E + Rust 主服务 | 证明当前工作台始终有明确库上下文 |
| 查询视频上传成功创建临时资产 | Rust 主服务 + UI E2E | 验证临时查询资产链路 |
| 非视频上传被拒绝 | Rust 主服务 + UI E2E | 验证输入错误不被伪装成无结果 |
| 时间范围缺失时按整段视频查询 | Rust 主服务 + UI E2E + GPU smoke | 验证默认整段视频语义 |
| 指定时间范围时按视频片段查询 | Rust 主服务 + UI E2E + GPU smoke | 验证视频片段作为实际查询输入 |
| 非法时间范围被拒绝 | Rust 主服务 | 验证非法范围不被隐式回退 |
| 库内 `source_id` 作为查询视频 | Rust 主服务 + UI E2E + GPU smoke | 验证整段视频或显式时间范围的库内对象复用路径 |
| 库内 `video_segment` 作为查询视频 | Rust 主服务 + UI E2E + GPU smoke | 验证直接复用相似时刻作为新查询片段 |
| 视频查询命中 `video_segment` | Rust 主服务 + GPU smoke | 证明 moment retrieval 主路径 |
| 视频查询命中 `image` | Rust 主服务 + GPU smoke | 证明跨对象混排中的静态画面命中 |
| 视频查询命中 `document_page` | Rust 主服务 + GPU smoke | 证明跨对象混排中的页面命中 |
| 统一结果集按实际分数混排 | Rust 主服务 + UI E2E + GPU smoke | 证明不做人为对象分桶 |
| `video_segment` 详情展示对应时间范围 | UI E2E | 证明对象详情与时间范围预览闭环 |
| `not_ready` 反馈 | Rust 主服务 + UI E2E | 证明未就绪不被伪装成空结果 |

## Fixture 与验证材料

- committed fixture 当前阶段先不强制要求视频样本进入仓库
- local-only smoke fixture 当前优先使用：
  - [generate_q2_report_from_csv_bank_data-720-512.local.manifest.json](../../data/generate_q2_report_from_csv_bank_data-720-512.local.manifest.json)
  - 配套视频样本 [generate_q2_report_from_csv_bank_data-720-512.mp4](../../data/generate_q2_report_from_csv_bank_data-720-512.mp4)
- 上述 local-only 样本用于：
  - GPU smoke
  - 人工验收
  - 后续 `smoke-video-search` 的时间范围样本基线
- 当前允许在本地 smoke 过程中直接从视频中派生所需截图、关键帧或 clip，用于查询输入、时间范围样本或人工比对；这些派生文件默认继续留在被忽略的 `data/` 或运行时目录
- 当前阶段允许通过 `tools/python/extract_video_artifacts.py` 基于 local-only manifest 自动生成：
  - 每个 moment 的中点截图
  - 每个 moment 的视频 clip
  - 可供后续 smoke / 人工验收复用的索引清单
- fixture 设计应优先支持窄测试、GPU smoke 与 UI E2E 分层复用，而不是只服务手工演示

## GPU Smoke

- GPU smoke 目标环境是 `Linux + NVIDIA`
- smoke 必须真实连接 Qdrant，并真实加载当前阶段本地视频查询模型链路
- 当前阶段计划通过 `bash scripts/local/smoke-video-search.sh` 执行 smoke；隔离开发配置使用 `bash scripts/local/smoke-video-search.sh --dev`；自动化应支持 `--json`
- 当前阶段 smoke 至少应覆盖以下路径：
  - 建库后导入视频
  - 建库后导入派生截图与派生 PDF，形成三类结果对象的最小混排样本
  - 导入任务推进到可搜索状态
  - 上传查询视频并获得临时查询资产引用
  - 按整段视频执行一次查询
  - 按指定时间范围执行一次查询
  - 使用库内 `source_id` 再次发起查询
  - 使用库内 `video_segment` 再次发起查询
  - 结果中出现 `video_segment`
  - 在合理样本下，结果中允许出现 `image` 与 `document_page`
- 当前 local-only 样本的首批人工标注时间范围包括：
  - `10000-24000`：浏览 bank transaction CSV
  - `43000-48000`：查看 Agent 终端命令
  - `82000-89000`：在浏览器打开生成的报告页面
  - `92000-105000`：再次查看 Agent 终端命令
- GPU smoke 若因环境、依赖或样本缺失无法运行，必须在验证结果中明确说明缺口，而不是默认为已验证

## Deferred Coverage

- 视频 URL 引用接入覆盖
- 视频 + 文本组合查询覆盖
- `video_segment` 直接作为查询输入的覆盖
- 多区间时间范围查询覆盖
- 关键帧手选、拖拽上传与区域搜索覆盖
- 查询视频长期保存、历史管理与复用覆盖
- 来源根扫描、规则、刷新与重扫覆盖
- 完整任务中心与任务动作覆盖
- 独立运行时健康页覆盖
- Qdrant 托管模式覆盖
- `single-vector` 可运行实现覆盖

这些条目在当前阶段明确留空，但必须在后续阶段进入专题测试设计，而不是长期停留为未覆盖区。

## 关联文档

- [spec.md](./spec.md)
- [plan.md](./plan.md)
- [../008-ui-ux/search-workspace.md](../008-ui-ux/search-workspace.md)
- [AGENTS.md](../../AGENTS.md)
