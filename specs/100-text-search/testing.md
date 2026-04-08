# 100 文本搜索测试设计

本文件是 `100-text-search` 专题的正式配套文档，承接与文本搜索能力直接相关的测试设计、覆盖维度与当前阶段验证方案。长期能力定义见 [spec.md](./spec.md)，当前阶段实施范围见 [plan.md](./plan.md)。仓库级通用测试规则继续以 [AGENTS.md](../../AGENTS.md) 为准，本文件不重复改写这些通用约束。

## 角色与边界

- 本文件只承接 `100-text-search` 专题内的测试设计，不替代仓库级测试规则
- 本文件同时覆盖：
  - 长期能力层的测试原则与覆盖维度
  - 当前阶段 `API + 最小 UI` 闭环的详细测试设计
- 本文件可以写当前阶段默认测试入口、fixture 策略与验证材料，但不重新定义 `004`、`009` 等基础事实源

## 长期测试原则

- 文本搜索测试长期上必须覆盖查询、结果与可搜索状态三大维度，而不是只验证理想命中路径
- 文本搜索长期结果对象覆盖 `document_page`、`image` 与 `video_segment`
- 成功命中、`not_ready` 与明确失败 / 不可搜索反馈都必须进入测试覆盖
- 文本搜索默认返回最小结果卡片；对象详情与邻近上下文应通过详情 / 展开路径另行验证
- 专题测试优先采用最贴近改动面的窄测试，再用更高层集成或端到端测试证明闭环

## 长期覆盖维度

### 查询

- 单库文本查询能够命中目标库内容
- 请求命中未启用索引线时返回明确拒绝
- 请求命中已启用但未 ready 的索引线时返回明确 `not_ready`

### 结果

- 文本搜索能够命中 `document_page`
- 文本搜索能够命中 `image`
- 文本搜索能够命中 `video_segment`
- 多类视觉对象允许在同一结果集中混排
- 结果卡片稳定返回最小字段，并能继续打开对象详情

### 可搜索状态

- 内容只有在正式接入与索引完成后才进入可搜索状态
- 导入 / 索引失败会产生明确失败或不可搜索反馈
- 无结果、未 ready 与失败三种外显结果彼此可区分

## 当前阶段默认测试入口

- Rust 主服务默认测试入口：`cargo test`
- Python sidecar 默认测试入口：`.venv-test/bin/python -m pytest sidecar/tests`
- 最小 UI 默认端到端入口：`Playwright`
- 无 GPU 快速检查入口：`bash scripts/local/check.sh`
- GPU smoke 默认验证入口：`bash scripts/local/smoke-text-search.sh`；隔离开发配置使用 `bash scripts/local/smoke-text-search.sh --dev`
- GPU smoke 默认验证路径：真实 `ColQwen3.5-4.5B-v3 + Qdrant`

这些入口是 `100-text-search` 当前阶段的实现假设，用于收敛测试设计；并不自动构成整个仓库的全局工具链事实源。

## 当前阶段测试分层

### Rust 主服务

- 使用 `cargo test` 覆盖当前阶段最贴近业务改动的逻辑
- 优先覆盖：
  - 建库配置中 `multivector` 的显式启用约束
  - 路径导入的部分接受与逐项原因汇总
  - 任务阶段推进与工作台所需任务摘要
  - 文本搜索命中 `document_page` / `image`
  - `not_ready` 与失败反馈映射

### Python sidecar

- 使用 `.venv-test/bin/python -m pytest sidecar/tests` 覆盖当前阶段模型与媒体处理链路
- 优先覆盖：
  - PDF 转页图与 `document_page` 视觉单元输入准备
  - 图片输入处理
  - `ColQwen3.5-4.5B-v3` 的查询与文档 embedding 路径
  - sidecar 在不可用、失败或输入异常时的稳定返回

### 最小 UI

- 使用 `Playwright` 覆盖单页工作台闭环
- 优先覆盖：
  - 创建 / 选择启用 `multivector` 的库
  - 路径导入与部分接受反馈
  - 嵌入式任务面板状态显示
  - 单文本框搜索
  - 混排结果列表
  - 侧边详情面板
  - `not_ready` 与失败反馈

### GPU Smoke

- 使用真实 `ColQwen3.5-4.5B-v3 + Qdrant` 环境做至少一次 GPU smoke
- GPU smoke 只证明当前阶段主链在目标环境中可运行，不替代窄测试或 UI E2E

## 当前阶段场景矩阵

| 场景 | 优先验证层 | 说明 |
| --- | --- | --- |
| 创建或选择启用 `multivector` 的目标库 | UI E2E + Rust 主服务 | 证明当前工作台始终有明确库上下文 |
| 路径导入的部分接受与逐项原因 | Rust 主服务 + UI E2E | 验证有效项进入单任务、无效项有逐项反馈 |
| 任务面板阶段显示与失败摘要 | Rust 主服务 + UI E2E | 验证嵌入式任务观察面 |
| 文本查询命中文档页 | Rust 主服务 + GPU smoke | 证明 `document_page` 命中路径 |
| 文本查询命中图片 | Rust 主服务 + GPU smoke | 证明 `image` 命中路径 |
| 同一结果集混排 | Rust 主服务 + UI E2E | 证明 `document_page + image` 混排呈现 |
| 侧边详情展示前后页或同源信息 | UI E2E | 证明对象详情与展开闭环 |
| `not_ready` 反馈 | Rust 主服务 + UI E2E | 证明未就绪不被伪装成空结果 |
| 导入 / 索引失败反馈 | Rust 主服务 + UI E2E | 证明失败结果可观察且不等于空结果 |

## Fixture 与验证材料

- 需要一个小型固定 PDF fixture，用于稳定验证：
  - PDF 导入
  - 页图生成
  - 文档页命中
  - 文档页详情中的前后页上下文
- 复用现有 [tests/fixtures/tatdqa-page-images/README.md](../../tests/fixtures/tatdqa-page-images/README.md) 所描述的图片 fixture，验证：
  - 图片导入
  - 图片命中
  - 图片详情中的同源基础信息
- fixture 设计应优先支持窄测试与集成测试复用，而不是只服务手工演示

## GPU Smoke

- GPU smoke 目标环境是 `Linux + NVIDIA`
- smoke 必须真实连接 Qdrant，并真实加载 `ColQwen3.5-4.5B-v3`
- 当前阶段默认通过 `bash scripts/local/smoke-text-search.sh` 执行 smoke；隔离开发配置使用 `bash scripts/local/smoke-text-search.sh --dev`；自动化可使用 `--json`；该脚本要求 app、sidecar 与 Qdrant 已经处于可访问状态
- smoke 至少验证以下路径：
  - 建库后导入 PDF / 图片
  - 导入任务推进到可搜索状态
  - 文本查询命中文档页
  - 文本查询命中图片
- GPU smoke 若因环境或依赖缺失无法运行，必须在验证结果中明确说明缺口，而不是默认为已验证

## Deferred Coverage

- `video_segment` 的实际索引、命中与详情覆盖
- 图片查询与视频查询覆盖
- 来源根扫描、规则、刷新与重扫的覆盖
- 完整任务中心与任务动作覆盖
- 独立运行时健康页覆盖
- Qdrant 托管模式覆盖
- `single-vector` 可运行实现覆盖

这些条目在当前阶段明确留空，但必须在后续阶段进入专题测试设计，而不是长期停留为未覆盖区。

## 关联文档

- [spec.md](./spec.md)
- [plan.md](./plan.md)
- [AGENTS.md](../../AGENTS.md)
