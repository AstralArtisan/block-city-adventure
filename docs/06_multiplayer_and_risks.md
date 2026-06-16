# 多人架构与风险说明

最后更新：2026-06-16

本页描述当前分支下多人联机的真实实现边界，重点覆盖 Coop。旧审计文档可以作为历史背景，但不应再作为当前修复计划的唯一依据。

## 当前结论

- `Coop` 使用 Lightyear + Host Authority。
- Host 负责权威模拟；Client 负责输入上传、复制体渲染和阶段 UI。
- Host 进程中 authority 实体与 replicated 展示体并存是架构设计，不是缺陷本身。
- 真正的高风险点不是“有双份实体”，而是某些玩法查询把 `Replicated` 误算进权威逻辑。
- `CoopSessionState` 是客户端阶段 UI 的主真相；客户端不要求持有完整的 Host-only 资源镜像。

## Coop 真实边界

### 权威侧

- `src/coop/runtime.rs`
  - 会话推进
  - 阶段切换
  - 楼层初始化
  - 奖励/选门/RPS/商店处理
- `src/gameplay/*`
  - 玩家、敌人、战斗、地图等核心玩法只在 `InGame` 或 `CoopGame + is_coop_authority` 下运行

### 客户端侧

- `src/coop/net.rs`
  - Lightyear 连接生命周期
  - Lobby -> `CoopGame` 自动进入 gating
- `src/coop/ui.rs`
  - 复制体可视化
  - Coop overlay / modal
  - 远端玩家视觉效果
- `src/ui/hud.rs`
  - 在 Coop 下优先读取 replicated `CoopSessionState`

## 当前已固化的规则

### 1. 权威查询必须排除 `Replicated`

以下类型的系统必须只读 authority 实体：

- 战斗碰撞与命中判定
- 伤害结算与击退衰减
- 敌人/投射物清理与房间清空计数
- 任何影响 phase 推进的实体计数

本轮修复已把 combat 和 room cleanup 相关查询显式收紧到 `Without<Replicated>`，避免未来协议或注册项扩展时再次出现 Host 污染。

### 2. Client 进入 `CoopGame` 必须等待最小世界就绪

当前最小条件：

- 已收到 replicated `CoopSessionState`
- 已确认本地控制玩家复制体
- 已收到至少一个 replicated `Door`

只有 `connected` 不足以进入 `CoopGame`。

### 3. Coop 下继续使用房间类型归一化

- `Puzzle` 在 Coop 下归一化为 `Normal`
- 该策略在 runtime 入口和楼层重建路径上继续生效
- 当前修复方向不是改地图生成器禁掉 `Puzzle`，而是保证 Coop 读取到的一致语义始终是 `Normal`

## 已知剩余风险

- Host authority / replicated 并存仍然提升了查询编写成本，新系统必须主动决定是否排除 `Replicated`
- 客户端可视化仍依赖本地补充的 `Transform` / `Sprite` 等显示组件，渲染问题需要同时检查复制数据和本地消费逻辑
- 工作树中仍存在较多历史 warning，与本轮 Coop 修复无直接关系，但会影响静态检查噪音

## 推荐排查顺序

1. 先判断问题是否只出现在 `CoopGame`
2. 再区分是连接生命周期、authority 逻辑，还是复制体渲染
3. 如果只有 Host 异常，先查是否有系统遗漏了 `Without<Replicated>`
4. 如果只有 Client 异常，先查 `CoopSessionState` 是否到达，再查 `src/coop/ui.rs`

## 本轮验证基线

- `cargo test --quiet`
- `cargo check --quiet`
- `cargo clippy --quiet --tests -- -D warnings`
  - 当前环境未安装 Clippy，未能执行

## 4. RPS 输入等待的超时兜底

- `CoopPhase::Rps` 下，若双方没有在 12 秒内同时完成出拳，Host 会为未出拳方自动补全随机出拳，避免会话永久卡死在猜拳阶段。
- RPS 弹窗会显示剩余等待时间，倒计时归零后自动进入补拳并继续按原有结算流程推进。
- 若本轮结果为平局，双方出拳与输入倒计时都会一起重置，重新开始下一轮等待。

