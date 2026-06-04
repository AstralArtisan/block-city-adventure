# 扩展与维护指南

- 适用版本：`main` 分支
- 最后校验：2026-05-30；结合当前模块结构、配置入口与联机实现整理
- 关联源码：`src/gameplay/`、`src/data/`、`src/coop/`、`src/pvp/`、`src/ui/`、`assets/configs/`
- 实验性内容：包含。联机相关扩展项需要按原型架构审慎推进

## 1. 扩展前的统一检查
无论新增什么内容，先回答五个问题：

1. 它属于单机专属、Coop 共享、还是 PVP 专属？
2. 它的平衡参数是否应该配置化？
3. 它是否需要新的状态、事件或网络消息？
4. 它是否会影响 HUD、奖励、商店、存档或成就？
5. 它是否应该先下沉到 `session_core`，而不是直接写死在某个 UI 或运行时文件里？

## 2. 新增敌人
### 2.1 典型触点
- `src/gameplay/enemy/components.rs`
- `src/data/definitions.rs`
- `assets/configs/enemies.ron`
- `src/gameplay/enemy/ai.rs`
- `src/gameplay/enemy/systems.rs`
- `src/gameplay/enemy/spawner.rs`

### 2.2 推荐步骤
1. 在 `EnemyType` 中增加新枚举。
2. 为它补充配置结构消费路径。
3. 在 `enemies.ron` 中给出数值模板。
4. 在 `ai.rs` 中补行为分支。
5. 在 `spawner.rs` 或刷怪逻辑里决定它的生成条件。
6. 检查是否需要新的投射物、特效或 Boss 互动。
7. 同步更新文档和测试说明。

### 2.3 常见遗漏
- 只加了配置，没加生成入口
- 只加了 AI，没加受击/死亡特化表现
- 忘了考虑 Coop 中是否需要复制展示

## 3. 新增奖励
### 3.1 典型触点
- `src/gameplay/rewards/data.rs`
- `src/gameplay/rewards/apply.rs`
- `src/gameplay/session_core/mod.rs`
- `assets/configs/rewards.ron`
- `src/ui/reward_select.rs`
- 可能还包括 `src/ui/hud.rs`

### 3.2 推荐步骤
1. 在 `RewardType` 中新增枚举。
2. 在 `rewards.ron` 中补充标题、描述和值。
3. 在 `apply.rs` 中实现对玩家属性或行为的影响。
4. 在 `session_core` 的奖励池与草案生成逻辑中接入它。
5. 检查单机奖励页和 Coop 奖励 UI 是否都能正确展示。
6. 若奖励影响玩家派生能力，更新 `RewardModifiers` 或相关组件。
7. 为规则层补测试。

### 3.3 维护原则
- 不要只改文本，不改实际效果
- 不要让单机和 Coop 使用不同的奖励语义
- 奖励曲线优先统一在 `session_core`

## 4. 新增房间类型
### 4.1 典型触点
- `src/gameplay/map/room.rs`
- `src/gameplay/map/generator.rs`
- `src/gameplay/enemy/systems.rs`
- `src/gameplay/session_core/mod.rs`
- `src/ui/hud.rs`
- 可能还包括 `src/gameplay/shop/` 或 `src/gameplay/puzzle/`

### 4.2 推荐步骤
1. 在 `RoomType` 中增加枚举。
2. 在地图生成器里定义生成策略。
3. 在房间进入逻辑中定义是否锁门、刷怪、生成事件对象。
4. 在 HUD/提示文本中补充房间标签。
5. 在共享规则层中决定清房后是否给奖励、是否自动开商店、是否直通下一层。
6. 检查单机与 Coop 是否都需要支持。

### 4.3 高风险点
- 房间类型改动会同时影响地图、敌人、商店、奖励、HUD 和联机阶段
- 如果只在单机支持，文档里必须明确标注

## 5. 新增增强（Augment）
### 5.1 典型触点
- `src/gameplay/augment/data.rs`
- `src/gameplay/augment/effects.rs`
- `src/gameplay/augment/tuning.rs`
- `assets/configs/augments.ron`

### 5.2 推荐步骤
1. 在 `AugmentId` 枚举中新增变体。
2. 在 `augments.ron` 中补充名称、描述、稀有度、槽位，并在各等级 `params` 中填入数值（伤害倍率、半径、`turn_factor` 等）。
3. 在 `effects.rs` 中实现运行时效果系统；数值一律经 `GameDataRegistry::augment_param(id, stacks, key)` / `augment_param_or(...)` 读取，必要时在 `tuning.rs` 加一个类型化薄包装函数——**不要在源码硬编码平衡数值**。
4. 注意效果系统的执行顺序约束（`.before()` / `.after()` 与 combat 系统协调）。
5. 检查增强选择 UI（`augment_select.rs`）是否能正确展示。
6. 若强化可升满级，确认满级后会从奖励 / 事件抽卡池正确剔除（`is_maxed` 过滤）。

### 5.3 常见遗漏
- 只加了枚举和配置，没在 `effects.rs` 中实现效果
- 忘了设置稀有度权重，导致永远不会被选中
- 在源码里硬编码数值而非走 `augments.ron::params`——会被 `loaders_fallback_matches_shipped_ron_files` 之外的人误改，且无法不重编译调参

## 6. `Curse`（历史快照）

- 当前单机已移除 `Curse` 数据模型和对应配置
- 如果后续需要重新引入负面状态，建议不要复活旧祝福祠堂链路
- 更推荐按“独立风险机制”重新设计，并先明确：
  1. 它属于单机专属还是单双机共享
  2. 它是否真的参与战斗数值消费
  3. 它与奖励房、事件房、商店的边界是什么

