# 内部接口契约与数据模型

- 适用版本：`main` 分支
- 最后校验：2026-05-30；`cargo check` 通过，`cargo test` 83 项通过
- 关联源码：`src/states.rs`、`src/core/events.rs`、`src/core/input.rs`、`src/core/save.rs`、`src/data/definitions.rs`、`src/data/registry.rs`、`src/gameplay/session_core/mod.rs`、`src/coop/components.rs`、`src/coop/net.rs`、`src/pvp/net.rs`
- 实验性内容：包含。联机契约和部分规则模型仍会继续调整

## 1. 说明边界
本项目**没有面向第三方的稳定公共 API**。本页所说的“API 设计”，指的是工程内部模块之间的接口契约，包括：

- 全局状态机
- 输入快照
- 事件总线
- 配置与注册表
- 存档数据模型
- 单机与 Coop 共享规则模型
- Coop 与 PVP 的网络消息/会话状态

如果后续要做模块拆分、DLL/动态插件或外部联机服务，这些内部契约才可能进一步演化成真正的公共 API。

## 2. 状态与输入契约
| 接口 | 定义 | 生产者 | 消费者 | 生命周期 | 扩展注意事项 |
| --- | --- | --- | --- | --- | --- |
| `AppState` | 全局状态机，覆盖 Loading、单机、Coop、PVP、结算与 UI 页面 | 菜单、奖励、商店、联机系统、本地联调 | 几乎所有插件的 `OnEnter` / `OnExit` / `run_if` | 全应用级 | 新增状态时，必须同步检查 setup/cleanup、导航入口和所有 `run_if` |
| `RoomState` | 当前房间推进状态：`Idle` / `Locked` / `Cleared` / `BossFight` | 地图切换、敌人系统、解谜系统、Coop 运行时 | 房门、转场、HUD、房间推进 | 单局级 | 新房间机制不要滥加状态，优先复用现有四态 |
| `PlayerInputState` | 来自键鼠的每帧输入快照 | `InputPlugin` | 单机玩家系统、商店、转场、Coop、PVP | 每帧重置 | 新增输入位后，要同步检查单机、Coop、PVP 三条消费链 |
| `PlayerDriveInput` | 作用到玩家实体上的可消费输入 | 单机 `push_local_input_to_players`、Coop Host 输入落地 | 玩家移动、朝向、攻击、冲刺系统 | 实体组件 | 它是玩家行为系统的真正输入层，不应直接从 UI 写业务状态 |

## 3. 事件总线契约
`src/core/events.rs` 是跨模块事件的主入口。

| 事件 | 语义 | 主要生产者 | 主要消费者 | 扩展注意事项 |
| --- | --- | --- | --- | --- |
| `DamageEvent` | 请求对目标造成伤害 | 命中框、接触伤害、陷阱等 | `combat::damage` | 如果新增伤害来源，优先发事件而非直接改血量 |
| `DamageAppliedEvent` | 伤害已落地，用于表现与统计 | `combat::damage` | 成就、伤害数字、表现层 | 适合接表现与统计，不适合再回写玩法逻辑 |
| `DeathEvent` | 实体死亡 | `combat::damage`、敌人/玩家死亡逻辑 | 成就、结算、清房逻辑 | 新增死亡后效果优先挂这里 |
| `RoomClearedEvent` | 房间目标已完成 | 敌人系统、解谜系统 | 奖励、成就、房门、流程推进 | 单机和 Coop 都高度依赖该事件语义 |
| `DoorOpenEvent` | 房门打开 | 房间推进 | 表现层或未来音效 | 当前使用相对有限 |
| `SpawnEnemyEvent` | 请求刷新房间敌人 | 房间/脚本逻辑 | 敌人系统 | 当前使用有限，可作为后续脚本化入口 |
| `BossPhaseChangeEvent` | Boss 阶段变化 | Boss 控制器 | HUD、特效、音频 | 适合扩展阶段提示和镜头反馈 |
| `AchievementUnlockedEvent` | 成就解锁 | 成就系统 | 通知 UI | 已形成稳定接口 |
| `ShopPurchaseEvent` | 购买行为发生 | 商店系统 | 成就系统 | 新增经济统计时优先复用 |