## 5. Client 输入粘滞与实体累积修复（2026-04-02）

### 已修复风险

1. **P2 输入粘滞**：`host_buffer_player_inputs`（Update 帧率）从 `net.latest_inputs` 读取 P2 的 `move_axis`，当 client 包丢失或延迟时旧的非零值无限持续。修复：新增帧计数器 `host_frame_counter`，超过 3 帧未收到新输入时自动清零持续量（`move_axis`、`held` 字段）。

2. **Replicated Player 实体累积**：`filter_replicated_player_duplicates` 只隐藏重复实体不 despawn，网络波动时隐藏实体不断累积，`sync_replicated_visuals` 每帧对所有实体做 lerp 计算导致性能线性退化直至卡死。修复：改为 despawn 非最佳实体。

3. **EventReader 未 drain**：`capture_server_inputs` 和 `receive_coop_command_messages` 在非对应角色时 early return 不 drain EventReader。修复：early return 前调用 `.clear()`。

4. **tick/replication rate 不对齐**：`server_replication_send_interval`（1/64s）与 `tick`（1/60s）不对齐，可能导致 Lightyear 内部缓冲区漂移。修复：统一为 1/60s。

### 新增的架构约束

- `CoopNetState.latest_inputs` 中的持续量（`move_axis`、`held`）不再被视为"永远有效"，消费侧必须检查输入新鲜度
- 重复的 Replicated Player 实体必须被 despawn 而非隐藏，避免 ECS 查询性能退化

## 6. 无头专用服务器（2026-06-16，#14/#15）

### 角色模型

`NetMode` 新增 `Server` 变体（原有 `None/Host/Client`）。专用服务器是**权威方但不占用玩家位**：

- **Coop**：`host_bootstrap_match` 在 Server 模式下两个玩家槽位都标 `RemoteControlled`（分别对应 `host_client_id` / `remote_client_id`），等两个客户端都连上才 bootstrap。`is_coop_authority` 对 `Host | Server` 同时为真。
- **PVP**：`pvp_host_simulation_system` 在 Server 模式下把 P1/P2 都映射到 `net.input_for_player(1/2)`，不读本地 `PlayerInputState`。

### 无头运行约束

- 服务器入口 `run_dedicated_server`（`src/lib.rs`）用 `MinimalPlugins` + `ScheduleRunnerPlugin`（固定 60Hz），不挂渲染/窗口/音频。
- `PlaceholderAssetsPlugin` 提供默认 `GameAssets`，保证带 `Sprite` 的实体在无头下也能 spawn 而不 panic；`HeadlessDataPlugin` 在 `Startup` 加载 RON；`HeadlessUiSupportPlugin` 补齐 UI 系统依赖的事件类型。
- `HeadlessCoopPlugin` / `HeadlessPvpPlugin` 只注册模拟与网络系统，剥离 UI、本地预测、可视化系统。
- 专用服务器初始 `AppState` 为 `MainMenu`（而非客户端的 `Loading`）。

### 安全约束

- PVP 收包处用 `PvpInputMsg::has_finite_vectors()` 丢弃含 `NaN/Inf` 的畸形输入包，防止位置被污染。
- 云部署需在安全组/防火墙放行 UDP 3456（PVP）/ 3457（Coop）。

### 剩余风险

- PVP 平衡数值（`PVP_MOVE_SPEED` / `PVP_MELEE_DAMAGE` / `PVP_PLAYER_MAX_HP` 等）仍为源码常量，未迁移 RON。
- `input_timed_out`（`coop/runtime.rs`）在无输入记录时回退判定偏保守，当前被 `bootstrapped` 守卫挡住无实际影响。
- 联机全链路（公网连接、反复重连）需在真实环境重测。

部署操作手册见 [`client_server_startup.md`](client_server_startup.md) 与 [`aliyun_linux_deploy.md`](aliyun_linux_deploy.md)。
