# 迭代经历与设计演化

- 适用版本：`main` 分支（HEAD `16cda9ea`）
- 最后校验：2026-06-16；`cargo check` / `cargo clippy --all-targets` 通过，`cargo test` 89 项通过
- 关联源码：`git log`、`src/`、`docs/superpowers/specs/`（设计规格文档）
- 实验性内容：包含。联机相关阶段记录的是“原型整合”而非稳定版本发布

## 1. 总览
这个仓库的演化不是线性“做完单机再做联机”，而是围绕课程项目目标，在很短的时间内交错推进：

- 单机主循环骨架
- 联机试探
- 战斗/成长深化
- 表现替换
- 多人合并
- 版本封存

下表按真实提交整理主线阶段。

| 阶段 | 时间 | 关键提交 | 主题 |
| --- | --- | --- | --- |
| 阶段 0 | 需求基线 | `rust_game_codex_requirements.txt` | 明确课程项目的功能边界与架构要求 |
| 阶段 1 | 2026-03-18 | `8a8758d7` | 初始工程与单机骨架 |
| 阶段 2 | 2026-03-18 | `bcdfc23e`、`60619dbe` | 联机测试试探 |
| 阶段 3 | 2026-03-18 | `29a01580` | 战斗平衡与多楼层推进 |
| 阶段 4 | 2026-03-18 | `541538b2`、`3ab30e32` | 角色表现升级与敌人成长细化 |
| 阶段 5 | 2026-03-18 | `bd3add48` | 多人能力与 Roguelike 主线合并 |
| 阶段 6 | 2026-03-28 ~ 2026-03-30 | `91a8bb1f`、`aa90cf3c` | 工作版本保存与文档审视期 |

## 2. 阶段 0：需求基线
时间：形成于项目开发前期  
基线材料：`rust_game_codex_requirements.txt`

### 目标
- 用 Rust 完成一个可演示的 2D 俯视角动作 Roguelike
- 必须体现工程结构、模块划分、状态管理、配置驱动和可扩展性
- 最终不仅要“能玩”，还要“能讲清楚设计”

### 对后续结构的直接影响
- 促成了 `src/app.rs` + 多插件装配结构
- 明确了 `core / data / gameplay / ui` 的模块边界
- 规定了状态机、房间推进、奖励系统、Boss、多类敌人、HUD 等最低交付项
- 为后续把单机和联机都塞进同一工程提供了“必须模块化”的约束

### 仍然可见的遗产
- 当前目录结构仍明显沿着需求文档建议的骨架展开
- `states.rs`、`data/definitions.rs`、`gameplay/*` 的职责划分与需求文档高度一致

## 3. 阶段 1：初始工程与单机骨架
提交：`8a8758d7`  
日期：2026-03-18  
标题：`first commit`

### 目标
- 建立基础工程
- 形成最小可运行的游戏框架

### 典型成果
- 初步建立 Bevy 项目
- 引入基础目录结构
- 确立“从主菜单进入游戏”的主线路

### 设计意义
这是“所有后续模块都围绕一个统一 App 生长”的起点。虽然提交标题非常朴素，但它决定了后续不是做多个独立 demo，而是做一个能持续叠加模块的工程。

### 遗留问题
- 此时仍接近骨架阶段，玩法、敌人、成长、联机都还不完整

## 4. 阶段 2：联机测试试探
提交：`bcdfc23e`、`60619dbe`  
日期：2026-03-18  
标题：`second commit 联机测试`、`third commit 联机测试`

### 目标
- 尝试多人通信能力
- 为后续合作联机和对战联机打基础

### 典型成果
- 开始引入“多人模式”这个独立问题域
- 把联机作为正式开发方向，而不是课设收尾时才临时附加

### 设计意义
这一步让仓库从“单机动作小游戏”变成“单机 + 多人扩展”的工程。后续 `Coop` 与 `PVP` 的存在，根子都在这一阶段的试探。

### 遗留问题
- 早期联机试验通常会带来协议、生命周期和 UI 路径的多次重写
- 当前仓库里保留的部分技术债，本质上都与这类快速试探有关

## 5. 阶段 3：战斗平衡与多楼层推进
提交：`29a01580`  
日期：2026-03-18  
标题：`Tune combat balance and add multi-floor progression`

### 目标
- 把游戏从“单房间对打”推进为“多楼层 Roguelike 闭环”
- 调整战斗体验和成长节奏

### 典型成果
- 多楼层推进成为正式能力，而不是一次性关卡
- 平衡参数和成长曲线开始系统化
- `progression`、奖励、商店、楼层难度这些概念开始真正串起来

### 设计意义
这是项目从“动作原型”转向“Roguelike 主循环”的关键一步。后续的 Boss、奖励、商店、成就、存档都建立在多楼层推进语义之上。

### 遗留问题
- 多楼层推进意味着更多状态和更多横切数据，复杂度显著增加
- 这也推动了后续需要更强的配置驱动和共享规则层

## 6. 阶段 4：角色表现升级与敌人成长细化
提交：`541538b2`、`3ab30e32`  
日期：2026-03-18  
标题：`Replace player block with hero sprite`、`Refine combat progression and enemy behaviors`

### 目标
- 把玩家从占位块升级到更可展示的角色表现
- 让战斗、敌人行为、成长反馈更接近完整课程作品

### 典型成果
- 玩家视觉表现不再只是占位色块
- 敌人 AI、战斗节奏、成长反馈继续细化
- 项目从“可运行”进一步走向“可演示”

### 设计意义
这一步决定了项目不是单纯追求结构，而是开始兼顾答辩观感。`player/animation.rs`、敌人行为、特效、HUD 等系统的重要性由此上升。

### 遗留问题
- 表现层升级后，UI 与玩法状态耦合开始增加
- 这也解释了为什么 `ui/hud.rs` 会逐渐成为一个复杂度热点

## 7. 阶段 5：多人能力与 Roguelike 主线合并
提交：`bd3add48`  
日期：2026-03-18  
标题：`Merge upstream multiplayer features with roguelike gameplay`

### 目标
- 把上游多人能力与本地单机/成长/地图主线整合到一个统一仓库

### 典型成果
- `coop/` 与 `pvp/` 正式并入主工程
- `AppState` 扩展出 `MultiplayerMenu`、`CoopMenu`、`CoopLobby`、`CoopGame`、`PvpMenu`、`PvpLobby`、`PvpGame`、`PvpResult`
- 单机、Coop、PVP 三条路线共处同一应用

### 设计意义
这是当前仓库形态真正形成的阶段。此后仓库不再只是“单机主线 + 附加试验”，而是一个必须同时解释三种流程的综合工程。

### 遗留问题
- 双网络栈并存：Coop 用 Lightyear，PVP 用手写 UDP
- 玩法共享边界变复杂：哪些系统能在 Coop 里复用，哪些只能留在单机
- 文档和调试方式开始容易过时

## 8. 阶段 6：工作版本保存与审视期
提交：`91a8bb1f`、`aa90cf3c`  
日期：2026-03-28、2026-03-30  
标题：`Save current version`、`chore: save current working version`

### 目标
- 冻结当前工作版本
- 为审查、回归、文档化和后续交接提供稳定基线

### 典型成果
- 形成可回溯的保存点
- 允许围绕当前工作树做系统性审计与文档整理

### 设计意义
这一步说明项目已经进入“从写功能转向整理与交付”的阶段。当前这套工程交接文档，本质上就是建立在这个阶段的稳定保存点之上。

### 当前质量状态
- 相比旧文档阶段，测试已增长到 24 个单元测试
- 但编译告警仍然较多，说明项目进入了“功能可用、结构需继续收敛”的典型中后期状态

## 9. 演化结论
从提交历史看，项目的主要演进方向很清晰：

1. 先确立课程型模块化骨架。
2. 再尽早试探联机能力，避免最后临时拼接。
3. 在单机主循环形成后，补足多楼层、成长和表现层。
4. 最后把多人分支合并进来，形成当前的综合工程。

这解释了当前仓库的两个核心特征：

- 单机主循环骨架已经相对扎实
- 联机系统真实存在，但仍带有快速整合期的复杂度和技术债

---

## 10. Coop 联机调试第三轮（2026-04-02）

### 改动内容
- 输入过期清零：`CoopNetState` 新增 `latest_input_ticks` 和 `host_frame_counter`，`host_buffer_player_inputs` 检测 P2 输入超过 3 帧未更新时自动清零 `move_axis` 和 `held` 字段
- Despawn 重复 Replicated Player：`filter_replicated_player_duplicates` 从隐藏改为 despawn 非最佳实体
- EventReader drain：`capture_server_inputs` 和 `receive_coop_command_messages` 在 early return 时调用 `.clear()`
- 对齐 tick/replication rate：`server_replication_send_interval` 从 1/64s 改为 1/60s

### 目的与动机
用户反馈 Client 端松键后角色持续滑行（输入粘滞），且游玩数分钟后完全卡死。三个并行调查 Agent 一致定位到两个根因：(1) `latest_inputs` 中 P2 的 `move_axis` 在无新包到达时永不归零；(2) 重复的 Replicated Player 实体只被隐藏不被销毁，随网络波动无限累积。

### 关键决策
- 输入过期采用帧计数器而非时间戳，避免引入 `Time` 资源依赖，且与 FixedUpdate 的离散特性更匹配
- 重复实体改为 despawn 而非隐藏，因为隐藏实体仍参与所有 ECS 查询，是性能退化的直接原因
- `CoopNetVelocity` 复制暂时保留，因为同时被 Player 和 Projectile 使用，移除涉及面较广

### 已知问题 / 后续工作
- 需要实际双开测试验证粘滞和卡死是否彻底解决
- `cargo test` 32 项全部通过

如果要继续演进，最合理的下一步不是再无节制加新功能，而是围绕文档、规则抽象、联机生命周期和复杂度热点做收敛。

## 11. 战斗蓄力与技能槽位系统（2026-04-03）

### 改动内容
- 能量系统重做：从"自然回复 + 消耗回血/限制远程"改为"战斗蓄力"——通过近战命中、远程命中、击杀、完美冲刺、连击维持充能，蓄满后主动释放终结技
- 技能槽位系统：数字键 1-4 对应 HUD 底部技能栏，1-3 号位放终结技，4 号位预留
- 三种终结技实现：剑气斩（1号位，大范围近战）、标记猎杀（2号位，锁定追踪弹）、闪电冲刺（3号位，穿透冲刺）
- HUD 技能栏 UI：技能图标、按键提示、能量满时发光闪烁、锁定模式遮罩
- 输入绑定：`PlayerInputState` 新增 `skill_1_pressed` ~ `skill_4_pressed`，绑定数字键 1-4
- 配置更新：`player.ron` 新增充能参数（`charge_on_melee_hit`、`charge_on_ranged_hit` 等）
- 移除能量回血：`heal_energy_cost_per_s`、`heal_hp_per_s` 不再生效
- Bug 修复：`update_skill_bar_ui` 中 Query 冲突（`&mut Text` 和 `&mut BackgroundColor` 的多查询冲突），通过 `Without<>` 过滤器解决