## 4. 配置与注册表契约
`data/` 把各类配置结构聚合为 `GameDataRegistry`。

| 接口 | 定义 | 生产者 | 消费者 | 生命周期 | 扩展注意事项 |
| --- | --- | --- | --- | --- | --- |
| `PlayerConfig` | 玩家基础数值与冷却/能量参数 | `loaders::load_all_configs` | 玩家系统、战斗系统 | 应用启动后常驻 | 改字段要同步默认值和 `.ron` |
| `EnemiesConfig` | 各敌人模板数值 | 同上 | 刷怪、敌人 AI、Boss 之外的敌人逻辑 | 常驻 | 新敌种要同时扩枚举与配置 |
| `BossesConfig` | 分楼层 Boss 参数 | 同上 | Boss 控制器 | 常驻 | `for_floor()` 已内置楼层映射策略 |
| `RewardsConfig` | 奖励文本和描述配置 | 同上 | 奖励页、商店文本、说明 | 常驻 | 文本型配置要与枚举保持一致 |
| `RoomGenConfig` | 房间生成参数 | 同上 | 地图生成 | 常驻 | 当前规模较小，后续大地图扩展会显著依赖它 |
| `GameBalanceConfig` | 全局平衡参数，如楼层数、房间数、精英概率 | 同上 | 地图、敌人、奖励、商店、进度 | 常驻 | 这是平衡改动的主要入口 |
| `GameDataRegistry` | 全部配置的统一容器 | `DataPlugin` | 几乎所有 gameplay 子模块 | 常驻 | 避免在系统中分别读取多个 ron 文件 |
| `AugmentsConfig` | 增强物定义（30 种） | 同上 | 增强选择、效果系统 | 常驻 | 新增增强要同步 `AugmentId` 枚举和 `augments.ron` |
| `AudioConfig` | 音量与 pitch 参数 | 同上 | 音频系统 | 常驻 | |
| `EffectsConfig` | 粒子、打击暂停、屏幕闪光参数 | 同上 | 效果系统 | 常驻 | |

> **唯一真相源约定（#9 起）**：所有玩法平衡数值都以 `assets/configs/*.ron` 为准，代码内的 `default_registry()` 仅作读取失败时的兜底，并由 `loaders_fallback_matches_shipped_ron_files` 测试锁定二者一致。强化数值统一经 `GameDataRegistry::augment_param(id, stacks, key) -> Option<f32>` 与 `augment_param_or(id, stacks, key, default) -> f32` 查询（key 对应 `augments.ron` 各等级 `params` 的键），`augment/tuning.rs` 退化为类型化薄包装，不再维护手写 match 表。`EliteAffixesConfig::param_or` / `EventsConfig::param_or` 是精英词缀、事件房的同形 helper。

### 4.1 增强/技能组件契约

| 接口 | 定义位置 | 语义 | 扩展注意事项 |
| --- | --- | --- | --- |
| `AugmentId` | `gameplay/augment/data.rs` | 30 种被动增强枚举（近战/远程/移动/通用 × 3 级稀有度） | 新增增强要同步枚举、`augments.ron`、`effects.rs` |
| `AugmentInventory` | `gameplay/augment/data.rs` | 玩家已获得的增强列表（`Vec<AugmentId>`） | 作为 Component 挂在玩家实体上 |
| `SkillSlots` | `gameplay/player/components.rs` | 4 个技能槽位的解锁状态和绑定技能类型 | `SkillSlot::ALL` 遍历，`unlock()` 解锁；`equipped()` 返回已绑定技能用于抽卡去重 |
| `SkillType` | `gameplay/player/components.rs` | 9 种终结技枚举（GroundSlam/BladeDance/ExecutionBlade/BulletBarrage/FrostField/MeteorFall/WarCry/LifeDrain/TimeRift） | 新增技能要同步枚举、`skills.ron`、`execute.rs` 释放逻辑、HUD |
| `SkillConfig` | `data/definitions.rs` | 单个终结技配置：能量消耗/CD + `damage_mult`/`knockback`/`aoe_radius`/`duration_s`/`tick_interval_s`/`projectile_*`/`status` | 终结技数值改 `skills.ron`，`execute.rs` 全部从此读取 |
| `PlayerBuff` | `gameplay/player/components.rs` | WarCry/TimeRift 施加的临时增益（攻速/移速封顶、攻击力等），移动/近战/远程冷却系统读取 | `merge` 按字段取最大值叠加，避免互相覆盖 |

