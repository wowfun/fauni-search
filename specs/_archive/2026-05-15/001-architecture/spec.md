# 001 架构 (Architecture)

定义 FauniSearch 的系统级稳定架构边界，明确一级架构组件 (Primary Architecture Components)、唯一编排中心 (Orchestration Center) 与允许的交互路径。

## 关键术语 (Terminology)

- 一级架构组件（Primary Architecture Components）
- 编排中心（Orchestration Center）

## 范围

- 一级架构组件及其职责边界
- 系统级编排中心与跨组件协调关系
- 组件间允许的直接交互路径与关键稳定协议

范围外：
- 定义状态与数据模型、对象身份或事实源归属
- 定义对外 API 形状、请求响应细节或跨进程 payload 契约
- 定义提供方的能力、配置档与配置模型
- 定义索引流水线状态机、检查点与激活细节
- 定义检索后端的具体产品选择与实现细节

## 设计原则

- 单一编排中心（Single Orchestration Center）：Rust 主服务是唯一系统级编排中心，系统级协调、状态收敛与对外入口都应由其承担
- 组件专职化（Component Specialization）：每个一级架构组件只承担一种主职责，不应跨边界吸收其他组件的系统级责任
- 显式边界（Explicit Boundaries）：一级架构组件只通过定义好的边界交互，不以隐式共享状态或旁路直连协作
- 窄跨进程边界（Narrow Cross-Process Boundary）：跨进程协作应通过稳定、最小化的结构化协议完成，不传递内部对象或形成隐式耦合

## 一级架构组件

- 前端/调用方：负责请求发起、结果消费与交互状态承载
- Rust 主服务：负责对外入口、跨组件调度、状态收敛、任务与搜索编排、结果组装
- Python sidecar：负责 ML 与媒体处理能力，例如模型加载、向量编码、视频分段、关键帧与预览拼图生成
- 检索后端：负责检索索引承载与查询执行
- 结构化存储：负责结构化持久状态存储

## 编排中心

- 系统级请求、状态协调与跨组件协作统一收敛到 Rust 主服务
- Rust 主服务负责发起和组织组件能力协作，不采用对等 mesh 式协作
- 其他一级架构组件不得形成与 Rust 主服务对等的系统级编排节点

## 交互边界

- 前端/调用方只与 Rust 主服务直接交互
- Rust 主服务可以与 Python sidecar 直接交互，稳定边界采用 HTTP/JSON；具体 payload 契约由 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义
- Rust 主服务可以与检索后端直接交互
- Rust 主服务可以与结构化存储直接交互
- 前端/调用方不应直接访问检索后端或结构化存储
- Python sidecar 不应成为前端/调用方的直接服务入口
- Python sidecar 不应直接承担系统级状态协调职责
- 检索后端与结构化存储之间的业务协作应由 Rust 主服务发起和组织

## 状态与数据模型承接

- `001` 只定义组件边界、编排中心与允许的交互路径，不再作为状态与数据模型的事实源
- 逻辑实体、状态族、事实源归属、可恢复性与核心对象身份语义由 [002-state-and-data-model](../002-state-and-data-model/spec.md) 定义
- 主结构化存储、检索命名空间、文件系统分区与迁移版本由 [007-storage-and-persistence](../007-storage-and-persistence/spec.md) 定义
- 摄取与索引、搜索、提供方、运行时执行、应用界面体验与接口契约的专题行为规则分别由 [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md)、[004-search](../004-search/spec.md)、[005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md)、[006-runtime-and-execution](../006-runtime-and-execution/spec.md)、[008-ui-ux](../008-ui-ux/spec.md) 与 [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接

## 关联主题

- [000-foundation](../000-foundation/spec.md) 定义项目级基础约束与上游设计原则
- [002-state-and-data-model](../002-state-and-data-model/spec.md) 承接逻辑实体、状态边界、事实源归属与身份语义
- [003-ingestion-and-indexing](../003-ingestion-and-indexing/spec.md) 承接来源边界、摄取流程、索引线生命周期、检查点、验证与激活语义
- [004-search](../004-search/spec.md) 承接搜索语义、结果组装边界与公开搜索行为规则
- [005-provider-capabilities-and-profiles](../005-provider-capabilities-and-profiles/spec.md) 承接提供方家族、提供方配置档、提供方能力与提供方绑定语义
- [006-runtime-and-execution](../006-runtime-and-execution/spec.md) 承接任务执行、运行时生命周期、健康状态与后台维护语义
- [007-storage-and-persistence](../007-storage-and-persistence/spec.md) 承接结构化存储、检索命名空间、文件系统持久化分层与迁移版本语义
- [008-ui-ux](../008-ui-ux/spec.md) 承接应用壳层、工作区、全局导航、管理体验与非搜索控制面接口族
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接公开 API 的请求 / 响应契约，以及 Rust 主服务与 Python sidecar 的稳定 payload 协议
