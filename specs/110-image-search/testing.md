# 110 图片搜索测试设计

本文件是 `110-image-search` 专题的正式配套文档，承接与图片搜索能力直接相关的测试设计、覆盖维度与当前阶段验证方案。长期能力定义见 [spec.md](./spec.md)，当前阶段实施范围见 [plan.md](./plan.md)。仓库级通用测试规则继续以 [AGENTS.md](../../AGENTS.md) 为准，本文件不重复改写这些通用约束。

## 角色与边界

- 本文件只承接 `110-image-search` 专题内的测试设计，不替代仓库级测试规则
- 本文件同时覆盖：
  - 长期能力层的测试原则与覆盖维度
  - 当前阶段 `API + 最小 UI` 闭环的详细测试设计
- 本文件可以写当前阶段默认测试入口、fixture 策略与验证材料，但不重新定义 `004`、`009`、`100` 等基础事实源

## 长期测试原则

- 图片搜索测试长期上必须覆盖查询输入、结果与可搜索状态三大维度，而不是只验证理想命中路径
- 图片搜索长期结果对象覆盖 `document_page`、`image` 与 `video_segment`
- 成功命中、无效查询输入、`not_ready` 与明确失败 / 不可搜索反馈都必须进入测试覆盖
- 图片搜索默认返回最小结果卡片；对象详情与邻近上下文应通过详情 / 展开路径另行验证
- 专题测试优先采用最贴近改动面的窄测试，再用更高层集成或端到端测试证明闭环

## 长期覆盖维度

### 查询输入

- 单库图片查询能够消费有效查询图片
- 查询图片缺失、过期、损坏或不可读取时返回明确失败
- 库内对象引用若被启用，只有当前阶段支持的视觉对象类型能够作为查询图片；未启用对象类型返回明确拒绝
- 单次图片搜索请求只承接一种图片输入，不伪装为图片 + 文本组合查询

### 结果

- 图片搜索能够命中 `document_page`
- 图片搜索能够命中 `image`
- 图片搜索能够命中 `video_segment`
- 多类视觉对象允许在同一结果集中混排
- 结果卡片稳定返回最小字段，并能继续打开对象详情

### 可搜索状态

- 目标内容只有在正式接入与索引完成后才进入可搜索状态
- 请求命中已启用但未 ready 的索引线时返回明确 `not_ready`
- 导入 / 索引失败会产生明确失败或不可搜索反馈
- 无效查询输入、无结果、未 ready 与失败四种外显结果彼此可区分

## 当前阶段默认测试入口

- Rust 主服务默认测试入口：`cargo test`
- Python sidecar 默认测试入口：`.venv-test/bin/python -m pytest sidecar/tests`
- 最小 UI 默认端到端入口：`Playwright`
- 无 GPU 快速检查入口：`bash scripts/local/check.sh`
- GPU smoke 计划验证入口：`bash scripts/local/smoke-image-search.sh`；隔离开发配置使用 `bash scripts/local/smoke-image-search.sh --dev`
- GPU smoke 默认验证路径：真实 `ColQwen3.5-4.5B-v3 + Qdrant`

这些入口是 `110-image-search` 当前阶段的实现假设，用于收敛测试设计；并不自动构成整个仓库的全局工具链事实源。

## 当前阶段测试分层

### Rust 主服务

- 使用 `cargo test` 覆盖当前阶段最贴近业务改动的逻辑
- 优先覆盖：
  - 查询图片上传成功创建临时查询资产
  - 非图片上传被拒绝
- 过期、缺失或不可读取的临时查询资产返回稳定失败
- 过期或缺失的临时查询资产会被主动回收，不在内存状态或临时目录中无限保留
- 库内 `image` 对象引用能够作为查询图片
- 库内 `document_page` 对象引用能够作为查询图片
  - 非 `image` / `document_page` 的库内对象引用返回稳定 `not_supported`
  - 图片搜索命中 `document_page` / `image`
  - `not_ready` 与失败反馈映射

### Python sidecar

- 使用 `.venv-test/bin/python -m pytest sidecar/tests` 覆盖当前阶段模型与图片处理链路
- 优先覆盖：
  - `image_query_embedding` 的成功路径
  - 图片查询 embedding 与文档 embedding 的输入区分
  - 非法图片路径或损坏图片的稳定错误返回
  - sidecar 在不可用、失败或输入异常时的稳定返回

### 最小 UI

- 使用 `Playwright` 覆盖共享搜索工作区中的图片搜索闭环
- 当前阶段 Playwright 至少覆盖以下 3 条路径：
  - 创建启用 `multivector` 的库
  - 先让图片 / PDF 内容进入可搜索状态
  - 切换到 `Image` 模式
  - 通过文件选择或剪贴板粘贴提供查询图片
  - 执行图片搜索
  - 看到真实结果、`score` 与详情 / 预览
