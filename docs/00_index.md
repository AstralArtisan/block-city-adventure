# 工程交接文档索引

- 适用版本：`main` 分支
- 最后校验：2026-05-30；`cargo check` 通过，`cargo test` 83 项通过
- 关联源码：`README.md`、`src/`、`assets/configs/`、`docs/`

本文档集面向工程交接与后续维护。帮助维护者快速搞清楚：

1. 程序如何启动，状态如何流转
2. 单机、Coop、PVP 的关系
3. 哪些目录是稳定骨架，哪些是复杂度热点
4. 还有哪些工作待完成

## 推荐阅读顺序

1. [`README.md`](../README.md)：项目概览、操作、游戏内容、架构
2. [`01_build_and_run.md`](01_build_and_run.md)：构建、运行、联调手册
3. [`02_architecture.md`](02_architecture.md)：状态机、ECS 分层、数据流
4. [`03_module_design.md`](03_module_design.md)：各模块职责与依赖
5. [`04_api_and_data_model.md`](04_api_and_data_model.md)：内部接口契约
6. [`06_multiplayer_and_risks.md`](06_multiplayer_and_risks.md)：联机原型说明
7. [`07_extension_guide.md`](07_extension_guide.md)：扩展指南
8. [`05_iteration_history.md`](05_iteration_history.md)：迭代历史

## 文档地图

| 文档 | 内容 | 适合谁 |
|------|------|--------|
| [`README.md`](../README.md) | 项目入口、游戏内容、架构概览 | 所有人 |
| [`01_build_and_run.md`](01_build_and_run.md) | 构建、运行、联调手册 | 首次接手者 |
| [`02_architecture.md`](02_architecture.md) | 总体架构、状态机、数据流 | 架构理解 |
| [`03_module_design.md`](03_module_design.md) | 模块职责与依赖 | 开发维护 |
| [`04_api_and_data_model.md`](04_api_and_data_model.md) | 内部接口、数据模型 | 改功能前必看 |
| [`05_iteration_history.md`](05_iteration_history.md) | 真实演化历史 | 追踪设计意图 |
| [`06_multiplayer_and_risks.md`](06_multiplayer_and_risks.md) | 联机原型与技术债 | 联机维护者 |
| [`07_extension_guide.md`](07_extension_guide.md) | 新增内容的改动路径 | 功能扩展者 |
| [`superpowers/specs/`](superpowers/specs/) | 设计规格文档（4-29 全面设计 + 增量修改计划） | 设计参考 |

## 设计规格文档

| 文档 | 内容 |
|------|------|
| [`2026-04-29-full-refactor-design.md`](superpowers/specs/2026-04-29-full-refactor-design.md) | 全面重构设计（权威游戏设计文档） |
| [`2026-04-29-full-refactor-implementation-plan.md`](superpowers/specs/2026-04-29-full-refactor-implementation-plan.md) | 实现计划 |
| [`2026-05-15-incremental-modification-plan.md`](superpowers/specs/2026-05-15-incremental-modification-plan.md) | 增量修改路线图 |

## 术语表

| 术语 | 含义 |
|------|------|
| AppState | 顶层状态机（11 个状态：Loading/MainMenu/InGame/CoopGame 等） |
| GamePhase | 游戏内子状态（SubStates：Playing/Paused/Shop/RewardSelect 等） |
| RoomState | 房间推进状态（Resource：Idle/Locked/Cleared/BossFight） |
| GameDataRegistry | 从 `assets/configs/*.ron` 加载的统一配置注册表 |
| InGameEntity | 局内实体标记，状态切换时自动清理 |
| session_core | 共享规则层（单机+Coop 共用的奖励/商店/死亡逻辑） |
| Augment | 强化系统（30 种 × 3 级，Lv3 质变） |
| TooltipContent | UI 悬停浮层数据组件 |

## 源码基线

- 主工程名：`block_city_adventure`
- 关键依赖：Bevy 0.14.2、bevy_rapier2d 0.27、bevy_kira_audio 0.20、Lightyear 0.17.1
- 源码文件：110 个 Rust 文件
- 测试：83 项通过
- 复杂度热点：`src/coop/runtime.rs`、`src/gameplay/enemy/systems.rs`、`src/gameplay/session_core/mod.rs`、`src/ui/hud.rs`
