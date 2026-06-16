# 构建与运行手册

- 适用版本：`main` 分支（HEAD `16cda9ea`）
- 最后校验：2026-06-16；`cargo check --all-targets` / `cargo clippy --all-targets` 通过，`cargo test` 89 项通过
- 关联源码：`Cargo.toml`、`src/lib.rs`、`src/bin/client.rs`、`src/bin/server.rs`、`src/app.rs`、`src/core/assets.rs`、`src/core/local_debug.rs`、`src/core/save.rs`、`assets/configs/`
- 实验性内容：包含。本页涉及 `Coop` / `PVP` 联调说明，默认按原型环境书写

## 1. 运行环境
当前仓库以 Windows + PowerShell 为主要调试环境，但核心命令都是标准 Cargo 命令，迁移到其他平台时只需要替换终端示例。

最低事实基线：

- Rust Edition：2024
- 构建工具：`cargo`
- 图形框架：`Bevy 0.14`
- 物理：`bevy_rapier2d 0.27`
- 音频接口：`bevy_kira_audio 0.20`
- 联机：`lightyear 0.17.1`（Coop）与手写 UDP（PVP）

## 2. 仓库中的关键运行资产
| 路径 | 作用 |
| --- | --- |
| `assets/configs/player.ron` | 玩家基础属性与冷却参数 |
| `assets/configs/enemies.ron` | 敌人类型的数值模板 |
| `assets/configs/boss.ron` | 分层 Boss 参数 |
| `assets/configs/rewards.ron` | 奖励文本与数值配置 |
| `assets/configs/rooms.ron` | 房间生成基础配置 |
| `assets/configs/game_balance.ron` | 楼层数、房间数、难度等全局平衡参数 |
| `assets/fonts/main_font.ttf` | 主要字体资源 |
| `assets/textures/player_hero.png` | 玩家精灵贴图 |
| `assets/textures/effects/melee_slash_sprites.png` | 近战挥砍特效图集 |
| `saves/` | 存档输出目录 |

`Loading` 状态会等待字体、玩家贴图和近战挥砍贴图完成加载，然后切入 `MainMenu`。

## 3. 基本命令
开发运行（默认启动游戏客户端，等价于 `cargo run --bin client`）：

```bash
cargo run
```

发布运行：

```bash
cargo run --release
```

启动无头专用服务器（自 #14/#15 起，详见 5.4）：

```bash
cargo run --bin server -- --coop-server --port 3457   # Coop 专用服务器
cargo run --bin server -- --pvp-server  --port 3456   # PVP 专用服务器
```

编译检查：

```bash
cargo check               # 默认 target
cargo check --all-targets # 含 client / server 两个二进制
```

测试：

```bash
cargo test
```

当前文档基线下，`cargo test` 通过 89 个单元测试，`cargo clippy --all-targets` 零告警。

> 入口结构：自 #14 起 `src/main.rs` 被拆为 `src/lib.rs`（导出 `run_game` / `run_dedicated_server`）+ `src/bin/client.rs` + `src/bin/server.rs`，`Cargo.toml` 设 `default-run = "client"`。

## 4. 首次启动后会发生什么
1. `src/main.rs` 创建 `App`，设置窗口标题和清屏色。
2. `src/app.rs` 的 `GamePlugin` 注册所有核心插件、玩法插件、联机插件和 UI 插件。
3. 初始状态为 `AppState::Loading`。
4. `AssetsPlugin` 与 `DataPlugin` 分别加载资源与配置。
5. 资源就绪后自动进入 `AppState::MainMenu`。

## 5. 运行模式
### 5.1 单机
默认流程：

`MainMenu -> InGame -> RewardSelect / Shop / Paused / GameOver / Victory`

主菜单点击“单人游戏”时，会插入：

- `FloorNumber(1)`
- `EnemySpawnCount { current: 0 }`

然后进入 `InGame`。

### 5.2 Coop
当前 Coop 是 Lightyear 主机权威架构：

- 端口：UDP `3457`
- Host 会同时启动 server 和本地 client
- Client 只负责上传输入与消费复制结果
- UI、房门、阶段状态等依赖 `CoopSessionState` 和复制体可视化

重要输入约束：

- Coop 加入地址只接受裸 IPv4
- 不接受 `IP:端口`
- 例如：`192.168.1.6` 合法，`192.168.1.6:3457` 非法

### 5.3 PVP
当前 PVP 是独立的手写 UDP 协议：

- 端口：UDP `3456`
- 协议消息：`Hello / Welcome / Input / State / Fire / Result`
- Host 推进权威模拟，Client 做本地预测和状态应用

### 5.4 无头专用服务器（#14/#15）
`server` 二进制以无头模式作为权威方托管房间，自身不占用玩家位，玩家以纯客户端连接：

