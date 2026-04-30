---
name: 0002 Content-centered core model
status: Accepted
date: 2026-04-28
---

## Context

FauniSearch 需要支持 `faus find <folder>` 这类 agent-first 工作流。Agent 关心的是在一个目录里快速找到可用位置，例如某个 PDF 页、某张图、某段视频，而不是只拿到一个库内对象 ID。与此同时，同一份内容可能出现在多个库、多个来源根或多个路径下。系统需要复用内容身份、派生载荷和向量结果，但不能把某个库里的位置关系误当成全局事实。

前一版规格把复用拆成了 `Source / VisualUnit / DerivedAsset / CanonicalContent / CanonicalVisualUnit / Membership / GlobalVectorCache` 等多个概念。这些概念分别能解释一部分问题，但组合起来过重。`CanonicalVisualUnit` 和 `Membership` 很容易和库内 `VisualUnit`、搜索结果位置、索引可见性互相重叠。`DerivedAsset` 也容易被误解为核心身份对象，而不是处理链路里的载荷。

模型能力也会限制可直接 embedding 的内容类型。PDF、Word、视频片段这类内容未必能被当前模型直接编码；同一个视频片段可能要先抽关键帧，再用图片模型索引。把可嵌入能力写进内容身份会导致模型能力、处理策略和内容事实耦合在一起。

因此核心模型需要重新收敛：只保留最少的业务对象，让每一层回答一个问题。

## Decision

FauniSearch 采用 Content-centered core model。核心链路固定为 `Source -> Asset -> Unit`，`Content` 作为全局内容身份，`ContentIndex` 承接索引状态。

`Content` 是全局内容身份。它只保存内容本身的稳定事实，例如 hash、size、media type 和 payload reference。它不保存来源关系、派生关系、可见性、模型能力或索引策略。原始文件、PDF 页渲染图、OCR 文本、视频关键帧、帧组和其他派生载荷，只要需要被复用，都可以形成自己的 `Content`。

`Source` 是库内来源位置。它属于某个 library 和可选 source root，保存 path、source kind、coverage state 等库内事实，并引用原始 `Content`。两个库可以各自有自己的 `Source`，但引用同一个 `Content`。

`Asset` 是 `Source` 内的可定位资产。它回答用户最终要定位到哪里，例如一张图片、一个 PDF 页、一个视频片段、一个文本块。`Asset` 面向产品结果和位置语义，带 locator、顺序、范围和展示属性。`Asset` 不是 `Unit`。

`Unit` 是为模型能力生成的索引执行单元。它回答实际拿什么去 embedding，例如 keyframe image unit、page image unit、text unit。一个 `Asset` 可以生成一个或多个 `Unit`。搜索命中 Unit 后，结果应回到对应 Asset 和 Source，向用户返回可操作位置。

`ContentIndex` 是索引状态对象。它以 `unit.content_id + vector_space_id` 为 key，记录 active / staging 状态、不透明 `vector_ref` 和可选 `job_id`。向量 payload 可以在检索后端中，但搜索可见性由 `ContentIndex` 解释。

最小关系如下：

```text
Source.source_content_id -> Content
Source -> Asset
Asset.content_id -> Content
Asset -> Unit
Unit.content_id -> Content
VectorSpace -> ContentIndex
Unit.content_id + vector_space_id -> ContentIndex
```

### 图片

图片 source 可以直接形成 image asset，再形成 image unit。Asset 和 Unit 可以引用同一个 Content。

```text
Source(photo.png) -> Content(photo bytes)
Source(photo.png) -> Asset(image, locator=image)
Asset(image) -> Unit(image_unit)
Unit(image_unit) -> Content(photo bytes)
ContentIndex(photo bytes + image VectorSpace)
```

### PDF

PDF source 引用 PDF 文件内容。PDF 页是可定位 Asset；如果当前模型不能直接 embed PDF 页对象，索引策略把页面渲染成 page image unit。