### 目的与动机
原能量系统被禁用（`ENERGY_SYSTEM_ENABLED = false`），因为能量回血鼓励保守玩法、能量限制远程攻速体验不佳。重做为战斗蓄力系统，鼓励进攻、奖励操作技巧，给战斗增加高光时刻。这是三阶段单机改进的第一阶段（后续：怪物与Boss差异化、成长与遗物系统）。

### 关键决策
- 能量不自然回复，只通过战斗行为充能——鼓励进攻而非龟缩
- 终结技消耗全部能量（100），稀有但强力，约每 1-2 个房间释放一次
- 玩家实时选择释放哪个终结技（按 1/2/3），而非自动跟随精通路线
- 基础操作（近战/远程/冲刺）完全不消耗能量，保持手感自由
- 标记猎杀替代了最初设计的"弹幕爆发"，因为后者只是"更多子弹"缺乏独特感

### 已知问题 / 后续工作
- 引导提示系统（`TutorialFlags`）尚未实现，计划在后续迭代中加入
- 终结技解锁机制需要与成长/遗物系统联动，待第三阶段设计
- `cargo test` 33 项全部通过
- 下一步：怪物与Boss差异化设计

## 12. Boss 差异化设计——四Boss独特机制（2026-04-05）

### 改动内容
- **Floor1 Guardian**：新增 `BossDirectionalDefense` 组件——Boss 缓慢追踪玩家方向，正面受到 40% 伤害（60% 减伤），背面受到 150% 伤害；玩家需绕背击打弱点
- **Floor2 MirrorWarden**：传送时留下幻象（`MirrorDecoy`），幻象会发射弹幕但不闪白，最多同存 3 个；玩家需分辨真身
- **Floor3 TideHunter**：Phase1 零弹幕，靠锁定冲刺逼近；玩家冲刺穿越冲锋路径（技能判定）触发 `Stunned` 状态，硬直期受到 2.0x 伤害（后调整为 1.5x）
- **Floor4 CubeCore**：4 个浮动子核心（`BossSubCore`）全部存活时主体完全免疫伤害（`BossCoreShield`）；消灭全部核心后主体变脆，之后阶段重生 2 个更强核心
- **首次遭遇提示**：复用 `TutorialFlags` 系统，每个 Boss 首次出现时在屏幕顶部显示机制提示（"攻击背部造成更多伤害"等）
- **配置扩展**：`boss.ron` 新增每层 Boss 独立的 `phase_thresholds`、`projectile_speed`、`contact_damage` 字段

### 目的与动机
原来 4 个 Boss 体型/移动/攻击模式高度相似，只有 HP 差异，缺乏辨识度和记忆点。参考 Hades（机制奖励）、Hollow Knight（阶段变化）、Isaac（特殊互动）的设计原则，给每个 Boss 引入一个"需要理解才能高效打的核心机制"。

### 关键决策
- Guardian 使用朝向判定而非位置绕背，因为 Boss 会缓慢旋转——这让"绕到背后"有一定时间窗口而不是即时反应
- TideHunter 的反制触发设计为"冲刺穿越冲锋路径"而非"挡刀"，因为检测两个运动体碰撞比检测输入时机更可靠，且有更好的视觉反馈
- CubeCore 核心数量选 4 而非 2/6：2 个清理太快缺乏层次感，6 个让玩家感到厌烦

### 已知问题 / 后续工作
- Guardian 正背面视觉上难以区分，需要添加可视化指示器（下一次迭代修复）
- `cargo test` 33 项全部通过

## 13. Guardian 盾牌可视化 + 转向速度修正（2026-04-05）

### 改动内容
- 新增 `GuardianShieldIndicator` 标记组件，Guardian 生成时同步生成橙色矩形子实体（宽 10px、高 52px）作为盾牌方向指示器
- 盾牌指示器随 Boss 朝向实时更新位置和旋转（始终指向面朝方向前方 40px）
- Guardian 转向速度大幅降低：每帧 lerp 系数 `0.06 → 0.012`，让玩家有足够时间绕背
- `boss_guardian_facing_system` 重构：移除 Sprite 查询（导致 Query 冲突），改为通过 `Children` + `GuardianShieldIndicator` 查询更新子实体变换

### 目的与动机
测试时发现 Guardian 正背面肉眼几乎无法区分（都是白色方块），玩家无法判断当前攻击面，机制设计失效。同时原转向速度过快，绕背后 Boss 几乎立即转回来，机制窗口极短。

### 关键决策
- 用子实体而非修改 Boss 本体颜色来显示朝向，避免与 Boss 受击闪白（`DamageFlash`）逻辑冲突
- 橙色（SRGBA 1.0, 0.55, 0.1, 0.85）选择有足够视觉对比度，且不会被误认为血量/状态条

## 14. 全局平衡调整第三阶段（2026-04-06）

### 改动内容
- **Boss HP 重平衡**：Floor1~4 的 max_hp 从 300/340/360/800 调整为 330/450/600/620；打斗时长目标 25-35/35-50/50-70/70-100 秒梯级增长
- **TideHunter 硬直增伤修正**：`amount *= 2.0 → 1.5`，防止机制窗口反而成为秒杀工具（玩家 MarkedHunt 技能在 floor3 约 ATK×8 × 27 = 216 伤害，叠加硬直 2x 可一套带走 360HP 的 Boss）
- **CubeCore 有效HP修正**：初始核心 HP 从 70×4=280 降至 40×4=160；阶段重生公式 `70+phase×20 → 40+phase×10`；CubeCore 总有效 HP 从 ~1560 降至 ~1040（TideHunter 的 1.73x，合理）
- **金币奖励递增修正**：普通敌人掉落从 floor3+ 的 7 金修正为 8/10/13/16（随层递增），修复后期购买力反而下降的问题
- **玩家能量微调**：`kill_charge_gain` 12→10，`elite_kill_charge_gain` 25→22，减少 Boss 前蓄满优势
- **小怪调整**：狙击手伤害 20→17（降低惊吓死亡概率），侧翼兵速度 208→195
- **SupportCaster 重设计**：HP 54→38（必须优先击杀），增益统一为速度×1.5 + 攻速+40%（`cooldown_mult: 1.67`），覆盖目标 2→3 个，新增向玩家发射慢速弹幕

### 目的与动机
试玩发现数值曲线失控：TideHunter（floor3）被一套技能秒杀，CubeCore（floor4）却因隐性有效 HP 极高而过难；金币经济后期反而变少但商店更贵；SupportCaster 不会攻击且多了反而占用刷怪槽位减少玩家压力。参考 Hades/Isaac/RoR2 的设计原则进行系统性修正。

### 关键决策
- CubeCore 降低本体 HP 同时降低核心 HP，而非只降本体——因为核心提供的是"免疫门"，玩家每次需要完整清理，有效 HP 必须单独核算
- SupportCaster 的 `cooldown_mult` 值需要与代码公式一致：代码为 `base_cooldown / cooldown_mult`，所以"加速40%"应填 `1.67` 而非 `0.60`（Codex 初次实现错误，已修正）
- 金币奖励改为硬编码递增，暂不移入配置文件（可作为后续技术债迁移）

### 已知问题 / 后续工作
- 金币奖励仍硬编码在 `systems.rs`，后续可迁移至 `game_balance.ron`
- 数值调整需要实际游玩验证，可能需要进一步微调
- `cargo test` 33 项全部通过
- 下一步：成长与遗物系统设计

## 15. 铭文系统+诅咒系统+祝福祠堂骨架（2026-04-06）

### 改动内容
- **铭文模块** (`src/gameplay/rune/`)：`RuneSlot`（近战/远程/冲刺/终结技 4 槽位）、`RuneTier`（普通/精英/传说 3 等级）、`RuneId`（30 个铭文枚举）、`RuneLoadout` Component（每槽装 1 个铭文，装新替旧）
- **诅咒模块** (`src/gameplay/curse/`)：`CurseId`（脆弱/迟缓/枯竭/暴露/虚弱 5 种）、`CurseState` Component（诅咒生命周期管理，每进入新房间递减，到期自动消除）
- **配置文件**：`runes.ron`（30 个铭文的名称、描述、取舍、价格）、`curses.ron`（5 种诅咒的名称、效果、持续时间）
- **祝福祠堂流程**：原 Reward 房在 Floor 2+ 触发 Blessing 模式，展示 2 个铭文+诅咒选项，玩家选择后铭文装入槽位+诅咒生效，或选择离开不拿
- **祝福房规则**：每层最多 1 个，Floor 1 不出现，有诅咒时不出现新的祝福房
- **HUD 显示**：技能栏旁显示 4 个铭文槽位（空槽灰色/已装备着色），HP 条下显示诅咒状态和剩余房间数
- **配置解析**：`GameDataRegistry` 新增 `runes` 和 `curses` 字段
- **Player spawn**：挂载 `RuneLoadout::default()` 和 `CurseState::default()`

### 目的与动机
当前奖励系统只有属性加成（+ATK/+HP 等），所有获取途径给的都是同质化数值提升，缺乏构建感。引入双轨系统：属性成长（保留现有，稳定变强）+ 铭文系统（改变玩法，构建差异化）。参考 Hades（Boon + 混沌之门）、Slay the Spire（卡牌 + 遗物）、Isaac（道具 + 恶魔房）的设计。

### 关键决策
- 铭文是"替换"而非"叠加"——4 个槽位永远只有 4 个铭文，迫使玩家做选择而非无限变强
- 祝福祠堂采用 Hades 混沌之门模式：强力铭文必须承受临时诅咒，诅咒未消除时不会出现新的祝福房（防止叠加失控）
- 本步只做数据流通（选择→装备→显示），不实现铭文战斗效果——降低单次改动风险，效果实现在后续迭代
- 复用 `AppState::RewardSelect` 而非新增状态，减少状态机复杂度

