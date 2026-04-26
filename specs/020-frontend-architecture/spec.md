# 020 前端架构 (Frontend Architecture)

定义 FauniSearch 前端实现层的稳定组织边界，明确生产 UI 代码、样式入口与 UI 侧端到端辅助代码的模块拆分原则，避免前端长期收敛为单体文件。

## 关键术语 (Terminology)

- 前端引导层 (Frontend Bootstrap)
- 前端共享状态 (Frontend Shared State)
- 工作区模块 (Workspace Module)
- 样式聚合入口 (Stylesheet Aggregation Entry)
- UI 场景辅助代码 (UI Scenario Helpers)

## 范围

- `ui/src` 中生产 UI 代码的稳定模块边界
- `ui/src` 中样式文件的聚合入口与分层组织
- `ui/tests/e2e/helpers` 中 UI 侧场景辅助代码的拆分原则
- 生产 Web 构建产物由 Rust server 托管时，前端产物边界与开发期 Vite 边界
- 保行为前提下的前端重构约束与依赖方向

范围外：
- 前端框架迁移、状态库迁移或路由方案替换
- 后端 JSON API 请求 / 响应契约或 Rust / Python 运行时语义调整
- 产品信息架构、工作区职责或可见交互规则变更
- 设计 token、像素级视觉规范或组件级样式细节

## 设计原则

- 保行为重构（Behavior-Preserving Refactor）：前端重构默认不得改变正式用户可见行为；仅允许修复直接阻塞拆分的结构性问题
- 粗粒度模块（Coarse Modules）：模块应按共享状态、共享渲染、工作区渲染、事件绑定与测试域拆分，不追求一函数一文件
- 单向依赖（One-Way Dependencies）：共享类型与共享状态位于依赖底层，渲染模块依赖共享状态，事件绑定依赖渲染与共享状态，引导层位于最上层
- 壳层连续性（Same-App Continuity）：前端代码组织不得削弱 `Search`、`Inventory`、`Settings` 作为同一壳层下正式工作区的连续性
- 样式集中入口（Single CSS Entry）：生产入口文件只应导入一个样式聚合入口，而不是继续直接依赖大型单体样式文件
- CLI 托管生产入口（CLI-hosted Production Entry）：生产 Web 入口由 `faus web` 的本地 Web server 从 `ui/dist` 托管，Vite 只作为开发期前端服务器与代理
- 测试辅助分域（Scenario Helpers By Domain）：UI 侧 E2E helper 应按搜索、工作区、来源管理、设置、运行时与共享 fixture 拆分，避免单体场景文件继续膨胀

## 生产 Web 托管

- `ui/dist` 是 `faus web` 托管生产 Web 的构建产物来源
- `faus web` 的本地 Web server `GET /` 应返回 `ui/dist/index.html`，作为正式 Web 入口
- `ui/dist/assets/*` 等构建产物应由 `faus web` 本地 Web server 作为静态资产提供；这些资产路径不属于前端 API 事实源
- 前端客户端路由需要的 SPA fallback 应回到 `index.html`，但不得覆盖 `/openapi.json`、`/health`、`/runtime/status` 或其他公开 App API proxy 路径
- 当 `ui/dist/index.html` 缺失时，App API server 不应因此启动失败；`faus web` 应返回明确的 Web assets 未构建错误
- Vite dev server 与 `/api` proxy 仅服务前端开发期；生产 Web 不依赖 Vite 运行
- 前端代码不得依赖 Vite-only 路径、环境或代理语义才能在 `faus web` 托管的生产 Web 中工作

## 生产 UI 代码组织

- `ui/src/main.ts` 必须保持为前端引导层，而不是继续承载完整应用实现
- 前端引导层当前至少承接：
  - 样式聚合入口导入
  - 应用初始化
  - 首次刷新或启动流程触发
  - 轮询或等价后台同步启动