```text
Source(report.pdf) -> Content(pdf bytes)
Source(report.pdf) -> Asset(page 3)
Asset(page 3) -> Unit(page image unit)
Unit(page image unit) -> Content(rendered page image bytes)
ContentIndex(rendered page image + image VectorSpace)
```

搜索命中 page image unit 后，结果返回 `report.pdf` 的第 3 页，而不是返回渲染图文件本身。

### 视频

视频 source 引用视频文件内容。视频片段是可定位 Asset；如果模型不支持直接 embed 视频片段，但支持 embed 图片，索引策略从片段抽关键帧并生成 image unit。

```text
Source(clip.mp4) -> Content(video file bytes)
Source(clip.mp4) -> Asset(video segment 00:10-00:20)
Asset(video segment) -> Unit(keyframe image unit)
Unit(keyframe image unit) -> Content(keyframe image bytes)
ContentIndex(keyframe image + image VectorSpace)
```

搜索命中 keyframe image unit 后，结果返回 `clip.mp4` 的 `00:10-00:20` 片段。keyframe 只是匹配证据和索引输入，不是用户最终要操作的位置。

## Consequences

`Asset` 和 `Unit` 必须分开。Asset 面向定位和产品结果，Unit 面向模型和 embedding。把二者合并会让视频片段和关键帧、PDF 页和页图、文本段和 OCR 载荷混在一起，后续很难解释搜索结果。

`DerivedAsset` 不再作为核心身份对象。派生链路表现为从某个 Asset 生成一个或多个 Unit，Unit 引用自己的 Content。需要保留派生过程时，可以记录处理关系和生成规格，但这些关系不写入 Content 身份本身。

`CanonicalContent` 合并为 `Content`。`CanonicalVisualUnit` 和 `Membership` 不进入最小核心模型。跨库复用通过 Content 和 Unit 的内容身份完成，位置通过 Source 和 Asset 还原。

搜索流程以 Unit 的 ContentIndex 命中为入口，以 Source + Asset 作为结果出口。折叠结果可以按 Unit content、Asset 或 Source 范围聚合，但 locations 必须能还原到具体 Source 和 Asset。

是否能直接 embedding 不属于 Content。索引策略层根据 content type 配置、provider/model 能力和 operation 决定 direct、derive 或 unsupported。PDF、Word、视频片段等不能直接索引时，通过 Asset -> Unit 的处理链路转成模型可处理内容。

`VectorSpace` 表示某个模型的一种具体向量表示空间。它不承载 active/staging、job 或物理 collection 生命周期；这些状态由 `ContentIndex` 和 `vector_ref` 表达。

## Alternatives Considered

### Source -> Unit -> Asset

这个方向让 Unit 先承担定位，再挂处理载荷。它能解释一部分搜索结果，但视频片段例子会变得别扭：用户要的是视频片段，模型实际索引的是关键帧。把 Unit 放在 Asset 前面会让 Unit 同时承担定位和 embedding 两个职责。

### Asset == Unit

把 Asset 和 Unit 合并可以减少对象数量，但会丢掉关键分工。PDF 页和页图、视频片段和关键帧不是同一层概念。合并后搜索命中的到底是用户位置还是模型输入，会变得不清楚。

### CanonicalContent / CanonicalVisualUnit / Membership

独立 canonical 层能表达跨库复用，但对象太多，且和 Source、VisualUnit、搜索 locations 重叠。`Membership` 也容易把位置、可见性和索引状态揉在一起。当前阶段更适合用 Content 统一身份，用 Source + Asset 表达位置。

### Embeddability on Content

在 Content 上保存 `is_embeddable` 或类似字段看起来简单，但模型能力是上下文相关的。同一个 Content 对某个模型不可嵌入，对另一个模型可能可嵌入；也可能通过派生 Unit 后可嵌入。这个判断应放在索引策略层和 ContentIndex 里，而不是写进 Content。