### 已知问题 / 后续工作
- 铭文战斗效果未实现（第三步）
- 事件房重设计未实现（第二步）
- 商店铭文商品未实现（第二步）
- Boss 奖励调整未实现（第二步）
- Coop 模式下祝福祠堂未同步（需单独工作）
- `cargo test` 33 项全部通过
- 完整设计文档：`docs/superpowers/specs/2026-04-06-reward-system-redesign.md`

## 16. TideHunter 重设计为"影子猎人" + HUD B0001 修复（2026-04-06）

### 改动内容
- **TideHunter 状态机重写**：从简单的"靠近→蓄力→冲刺→冷却"改为"Stalk→Telegraph→ShadowDash→Reposition"循环，核心机制是快速穿越留下紫色影子轨迹（持续伤害地带）
- **影子轨迹系统**：新增 `ShadowTrail` 组件，Boss 穿越路径每 25px 生成一个影子实体，半透明深紫色，线性淡出后 despawn；站在影子上持续受伤，冲刺穿越免疫
- **三阶段递进**：Phase1 单次穿越/影子 2.5s/停顿 0.9s；Phase2 连续 2 次穿越/影子 3.5s/停顿 0.7s + 1 颗弹；Phase3 连续 3 次穿越（三角形包围）/影子 4.5s/停顿 0.6s + 3 颗扇形弹
- **反制机制调整**：从 WindupTelegraph 阶段改为 Reposition 阶段检测——玩家需要穿越影子接近 Boss 才能触发 Stunned（1.4s，伤害 ×1.5）
- **HUD B0001 修复**：`update_rune_and_curse_ui` 的两个 `Text` Query 加 `Without<>` 过滤，解决运行时 panic

### 目的与动机
试玩反馈第三层 Boss 设计单调：循环太简单，只有一个交互机制（冲刺打断），阶段差异仅为攻速变化。重设计为"影子猎人"风格，让战场空间随阶段推进被逐渐压缩，玩家需要在影子缝隙中走位并找准反击窗口。B0001 是铭文系统引入后的 Bevy Query 冲突 bug。

### 关键决策
- 影子轨迹是持续伤害而非一次性伤害——鼓励玩家主动走位而非站桩输出
- 反制窗口从蓄力阶段移到停顿阶段——玩家需要穿越危险的影子区域才能反击，风险与回报对等
- Phase3 三角形穿越路径——让战场被三条影子线切割，空间极度压缩，体现最终阶段的压迫感

### 已知问题 / 后续工作
- 需要实际游玩验证影子伤害数值和持续时间是否合理
- `cargo test` 33 项全部通过

## 17. 奖励系统重构——强化数据模型 + XP/升级系统骨架（2026-04-06）

### 改动内容
- **强化模块** (`src/gameplay/augment/`)：`AugmentId`（30 个强化枚举）、`AugmentRarity`（普通/精英/传说）、`AugmentCategory`（近战/远程/机动/通用）、`HeldAugment`（含叠加层数）、`AugmentInventory` Component（自由收集，无槽位限制）
- **经验系统** (`src/gameplay/progression/experience.rs`)：`PlayerLevel` Component（等级/XP/升级阈值）、`XpGainEvent`/`LevelUpEvent` 事件、`process_xp_gains` 系统
- **配置文件** `assets/configs/augments.ron`：30 个强化的完整定义（id、类别、稀有度、名称、描述、升级描述、商店价格）
- **数据注册**：`AugmentConfig`/`AugmentsConfig` 接入 `GameDataRegistry`，加载器支持 `augments.ron`
- **玩家 spawn**：`spawn_player` 挂载 `AugmentInventory::default()` + `PlayerLevel::default()`

### 目的与动机
试玩反馈：奖励房太少（4 层只见 1 个）、成长存在感低、所有成长走同一管道（三选一）缺乏构建感。决定将成长拆为两层独立系统：
- 第 1 层：属性成长（经验升级 + 简单属性选择，确定性）
- 第 2 层：强化构建（Augment 系统，随机性，替代旧铭文+精通）

本次为阶段 1，只建立数据骨架和事件管线，不实现战斗效果。

### 关键决策
- 强化系统替代旧铭文系统（RuneLoadout 4 槽位 → AugmentInventory 无槽位限制），旧铭文模块暂时保留以避免大规模破坏性重构
- 同类强化可升级（stacks max 2），而非无限叠加
- XP 阈值公式 `40 + (level-1) × 15`，保证前期升级快、后期放缓
- augments.ron 使用 `unwrap_or` 容错加载，缺失时不阻塞游戏启动

### 已知问题 / 后续工作
- 阶段 2：强化获取流程 + 升级选择 UI（AugmentSelect/LevelUpSelect 状态）
- 阶段 3：30 个强化的战斗效果实现
- 阶段 4：事件房 + 商店扩展 + 祝福祠堂改造
- 阶段 5：新怪物（Bomber/Shielder/Summoner）+ 精英词缀 + TideHunter 调整
- 阶段 6：HUD + 平衡 + 旧代码清理
- 完整设计文档：`PLANS.md`
- `cargo test` 33 项全部通过

## 2026-04-06 游戏表现力增强：音效 + 视觉特效 + UI 打磨

### 改动内容

**Phase 1 — 基础设施 + 核心音效 + 核心特效：**
- 新增事件：`SfxEvent`（13 种音效类型）、`HitStopRequest`、`ScreenFlashRequest`
- 新增缓动函数：`ease_out_cubic`、`ease_out_expo`、`ease_out_elastic`、`ease_out_back`
- 新增配置：`audio.ron`（音量/pitch 变化）、`effects.ron`（粒子数/打击暂停/闪光参数）
- 重写 `src/core/audio.rs`：程序化波形合成生成 13 种音效，WAV 编码后插入 `Assets<AudioSource>`，事件驱动播放
- 新建 `hitstop.rs`：基于 `Time<Virtual>` 时间缩放的打击暂停系统
- 新建 `screen_flash.rs`：全屏半透明覆盖 + `ease_out_expo` 衰减
- 新建 `death_effect.rs`：敌人死亡粒子爆炸（普通 16 / Boss 32 粒子 + 闪光 + 震动）
- 桥接系统复用已有事件，直接集成近战/远程/冲刺音效

**Phase 2 — UI 动画 + 增强粒子：**
- 血条/能量条平滑 lerp 动画（`BarAnimState`，指数衰减插值）
- 粒子增强：随机大小、角度偏移、速度衰减、辉光模拟层
- 技能槽位能量不足时渐变灰暗
- 奖励卡片弹入动画（`CardAnim`，`ease_out_back` 缩放）

**Phase 3 — 打磨 + BGM 框架：**
- Boss 阶段切换增强：屏幕闪光 + 震动 + 打击暂停
- BGM 状态机框架：`BgmState` 资源，根据游戏状态自动切换曲目类型

### 目的与动机
游戏手感基础不错但存在零音频、视觉反馈缺失、UI 缺乏动感三个短板。程序化音效生成避免外部资源依赖，保持几何风格一致性。

### 关键决策
- 程序化波形合成：零依赖、即时可用、风格统一
- 桥接系统模式：复用已有事件，避免侵入式修改大量系统签名
- `Time<Virtual>` 时间缩放实现打击暂停
- 配置驱动：粒子数、暂停时长等均从 `effects.ron` 读取

### 已知问题 / 后续工作
- BGM 框架已就绪但无实际音频播放（需外部 .ogg 或程序化环境音）
- 连击计数器 UI 未实现（当前无 ComboCounter 系统）
- `cargo test` 33 项全部通过

---

## 2026-04-06 强化系统阶段2——XP/升级 + 强化选择 + 奖励流程简化

### 改动内容

**阶段1（数据骨架，前次提交已完成）：**
- 新增 `AugmentId`（30个强化）、`AugmentInventory` 组件、`PlayerLevel` 组件
- `assets/configs/augments.ron` 定义全部30个强化的稀有度、类别、描述
- `GameDataRegistry` 新增 `augments` 字段

**阶段2（本次提交）：**
- 敌人死亡发送 `XpGainEvent`（普通怪 8-15 XP，精英 25-40，Boss 100+）
- `process_xp_gains` 系统处理 XP 累积和升级，支持 `XpBonus` 强化加成
- `handle_levelup_event` 系统：升级时生成3个随机属性选项，进入 `LevelUpSelect` 状态
- 新增 `AugmentSelect` / `LevelUpSelect` 两个 `AppState`，完整 UI 生命周期
- 奖励流程简化：普通房通关不再弹 `RewardSelect`，仅40%概率弹 `AugmentSelect`
- Boss房：治疗80% → 必定 `AugmentSelect` → `RewardSelect`（楼层转换）
- `GoldBonus` / `XpBonus` 强化效果已接入战斗系统
- 部分 Common 强化效果已实现（SpeedBoost、HeavyStrike、DashTrail、ExtendedInvuln 等）
- `DashEnergy` 系统：冲刺穿敌回复能量

### 目的与动机
试玩反馈：奖励房太少、成长管道单一、每房三选一打断节奏。将成长拆为两层独立系统（属性升级 + 强化收集），大幅增加获取频率和构建多样性。普通房通关改为即时反馈（金币+XP已在击杀时获得），选择界面只在概率触发或Boss时出现。

### 关键决策
- 普通房不再弹 RewardSelect：减少选择疲劳，XP/金币已在击杀时即时获得
- 强化掉落概率40%/100%：保证每局能收集到足够强化，但不是每房都打断
- `AugmentSelect` 返回状态可配置：普通房返回 `InGame`，Boss房返回 `RewardSelect`
- Codex 调用方式确认：`codex-companion.mjs task --write` 可从 Claude 内部写代码

### 已知问题 / 后续工作
- 阶段3：30个强化的战斗效果实现（大部分尚未接入）
- 阶段4：事件房、商店扩展、掉落物系统
- 阶段5：新怪物（Bomber/Shielder/Summoner）+ 精英词缀
- 阶段6：HUD 更新 + 数值平衡 + 清理旧铭文代码
- `cargo check` 通过，`cargo test` 33 项全部通过

---

## 2026-04-06 修复Boss通关后白光遮挡+状态转换丢失

### 改动内容
- `ScreenFlash` timer 改用 `Time<Real>`，不再受 hitstop 的 `Time<Virtual>` 缩放影响
- 进入 `AugmentSelect`/`LevelUpSelect` 时强制调用 `clear_screen_flash` 清除残留闪光
- `LevelUpEvent` 改为延迟队列 `PendingLevelUps`，当同帧有 `RoomClearedEvent` 时不抢占 `NextState`
- 升级选择延迟到回到 `InGame` 后再弹出

