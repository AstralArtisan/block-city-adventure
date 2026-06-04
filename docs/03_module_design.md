# 模块设计说明

- 适用版本：`main` 分支
- 最后校验：2026-05-30；`cargo check` 通过，`cargo test` 83 项通过
- 源码文件数：110 个 Rust 源文件
- 关联源码：`src/core/`、`src/data/`、`src/gameplay/`、`src/coop/`、`src/pvp/`、`src/ui/`、`src/utils/`
- 实验性内容：包含。联机模块与部分 UI/规则层仍在持续收敛

## 1. 模块总表
| 模块 | 核心责任 | 主要依赖 | 当前定位 |
| --- | --- | --- | --- |
| `core` | 基础设施与横切能力 | Bevy 资源/事件/输入 | 稳定骨架 |
| `data` | 配置定义与加载 | `serde`、`ron` | 稳定骨架 |
| `gameplay` | 玩法域模型与主要系统 | `core`、`data`、`states` | 核心价值层 |
| `coop` | 合作联机协议、权威运行时、联机 UI | `gameplay`、Lightyear | 原型 |
| `pvp` | 对战协议、模拟与菜单 | `gameplay`、UDP | 原型 |
| `ui` | 菜单、HUD、暂停、通知、结算 | `core`、`gameplay`、`states` | 稳定骨架 |
| `utils` | 通用数学、碰撞、随机数、实体工具 | 各模块 | 辅助层 |

## 2. `core/`：基础设施层
### 2.1 目录职责
- `assets.rs`：加载字体、贴图、基础图集，并构造 `GameAssets`
- `input.rs`：统一采集键鼠输入，生成 `PlayerInputState`
- `audio.rs`：音频接口占位，当前预留多于实际使用
- `camera.rs`：相机跟随与镜头能力
- `events.rs`：统一事件总线
- `save.rs`：F5/F9 存读档与 `SaveData`
- `achievements.rs`：成就追踪与通知入口
- `local_debug.rs`：本地联机联调入口

### 2.2 设计价值
- 把“所有模式都会用到的能力”抽离出来
- 减少 gameplay、ui、network 之间的重复基础逻辑
- 让后续重构联机或 UI 时，不需要触碰资源加载和存档机制

### 2.3 维护注意事项
- `PlayerInputState` 是单机、Coop、PVP 共用输入源，新增输入时要同步检查三套消费链
- `EventsPlugin` 中的事件名称已经带有明显领域语义，新增事件优先放这里而不是散落到局部模块
- `LocalDebugPlugin` 已替代旧脚本，是当前多人本地联调的唯一真实入口

## 3. `data/`：配置定义层
### 3.1 目录职责
- `definitions.rs`：定义 `PlayerConfig`、`EnemiesConfig`、`BossesConfig`、`RewardsConfig`、`RoomGenConfig`、`GameBalanceConfig`
- `loaders.rs`：从 `assets/configs/*.ron` 加载配置，失败时回退默认值
- `registry.rs`：把所有配置聚合为 `GameDataRegistry`

### 3.2 设计价值
- 把平衡参数和运行逻辑分离
- 让新增敌人、奖励、Boss 和房间参数时可以走配置驱动
- 提供明确的 fallback，保证素材或配置不完整时项目仍可启动

### 3.3 维护注意事项
- 新增配置字段时，要同时更新：
  - `definitions.rs`
  - 对应 `.ron` 文件
  - `loaders.rs` 的默认值
  - 实际消费这些配置的 gameplay 模块

## 4. `gameplay/`：玩法域层
`gameplay/` 是工程的核心业务域，也是单机和 Coop 的共享底座。

### 4.1 `map/`
职责：

- 生成楼层布局与房间图
- 控制门、房间切换和地块生成
- 记录访问房间和房间进入奖励

关键对象：

- `FloorLayout`
- `CurrentRoom`
- `RoomId`
- `RoomType`
- `VisitedRooms`
- `RewardRoomGoldBonusSeen`
- `RoomTransition`

依赖关系：

- 依赖 `data` 获取房间配置和全局平衡
- 依赖 `player` 获取玩家位置
- 依赖 `states::RoomState` 决定房门是否可交互

维护注意事项：

- `TransitionsPlugin` 只在 `InGame` 下运行，Coop 改为独立的房门交互推进
- 地图相关资源常被 HUD、敌人刷新、商店、解谜共同依赖，改动时要做横向检查