## 7. 新增事件房类型
### 7.1 典型触点
- `src/gameplay/event_room/mod.rs`
- `src/ui/event_room.rs`

### 7.2 推荐步骤
1. 在 `EventType` 枚举中新增变体。
2. 在 `init_event_for_room` 的事件池中加入新类型。
3. 在 `event_interact_system` 中实现交互逻辑（锁房/生敌人/开 UI）。
4. 在 UI 层补充选项展示和效果描述。
5. 如果是战斗/谜题类事件，确保 `resolve_event_room_clear` 能正确结算。

## 8. 新增 Boss 或 Boss 阶段
### 5.1 典型触点
- `src/gameplay/enemy/components.rs`
- `src/gameplay/enemy/boss.rs`
- `assets/configs/boss.ron`
- `src/ui/hud.rs`
- `src/core/events.rs`

### 5.2 推荐步骤
1. 决定是新增 `BossArchetype` 还是扩展现有 Boss 阶段。
2. 在 `boss.ron` 中补数值。
3. 在 `boss.rs` 中接入阶段阈值、攻击模式和阶段切换。
4. 若需要新提示，使用 `BossPhaseChangeEvent` 和 HUD/Boss 血条。
5. 检查单机与 Coop Host 是否都能复用。

## 9. 新增 UI 页面
### 9.1 典型触点
- `src/ui/mod.rs`
- 新页面文件，例如 `src/ui/<page>.rs`
- `src/states.rs`
- 相关业务模块的状态切换逻辑

### 9.2 推荐步骤
1. 先决定它是否需要独立 `AppState`。
2. 在 `UiPlugin` 中注册 `OnEnter` / `Update` / `OnExit`。
3. 页面必须配套 cleanup。
4. 如果它依赖 gameplay 数据，优先从资源/事件读取，不要直接写玩法状态。
5. 如果它是 Coop 专属页面，优先考虑放在 `coop/ui.rs` 而不是通用 `ui/`。

## 10. 新增配置项
### 10.1 典型触点
- `src/data/definitions.rs`
- `src/data/loaders.rs`
- `src/data/registry.rs`
- 对应 `assets/configs/*.ron`

### 10.2 推荐步骤
1. 先在 `definitions.rs` 增字段。
2. 更新 `.ron` 文件。
3. 更新 `default_registry()` 默认值。
4. 在消费它的 gameplay 模块中接线。
5. 如果会影响平衡，更新文档和校验说明。

## 11. 新增 Coop 阶段或联机交互
### 11.1 典型触点
- `src/coop/components.rs`
- `src/coop/net.rs`
- `src/coop/runtime.rs`
- `src/coop/ui.rs`
- 必要时 `src/gameplay/session_core/mod.rs`

### 11.2 推荐步骤
1. 判断它是纯状态同步，还是需要命令消息。
2. 如果需要阶段，先扩 `CoopPhase`。
3. 如果需要同步数据，扩 `CoopSessionState` 的对应子状态。
4. 如果需要客户端提交操作，扩 `CoopCommandMessage`。
5. 在 Host 运行时处理真实逻辑。
6. 在 UI 层只做输入采集和状态展示。
7. 补充本机双开验证步骤。

### 11.3 原则
- 不要把玩法决策直接塞进 `coop/ui.rs`
- 不要跳过 `CoopSessionState` 私自做“本地猜测状态”
- 如能抽象到 `session_core`，优先抽象

## 12. 新增 PVP 消息或对局能力
### 12.1 典型触点
- `src/pvp/net.rs`
- `src/pvp/systems.rs`
- `src/pvp/ui.rs`

### 12.2 推荐步骤
1. 先判断它属于输入、状态快照还是即时事件。
2. 扩展 `PvpMsg` 及对应结构体。
3. 同时修改 Host 和 Client 的收发分支。
4. 检查本地预测是否仍成立。
5. 确认结果页和 HUD 是否需要同步更新。

## 13. 扩展后最低验证清单
### 13.1 通用
1. `cargo check`
2. `cargo test`
3. 手动跑一轮 `cargo run`

### 13.2 如果改了单机玩法
1. 主菜单进入单机
2. 验证房间推进
3. 验证奖励/商店/结算
4. 验证 `F5` / `F9`

### 13.3 如果改了 Coop
1. 本机双开 Host/Client
2. 验证连接建立
3. 验证进入 `CoopGame`
4. 验证相关交互阶段和断开恢复

### 13.4 如果改了 PVP
1. 本机双开
2. 验证 `Hello/Welcome`
3. 验证状态同步和结果页

## 14. 文档同步要求
以下改动完成后，必须同步文档：

- 新增 `AppState` 或 `RoomType`
- 新增配置文件或新增配置字段
- 新增奖励、敌人、Boss、房间类型
- 新增联机阶段、网络消息或联调入口
- 更改多人地址输入规则或端口

最低同步位置：

- `README.md`
- `docs/03_module_design.md`
- `docs/04_api_and_data_model.md`
- 如涉及联机，再更新 `docs/06_multiplayer_and_risks.md`

## 15. 最后建议
- 新增功能时，优先沿着现有模块边界扩展，不要把临时逻辑堆进大文件
- 复杂规则优先下沉到 `session_core`
- 任何联机改动都要先确认所属体系是 Coop 还是 PVP
- 任何“只是 UI 改动”的说法都值得警惕，因为本项目 UI 往往绑定着状态机和流程语义