### 目的与动机
Boss 死亡同时触发 hitstop（`Time<Virtual>` 降到 0.05x）和 ScreenFlash（0.3s），导致闪光实际持续约6秒遮挡 UI。同时 Boss 击杀给大量 XP 导致升级，`handle_levelup_event` 与 `enter_reward_selection` 竞争 `NextState`，`RoomClearedEvent` 被消费后丢失，玩家无法进入下一层。

### 关键决策
- 闪光用真实时间而非虚拟时间：闪光是视觉效果，不应受游戏逻辑时间缩放影响
- 升级延迟队列：奖励流程优先级高于升级选择，避免状态竞争

---

## 2026-04-06 强化系统阶段3：8个传说级战斗效果实现

### 改动内容
- **Freeze**（`effects.rs`）：远程命中敌人有概率冻结，冻结期间减速+蓝色染色，计时器到期自动解除
- **DashShield**（`effects.rs` + `dash.rs` + `damage.rs`）：冲刺结束后获得护盾，吸收下一次伤害；`damage.rs` 新增 `Commands` 参数以移除护盾组件
- **Phoenix**（`effects.rs`）：生命归零时自动复活（每局一次），恢复 50%/80% HP，触发金色屏幕闪光
- **Executioner**（`hitbox.rs`）：近战攻击对低血量非Boss敌人直接斩杀（15%/25% HP 阈值）
- **SwordWave**（`combat.rs`）：近战攻击额外发射剑气波，强化版带穿透（PierceCount）
- **Greed**（`combat.rs`）：根据持有金币提升伤害（每100/80金+5%），同时影响近战和远程
- **Blink**（`dash.rs`）：冲刺变为瞬移，通过极高速度+极短持续时间实现；强化版1.5倍距离
- **BulletStorm**（`execute.rs`）：任意终结技释放时额外发射环形弹幕（10/16发）

### 目的与动机
阶段3目标是为30个强化（Common/Elite/Legendary）实现战斗效果。本次完成8个传说级，是阶段3中最复杂的部分。传说级强化需要新增组件、系统和跨模块交互，影响范围涵盖伤害管线、冲刺系统、技能系统。

### 关键决策
- **Blink 用速度模拟瞬移**：避免在已有10个字段的 dash query 中再加 `&mut Transform`，通过设置极高 speed（distance/0.016）和极短 timer（0.016s）让移动系统在一帧内完成位移
- **BulletStorm 挂在终结技通用逻辑后**：不区分具体技能类型，任何终结技都触发弹幕，简化实现
- **Phoenix 用 `PhoenixUsed` 标记**：每局只能触发一次，通过 `Without<PhoenixUsed>` 过滤避免重复复活
- **DashShield 在 damage.rs 中拦截**：在伤害应用前检查并消耗护盾，需要给 `apply_damage_events` 新增 `Commands` 参数

### 已知问题 / 后续工作
- 阶段3的12个普通级和10个精英级效果已在之前由 Codex 实现，本次补完传说级
- 阶段4：事件房、商店扩展、掉落物系统
- `cargo check` 通过，`cargo test` 44 项全部通过

---

## 2026-04-06 Phase 4a：掉落物系统

### 改动内容
- **新建 `src/gameplay/drops/mod.rs`**（~280 行）：`DropPlugin` + 5 个系统
  - `spawn_drops_on_death`：监听 `DeathEvent`，根据敌人类型/精英/Boss/楼层生成金币和 XP 球实体，随机散射方向
  - `drop_physics`：速度指数衰减 + sin 浮动 bob 动画
  - `drop_magnet`：玩家附近的掉落物自动飞向玩家，`PickupRange` 强化扩大磁吸范围（1.6x/2.0x）
  - `drop_collect`：距离 < 28px 时拾取，金币直接加 `Gold`，XP 发送 `XpGainEvent`，播放 `SfxKind::RewardPickup`
  - `drop_expire`：15s 后自动 despawn
- **修改 `src/gameplay/enemy/systems.rs`**：`enemy_death_system` 移除直接加金币（~L964-976）和发 XP 事件（~L962）的逻辑，这些职责迁移到 drops 模块
- **修改 `src/gameplay/mod.rs`**：注册 `drops::DropPlugin`

### 目的与动机
原来敌人死亡时金币和 XP 瞬间发放到玩家账户，无视觉反馈。改为物理掉落物实体：金币（8×8 金色方块）和 XP 球（6×6 青色方块），散射后浮动，玩家靠近自动磁吸拾取。增加战斗的即时反馈感和"捡东西"的满足感。

### 关键决策
- `spawn_drops_on_death` 排序 `.before(enemy_death_system)`：因为 death system 会 despawn 实体，必须在 despawn 前读取敌人信息
- GoldBonus 在掉落生成时按玩家乘数应用，XpBonus 在 `experience.rs` 处理——避免双重计算
- Boss/精英/SubCore 各有独立掉落数值，普通怪按楼层递增
- 系统同时支持 `InGame`（单机）和 `CoopGame`（主机权威）

### 已知问题 / 后续工作
- Phase 4b：商店扩展（强化购买、消耗品、诅咒移除）
- Phase 4c：事件房（合并 Puzzle → Event，11 种事件类型）
- `cargo check` 通过，`cargo test` 44 项全部通过

---

## 2026-04-06 Phase 4b：商店扩展

### 改动内容
- **修改 `src/gameplay/session_core/mod.rs`**：`SharedShopItem` 新增 `Augment(AugmentId)`/`HealingPotion`/`RemoveCurse` 三个变体；`ShopDraft` 扩展为三区（`offers`/`augment_offers`/`utility_offers`）；新增 `build_augment_offers()`（从 registry 随机选 2-3 个强化，按稀有度定价 Common=40/Elite=70/Legendary=120）和 `build_utility_offers()`（回血药水 30g + 诅咒移除 80g）；`apply_shop_item()` 新增 HealingPotion 分支（回复 25% max HP）
- **修改 `src/gameplay/shop/mod.rs`**：`ShopItem`/`ShopOffers`/`CachedShopState` 同步扩展三区；`handle_shop_purchase_input()` 按键映射改为 1/2/3 属性 | 4/5/6 强化 | 7/8 工具；强化购买直接调用 `AugmentInventory::add()`，诅咒移除调用 `CurseState::active.remove(0)`；新增 `AugmentInventory` 和 `CurseState` 到 player query
- **修改 `src/ui/shop.rs`**：UI 分三区渲染，每区有标题（属性/强化/工具）；抽取 `spawn_shop_section()` 复用渲染逻辑；说明文字更新为 "1/2/3 属性 | 4/5/6 强化 | 7/8 工具 | R 刷新 | Esc 关闭"

### 目的与动机
原商店只卖 8 种属性（治疗/强健/锋刃等），缺乏构建多样性。扩展为三区后，玩家可以在商店购买强化（与战斗掉落互补）、使用消耗品（回血药水）、移除诅咒（80g），增加商店的战略价值和访问动机。

### 关键决策
- 强化区价格按稀有度硬编码（Common=40/Elite=70/Legendary=120），未使用 augments.ron 的 shop_cost 字段（简化实现）
- 诅咒移除只在有诅咒时出现在工具区，避免浪费展示位
- 三区共享同一个刷新机制（R 键刷新所有区域）
- 由 Codex（`codex exec --dangerously-bypass-approvals-and-sandbox`）实现，Claude 审查

### 已知问题 / 后续工作
- Phase 4c：事件房（合并 Puzzle → Event，11 种事件类型）
- 商店 cache 按房间级别存储，诅咒状态变化后需刷新才能更新工具区
- `cargo check` 通过，`cargo test` 44 项全部通过

---

## 2026-04-06 Phase 4c：事件房系统（Puzzle → Event 合并）

### 改动内容
- **`RoomType::Puzzle` → `RoomType::Event`**：全局重命名，涉及 17 个文件（room.rs/generator.rs/tiles.rs/doors.rs/hud.rs/achievements.rs/session_core/enemy systems/coop runtime/coop ui 等）
- **新增 `AppState::EventRoom`**：用于非战斗事件的 UI 交互状态
- **新建 `src/gameplay/event_room/mod.rs`**（748 行）：`EventRoomPlugin` + `EventType` 枚举（11 种事件）+ `ActiveEvent` 资源 + 事件选择/执行/完成系统
  - 3 种旧谜题保留：PressurePlate/SwitchOrder/TrapSurvival（调用 puzzle 模块）
  - 6 种非战斗事件：Gambler（50g→随机强化）、CurseAltar（接受诅咒→精英强化）、BloodPact（-30%HP→2选1强化）、Treasure（免费Common强化+30g）、HealingSpring（回复40%HP）、Merchant（2个半价强化）
  - 2 种战斗事件：TimedChallenge（30s击杀→精英强化）、EliteEncounter（单挑精英→精英强化）
- **新建 `src/ui/event_room.rs`**（60 行）：事件房选择 UI（标题+描述+选项列表）
- **修改 `src/gameplay/enemy/systems.rs`**：Event 房 spawn 改由 event_room 模块统一调度
- **修改 `src/app.rs`**：注册 EventRoom 的 OnEnter/OnExit
- **Coop 兼容**：Event 房在 Coop 中仍转为 Normal（保持现有行为）

### 目的与动机
原 Puzzle 房只有 3 种谜题，内容单薄且 Coop 中被降级为 Normal。合并为 Event 房后，房间类型从 3 种扩展到 11 种，增加了风险回报型（赌博/诅咒祭坛/血之契约）、纯收益型（宝箱/治疗泉/旅商）和挑战型（限时/精英遭遇）事件，大幅提升房间多样性和重玩价值。

### 关键决策
- 复用 `puzzle` 模块而非重写：3 种谜题作为 EventType 的子类型，由 event_room 调度 puzzle 模块执行
- 非战斗事件转到 `AppState::EventRoom` UI，战斗事件保持 `InGame` + `RoomState::Locked`
- Event 房通关不触发 RewardSelect（奖励由事件自身处理）
- 由 Codex（`codex exec --dangerously-bypass-approvals-and-sandbox`）实现，Claude 审查

### 已知问题 / 后续工作
- Phase 4 全部完成（4a 掉落物 + 4b 商店扩展 + 4c 事件房）
- 下一步：Phase 5（新怪物 + 精英词缀 + TideHunter 调整）或 Phase 6（HUD + 平衡 + 旧代码清理）
- 需要手动游玩验证事件房各分支的运行时表现
- `cargo check` 通过，`cargo test` 44 项全部通过

---

## 2026-04-06 Phase 5a：三种新怪物（Bomber/Shielder/Summoner）