- 使用剪贴板粘贴图片并完成图片搜索 happy path
- 建库后直接切换到 `Image` 模式并发起查询，验证 `not_ready`
- 上传非图片文件，验证查询图片上传拒绝反馈
- 使用库内 `image` 结果对象直接作为查询图片再次发起搜索
- 使用库内 `document_page` 结果对象直接作为查询图片再次发起搜索
- 当前阶段第一条 UI smoke 默认使用 `--dev` 隔离配置；若 `--dev` 服务已存在则复用，否则自行启动并在结束后自清理

### GPU Smoke

- 使用真实 `ColQwen3.5-4.5B-v3 + Qdrant` 环境做至少一次 GPU smoke
- GPU smoke 只证明当前阶段主链在目标环境中可运行，不替代窄测试或 UI E2E

## 当前阶段场景矩阵

| 场景 | 优先验证层 | 说明 |
| --- | --- | --- |
| 创建或选择启用 `multivector` 的目标库 | UI E2E + Rust 主服务 | 证明当前工作台始终有明确库上下文 |
| 查询图片上传成功创建临时资产 | Rust 主服务 + UI E2E | 验证临时查询资产链路 |
| 剪贴板粘贴图片成功进入查询资产链路 | UI E2E | 验证工作区支持像搜索框一样的粘贴图片入口 |
| 非图片上传被拒绝 | Rust 主服务 + UI E2E | 验证输入错误不被伪装成无结果 |
| 过期或缺失的临时查询资产被主动回收 | Rust 主服务 | 验证临时查询资产不会无限堆积 |
| 库内 `image` 对象引用作为查询图片 | Rust 主服务 + UI E2E | 验证共享工作区内的对象复用路径 |
| 库内 `document_page` 对象引用作为查询图片 | Rust 主服务 + UI E2E + GPU smoke | 验证文档页作为 query image 的路径 |
| 非 `image` / `document_page` 的库内对象引用被拒绝 | Rust 主服务 | 验证当前阶段只启用受支持的对象类型 |
| 图片查询命中文档页 | Rust 主服务 + GPU smoke | 证明 `document_page` 命中路径 |
| 图片查询命中图片 | Rust 主服务 + GPU smoke | 证明 `image` 命中路径 |
| 同一结果集混排 | Rust 主服务 + UI E2E | 证明 `document_page + image` 混排呈现 |
| 侧边详情展示前后页或同源信息 | UI E2E | 证明对象详情与展开闭环 |
| `not_ready` 反馈 | Rust 主服务 + UI E2E | 证明未就绪不被伪装成空结果 |
| 查询输入失效或上传失败反馈 | Rust 主服务 + UI E2E | 证明输入错误可观察且不等于空结果 |

## Fixture 与验证材料

- 复用现有 [tests/fixtures/tatdqa-page-images/README.md](../../tests/fixtures/tatdqa-page-images/README.md) 所描述的图片 fixture，验证：
  - 查询图片上传
  - 图片命中
  - 图片详情中的同源基础信息
- 复用或补充一个小型固定 PDF fixture，验证：
  - 文档页命中
  - 文档页详情中的前后页上下文
- fixture 设计应优先支持窄测试、GPU smoke 与 UI E2E 复用，而不是只服务手工演示

## GPU Smoke

- GPU smoke 目标环境是 `Linux + NVIDIA`
- smoke 必须真实连接 Qdrant，并真实加载 `ColQwen3.5-4.5B-v3`
- 当前阶段计划通过 `bash scripts/local/smoke-image-search.sh` 执行 smoke；隔离开发配置使用 `bash scripts/local/smoke-image-search.sh --dev`；自动化应支持 `--json`
- smoke 至少验证以下路径：
  - 建库后导入 PDF / 图片
  - 导入任务推进到可搜索状态
  - 上传查询图片并获得临时查询资产引用
  - 使用库内 `image` 结果对象作为查询图片再次发起搜索
  - 使用库内 `document_page` 结果对象作为查询图片再次发起搜索
  - 图片查询命中文档页
  - 图片查询命中图片
- GPU smoke 若因环境或依赖缺失无法运行，必须在验证结果中明确说明缺口，而不是默认为已验证

## Deferred Coverage

- `video_segment` 的实际索引、命中与详情覆盖
- 图片 + 文本组合查询覆盖
- 除 `image` / `document_page` 之外的其他对象直接作为查询图片的覆盖
- 查询图片圈选、裁剪与区域搜索覆盖
- 查询图片长期保存、历史管理与复用覆盖
- 视频查询覆盖
- 来源根扫描、规则、刷新与重扫的覆盖
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
