//! Factual tactical inputs and class action state; numeric character totals remain derived.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TacticalState {
    pub wounded_in_melee: BTreeSet<String>,
    pub opportunity_round: u64,
    pub opportunities_used: u32,
    pub actors: BTreeMap<String, TacticalFacts>,
    pub fighting: Option<FightingChallenge>,
    pub mettle: BTreeMap<String, u64>,
    pub mettle_used: BTreeSet<String>,
    pub daunting_used: BTreeSet<String>,
    pub mettle_reward: bool,
    pub extra_challenge: u32,
    pub channel: Option<ChannelState>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TacticalFacts {
    pub armor_class: i32,
    pub challenge_rating: f64,
    pub language: bool,
    pub hears: bool,
    pub fled: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FightingChallenge {
    pub target: String,
    pub bonus: i32,
    pub expires: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelState {
    pub mode: ChargeMode,
    pub remaining: u32,
    pub attacks_used: u32,
    pub attack_limit: u32,
    pub attacks_closed: bool,

    pub casting: String,
    pub expires: Option<u64>,
    pub targets: BTreeSet<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeMode {
    Touch,
    StandardChannel,
    FullChannel,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TouchRequest {
    pub roll: Option<u32>,
    pub confirmation: Option<u32>,
    pub defense: crate::class_play::DefenseRequest,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeMode {
    Fighting,
    Mettle,
    Daunting,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeleeStrike {
    pub target: String,
    pub roll: u32,
    pub weapon_dice_roll: i64,
    pub confirmation_roll: Option<u32>,
    pub critical_dice_roll: Option<i64>,
    pub spell: crate::class_play::DefenseRequest,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelRequest {
    pub opportunity: bool,
    pub finish_attacks: bool,
    pub cast: crate::magic::CastRequest,
    pub full_attack: bool,
    pub resume: bool,
    pub strikes: Vec<MeleeStrike>,
}