### 改动内容
- **EnemyType 新增 3 变体**：Bomber、Shielder、Summoner
- **Bomber**（Floor 2+）：靠近玩家后 1s 蓄力自爆（脉冲红白），AoE 65px 范围伤害 28 点。蓄力期可击杀阻止爆炸。HP=30，速度=185
- **Shielder**（Floor 3+）：正面 60° 盾牌挡远程弹幕，缓慢推进（速度=80）。需绕背或近战。HP=72
- **Summoner**（Floor 4+）：保持 350-500 距离，每 4s 召唤 1-2 个 MeleeChaser（最多 3 个活跃）。本体脆弱 HP=28，死亡时所有召唤物 despawn
- **5 个新系统**：bomber_fuse_system、shielder_facing_system、shielder_block_system、summoner_summon_system、summoner_death_cleanup
- **配置**：enemies.ron 新增三种敌人数值
- **AI**：ai.rs 新增 3 个行为分支；spawner.rs 按楼层解锁加入 spawn pool

### 目的与动机
小怪池只有 6 种，Floor 2-4 缺乏新鲜感。3 种新怪物各有独特机制（自爆/盾牌/召唤），增加战术多样性和楼层差异化。

### 关键决策
- Bomber 用 BomberPhase 状态机（Approach→Fuse→Exploded），爆炸生成 Hitbox 而非直接扣血
- Shielder 的盾牌用角度判定（facing 与攻击方向夹角 < 60°），只挡 PlayerRanged 不挡近战
- Summoner 的召唤物用 SummonedBy(Entity) 标记，死亡时批量 despawn

### 已知问题 / 后续工作
- `cargo check` 通过，`cargo test` 44 项全部通过

---

## 2026-04-06 Phase 5b：精英词缀系统

### 改动内容
- **EliteAffix 枚举**：6 种词缀（Swift/Splitting/Shielded/Vampiric/Berserk/Teleporting）
- **Swift**：移速 +50%，攻速 +30%（cooldown ×0.77），spawn 时直接修改 stats，无运行时系统
- **Splitting**：死亡分裂为 2 个同类型弱化版（50% HP，70% damage，无词缀）
- **Shielded**：开局 1 层护盾，在 damage.rs 拦截首次伤害
- **Vampiric**：命中玩家回复自身 10% max HP
- **Berserk**：HP<30% 时 damage ×2，sprite 变红
- **Teleporting**：每 3s 短距离闪现（80-120px）靠近玩家
- **视觉**：精英体型 1.3×，金色色调叠加
- **5 个运行时系统**：elite_splitting_system、elite_shielded（damage.rs 拦截）、elite_vampiric_system、elite_berserk_system、elite_teleport_system

### 目的与动机
精英怪原本只是纯数值放大（HP×2, damage×1.55），缺乏机制区别和辨识度。词缀系统让每个精英都有独特行为，增加战斗策略深度。

### 关键决策
- 每个精英随机分配 1 个词缀，避免多词缀叠加导致的复杂度爆炸
- Shielded 在 damage.rs 伤害管线中拦截，而非独立系统，确保所有伤害来源都被正确拦截
- Splitting 生成的弱化版不带词缀，防止无限分裂

### 已知问题 / 后续工作
- `cargo check` 通过，`cargo test` 44 项全部通过

---

## 2026-04-06 Phase 5c：TideHunter 威胁度提升

### 改动内容
- **Stalk 时间缩短**：P1/P2/P3 从 1.8/1.4/1.0s 降至 1.2/0.8/0.5s，攻击更频繁
- **暗影伤害提升**：shadow_damage_mult 从 0.6 提升至 1.0（全额攻击力）
- **P3 暗影持续延长**：shadow_duration 从 4.5s 增至 6.0s，地面危险区域更持久
- **ShadowDash 接触伤害**：新增 tide_hunter_contact_damage_system，冲刺路径上接触玩家造成 0.5× attack_damage，0.3s 冷却防止连续命中
- **P2+ 目标预判**：Telegraph 阶段计算 predicted_pos = player_pos + velocity × 0.3s，冲刺目标不再是玩家当前位置而是预判位置

### 目的与动机
Floor 3 Boss TideHunter 威胁偏低，玩家可以轻松站撸。通过缩短攻击间隔、提升伤害、增加接触伤害和目标预判，让 Boss 战更有压迫感和躲避需求。

### 关键决策
- 接触伤害用 TideHunterState 内置 contact_hit_cooldown Timer，避免新增组件
- 目标预判只在 P2+ 生效，P1 保持原始行为作为学习阶段
- player query 新增 Option<&Velocity>，兼容无 Velocity 组件的情况

### 已知问题 / 后续工作
- Phase 5 全部完成（5a 新怪物 + 5b 精英词缀 + 5c TideHunter）
- 下一步：Phase 6（HUD + 平衡 + 旧代码清理）
- 需要手动游玩验证 TideHunter P2/P3 的压迫感和接触伤害触发频率
- `cargo check` 通过，`cargo test` 44 项全部通过

---

## 2026-04-06 Phase 6a：编译警告清零 + 死代码清理

### 改动内容
- 消除全部 91 个编译警告（废弃 API、未使用 import、多余 mut、死代码）
- `ReceivedCharacter` → `KeyboardInput` 替换（coop/ui.rs、pvp/ui.rs）
- 删除 minimap 死代码（MinimapRoomNode、MinimapDynamic、update_minimap、room_color）
- 未来可能使用的函数/字段加 `#[allow(dead_code)]`
- 删除 18 处多余 mut（coop/runtime.rs）

### 目的与动机
91 个警告严重干扰编译输出，掩盖真正的问题。`ReceivedCharacter` 在 Bevy 0.14 已废弃，minimap 代码从未被调用。零警告是后续开发的基础。

### 关键决策
- 对确认无用的代码直接删除，对可能未来使用的加 `#[allow(dead_code)]` 而非删除
- `ReceivedCharacter` 替换保持原有字符过滤逻辑不变

### 已知问题 / 后续工作
- `cargo check` 0 警告，`cargo test` 44 项全部通过

---

## 2026-04-06 Phase 6b：精英词缀浮动标签

### 改动内容
- `EliteAffix` 新增 `label()` 方法（中文名：迅捷/分裂/护盾/吸血/狂暴/闪现）和 `color()` 方法
- 新增 `EliteAffixLabel` 标记组件
- `spawn_enemy` 中为精英怪生成 Text2dBundle 子实体，头顶显示词缀名+对应颜色

### 目的与动机
精英词缀对玩家不可见，无法识别精英类型并调整策略。浮动标签让玩家一眼看出精英的特殊能力。

### 关键决策
- 标签作为子实体自动跟随敌人移动，无需额外更新系统
- 每种词缀用不同颜色区分，增强视觉辨识度

### 已知问题 / 后续工作
- `cargo check` 0 警告，`cargo test` 44 项全部通过

---

## 2026-04-06 Phase 6c：奖励数值外部化

### 改动内容
- 新增 `FloorGains` 和 `RewardScalingConfig` 结构体（src/data/definitions.rs）
- 7 种属性增益曲线（attack_speed, attack_power, max_health, dash_cooldown, lifesteal, crit_chance, move_speed）+ 治疗公式从硬编码 match 改为配置驱动
- `assets/configs/rewards.ron` 新增 `scaling` 段，28 个数值可热调
- 所有调用链更新：apply.rs → session_core → shop → coop/runtime → reward_select UI
- 9 个文件修改，153 行新增，84 行删除

### 目的与动机
奖励增益曲线硬编码在 Rust 源码中，每次调数值都需要重新编译。外部化到 RON 配置文件后，可以直接修改数值文件进行平衡调整。

### 关键决策
- 使用 `#[serde(default)]` 确保旧 rewards.ron 文件向后兼容
- `RewardScalingConfig::default_config()` 保留与原硬编码完全一致的默认值，行为零变更
- 只外部化楼层增益曲线，不动武器精通、技能常量等（留给后续迭代）

### 已知问题 / 后续工作
- Phase 6 全部完成（6a 警告清零 + 6b 精英标签 + 6c 数值外部化）
- 已在 `dfdd2e19` 打 tag `pre-xp-refactor`，便于回退到 XP/强化系统引入之前的版本
- `cargo check` 0 警告，`cargo test` 44 项全部通过

---

## 2026-04-07 Phase 7：9 个游戏体验问题修复

### 改动内容
- **系统执行顺序修复（核心）**：`spawn_drops_on_death` 新增 `.after(apply_damage_events)`，`death_effect_system` 新增 `.after(apply_damage_events).before(enemy_death_system)`，修复掉落物不生成和屏幕中心红色团块两个 bug
- **死亡粒子 fallback 修复**：`death_effect.rs` 中 transform 查询失败时改为 `continue` 跳过，而非 fallback 到 `Vec2::ZERO`（屏幕中心）
- **掉落物视觉增强**：金币 8px→12px（金黄色），经验球 6px→10px（蓝色），各添加 glow 子实体；拾取距离 28→36px；拾取时生成浮动文字（`+N` 黄色 / `+NXP` 蓝色）
- **死亡粒子调整**：数量 16→8，尺寸 4-10px→2.5-6px，生命周期 0.35s→0.25s
- **经验条 UI**：HUD 生命条下方新增紫色经验条 + `LevelText`（Lv.N）+ `ExperienceText`（XP/总量）
- **事件房防重复触发**：`resolve_event_room_clear` 加 `run_if`，`event_room_input` 加 resolved 防护，`mark_event_resolved` 清除 `event_type`
- **Boss 奖励简化**：Boss 通关 `reward_mode` 改为 `None`，只保留 AugmentSelect 三选一，移除旧的 DualBuff 双列属性选择
- **强化视觉效果**：新增 `ExpandingRing` 组件和 `update_expanding_rings` 系统；旋风斩（扩散环）、连锁闪电（闪电线段）、散射（扇形光效）、冲刺护盾（蓝色圆环子实体）、荆棘（橙色刺粒子）、弹幕风暴（爆发光环）、闪现（紫色传送粒子）、冰冻（冰晶飘浮粒子）
- **精英系统增强**：标签字号 12→18，添加 4 方向描边 + 彩色光晕子实体（`EliteGlow`）；新增 `RoomType::Elite`（Floor 2+ 出现，精英强度 ×1.2）；精英金币奖励 +5→+10，精英经验 `25+(floor-1)*5`→`35+(floor-1)*8`
- **音效静默**：`sfx_playback_system` 和 `bgm_state_sync_system` 静默，`audio.ron` 音量归零，保留框架代码

