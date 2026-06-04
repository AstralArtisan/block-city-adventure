# 勇闯方块城 / Block City Adventure

一款基于 Rust + Bevy 0.14 的 2D 俯视角动作 Roguelike。四层楼、十房间、三十种强化、九种终结技、十种怪物、四个 Boss、十七种事件——在方块城的裂隙中杀出一条路。

## 快速开始

```bash
cargo run            # 开发运行
cargo run --release  # 发布构建
cargo test           # 83 个单元测试
```

## 操作

| 按键 | 功能 |
|------|------|
| WASD / 方向键 | 移动 |
| 鼠标左键 | 近战攻击 |
| 鼠标右键 | 远程攻击 |
| Space | 冲刺 |
| 1 / 2 / 3 / 4 | 释放终结技 |
| E | 交互（进门、触发事件、Boss 传送门） |
| Esc | 暂停 / 返回 |
| F5 | 保存进度 |
| F9 | 读取存档 |
| R | 商店刷新 / 使用治疗药水 |
| F | 使用能量药水 |

## 游戏内容

### 核心循环

```
主菜单 → 进入游戏 → 4 层 × 10 房间
每层：战斗房 / 事件房 / 商店房 / 圣所（奖励房） / 精英房 → Boss 房 → 下一层
通关 4 层 → 胜利结算
```

### 成长系统

| 系统 | 说明 |
|------|------|
| 强化（Augment） | 30 种，3 稀有度（普通/精英/传说），可升 3 级，Lv3 产生质变效果 |
| 终结技 | 4 个槽位（数字键 1-4），9 种终结技，消耗能量释放 |
| 能量蓄力 | 近战命中 +2、远程 +1、击杀 +5，不自然回复 |
| 升级 | 击杀获取 XP，升级时选择回血或属性强化 |
| 商店 | 三区（属性/强化/工具），可购买药水、护身符、强化升级 |

### 怪物与 Boss

- **10 种怪物**：按楼层递进解锁（1 层 MeleeChaser/Lobber/Charger → 4 层 SupportCaster/Summoner）
- **精英词缀**：6 种（迅捷/分裂/铁壁/吸血/狂暴/闪现），3 层后双词缀
- **4 个 Boss**：Guardian / MirrorWarden / TideHunter / CubeCore，各有多阶段机制

### 特殊房间

- **圣所**：三选一（疗愈 / 锻造 / 启示）
- **商店**：三区横排，悬停查看详情
- **事件房**：17 种事件（谜题/非战斗/战斗），按 E 触发

## 技术栈

| 依赖 | 版本 | 用途 |
|------|------|------|
| Bevy | 0.14.2 | 游戏引擎（ECS） |
| bevy_rapier2d | 0.27 | 2D 物理碰撞 |
| bevy_kira_audio | 0.20 | 音频播放 |
| Lightyear | 0.17.1 | 合作模式网络（主机权威） |
| serde + ron | - | 配置序列化 |
| bincode | - | PVP 网络序列化 |

## 架构概览

```
src/
├── main.rs / app.rs     → 入口 + GamePlugin 装配
├── states.rs            → 两层状态机：AppState + GamePhase(SubStates)
├── core/                → 资源、输入、音频、相机、存档、成就
├── data/                → RON 配置加载 → GameDataRegistry
├── gameplay/
│   ├── player/          → 组件、战斗、冲刺、技能
│   ├── enemy/           → 10 种怪物 AI + 精英词缀
│   ├── augment/         → 30 种强化 + 效果系统
│   ├── skills/          → 9 种终结技
│   ├── rewards/         → 圣所流程
│   ├── shop/            → 商店逻辑
│   ├── event_room/      → 事件房
│   ├── session_core/    → 共享规则（单机+Coop）
│   └── effects/         → 打击暂停、屏幕闪光、粒子
├── coop/                → Lightyear 合作模式
├── pvp/                 → 自定义 UDP 对战
└── ui/                  → 全套 UI（菜单/HUD/暂停/商店/圣所/事件/结算/Tooltip）
```

### 状态机

- **AppState**（顶层）：Loading / MainMenu / InGame / CoopGame / PvpGame 等 11 个状态
- **GamePhase**（SubStates，存在于 InGame|CoopGame）：Playing / Paused / Shop / RewardSelect / AugmentSelect / LevelUpSelect / SkillSelect / EventRoom / GameOver / Victory

### 配置驱动

所有数值在 `assets/configs/*.ron`：

| 文件 | 内容 |
|------|------|
| player.ron | 血量、速度、冲刺、能量 |
| enemies.ron | 各怪物数值 |
| boss.ron | Boss 阶段参数 |
| augments.ron | 30 种强化定义（3 级 params 矩阵，唯一真相源） |
| skills.ron | 终结技消耗/CD/伤害/AOE/时长/buff |
| elite_affixes.ron | 6 种精英词缀数值 |
| shop.ron | 商店价格、重复购买涨价、折扣 |
| events.ron | 17 种事件配置与 payload |
| game_balance.ron | 楼层敌人池、成长曲线、精英概率、全局难度 |
| balance.ron / rewards.ron | XP 曲线、掉落数量、升级属性增量 |
| audio.ron / effects.ron | 音效/特效参数 |

> 自 #9 数值配置化重构起，所有玩法平衡数值均从 RON 读取（`GameDataRegistry`），`loaders_fallback_matches_shipped_ron_files` 测试锁定代码内 fallback 与 RON 一致——调平衡改 RON，不改源码常量。

## 联机模式

### 合作模式（Coop）

基于 Lightyear 的主机权威架构。本地调试：

```powershell
# 主机
$env:LOCAL_NET_DEBUG=”1”; $env:LOCAL_NET_DEBUG_MODE=”coop”; $env:LOCAL_NET_DEBUG_ROLE=”host”; cargo run
# 客户端
$env:LOCAL_NET_DEBUG=”1”; $env:LOCAL_NET_DEBUG_MODE=”coop”; $env:LOCAL_NET_DEBUG_ROLE=”client”; $env:LOCAL_NET_DEBUG_HOST=”127.0.0.1”; cargo run
```

### 对战模式（PVP）

自定义 UDP 协议，两人竞技场对战：

```powershell
# 主机
$env:LOCAL_NET_DEBUG=”1”; $env:LOCAL_NET_DEBUG_MODE=”pvp”; $env:LOCAL_NET_DEBUG_ROLE=”host”; cargo run
# 客户端
$env:LOCAL_NET_DEBUG=”1”; $env:LOCAL_NET_DEBUG_MODE=”pvp”; $env:LOCAL_NET_DEBUG_ROLE=”client”; $env:LOCAL_NET_DEBUG_HOST=”127.0.0.1”; cargo run
```

## 文档

| 文档 | 内容 |
|------|------|
| [docs/00_index.md](docs/00_index.md) | 文档索引与术语表 |
| [docs/01_build_and_run.md](docs/01_build_and_run.md) | 构建、运行、联调手册 |
| [docs/02_architecture.md](docs/02_architecture.md) | 架构设计 |
| [docs/05_iteration_history.md](docs/05_iteration_history.md) | 迭代历史 |
| [docs/03_module_design.md](docs/03_module_design.md) | 模块职责 |
| [docs/superpowers/specs/](docs/superpowers/specs/) | 设计规格文档 |

## 质量状态

- `cargo check`：通过（3 个基线 audio dead-code 警告）
- `cargo test`：83 项通过
- 覆盖：XP 曲线、Boss 决策、奖励规则、商店逻辑、强化系统、敌人楼层池、角色面板、RON fallback 一致性