### 4.2 `player/`
职责：

- 玩家生成、移动、朝向、动画
- 近战/远程输入和冷却
- 冲刺、连击、死亡、受伤无敌
- 奖励修正与派生能力

关键对象：

- `Player`
- `PlayerDriveInput`
- `Health`
- `MoveSpeed`
- `AttackCooldown`
- `DashCooldown`
- `RewardModifiers`
- `DashState`
- `AnimationState`

设计要点：

- 单机时直接从 `PlayerInputState` 写入
- Coop 时由 Host 权威写入 `PlayerDriveInput`
- `RewardModifiers` 承担了大量成长语义，是玩家构建的中心聚合点

### 4.3 `combat/`
职责：

- 命中判定、伤害分发、击退和 DOT
- 投射物移动与过期销毁
- 命中框寿命管理

关键对象：

- `Hitbox`
- `Hurtbox`
- `Damage`
- `Knockback`
- `Lifetime`
- `Projectile`
- `Team`

设计要点：

- `CombatSystemsPlugin` 同时服务单机和 Coop Host
- 伤害通过事件流转，降低实体查询耦合

### 4.4 `enemy/`
职责：

- 敌人组件、AI、攻击、死亡、Boss 模式
- 房间进入时的刷怪逻辑
- 精英和 Boss 的特化行为
- 小怪头顶血条（`EnemyHealthBar` + `EnemyHealthBarFill`）

关键对象：

- `Enemy`
- `EnemyKind`
- `EnemyStats`
- `BossPhase`
- `EnemySpawnCount`
- `SpawnedForRoom`
- `ClearGrace`
- `Elite`、`EliteAffix`、`EliteAffixMarker`
- `EnemyHealthBar`、`EnemyHealthBarFill`

设计要点：

- `room_entry_spawner` 是房间推进的重要枢纽
- `RoomType::Elite` 独立分支：`spawn_elite_room_enemies` 固定 1 精英（1.4x 体积）+ 2 普通
- 精英词缀系统：6 种词缀（Swift/Splitting/Shielded/Vampiric/Berserk/Teleporting），各有独立系统
- 小怪血条：世界空间 Sprite 跟随敌人位置，颜色随血量变化，Boss 排除
- Boss 死亡时同时清理 `BossSummoned` 和 `BossSubCore` 实体
- Boss 与普通敌人共处一个模块树，维护时要区分共享逻辑与特化逻辑
- Coop 中大量敌人逻辑仍复用这里，但只在 Host 权威侧运行

### 4.5 `rewards/`
职责：

- 单机奖励房（圣所房）选择页
- 奖励房服务生成、应用和结算后跳转
- Boss 通关传送门（`BossPortal`）生成与交互

设计要点：

- 当前单机 `RewardSelect` 只服务 `Reward` 房，不再承载旧的祝福祠堂或属性三选一
- `Reward` 房已重做为低频、安全、无战斗的“圣所房”
- 圣所房固定提供 3 项服务：`疗愈`、`淬炼/觉醒`、`启示`
- `RewardFlow` 记录当前圣所页的上下文，以及 `spawn_portal`/`portal_is_victory` 控制 Boss 传送门
- Boss 通关流程：AugmentSelect → 返回 InGame → 地图中心生成传送门 → 玩家按 E 推进楼层
- 精英房通关 100% 触发 AugmentSelect（普通房 40%）

### 4.6 `shop/`
职责：

- 商店房亭子生成
- 商店商品生成、缓存、刷新和购买

设计要点：

- 单机商店通过 `AppState::Shop` 驱动
- Shop 的商品成本曲线和效应计算已经与 `session_core` 对齐

### 4.7 `puzzle/`
职责：

- 压力板、开关顺序、陷阱生存三类解谜

现状：

- 当前只在 `AppState::InGame` 运行
- Coop 中未形成完整等价运行时
- Puzzle 完成后给予 augment 奖励（`AugmentPool::Any`），通过 `resolve_event_room_clear` 处理

### 4.8 `progression/`
职责：

- 楼层初始化
- 难度系数
- 运行统计
- 经验升级系统（`PlayerLevel`、`XpGainEvent`、`LevelUpEvent`）

现状：

- XP 升级曲线：
  - `Lv1 -> Lv2`: 30
  - `Lv2 -> Lv3`: 45
  - `Lv3 -> Lv4`: 60
  - `Lv4+`: 在前一段基础上继续递增