### 目的与动机
用户实际游玩后发现大量体验问题：掉落物不生成（系统顺序 bug）、屏幕中心红色团块（Vec2::ZERO fallback bug）、经验条缺失、Boss 旧双选奖励遗留、强化无视觉效果、精英标签看不清、音效难听。本次集中修复所有问题。

### 关键决策
- 掉落物和死亡粒子的 bug 根因相同：`DeathEvent` 发送后敌人实体在同帧被 despawn，读取顺序不对导致查询失败
- 精英房作为新的 `RoomType` 而非修改现有 Normal 房逻辑，保持关注点分离
- 音效框架保留（`SfxHandles`、`SfxEvent` 等），只静默播放，便于后续重做

### 已知问题 / 后续工作
- 音效需要重新设计（当前程序化合成音效已静默）
- `cargo check` 通过（有 dead_code warnings 来自静默的音效字段），`cargo test` 44 项全部通过

---

## 2026-04-07 Phase 8：UI 清理 + 事件房流程重设计 + 进门位置修复

### 改动内容
- **铭文 UI 清理**：删除 `RuneHudSlot`/`RuneHudText` 组件定义，删除 `setup_hud` 中铭文槽位 UI 节点，`update_rune_and_curse_ui` 保留但移除铭文相关查询（只保留诅咒状态更新）
- **事件房流程重设计**：非战斗事件（Gambler/CurseAltar/BloodPact/Treasure/HealingSpring/Merchant）进入时不再立即弹 UI，改为锁定房间 + 生成世界内交互提示（`EventInteractPrompt`，彩色文字"【事件名】\n按 E 交互"）；新增 `event_interact_system`，玩家按 E 后才进入 `AppState::EventRoom`；事件完成/放弃均设置 `RoomState::Cleared`，门开启，玩家自由选门离开
- **事件 UI 视觉更新**：scrim 透明度 0.62→0.40，面板左侧添加 8px 彩色竖条（按事件类型着色），标题前加符号前缀（◈/☠/♦/✦/✿/⚙）
- **事件房防重复触发加强**：`select_and_spawn_event` 中只要 `active.room == Some(current_room.0)` 就直接返回
- **进门位置固定**：`player_spawn_position` 改为始终返回左侧固定位置（`-ROOM_HALF_WIDTH * 0.6`），不再根据进入方向变化
- **敌人生成保护区**：`spawn_room_enemies` 中过滤距玩家出生点 120px 内的生成点，不足时补充距离最远的备用点

### 目的与动机
用户反馈：铭文系统已废弃但 UI 残留；事件房直接弹 UI 与游戏整体风格不符，应有交互过程；进门后刷新位置不固定且可能刷在怪脸上。

### 关键决策
- 事件房新流程复用战斗房的门机制（`RoomState::Cleared` → 门变金色可通行），无需新增状态
- Esc/Leave 路径也设置 `RoomState::Cleared`，防止玩家放弃事件后被困在锁住的房间
- 进门位置固定到左侧而非根据方向变化，与游戏的横向关卡设计一致

### 已知问题 / 后续工作
- 需要手动游玩验证事件房新流程的体验

---

## 2026-04-07~08 Phase 9：事件房重构 + Boss传送门 + 掉落物平衡 + 升级回血 + 精英房重设计

### 改动内容

**事件房交互重构（仿商店模式）：**
- 将 `select_and_spawn_event` 拆分为 `init_event_for_room`（只选事件+设标记）和扩展后的 `event_interact_system`（按 E 激活）
- 进入事件房后不再立即锁房/生敌人/开UI，而是显示"按 E 交互"提示，玩家可自由选择其他门离开
- 按 E 后根据事件类型激活：puzzle 锁房+生成谜题，战斗锁房+生成敌人，非战斗打开 EventRoom UI
- Esc 不解决事件，设 `interaction_ready = true` 允许重新按 E 打开
- Puzzle 完成后给予 augment 奖励（`AugmentPool::Any`）
- 修复 `room_entry_spawner` 中 `reset_active_event` 导致事件类型被重置的 bug

**Boss 通关传送门：**
- Boss `reward_mode` 恢复为 `Some(DualBuff)`，但不走 RewardSelect，只走 AugmentSelect
- AugmentSelect 完成后返回 InGame，地图中心生成紫色传送门（`BossPortal` 组件）
- 玩家靠近传送门按 E 推进到下一层或通关（Victory）
- 传送门交互时执行楼层清理、FloorLayout 重建、玩家位置重置

**Boss 子核心清理：**
- `enemy_death_system` 中 Boss 死亡时同时清理所有 `BossSubCore` 实体
- `room_ctx` 改为 `Option` 类型，避免楼层推进后 FloorLayout 不存在导致 panic

**掉落物数值平衡：**
- 掉落物生命周期 15s → 8s
- XP 升级曲线 `40+(n-1)*15` → `25+(n-1)*10`（Floor 1 可升 2-3 级）
- Boss/精英死亡生成多个掉落物（Boss 8金+6经验，精英 4金+3经验），Floor 3+ 翻倍

**升级 UI 重构为"回血或强化"双栏布局：**
- 左栏固定"回血"按钮（按 1），显示基于楼层和最大生命值计算的恢复量
- 右栏 3 个随机属性强化卡片（按 2/3/4）
- 新增 `LevelUpStat::RecoverHealth(f32)` 变体
- 回血量使用 `heal_amount` 函数（`src/gameplay/rewards/apply.rs`）

**精英房重新设计：**
- `RoomType::Elite` 独立分支，不再与 Normal 合并
- 新增 `spawn_elite_room_enemies`：固定 3 个敌人（1 精英 + 2 普通）
- 精英怪体积 1.4x，`floor_multiplier *= 1.3`
- 精英房通关 100% 触发 AugmentSelect（普通房 40%）

**小怪头顶血条：**
- 新增 `EnemyHealthBar` + `EnemyHealthBarFill` 组件
- 世界空间 SpriteBundle 跟随敌人位置，Boss 排除
- 颜色随血量变化：绿→黄→红，敌人死亡自动清理

**精英词缀标签乱码修复：**
- 精英词缀 `Text2dBundle` 的 `TextStyle` 添加 `font: assets.font.clone()`，修复中文乱码

### 目的与动机
用户游玩测试反馈：事件房进门立即触发不合理、Boss 通关无奖励且无法前往下一层、经验升级困难、掉落物缺乏视觉冲击、回血途径太少容易死、精英房与普通房差别不大、小怪血量不可见、精英词缀标签乱码。

### 关键决策
- 事件房仿照商店模式（先放柱子，按 E 激活），保持交互一致性
- Boss 通关不自动推进楼层，而是生成传送门让玩家有时间收集掉落物
- 升级回血参考旧版 `HealOrBuff` UI 布局，但放在升级系统而非奖励系统中
- 精英房保证有精英怪（不依赖概率），敌人数量减少但更强

### 已知问题 / 后续工作
- 音效系统仍静默，需要重新设计
- `cargo check` 通过（有 dead_code warnings 来自静默的音效字段），`cargo test` 44 项全部通过
- `cargo check` 通过，`cargo test` 44 项全部通过

---

## Phase 10：组长例会准备——架构审查与文档全面更新（2026-04-11）

### 改动内容

**汇报材料生成：**
- 新增 `docs/meeting_briefing.md`：口头汇报提纲，覆盖项目定位、技术选型、五层架构、8 个 Rust 语言特性实际运用（配代码片段）、当前进度、后续计划

**架构审查：**
- 新增 `docs/architecture_refactor_suggestions.md`：记录 6 个架构问题及修复建议
  - P0：铭文系统残留代码未清理（14 个文件 + 1 个配置）
  - P1：AugmentPlugin/RunePlugin/CursePlugin 注册位置不一致
  - P1：EventRoom UI 系统泄漏到 app.rs 顶层
  - P2：TeamMarker 在 player 和 enemy 中重复定义
  - P2：大量模块级 `#![allow(dead_code)]`
  - P3：session_core 单文件 1200 行过大

**文档全面更新：**
- `docs/02_architecture.md`：mermaid 插件树图补 6 个缺失插件，状态图补 3 个缺失状态，元信息刷新
- `docs/03_module_design.md`：新增 augment/rune/curse/skills 4 个子模块说明
- `docs/04_api_and_data_model.md`：补充增强/诅咒/技能组件契约表
- `docs/07_extension_guide.md`：新增 3 条扩展路径（增强/诅咒/事件房）
- `README.md`：测试数 24→44，源文件 92→110，能力矩阵补增强/诅咒/技能行
- `docs/00_index.md`：源文件数刷新，文档地图补 2 个新文档

### 目的与动机
Rust 程序设计课程第一次组长例会准备。需要理清项目架构以便口头汇报，同时借此机会全面审查代码与文档的一致性。

### 关键决策
- 发现铭文系统设计上已移除但代码完全保留（14 个文件引用），标记为 P0 清理任务，本次不动代码，后续单独处理
- 文档更新原则：不删除旧内容，在过时处加 `[历史快照]` 标注；mermaid 图直接更新为当前事实
- 汇报聚焦单机架构 + Rust 语言特性，联机暂不展开

### 已知问题 / 后续工作
- 铭文系统残留代码清理（P0，涉及 14 个源文件 + runes.ron）
- 插件注册位置统一（P1，AugmentPlugin/CursePlugin 移入 GameplayPlugin）
- EventRoom UI 系统归位（P1）

---

## little-refactor Phase 1：流程 bug 修复 + 增量修改计划（2026-05-15）

新分支 `little-refactor`（从 `claude-playground` 创建，承接 rune/curse→AugmentInventory 进行中工作树）。关键提交 `0b20f499`。

### 改动内容
- 新增 `docs/superpowers/specs/2026-05-15-incremental-modification-plan.md`：将全面重构 spec 转译为「就地增量修改」方案——保持 Bevy 0.14.2、统一 spec 自相矛盾的口径、bug 清单、分阶段路线图
- `src/data/loaders.rs`：配置加载改为**按文件独立回退**。原 `try_load_all()` 对任一必需 RON 用 `?`，单个文件损坏会令整个 registry 回退默认值；现每个文件失败只回退该项并在日志中点名
- `src/core/save.rs`：存档补齐 `AugmentInventory`/`PlayerLevel`/`SkillSlots`，`version` 1→2，新字段 `#[serde(default)]` 兼容旧档
- `src/gameplay/event_room/mod.rs`：非战斗事件 Esc 退出后再交互时**复用已生成选项**而非重掷，堵住 Gambler/Treasure/BloodPact 的免费 re-roll；保留「Esc 不解决事件」语义
- `src/gameplay/rewards/systems.rs`：圣所从 UpgradePick/AwakeningPick 按 Back 时保留原选项不再 RNG 重掷；强化锻造空池时安全收敛奖励房，消除死按钮软锁
- `src/gameplay/shop/mod.rs`：商店增加 Esc 退出，避免无金币玩家被困

