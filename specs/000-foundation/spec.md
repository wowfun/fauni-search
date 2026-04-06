# 000 基础 (Foundation)

定义 FauniSearch 的基础约束与默认前提，作为 001 及后续主题的上游基线。

## 关键术语 (Terminology)

- 本地优先（Local-First）
- 单向量（single-vector）
- 多向量（multivector）
- 多库（Multi-Library）
- 单用户（Single-User）
- 提供方驱动架构（Provider-based architecture）

## 范围

- 产品定位与能力边界
- 技术栈基线与平台兼容性基线

范围外：
- 系统拓扑或进程职责拆分
- 详细数据模型或数据库 schema
- 接口细节

## 设计原则

- 单一事实源（Single Source of Truth）：每一类状态应只有一个规范事实源，其他表示只能是派生物、缓存、索引或投影视图，不应让多个存储同时承担同一类状态的真相职责

## 基础定位

- 项目采用本地优先定位，索引与结构化状态默认存储在本地
- 只做检索，不做生成
- 采用纯视觉检索路线，不引入 OCR 或文本解析主链
- 支持三类查询输入：文本、图片、视频
- 支持三类检索对象：文档页、图片、视频片段
- 单向量与多向量均为一等能力
- 系统采用多库（Multi-Library）组织模型，并按单用户（Single-User）使用场景设计
- 不包含内建微调（Fine-Tuning）工作流

## 技术栈基线

- Rust 是主后端语言
- Python 用于 ML 与媒体处理
- SQLite 是默认结构化元数据存储
- 提供方驱动架构（Provider-based architecture）是默认扩展模型
- Rust 与 Python 之间默认采用 HTTP/JSON 边界

## 平台与兼容性

- Linux + NVIDIA：完整支持
- Windows + NVIDIA：完整支持
- macOS Apple Silicon：支持索引与检索，但不承诺与 Linux/Windows 同等性能

## 关联主题

- [001-architecture](../001-architecture/spec.md) 承接系统拓扑、进程职责拆分、一级组件边界与稳定交互路径
- [009-interfaces-and-protocol-contracts](../009-interfaces-and-protocol-contracts/spec.md) 承接公开接口契约与 Rust / Python 之间的稳定 payload 协议
