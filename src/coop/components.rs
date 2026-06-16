#![allow(dead_code)]

use bevy::prelude::*;
use lightyear::prelude::ClientId;
use serde::{Deserialize, Serialize};

use crate::gameplay::map::room::{Direction, RoomType};
use crate::gameplay::player::components::AnimationState;
use crate::gameplay::rewards::data::RewardType;
use crate::states::RoomState;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CoopParticipant;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlayerSlot {
    #[default]
    P1,
    P2,
}

impl PlayerSlot {
    #[allow(dead_code)]
    pub const ALL: [Self; 2] = [Self::P1, Self::P2];

    pub fn index(self) -> usize {
        match self {
            Self::P1 => 0,
            Self::P2 => 1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::P1 => "1P",
            Self::P2 => "2P",
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LocalControlled;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteControlled {
    pub client_id: ClientId,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CoopNetPosition(pub Vec2);

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CoopNetVelocity(pub Vec2);

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CoopNetRotation(pub f32);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GhostState {
    #[default]
    Alive,
    Ghost,
}

impl GhostState {
    #[allow(dead_code)]
    pub fn can_interact(self) -> bool {
        matches!(self, Self::Alive)
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CoopMeleeFlashState {
    pub sequence: u16,
    pub slash_angle_rad: f32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CoopDashVisualState {
    pub active: bool,
    pub dir: Vec2,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct BufferedCoopInput(pub CoopInputState);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CoopInputState {
    pub move_axis: Vec2,
    pub aim_world: Option<Vec2>,
    pub attack_pressed: bool,
    pub attack_held: bool,
    pub ranged_pressed: bool,
    pub ranged_held: bool,
    pub dash_pressed: bool,
    pub interact_pressed: bool,
    pub pause_pressed: bool,
    pub shop_pressed: bool,
    pub menu_confirm_pressed: bool,
    pub menu_cancel_pressed: bool,
}

impl CoopInputState {
    /// 合并另一帧输入：持续量取最新值，边缘事件用 OR 累积。
    /// 用于 capture_server_inputs 当多个 tick 在同一 FixedUpdate 内到达时，
    /// 防止后面 tick 的 false 覆盖前面 tick 的 true（冲刺/E键丢失问题）。
    pub fn merge_incoming(&mut self, newer: &CoopInputState) {
        self.move_axis = newer.move_axis;
        self.aim_world = newer.aim_world;
        self.attack_held = newer.attack_held;
        self.ranged_held = newer.ranged_held;
        self.attack_pressed |= newer.attack_pressed;
        self.ranged_pressed |= newer.ranged_pressed;
        self.dash_pressed |= newer.dash_pressed;
        self.interact_pressed |= newer.interact_pressed;
        self.pause_pressed |= newer.pause_pressed;
        self.shop_pressed |= newer.shop_pressed;
        self.menu_confirm_pressed |= newer.menu_confirm_pressed;
        self.menu_cancel_pressed |= newer.menu_cancel_pressed;
    }

    /// 清除所有边缘事件（一次性按键）。在 host 消费后调用，防止跨帧重复触发。
    pub fn clear_edge_events(&mut self) {
        self.attack_pressed = false;
        self.ranged_pressed = false;
        self.dash_pressed = false;
        self.interact_pressed = false;
        self.pause_pressed = false;
        self.shop_pressed = false;
        self.menu_confirm_pressed = false;
        self.menu_cancel_pressed = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CoopPhase {
    #[default]
    None,
    Paused,
    Reward,
    DoorChoice,
    Rps,
    Shop,
    MatchOver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CoopRewardMode {
    #[default]
    None,
    SingleBuff,
    HealOrBuff,
    DualBuff,
    LoneSurvivor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoopRewardSelectionGroup {
    Heal,
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoopRewardOption {
    Buff(RewardType),
    Rest,
    Revive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PlayerRewardState {
    pub slot: PlayerSlot,
    pub can_interact: bool,
    pub mode: CoopRewardMode,
    pub primary_options: Vec<CoopRewardOption>,
    pub secondary_options: Vec<CoopRewardOption>,
    pub selected_primary: Option<CoopRewardOption>,
    pub selected_secondary: Option<CoopRewardOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RewardChoiceState {
    pub lone_survivor: Option<PlayerSlot>,
    pub players: [PlayerRewardState; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CoopDoorOption {
    pub index: u8,
    pub dir: Direction,
    pub room_type: RoomType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DoorChoiceState {
    pub chooser: Option<PlayerSlot>,
    pub options: Vec<CoopDoorOption>,
    pub p1_choice: Option<u8>,
    pub p2_choice: Option<u8>,
    pub input_timeout_s: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoopRpsChoice {
    Rock,
    Paper,
    Scissors,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CoopRpsState {
    pub p1_choice: Option<CoopRpsChoice>,
    pub p2_choice: Option<CoopRpsChoice>,
    pub winner: Option<PlayerSlot>,
    pub winning_door: Option<u8>,
    pub reveal_timer_s: f32,
    pub input_timeout_s: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReviveChoiceState {
    pub dead_slot: Option<PlayerSlot>,
    pub revived: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CoopShopItem {
    #[default]
    Heal,
    IncreaseMaxHealth,
    IncreaseAttackPower,
    ReduceDashCooldown,
    IncreaseMoveSpeed,
    IncreaseEnergyMax,
    IncreaseCritChance,
    IncreaseAttackSpeed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CoopShopOffer {
    pub item: CoopShopItem,
    pub title: String,
    pub description: String,
    pub cost: u32,
    pub purchased: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PlayerShopState {
    pub slot: PlayerSlot,
    pub can_interact: bool,
    pub refresh_count: u32,
    pub offers: Vec<CoopShopOffer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CoopShopState {
    pub players: [PlayerShopState; 2],
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CoopSessionState {
    pub phase: CoopPhase,
    pub room_state: RoomState,
    pub room_type: RoomType,
    pub current_room: u32,
    pub floor_number: u32,
    pub reward: RewardChoiceState,
    pub door_choice: DoorChoiceState,
    pub revive: ReviveChoiceState,
    pub rps: CoopRpsState,
    pub shop: CoopShopState,
    pub match_victory: bool,
    pub match_over: bool,
    /// Host 端每帧同步：0.0=就绪，>0.0=冷却中（剩余比例）
    pub p1_dash_cooldown_frac: f32,
    pub p2_dash_cooldown_frac: f32,
}

/// 由 host 广播给所有 client 的伤害事件，用于 client 端显示伤害数字。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CoopDamageEvent {
    pub amount: f32,
    pub is_crit: bool,
    pub pos: Vec2,
    pub attacker_is_player: bool,
}

/// Client 端本地动画预测：按键后立即切换动画，不等待 host 确认。
/// override_timer_s > 0 时优先使用 predicted_anim，超时后回归 host 状态。
#[derive(Component, Debug, Clone, Copy)]
pub struct LocalAnimPrediction {
    pub predicted_anim: AnimationState,
    pub override_timer_s: f32,
}

impl Default for LocalAnimPrediction {
    fn default() -> Self {
        Self {
            predicted_anim: AnimationState::Idle,
            override_timer_s: 0.0,
        }
    }
}

#[derive(Component)]
pub struct CoopSessionEntity;

#[derive(Component)]
pub struct CoopHudRoot;

#[derive(Component)]
pub struct CoopOverlayRoot;

#[derive(Component)]
pub struct CoopRemoteHealthBarRoot;

#[derive(Component)]
pub struct CoopRemoteHealthBarFill;

#[derive(Component)]
pub struct CoopVisualReady;