- 共享环境变量解析、稳定端点配置、共享常量、全局应用状态与共享状态选择器必须从前端引导层中抽离
- 共享 API 请求辅助逻辑必须独立于工作区渲染代码，不得继续散落在工作区实现之间
- `Search`、`Inventory`、`Settings` 三个正式工作区必须拥有各自可辨认的工作区模块；工作区模块至少应承接：
  - 该工作区的渲染函数
  - 该工作区特有的派生计算
  - 该工作区特有的事件处理
- 共享壳层、共享图标、上下文条、统一辅助面、跨工作区状态摘要与共享 UI 原语必须从工作区模块中抽离，进入共享渲染层或等价共享模块
- 事件绑定与启动期 DOM 线缆不得继续与共享渲染或单个工作区渲染混写在同一大段单体实现中

### 第二轮共享边界

- `ui/src/app/core.ts` 不得继续作为“大而全共享入口”；第二轮完成后，它只能作为薄兼容层或 barrel，不再承载共享状态、共享选择器、API 请求、query asset 辅助、runtime/job 摘要或共享 UI 片段本体
- 第二轮前端底层边界固定为：
  - `app/state/`：全局 store、初始状态、共享常量、DOM root 与最基础 state mutation
  - `app/selectors/`：`library`、`search`、`inventory`、`settings`、`runtime/jobs` 的纯派生读取与格式化摘要
  - `app/api/`：`request`、`refresh`、`search`、`query-assets`
  - `app/render/shared/`：icons、preview surface、library context、shared strip / badge / bridge / jobs 视图片段
- 禁止把 `state`、selector 计算、API 调用、query asset helper、runtime/job 摘要或共享 UI 片段重新放回同一个共享文件
- 第三轮收口后，`ui/src/app/core/legacy.ts` 与 `ui/src/app/events/legacy.ts` 不得继续存在；历史兼容实现必须迁入真实领域模块，`core.ts` 与 `events.ts` 只能作为稳定 barrel 入口
- `Search`、`Inventory`、`Settings` 不得继续直接互相借用工作区级 render helper；任何被两个以上工作区复用的视图片段都必须提升到 `app/render/shared/`
- 第二轮必须至少把以下跨工作区片段提升到共享渲染层：
  - preview surface
  - library context cluster / search scope bar
  - provider bridge / inventory bridge 一类跨工作区桥接块
  - jobs 列表与轻量状态表达
- `Inventory` 与 `Settings` 的当前库工作区头部不得继续保留两套并行 DOM 骨架；它们必须通过同一个 shared library-context workspace toolbar helper 输出，并仅通过显式 slot / capability 参数表达“可编辑管理”与“只读上下文”的差异
- 共享渲染层还必须承接跨工作区一致性组件：detail card、tag / badge、action row、meta / stats list、notice / empty 与常用 list item shell；`Search`、`Inventory`、`Settings` 不得各自拼装同职责组件的完整结构
- 当某个工作区需要领域差异时，应通过共享 helper 的 variant / slot / className 参数表达；不得复制 helper 后在工作区内维护第二套结构
- 共享组件一旦覆盖某类 UI 语义，工作区不得继续直接拼接同职责 HTML 或 class 组合；例如当前库表达必须经 library-context helper，标签必须经 tag helper，常用动作必须经 button/action-row helper，对象列表必须经 list-item shell，空态与提示必须经 notice/empty helper
- `ui/src/app/render/shell.ts` 不得继续导出与 `app/render/shared/` 重复的 library context、search scope、source-root、bridge 或 preview helper；壳层只负责组合共享片段

## 类型组织

- `ui/src/types.ts` 不应继续作为全部前端类型的单体承载点
- 生产 UI 类型至少应拆分为以下边界：
  - API / wire payload 类型
  - 应用状态与共享 UI 类型
  - 必要时的工作区局部辅助类型
- 若为降低迁移期 churn 需要，可保留单一导出入口；但单一导出入口只应作为 barrel，而不应继续承载全部声明本体

## 样式组织