## 5. 存档契约
`src/core/save.rs` 定义了当前单机存档模型。

| 接口 | 定义 | 生产者 | 消费者 | 生命周期 | 扩展注意事项 |
| --- | --- | --- | --- | --- | --- |
| `SaveData` | 存档根结构，包含版本、楼层、玩家、刷怪计数、成就 | `save_hotkey_system` | `load_hotkey_system`、`apply_pending_load` | 持久化到 `saves/*.ron` | 字段变更要考虑版本兼容 |
| `PlayerSave` | 玩家属性快照 | 同上 | 同上 | 存档内部 | 与 `RewardModifiers` 强耦合，新增成长字段要同步 |
| `PendingLoad` | 读档缓冲 | `load_hotkey_system` | `apply_pending_load` | 运行时短生命周期 | 避免在未进入 `InGame` 时直接写世界 |

当前存档覆盖范围：

- 楼层
- 玩家基础属性和奖励修正
- 金币
- 敌人刷新计数
- 已解锁成就

当前未覆盖的典型信息：

- Coop / PVP 会话
- 当前房间布局完整快照
- 地图实体级运行时状态

## 6. 共享规则层契约：`session_core`
`src/gameplay/session_core/mod.rs` 是当前最接近“领域 API”的部分。

| 接口 | 定义 | 生产者 | 消费者 | 生命周期 | 扩展注意事项 |
| --- | --- | --- | --- | --- | --- |
| `SessionMode` | `Solo` / `Coop` | 单机奖励、Coop 运行时 | 规则决策函数 | 临时值 | 任何规则都应先分清模式 |
| `SessionRuleContext` | 当前模式、楼层、总楼层、房间类型、Boss 是否直接胜利等上下文 | 单机 Boss 结算、Coop 运行时 | `on_room_cleared` | 临时值 | 新规则上下文优先加在这里，而不是散落参数 |
| `ShopDraft` / `ShopOfferDraft` | 商店候选集合 | `build_shop_draft` / `refresh_shop_draft` | 单机商店、Coop 商店 | 临时值 | 如果后续商店升级，应先统一这里的语义 |
| `SharedShopItem` | 共享商店物品枚举 | 规则层 | 单机商店、Coop 商店 | 常量枚举 | 不要让单机和 Coop 使用不同价目语义 |
| `DeathDecision` | 死亡后是继续、GameOver 还是 MatchOver | `evaluate_death` | 单机/Coop 流程 | 临时值 | 适合继续扩展多人死亡判定 |
| `RewardFlow` | 单机圣所房奖励页上下文与 Boss 传送门状态 | 单机奖励系统 | `RewardSelect` UI、Boss 传送门 | 单局资源 | 当前只服务单机；Coop 仍保留独立奖励状态 |

## 7. Coop 契约
Coop 的接口分成三类：网络配置、输入/命令、会话状态。

### 7.1 网络配置与运行时
| 接口 | 定义 | 生产者 | 消费者 | 生命周期 | 扩展注意事项 |
| --- | --- | --- | --- | --- | --- |
| `CoopNetConfig` | 联机模式与目标 Host 地址 | 菜单、本地联调 | 网络生命周期系统 | 会话级 | `host_ip` 当前只接受裸 IPv4 |
| `CoopNetState` | 连接状态、client/server 是否启动、输入缓存、命令缓存 | `net.rs` 生命周期系统 | `runtime.rs`、`ui.rs` | 会话级 | 修改字段前先确认 Host/Client 两条路径都一致 |
| `CoopSessionFlow` | 待进入游戏、待退出会话、大厅提示文本 | 大厅/断线流程 | `net.rs`、`runtime.rs`、`ui.rs` | 会话级 | 它承担了“流程控制”而不是玩法状态 |