### 目的与动机
用户要求复用全面重构 spec 的全部设计，但以「修改而非重构」就地落地，并优先修复游戏流程 bug。本阶段只做**状态机无关**的确定性 bug，确保后续状态机迁移不返工。

### 关键决策
- 引擎保持 Bevy 0.14.2（已核实其原生支持 `SubStates`/`ComputedStates`，三层状态机迁移可行，无需升级引擎）
- Bug#7（progression/skills 不在 CoopGame 运行）**推迟到 Phase 2**：朴素放宽 `run_if` 会让 coop client 跑单机楼层/升级逻辑并与 coop 自有 runtime 冲突造成 desync，根因是状态机设计，留待 Phase 2 一并解决
- 商店「同帧双购 / 楼层重置残留」经核实在当前代码不可达，不加防御性死代码
- Phase 1 提交为 little-refactor 基线：承接的 rune/curse→augment 工作树内部一致（cargo check/test 通过），与 Phase 1 修复一同作为分支起点提交

### 已知问题 / 后续工作
- Phase 2：三层状态机迁移（AppState→GamePhase→RoomPhase），并根因修复 Bug#7、RoomState 非状态问题
- 验证：`cargo check` 通过（仅 3 个既有 audio dead-code 警告，无新增），`cargo test` 45/45 通过

---

## little-refactor Phase 2：AppState→GamePhase 状态机分层（2026-05-15）

关键提交见 little-refactor 分支 Phase 2。

### 改动内容
- `src/states.rs`：`AppState` 瘦身为 11 个顶层态（Loading/MainMenu/InGame/Multiplayer/Coop*/Pvp*）；新增 `GamePhase`（**manual `impl SubStates`**，`SourceStates=Option<AppState>`，对 `InGame|CoopGame` 存在，默认 `Playing`），变体 `{Playing,Paused,RewardSelect,AugmentSelect,LevelUpSelect,Shop,EventRoom,GameOver,Victory}`；`RoomState` 保持 Resource 不变
- `src/app.rs`：`add_sub_state::<GamePhase>()`
- 10 文件覆盖层迁移：覆盖层 `AppState::X` → `GamePhase::X`（OnEnter/OnExit/in_state/NextState 参数类型），含 `pause`/`cursor`/`rewards`/`shop`/`event_room`/`progression`/`player` 死亡/`augment_select`/`levelup_select`/`game_over` 关联；`AugmentChoices`/`LevelUpChoices.return_state` 改 `Option<GamePhase>`
- 行为保持 sweep：~17 文件 `in_state(AppState::InGame)` → `in_state(AppState::InGame).and_then(in_state(GamePhase::Playing))`，使覆盖层（暂停/商店/升级…）正确暂停玩法，语义与迁移前完全等价
- HUD 改为整局存活（`OnEnter/OnExit(AppState::InGame|CoopGame)`），不再随 RewardSelect 重建/清理

### 目的与动机
扁平 18 态状态机靠 AppState 切换隐式暂停玩法。三层化后覆盖层成为 `InGame/CoopGame` 的子状态，调度更精确，并为后续阶段铺路。本阶段只做第一层，控制风险。

### 关键决策
- Bevy 0.14.2 原生支持 `SubStates`；GamePhase 需同时存在于 InGame 与 CoopGame，故用 manual `SubStates` impl（derive 仅支持单源）
- **RoomState 保持 Resource**（同帧写后读语义、BossFight 变体、coop 主机权威——属语义重设计，列 Phase 2b 延后）
- **不动 coop CoopPhase**（与 Phase 5 重叠，coop 下 GamePhase 恒为 Playing）
- **Bug#7 重新定位到 Phase 5**：progression/skills 进 coop 需 `is_coop_authority` 门控，Phase 2 仅行为保持改写（仍单机门控），避免 coop client desync
- 含 PvpGame 的复合条件只对 InGame 分支加 `.and_then(GamePhase::Playing)`（PvpGame 无 GamePhase）；`core/input.rs` 输入采集为基础设施不门控（覆盖层需读输入）

### 已知问题 / 后续工作
- Phase 2b：RoomState→RoomPhase 房间流程语义重设计（可并入 Phase 5）
- Phase 3：强化 2→3 层质变、9 终结技、事件房 17 种等内容
- 验证：`cargo check` 通过（仅 3 个既有 audio dead-code 警告，无新增），`cargo test` 45/45 通过

---

## little-refactor Phase 3：Codex 产出审查 + 敌人楼层池根因修复（2026-05-15）

Phase 3 内容由 Codex 实现（30+ 文件 + 新配置）。Claude 按用户指示逐项审查并就地对齐 4-29 设计文档，修复根因 bug。

### 改动内容（Claude 本轮）
- **修复"floor 1 刷高层怪"根因**：`game_balance.ron` 的 `enemy_types` 扁平满列表触发 `choose_enemy_types` 的"非空即全局覆盖"短路，跳过楼层门控。
  - `data/definitions.rs`：`enemy_types: Vec<EnemyType>` → `#[serde(default)] enemy_pools_by_floor: Vec<Vec<EnemyType>>`
  - `game_balance.ron`：改为每层新解锁池 `[[MeleeChaser,Lobber,Charger],[RangedShooter,Flanker,Bomber],[Sniper,Shielder],[SupportCaster,Summoner]]`（4-29 §7.1）
  - `enemy/spawner.rs`：`choose_enemy_types` 改为按楼层 1..=floor 累积并集；空配置回退到内置 spec §7.1 表；**删除全局覆盖短路**；Lobber 归入 `backline_enemy_types`
  - `data/loaders.rs`：默认值改 `enemy_pools_by_floor: vec![]`；改写锁死错误形状的旧测试；**新增回归测试** `floor1_enemy_pool_excludes_higher_floor_enemies`

### 审查结论（Codex Phase 3 vs 4-29）
- §4 强化：`augments.ron` 30×3 层质变、类别/稀有度/价格、`data.rs` 3 层 cap —— 与 §4.4 **忠实一致**
- §5 终结技：`skills.ron` 9 个 + 档位/能量/CD、`skills/execute.rs` 9 分支、`slots.rs` floor≥2/3/4 解锁 —— 与 §5 **一致**
- §6 事件/圣所/商店：`events.ron` 17（3+10+4）、`shop.ron` 价格全表、圣所流程 —— 与 §6 **一致**；**Phase 1 修复（事件房 Esc 防重 roll、圣所 Back 不重抽/空池收敛）全部保留未回退**
- §7 怪物：`enemies.ron` 含 Lobber、Lobber 落点预警、Bomber 引信、Shielder 格挡、精英 floor≥3 双词缀、MirrorWarden 分身 —— 与 §7 **一致**
- §8 地图/经济：每层 10 房 + 门规则、`balance.ron` 经济区间 —— 与 §6.4/§8 **一致**
- coop 改动（`runtime.rs` 测试断言 + `ui.rs` Lobber match 臂）：EnemyType 新增变体的穷尽匹配 + 共享刷新曲线连带，**编译/测试必要，非越界，保留**

### 关键决策
- 4-29 作为设计文档，只对齐其设计要求（流程/怪物/事件/技能），不强求其技术性描述
- 数据驱动按楼层池替代"扁平列表+全局覆盖"反模式；空配置走内置 spec 表，永不与楼层无关
- 文档教训：交给委托实现者的 plan/交接须单一权威源、内联精确数值、预警雷区（详见记忆）

### 已知问题 / 后续工作
- Charger"撞墙眩晕 1.5s"为细粒度行为，未在本轮深核（设计结构无偏差，列后续打磨）
- Phase 4+（成长/NG+/coop 重写等）后续推进
- 验证：`cargo check` 通过（仅 3 个既有 audio dead-code 警告，无新增），`cargo test` **67/67 通过**（含新回归测试）

---

## 2026-05-16 Clippy 全量警告清零

**改动内容：**
- `Cargo.toml` 新增 `[lints.clippy]` 段，allow `type_complexity` 和 `too_many_arguments`
- `src/core/audio.rs` 的 `SfxHandles` 结构体及其 `impl` 块添加 `#[allow(dead_code)]`

**目的与动机：**
项目积累了 187 个 clippy 警告（126 type_complexity + 59 too_many_arguments + 2 dead_code），全部是 Bevy ECS 架构固有特征或待重设计模块的预留代码。统一配置 lint allow 使 `cargo clippy` 输出干净，后续新增的真正代码问题不会被淹没。

**关键决策：**
- 选择 `Cargo.toml [lints.clippy]` 全局配置而非逐函数 `#[allow(...)]`，因为 Bevy 系统函数天然需要多参数和复杂 Query 类型，逐一标注无意义
- `SfxHandles` 保留代码（已构建并插入为 Resource），待音效系统重设计后移除 allow

**验证：**
- `cargo clippy`：0 警告
- `cargo test`：86/86 通过

---

## 2026-05-16 Rust 2024 Edition 代码现代化

**改动内容：**
- 30 个文件使用 let chains 合并嵌套 if-let 条件
- `#[derive(Default)]` + `#[default]` 替换手写 `impl Default`
- 移除重复的 `#[allow(dead_code)]` 注解
- 编译期断言改用 const block
- 单臂 match 改为 irrefutable let 解构
- `.gitignore` 解除 `.github/` 目录排除（配合新增的 CI 工作流）

**目的与动机：**
项目已使用 `edition = "2024"`，但代码中仍保留大量旧版写法（嵌套 if-let、手写 Default 等）。统一升级为 2024 惯用写法，净减 58 行，提升可读性。

**关键决策：**
- 等价重构，不改变任何运行时行为
- let chains 是 Rust 2024 稳定特性，减少嵌套层次使控制流更清晰

---

## 2026-05-19 测试模式：单机死亡满血复活

**改动内容：**
- 主菜单新增「测试模式」入口，开启 `TestMode` 资源（`src/core/test_mode.rs`）
- 测试模式下玩家死亡不进 `GameOver`，原地满血复活并附带 2 秒无敌（复用 `InvincibilityTimer`）
- 仅单机 `InGame` 生效，Coop 路径不受影响，改动高度局部化