- 生产前端必须使用单一样式聚合入口，例如 `ui/src/styles/index.css`
- 样式聚合入口下的样式文件应至少按以下层次拆分：
  - token / 变量
  - base / reset
  - layout
  - shell
  - search
  - inventory
  - settings
  - utilities
  - responsive overrides
- 样式拆分必须保持 plain CSS，不引入 CSS Modules 或 CSS-in-JS
- 选择器命名默认保持稳定；仅当文件组织或作用域边界需要时，才允许最小必要改名
- 响应式覆写应优先汇总到专门的 responsive 层，而不是继续零散附着在所有模块尾部
- 基础按钮、标签、详情卡、预览面、meta / stats、notice / empty 等组件样式应定义在共享样式层；`search.css`、`inventory.css`、`settings.css` 与 `shell.css` 只保留布局、密度和领域变体
- 迁移共享组件时必须删除对应旧选择器与旧本地实现，不保留 `.pill`、`.secondary-button` 等旧选择器作为兼容 alias；未迁移的调用点应显式改用稳定 `ui-*` 类名

## UI 场景辅助代码组织

- `ui/tests/e2e/helpers/scenarios.ts` 不应继续作为 UI 侧端到端辅助代码的单体事实源
- UI 场景辅助代码至少应按以下域拆分：
  - search
  - workspace
  - inventory / source management
  - settings
  - runtime / jobs
  - shared fixtures / utilities
- 共享 fixture 创建、临时文件生成、共享选择器与常用页面操作必须进入共享 helper，而不是在多个场景模块间复制
- 正式 spec 文件对场景辅助代码的入口命名可以保持稳定，但入口实现必须允许内部继续按域拆分

### 第二轮场景辅助拆分

- `ui/tests/e2e/helpers/search.ts` 在第二轮必须继续下沉为子域模块：
  - `search-text.ts`
  - `search-image.ts`
  - `search-video.ts`
  - `search-document.ts`
- `ui/tests/e2e/helpers/workspace.ts` 在第二轮必须继续拆成更小的工作区辅助模块，例如：
  - shell
  - drawer
  - jobs
  - mobile detail
  - refresh preservation
- `ui/tests/e2e/helpers/scenarios.ts` 继续保持稳定 barrel 入口，spec 文件与调用习惯不应因内部拆分而改变
- 共享 fixture 继续只放在共享 helper 中，不得在新的子域 helper 内重新复制临时目录创建、mock setup 或常用页面操作

## 当前阶段重构约束

- 当前阶段前端重构继续采用 Vite + vanilla TypeScript + template-string rendering，不引入 React 或其他新框架
- 当前阶段不引入新的 path alias、构建系统升级、严格度策略升级或状态管理库
- 当前阶段不改变公开产品 IA，不改变后端 API 契约，也不把重构与新功能开发混在同一分支
- 当前阶段的前端代码拆分必须覆盖：
  - `ui/src/main.ts`
  - `ui/src/types.ts`
  - `ui/src/styles/index.css`（并移除旧的 `ui/src/style.css` 单体入口）
  - `ui/tests/e2e/helpers/scenarios.ts`
- 当前阶段必须保留现有前端与服务端之间的兼容请求语义；若某些正式搜索端点仍要求单库 `library_id` 顶层字段，前端在重构后仍必须继续发送，不得因模块拆分而静默丢失
- 当前阶段的第二轮拆分不得改变以下兼容语义：
  - 非文本单库搜索继续发送顶层 `library_id`
  - 单库结果缺失 `library_id` 时，前端仍回退到当前库，以保持详情加载与“作为查询输入”动作稳定

## 关联主题

- [001-architecture](../001-architecture/spec.md) 定义前端只与 Rust 主服务直接交互的系统边界
- [008-ui-ux](../008-ui-ux/spec.md) 定义应用壳层、工作区与正式用户可见 UI/UX 语义
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 定义前端所依赖的公开请求 / 响应契约