### 7.2 输入与命令
| 接口 | 定义 | 生产者 | 消费者 | 生命周期 | 扩展注意事项 |
| --- | --- | --- | --- | --- | --- |
| `CoopInputState` | 上传到 Host 的输入快照 | Client、本地 Host client | Host 输入捕获 | 每 tick | 新增输入位要同步 `PlayerInputState` 和复制输入 |
| `BufferedCoopInput` | 绑定在权威玩家上的输入缓存 | Host 运行时 | 玩家行为系统 | 实体组件 | 不要在 UI 层直接写玩家状态 |
| `CoopCommandMessage` | 阶段性交互命令：奖励、RPS、商店 | Coop UI | Host 运行时 | 网络消息 | 新增命令后要同步 Lightyear 注册和消费逻辑 |
| `PlayerSlot` | `P1` / `P2` 逻辑槽位 | Coop 运行时 | UI、命令、奖励、商店 | 常量枚举 | 它比 ClientId 更适合玩法层语义 |

### 7.3 会话状态
| 接口 | 定义 | 生产者 | 消费者 | 生命周期 | 扩展注意事项 |
| --- | --- | --- | --- | --- | --- |
| `CoopPhase` | 当前联机阶段，如 `Reward`、`DoorChoice`、`Shop` | Host 运行时 | Overlay UI、输入处理 | 会话级 | 新阶段通常意味着 UI、运行时、测试都要同步改 |
| `CoopSessionState` | 联机共享状态总对象 | Host 权威运行时 | Client UI、Host 本地 UI | 复制组件 | 它是当前 Coop 最重要的同步接口 |
| `RewardChoiceState` | Coop 奖励面板状态 | Host 运行时 | 客户端奖励 UI | 会话子状态 | 与 `session_core` 的草案结构一一对应 |
| `DoorChoiceState` | 路线选择状态 | Host 运行时 | 门选择 UI | 会话子状态 | 当前实现仍混合真实房门交互语义 |
| `CoopRpsState` | RPS 对决状态 | Host 运行时 | 联机 Overlay | 会话子状态 | 改规则时优先保持纯状态表达 |
| `CoopShopState` | 双方商店报价与购买状态 | Host 运行时 | 联机商店 UI | 会话子状态 | 继续保持与 `ShopDraft` 对齐 |

## 8. PVP 契约
PVP 使用独立消息协议。

| 接口 | 定义 | 生产者 | 消费者 | 生命周期 | 扩展注意事项 |
| --- | --- | --- | --- | --- | --- |
| `PvpNetConfig` | 模式与目标 Host | 菜单、本地联调 | `pvp_net_tick_system` | 会话级 | 仍采用固定端口 3456 |
| `PvpNetState` | Socket、peer、连接状态、最近输入与状态快照 | `pvp/net.rs` | `pvp/systems.rs` | 会话级 | 和 Coop 完全不是一套语义 |
| `PvpMsg` | 网络消息枚举 | Host / Client | Host / Client | 消息级 | 扩协议时要同步序列化和双方分支 |
| `PvpInputMsg` | 客户端上传输入 | Client | Host 模拟 | 每 tick | 保持轻量，避免塞入过多表现字段 |
| `PvpStateMsg` | Host 下发状态快照 | Host | Client | 每 tick | 是 PVP 客户端显示的核心输入 |
| `PvpFireMsg` | 子弹/射击事件 | Host | Client | 事件级 | 用于补充状态快照外的即时表现 |

## 9. 扩展接口时的统一规则
1. 先判断它属于“状态、事件、配置、规则、网络消息”中的哪一类。
2. 如果是玩法规则，优先下沉到 `session_core`。
3. 如果是跨模块通知，优先走事件，不要直接跨层调用。
4. 如果是平衡参数，优先放 `assets/configs/*.ron` 并接入 `GameDataRegistry`。
5. 如果是联机交互，先确认是 `Coop` 还是 `PVP`，不要假设它们共享网络语义。

本页的作用不是列出所有结构体，而是帮助维护者识别“哪些对象是契约，改了会影响多个模块”。