- 升级时进入 `AppState::LevelUpSelect`，提供"回血或强化"双栏选择
- `PendingLevelUps` 队列防止升级与房间清理事件冲突
- 主要服务单机主循环
- 其中部分概念如 `FloorNumber` 会被存档、HUD、商店和联机流程共同引用

### 4.9 `event_room/`
职责：

- 事件房交互系统（11 种事件类型：3 谜题 + 6 非战斗 + 2 战斗）
- 仿商店模式：进入房间只显示交互提示，按 E 激活事件

设计要点：

- `init_event_for_room`：进入事件房时选事件+设标记，不锁房不激活
- `sync_event_interact_prompt`：同步显示/清理"按 E 交互"提示
- `event_interact_system`：按 E 后根据事件类型锁房/生敌人/开 UI
- Esc 不解决事件，允许重新交互；选择效果后 `mark_event_resolved` 阻止再次交互
- `resolve_event_room_clear`：战斗/谜题事件完成后给予 augment 奖励

### 4.10 `drops/`
职责：

- 金币和经验掉落物的生成、物理、磁铁吸收、收集和消失

设计要点：

- Boss/精英死亡生成多个掉落物（爆金币效果）：Boss 8金+6经验，精英 4金+3经验
- Floor 3+ 掉落数量翻倍
- 掉落物生命周期 8 秒
- 磁铁吸收范围 140px（可通过 PickupRange augment 扩大）

### 4.11 `effects/`
职责：

- 受击闪白
- 粒子
- 残影
- 伤害数字
- 屏幕震动请求

设计要点：

- 主要是表现层系统
- 在 `InGame`、`CoopGame`、`PvpGame` 中都可能运行

### 4.12 `augment/`
职责：

- 增强系统（被动能力），30 种增强分为近战/远程/移动/通用四类，三级稀有度（Common/Elite/Legendary）
- `data.rs`：`AugmentId` 枚举（30 种）、`AugmentRarity`、`AugmentSlot`、增强元数据
- `effects.rs`：每种增强的运行时效果实现（反弹、追踪弹、连锁闪电、荆棘、冰冻、不死鸟、爆炸弹等）
- `tuning.rs`：类型化薄包装层，统一经 `GameDataRegistry::augment_param` 从 `augments.ron::params` 取数值（#9 起不再维护手写 match 表）

设计要点：

- `AugmentPlugin` 已归入 `GameplayPlugin`
- 所有效果系统通过 `.run_if()` 同时服务单机和 Coop Host
- 效果系统有明确的执行顺序约束（`.before()` / `.after()` 与 combat 系统协调）

### 4.13 `rune/` / `curse/`（历史快照）

- 这两套模块已从当前单机架构中移除
- 历史设计曾让 `Reward` 房承载“祝福祠堂 -> 铭文 + 诅咒”链路
- 当前单机已统一到 `经验升级 + 强化组合 + 圣所房`
- `Coop` 仍保留独立奖励语义，但不再依赖 rune / curse 数据模型

### 4.14 `skills/`
职责：

- 9 种终结技：裂地斩 / 剑舞 / 处刑之刃 / 弹幕倾泻 / 冰晶领域 / 天降陨石 / 战吼 / 吸血 / 时空裂隙（`SkillType` 枚举），装入 4 个槽位（数字键 1-4）
- `slots.rs`：技能槽位管理（`SkillSlots` 组件，4 槽位解锁状态，`equipped()` 供抽卡去重）
- `execute.rs`：技能释放逻辑，伤害 / AOE / 时长 / buff 全部从 `skills.ron` 读取

设计要点：

- 能量系统驱动：最大 100 能量，通过战斗行为充能（近战+8、远程+4、击杀+12）
- 数字键 1-4 对应 HUD 底部技能栏
- `PlayerSkillState` 管理释放中的技能状态

### 4.15 `session_core/`
职责：

- 商店草案生成
- Boss 房间清空后的规则决策
- 死亡后的结算决策

为什么关键：

- 它是目前“玩法规则抽象化”最明显的模块
- 大部分单元测试都围绕这一层展开
- 它仍让单机与 Coop 共享商店/死亡判定曲线
- 单机奖励房已改由 `rewards/` 直接管理，`Coop` 奖励阶段仍保留独立分支