```bash
cargo run --bin server -- --coop-server --port 3457   # Coop（Lightyear）
cargo run --bin server -- --pvp-server  --port 3456   # PVP（手写 UDP）
```

- 服务器走 `run_dedicated_server`（`MinimalPlugins` + 60Hz `ScheduleRunnerPlugin`），无渲染/窗口/音频，用占位资源插件保证玩法系统正常运行。
- CLI 模式由 `src/core/local_debug.rs` 解析；亦可通过环境变量配置（见 `deploy/linux/server.env.example`）。
- Linux 云部署（含 systemd、打包脚本、安全组放行）见 [`client_server_startup.md`](client_server_startup.md) 与 [`aliyun_linux_deploy.md`](aliyun_linux_deploy.md)。

## 6. 本地联调
仓库当前不使用 `local_debug/*.ps1` 脚本。真实联调入口是 `src/core/local_debug.rs` 的 `LocalDebugPlugin`。

### 6.1 Coop 本机双开
Host 窗口：

```powershell
$env:LOCAL_NET_DEBUG="1"
$env:LOCAL_NET_DEBUG_MODE="coop"
$env:LOCAL_NET_DEBUG_ROLE="host"
cargo run
```

Client 窗口：

```powershell
$env:LOCAL_NET_DEBUG="1"
$env:LOCAL_NET_DEBUG_MODE="coop"
$env:LOCAL_NET_DEBUG_ROLE="client"
$env:LOCAL_NET_DEBUG_HOST="127.0.0.1"
cargo run
```

说明：

- Host 调试窗口默认放在左上
- Client 调试窗口默认放在右上
- Coop Client 的 `LOCAL_NET_DEBUG_HOST` 仍必须是裸 IPv4

### 6.2 PVP 本机双开
Host 窗口：

```powershell
$env:LOCAL_NET_DEBUG="1"
$env:LOCAL_NET_DEBUG_MODE="pvp"
$env:LOCAL_NET_DEBUG_ROLE="host"
cargo run
```

Client 窗口：

```powershell
$env:LOCAL_NET_DEBUG="1"
$env:LOCAL_NET_DEBUG_MODE="pvp"
$env:LOCAL_NET_DEBUG_ROLE="client"
$env:LOCAL_NET_DEBUG_HOST="127.0.0.1"
cargo run
```

### 6.3 可选联调辅助变量
| 变量 | 作用 |
| --- | --- |
| `LOCAL_NET_DEBUG` | 总开关 |
| `LOCAL_NET_DEBUG_MODE` | `coop` 或 `pvp` |
| `LOCAL_NET_DEBUG_ROLE` | `host` 或 `client` |
| `LOCAL_NET_DEBUG_HOST` | 目标 Host 地址 |
| `LOCAL_NET_DEBUG_SAVE_SUFFIX` | 调试存档后缀，自定义存档文件名 |

## 7. 存档与读档
`SavePlugin` 默认使用以下热键：

- `F5`：写入存档
- `F9`：读取存档

默认存档路径：

- 常规：`saves/run_save.ron`
- 调试模式：`saves/run_save_debug_<suffix>.ron`

当前存档数据包括：

- 版本号
- 当前楼层
- 玩家血量、金币、基础属性、奖励修正
- 敌人刷新计数
- 已解锁成就

限制：

- `F9` 在 `PvpMenu`、`PvpLobby`、`PvpGame`、`PvpResult` 中不会执行
- `PendingLoad` 只在 `InGame` 中应用

## 8. 常见运行事实
| 主题 | 当前事实 |
| --- | --- |
| 默认窗口标题 | `勇闯方块城` |
| 默认二进制名 | `block_city_adventure` |
| Coop 端口 | UDP `3457` |
| PVP 端口 | UDP `3456` |
| Coop 地址格式 | 只接受裸 IPv4 |
| 测试数量 | 24 个单元测试 |
| 当前质量状态 | 编译通过，但仍有大量 warnings |

## 9. 建议的交接校验动作
1. 先执行 `cargo check`
2. 再执行 `cargo test`
3. 手动执行一次 `cargo run`，确认能进入主菜单
4. 使用 `LOCAL_NET_DEBUG` 做一轮 Coop 本机双开
5. 使用 `LOCAL_NET_DEBUG` 做一轮 PVP 本机双开
6. 在单机流程里验证 `F5` / `F9`

## 10. 当前已知质量备注
- `cargo check` 仍有大量告警，主要包括未使用代码、待清理旧路径、部分联机和 UI 技术债
- `src/coop/ui.rs` 与 `src/pvp/ui.rs` 仍使用 `ReceivedCharacter`，在 Bevy 0.14 下已属于弃用接口
- 这些问题不影响当前文档基线描述，但维护时应作为后续整治项跟踪