**目的与动机：**
方便组员快速测试中后期内容（强化 / Boss / 事件），不必每次从头打。

**已知问题 / 后续工作：**
- 这是临时测试设施，正式发布前应按 `docs/test_mode_temp.md` 的指引整体移除。

---

## 2026-05-21 Charger 重做：冲撞机制 + 配置化（#8）

**改动内容：**
- Charger 行为重做：新增碰撞伤害、撞墙眩晕（1.5s）、提升冲刺距离、Windup 预警蓄力条与眩晕特效
- 冲撞期间从通用近战分支拆出，不再生成红色 melee 命中框，避免双重判定
- 新增 `ChargerConfig`（冲刺时长 / 速度倍率 / 撞墙眩晕 / 冷却 / 碰撞伤害倍率 / 击退），数值从硬编码提取到 `enemies.ron`
- 平衡微调：`contact_damage_mult` 1.20→1.00，`contact_knockback` 320→220

**目的与动机：**
让 Charger 从「贴脸近战」变为有预警、有节奏的冲撞型威胁，提升战斗辨识度与公平性；同时把行为参数收口到 RON，便于后续调平衡。

**关键决策：**
- 新增 `ChargerWindupVisual` 纯视觉预警（无伤害），给玩家留出闪避窗口。

---

## 2026-05-25 数值配置化重构：所有玩法数值入 RON + design 校正（#9）

**改动内容（大型重构，多 commit 合并）：**
- **强化（Augment）**：`tuning.rs` 从 ~530 行手写 match 表瘦身到 ~290 行类型化包装；新增统一查询 API `GameDataRegistry::augment_param(id, stacks, key)` / `augment_param_or(..., default)`，`augments.ron` 成为唯一真相源
- **终结技（Skill）**：`SkillConfig` 扩展 `damage_mult / knockback / aoe_radius / duration_s / tick_interval_s / projectile_* / status`，9 种终结技伤害 / AOE / 时长全部入 `skills.ron`（对齐 design §5.4）；WarCry / TimeRift 接入新 `PlayerBuff` 组件，FrostField 真正冻结 AOE 目标
- **敌人 / Boss**：楼层成长曲线、敌人类型曲线、精英概率、每层怪物数、Floor 1 教学缓和移入 `game_balance.ron`；Boss 分层缩放与子核 HP 移入 `boss.ron`
- **精英词缀**：词缀数值入 `elite_affixes.ron`（含 design 校正：Swift / Berserk / Vampiric / Shielded 行为对齐），并接通 Shielded 的 `immune_knockback` / `immune_freeze`
- **商店 / 事件 / 掉落 / 升级**：重复购买涨价、刷新价、药水 / 折扣幅度、事件房 payload、击杀 XP 与掉落物数量、升级属性增量全部入 RON
- **loaders 兜底同步**：`default_registry()` 与现行 RON 对齐，新增 `loaders_fallback_matches_shipped_ron_files` 测试防止再次漂移
- **死代码清理**：移除 `RewardModifiers::bonus_projectile`、`Skill1Cooldown`、`ENERGY_SYSTEM_ENABLED`

**目的与动机：**
此前大量平衡数值散落在 Rust 源码（且 fallback 已与 RON 漂移，如 Floor 1 Boss HP 245 vs RON 330），违背「调数值改 RON」的项目约定。本轮把所有玩法平衡参数收口到 `assets/configs/*.ron`，让设计者无需重编译即可调参，并以测试锁定 fallback 与 RON 一致。

**关键决策：**
- 行为尽量「逐位保持」（bit-for-bit），仅在 commit 标注 `[design-align]` 处按 design.md 做有意校正
- feel / timing 类参数（掉落物磁吸 / 寿命、攻击手感等）按用户决定保留为代码常量，不入 RON
- 顺带修复多个潜伏 bug：升级攻速 / 冲刺 CD 被攻击重置、追踪弹 `turn_factor` 被误当角度、Scatter 吞掉额外弹、已满级强化 / 已装备技能仍进抽卡池

**验证：**
- `cargo test` 全通过（重构后 83 项）

---

## 2026-05-25 追踪弹重定向修复（#10）

**改动内容：**
- 为追踪弹（Homing 强化）新增可配置的 snap 吸附半径
- 选目标时跳过本帧已命中的目标，避免反复锁定同一敌人

**目的与动机：**
修复追踪弹在密集敌群中「原地打转 / 反复命中同一目标」的手感问题，让其能正确转向新目标。

---

## 2026-05-25 Claude PR 审核评论兜底

**改动内容：**
- `.github/workflows/claude-code-review.yml` 显式允许 Claude 使用 PR 评论、PR diff/view 与 inline comment 工具
- Claude 审核步骤成功但本次 run 没有产生顶层或行内审核评论时，workflow 自动补发一条顶层完成评论

**目的与动机：**
PR10 的 Claude Code Review action 成功完成，但 PR 页面没有任何 Claude 评论。该兜底让自动审核的成功状态在 PR 上可见，避免维护者误判为审核未运行。

**关键决策：**
- 保留 `pull_request_target` 的 base checkout + PR head 子目录检出结构，不执行不可信 PR 代码
- 不开启 `track_progress`，只在没有可见审核反馈时补发结果评论，降低 PR 噪音

**验证：**
- `git diff --check` 通过
- `npx --yes js-yaml .github/workflows/claude-code-review.yml` 解析通过

---

## 2026-05-26 成就图标卡片（#11）

**改动内容：**
- 通过 `GameAssets` 加载 9 张成就图标贴图（`assets/textures/achievements/*.png`）
- 成就菜单改为图标卡片渲染，区分已解锁 / 未解锁状态

**目的与动机：**
把纯文字成就列表升级为带图标的卡片，提升成就页的视觉表现与可读性。

---

## 2026-05-30 爆裂射击替换蓄力射击（#12）

**改动内容：**
- 移除远程蓄力射击（charged shot）机制
- 将 `Piercing`（远程）强化重做为「爆炸弹」：远程弹命中后对附近敌人造成范围伤害 + 击退，三级递增爆炸范围 / 伤害（`augments.ron` params: `radius` / `damage` / `knockback`）
- 新增爆炸半径 / 伤害比例 / 击退的配置与爆炸粒子特效

**目的与动机：**
蓄力射击操作收益不直观，替换为命中即爆的范围效果，强化远程 Build 的清群手感与正反馈。

---

## 2026-06-16 专用联机服务器与 client/server 二进制拆分（#14 → #15）

**改动内容：**
- 入口结构调整：删除 `src/main.rs`，改为 `src/lib.rs`（导出 `run_game` / `run_dedicated_server`）+ `src/bin/client.rs` + `src/bin/server.rs` 两个二进制；`Cargo.toml` 设 `default-run = "client"`，`cargo run` 仍等价于启动游戏客户端。
- 无头专用服务器：新增 `DedicatedServerPlugin`，与 `GamePlugin` 共用 `configure_shared_game` 完成状态机/物理初始化。服务器用 `MinimalPlugins`（含 `ScheduleRunnerPlugin` 固定 60Hz）+ 手动挂 `LogPlugin/TransformPlugin/HierarchyPlugin/StatesPlugin`，无渲染/窗口/音频。
- 无头占位插件：`PlaceholderAssetsPlugin`（提供默认 `GameAssets`，使带 `Sprite` 的实体在无头下也能 spawn 而不 panic）、`HeadlessDataPlugin`（`Startup` 加载 RON 配置）、`HeadlessUiSupportPlugin`（补齐 UI 系统依赖的事件类型）。
- 两套网络栈新增 `NetMode::Server`：Coop（Lightyear，UDP 3457）与 PVP（自定义 UDP，3456）。服务器作为权威方但不占用玩家位——Coop 下两个槽位都标 `RemoteControlled`，PVP 下 `pvp_host_simulation_system` 把 P1/P2 都映射到网络输入。
- 配套 `HeadlessCoopPlugin` / `HeadlessPvpPlugin`，仅注册模拟与网络系统，剥离 UI / 本地预测 / 渲染系统。
- 敌人 HP 缩放扩展到 Server 模式（`matches!(mode, Host | Server)`，boss.rs 与 enemy/systems.rs 共 4 处）。
- 部署设施：`deploy/linux/` 下 Linux 启动脚本（`run-server.sh`）、systemd 模板、环境变量样例、打包脚本（PowerShell）；文档 `docs/aliyun_linux_deploy.md`、`docs/client_server_startup.md`。
- 新增窗口控制 UI `src/ui/window_controls.rs`（客户端最小化/最大化/关闭按钮）。

**#15 针对评审反馈的修复：**
- 还原客户端窗口标题为中文「勇闯方块城」（#14 误改为英文）。
- 清除 3 个新增 clippy 告警（`or_else`→`or`、删冗余 `..default()`、`unwrap_or_else`→`unwrap_or`）。
- PVP 收包处新增 `PvpInputMsg::has_finite_vectors()` 校验，丢弃含 `NaN/Inf` 的畸形输入包，避免位置被污染（公网服务器健壮性）。
- 抽出 `PVP_PLAYER_MAX_HP` 常量，消除无头/有界面两处 spawn 的重复硬编码。

**目的与动机：**
此前联机只能由某个玩家充当 Host，要求其常驻在线且暴露公网地址。引入无头专用服务器后，可在云主机长期托管 Coop/PVP 房间，玩家以纯客户端连接，更接近真实联机部署形态。

**关键决策：**
- 不拆成多个独立程序，而是同一 codebase 通过插件组合区分客户端/服务器，最大化复用 `gameplay/` 共享玩法层。
- 无头模式用占位资源而非条件编译，保持玩法系统签名一致、改动面最小。
- PVP 仍是独立简化战斗（移速/伤害/HP 等仍为源码常量），本次未将其接入 augment/skills 或迁移到 RON——属既有设计债，不在本 PR 范围。

**已知问题 / 后续工作：**
- PVP 平衡数值应整体迁移到 RON（`PVP_MOVE_SPEED` 等仍硬编码）。
- `input_timed_out`（`coop/runtime.rs`）在无输入记录时回退判定偏保守，当前被 `bootstrapped` 守卫挡住无实际影响，可后续改为显式 `false`。
- 联机改动需在真实公网环境重测：客户端连 UDP 3456/3457、反复重连、单机/客户端基础启动流程。

**验证：**
- `cargo fmt --check` 通过；`cargo clippy --all-targets` 零告警；`cargo test` 89 项全部通过。
- 实测 `server --coop-server --port 3457` 与 `server --pvp-server --port 3456` 均成功启动、无 panic。