## 5. `coop/`：合作联机模块
### 5.1 子模块职责
- `components.rs`：Coop 输入、玩家槽位、阶段枚举、复制态和会话状态
- `net.rs`：Lightyear 网络配置、消息协议、连接生命周期、输入/命令收发
- `runtime.rs`：Host 权威玩法推进、房间阶段、奖励/商店/RPS/会话控制
- `ui.rs`：Coop 菜单、大厅、复制体显示、联机叠层 UI

### 5.2 模块边界
- 网络协议在 `net.rs`
- Host 游戏规则在 `runtime.rs`
- 显示与交互在 `ui.rs`
- 共享的阶段状态通过 `CoopSessionState` 连接各层

### 5.3 当前定位
- 已能表达完整的合作原型闭环
- 但复杂度高，且存在 Host 权威实体与复制实体并存的维护成本

## 6. `pvp/`：对战联机模块
### 6.1 子模块职责
- `components.rs`：PVP 对局组件和基础标记
- `net.rs`：手写 UDP 消息协议
- `systems.rs`：模拟、预测、状态应用、HUD 更新
- `ui.rs`：大厅、加入、结果页

### 6.2 设计特点
- 明确不复用 Lightyear
- 面向简单双人对战场景
- 逻辑更直接，但多人基础设施与 Coop 形成两套维护路径

## 7. `ui/`：通用界面层
### 7.1 目录职责
- `menu.rs`：主菜单
- `hud.rs`：游戏内 HUD、Boss 血条、提示文本、阶段信息
- `pause.rs`：暂停菜单
- `reward_select.rs`：单机奖励选择页
- `shop.rs`：商店 UI
- `notifications.rs`：成就通知
- `game_over.rs`：失败/胜利页
- `cursor.rs`：自定义光标和准星
- `widgets.rs`：复用 UI 组件工厂

### 7.2 设计要点
- 绝大多数页面都有 setup / update / cleanup 生命周期
- `UiPlugin` 是状态驱动的 UI 调度器
- Coop 使用独立叠层 UI，不直接借用单机 `Paused` 和 `RewardSelect`

## 8. `utils/`：辅助工具
包含：

- `math.rs`
- `collision.rs`
- `entity.rs`
- `rng.rs`
- `timers.rs`
- `easing.rs`

作用：

- 放置跨模块复用但不值得提升为插件的工具逻辑
- 避免在玩法系统里散落重复数学和实体清理代码

## 9. 当前复杂度热点
### 9.1 `src/coop/runtime.rs`
当前全仓库最大文件之一。负责：

- 会话启动
- Host 权威玩家驱动
- 房间推进
- 奖励、RPS、商店、死亡与复活
- 复制态同步辅助

维护建议：

- 任何对 Coop 规则的新增都优先考虑能否先下沉到 `session_core`
- 不要直接在 UI 里写玩法决策；统一交回 `runtime.rs`

### 9.2 `src/coop/ui.rs`
职责混合了：

- 菜单
- 大厅
- 复制体视觉绑定
- Overlay 文本与输入
- 远端血条

维护建议：

- 继续拆分时，应优先按“大厅 UI / 游戏内 Overlay / 复制体可视化”拆层

### 9.3 `src/gameplay/enemy/systems.rs`
该文件同时处理：

- 进房刷怪
- 房间清理
- 敌人死亡与结算
- Boss 接触伤害
- Puzzle 与 Shop 入口清理

维护建议：

- 房间进入时的资源重置和敌人逻辑最好继续拆分，避免一个系统承担过多副作用

### 9.4 `src/gameplay/session_core/mod.rs`
这是规则抽象层，也是最适合继续增加测试的区域。

维护建议：

- 任何奖励、商店、死亡、过关判定的变化，优先在这里做规则变更并补测试

### 9.5 `src/ui/hud.rs`
HUD 文件承担了：

- 玩家血量、金币、层数
- 房间文本与提示
- 敌人数量
- Boss 血条
- 阶段进度

维护建议：

- 如果继续扩 HUD，建议先把“数据准备”和“节点更新”分开
- 联机下依赖 Host-only 资源的部分要格外小心

## 10. 维护结论
- `core + data + gameplay` 组成稳定主骨架
- `coop` 和 `pvp` 是高风险扩展区
- 真正的维护成本集中在少数大文件，不在目录数目
- 新维护者如果只记三件事，应记住：
  1. `GamePlugin` 是总入口
  2. `session_core` 是共享规则层
  3. `coop/runtime.rs` 是联机玩法的主战场
