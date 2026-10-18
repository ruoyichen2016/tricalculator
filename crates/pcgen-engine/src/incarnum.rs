//! Shared, bounded incarnum allocation. Never grant each feature the entire pool.
use crate::{CharacterDraft, Engine};
use pcgen_core::actions::{ActionCost, TurnBudget};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncarnumLoadout {
    pub shaped: Vec<String>,
    /// Soulmeld -> chakra; one soulmeld per chakra without an explicit exception.
    pub binds: BTreeMap<String, String>,
    pub investments: BTreeMap<String, u32>,
}
#[derive(Debug, Default, Serialize)]
pub struct IncarnumSnapshot {
    pub pool: u32,
    pub capacity: u32,
    pub soulmeld_limit: u32,
    pub bind_limit: u32,
    pub meldshaper_level: u32,
    pub allocated: u64,
    pub unallocated: u32,
    pub investment_caps: BTreeMap<String, u32>,
    pub errors: Vec<String>,
    pub effects: IncarnumEffects,
}
#[derive(Debug, Default, Serialize)]
pub struct IncarnumEffects {
    pub skill_enhancement: u32,
    pub miss_chance: u32,
    pub hide_in_plain_sight: bool,
    pub blindsight_feet: u32,
    pub own_turn_reach_feet: u32,
    pub threatened_reach_feet: u32,
    pub soulchilling_strength: u32,
    pub soulchilling_dc: i32,
    pub melee_damage: u32,
    pub natural_damage_morale: u32,
    pub natural_armor_enhancement: u32,
    pub fast_healing: u32,
    pub healing_rounds_per_day: u32,
    pub blink_distance_feet: u32,
    pub blink_move_action: bool,
}
pub(crate) fn class_levels(d: &CharacterDraft) -> BTreeMap<String, u32> {
    d.class_levels()
        .into_iter()
        .map(|c| (c.class_id, c.level))
        .collect()
}
pub(crate) fn has_feat(d: &CharacterDraft, name: &str) -> bool {
    d.abilities
        .iter()
        .any(|(category, n)| category == "FEAT" && n == name)
}
/// The supported soulmeld set is explicit; unknown names never acquire inferred effects.
pub const CASE_SOULMELDS: [&str; 4] = [
    "Blink Shirt",
    "Wormtail Belt",
    "Totem Avatar",
    "Girallon Arms",
];
impl Engine {
    pub fn incarnum_snapshot(&self, d: &CharacterDraft, l: &IncarnumLoadout) -> IncarnumSnapshot {
        let mut s = IncarnumSnapshot::default();
        let levels = class_levels(d);
        let con = d.stats.get("CON").copied().unwrap_or(10);
        if con <= 0 {
            s.errors
                .push("Incarnum requires a positive Constitution score".into());
        }
        let mut chakras = BTreeSet::new();
        let mut totem_bonus = 0;
        s.capacity = match d.total_level() {
            0 => 0,
            1..=5 => 1,
            6..=11 => 2,
            12..=17 => 3,
            _ => 4,
        };
        if d.total_level() > 20 || d.is_gestalt() {
            s.errors
                .push("Incarnum replay supports non-gestalt levels 1-20 only".into());
        }
        for (c, n) in &levels {
            if let Some(i) =
                crate::magic::class_magic(&self.pack, c).and_then(|m| m.incarnum.as_ref())
            {
                if let Some(row) = i.at_level(*n) {
                    s.pool += row.essentia;
                    if row.soulmelds > 0 {
                        s.soulmeld_limit += row.soulmelds.min(con.saturating_sub(10).max(0) as u32);
                        s.bind_limit += row.binds;
                        s.meldshaper_level += n;
                        totem_bonus = row.totem_capacity_bonus;
                        chakras.extend(row.chakras.iter().cloned());
                    }
                    for (name, at) in &i.investments {
                        if n >= at {
                            s.investment_caps.insert(name.clone(), s.capacity);
                        }
                    }
                } else {
                    s.errors.push(format!("{c}: missing incarnum level {n}"));
                }
            }
        }
        let feats = d
            .abilities
            .iter()
            .filter(|(k, n)| k == "FEAT" && n == "Bonus Essentia")
            .count();
        if feats > 0 {
            if con < 13 || d.total_level() < 6 {
                s.errors
                    .push("Bonus Essentia requires CON 13 and character level 6".into());
            } else {
                s.pool += if s.meldshaper_level > 0 { 2 } else { 1 };
            }
        }
        if feats > 1 {
            s.errors
                .push("Bonus Essentia is not repeatable; its benefit is counted once".into());
        }
        let shaped: BTreeSet<String> = l.shaped.iter().cloned().collect();
        if shaped.len() != l.shaped.len() {
            s.errors.push("A soulmeld cannot be shaped twice".into());
        }
        if shaped.len() > s.soulmeld_limit as usize {
            s.errors.push(format!(
                "Shaped soulmelds exceed limit {} (class and CON)",
                s.soulmeld_limit
            ));
        }
        let mut occupied = BTreeSet::new();
        if l.binds.len() > s.bind_limit as usize {
            s.errors
                .push(format!("Chakra binds exceed limit {}", s.bind_limit));
        }
        for (meld, chakra) in &l.binds {
            if !shaped.contains(meld) {
                s.errors
                    .push(format!("{meld}: bind requires a shaped soulmeld"));
            }
            if !chakras.contains(chakra) {
                s.errors.push(format!("{chakra}: chakra not open"));
            }
            if !occupied.insert(chakra) {
                s.errors
                    .push(format!("{chakra}: more than one soulmeld bound"));
            }
            let allowed = match meld.as_str() {
                "Blink Shirt" => ["Totem", "Heart"].as_slice(),
                "Wormtail Belt" => ["Totem", "Waist"].as_slice(),
                "Totem Avatar" => ["Totem", "Arms", "Feet", "Heart", "Shoulders"].as_slice(),
                "Girallon Arms" => ["Totem", "Arms"].as_slice(),
                _ => [].as_slice(),
            };
            if !allowed.contains(&chakra.as_str()) {
                s.errors.push(format!("{meld} cannot bind to {chakra}"));
            }
            // These high-level effects need dedicated resolvers; never silently treat them as Totem.
            if chakra != "Totem" {
                s.errors.push(format!(
                    "{meld} ({chakra}): encounter effect not supported by this replay"
                ));
            }
        }
        for name in &shaped {
            if !CASE_SOULMELDS.contains(&name.as_str()) {
                s.errors.push(format!("Unsupported soulmeld: {name}"));
                continue;
            }
            let cap = s.capacity
                + if l.binds.get(name).is_some_and(|c| c == "Totem") {
                    totem_bonus
                } else {
                    0
                };
            s.investment_caps.insert(name.clone(), cap);
        }
        for (name, e) in &l.investments {
            s.allocated += u64::from(*e);
            match s.investment_caps.get(name) {
                Some(cap) if e <= cap => {}
                Some(cap) => s
                    .errors
                    .push(format!("{name}: investment {e} exceeds capacity {cap}")),
                None => s
                    .errors
                    .push(format!("{name}: investment target not acquired or shaped")),
            }
        }
        if s.allocated > u64::from(s.pool) {
            s.errors.push(format!(
                "Allocated {} essentia exceeds shared pool {}",
                s.allocated, s.pool
            ));
        }
        s.unallocated = u64::from(s.pool).saturating_sub(s.allocated) as u32;
        if !s.errors.is_empty() {
            return s;
        }
        let e = |name: &str| l.investments.get(name).copied().unwrap_or(0);
        let bound = |name: &str| l.binds.get(name).is_some_and(|c| c == "Totem");
        let worm = if shaped.contains("Wormtail Belt") {
            2 + e("Wormtail Belt")
        } else {
            0
        };
        let avatar = if shaped.contains("Totem Avatar") {
            e("Totem Avatar")
        } else {
            0
        };
        s.effects = IncarnumEffects {
            skill_enhancement: 2 * e("Step of the Bodiless"),
            miss_chance: 10 * e("Embrace of Shadow"),
            hide_in_plain_sight: e("Embrace of Shadow") >= 2,
            blindsight_feet: 10 * e("Sight of the Eyeless"),
            own_turn_reach_feet: 5 + 5 * e("Kiss of the Shadows"),
            threatened_reach_feet: 5,
            soulchilling_strength: e("Soulchilling Strike"),
            soulchilling_dc: 10 + e("Soulchilling Strike") as i32 + (con - 10).div_euclid(2),
            melee_damage: e("Incandescent Strike"),
            natural_damage_morale: if bound("Totem Avatar") { avatar } else { 0 },
            natural_armor_enhancement: worm.max(avatar),
            fast_healing: e("Fast Healing"),
            healing_rounds_per_day: levels
                .get("Incandescent Champion")
                .copied()
                .filter(|n| *n >= 2)
                .unwrap_or(0),
            blink_distance_feet: if shaped.contains("Blink Shirt") {
                10 + 10 * e("Blink Shirt")
            } else {
                0
            },
            blink_move_action: bound("Blink Shirt"),
        };
        s
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum IncarnumAction {
    Reallocate {
        investments: BTreeMap<String, u32>,
    },
    Blink {
        feet: u32,
    },
    FullAttack,
    SuddenLeap {
        feet: u32,
        jump_check: i32,
        running_start: bool,
    },
    DancingMongoose,
    PouncingCharge {
        feet: u32,
        path_clear: bool,
    },
    WhiteRavenTactics {
        target_is_self: bool,
        feet: u32,
        actor_initiative: i32,
        ally_initiative: i32,
    },
    FastHealingTick,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncarnumTurn {
    pub actions: Vec<IncarnumAction>,
    pub readied: BTreeSet<String>,
    pub gloves: bool,
    pub dual_wield: bool,
    /// Explicit scenario assumption; some tables disallow a slam with both hands occupied.
    pub slam_available: bool,
    pub target_feet: u32,
    pub denied_dex: bool,
    pub flanking: bool,
    pub precision_immune: bool,
    pub target_concealment: bool,
    pub healing_rounds_spent: u32,
    pub soulchilling_used_this_round: bool,
}
#[derive(Debug, Default, Serialize)]
pub struct IncarnumReplay {
    pub errors: Vec<String>,
    pub log: Vec<String>,
    pub attacks: Vec<IncarnumAttack>,
    pub soulchilling_triggers_max: u32,
    pub healing_rounds_remaining: u32,
    pub healing_this_turn: u32,
    pub ally_initiative: Option<i32>,
    pub final_allocation: BTreeMap<String, u32>,
}
#[derive(Debug, Serialize)]
pub struct IncarnumAttack {
    pub weapon: String,
    /// BAB plus iterative/TWF/secondary penalties; ability, enhancement and target bonuses excluded.
    pub base_bonus: i32,
    pub sneak_dice: u32,
    pub craven_damage: u32,
    pub incarnum_damage: u32,
}
/// Natural attacks never gain iterative attacks from BAB; Second Slam is an explicit exception.
fn attack_routine(
    d: &CharacterDraft,
    bab: i32,
    l: &IncarnumLoadout,
    s: &IncarnumSnapshot,
    p: &IncarnumTurn,
    mongoose: bool,
) -> Result<Vec<IncarnumAttack>, String> {
    let ls = class_levels(d);
    let ranger = ls.get("Ranger").copied().unwrap_or(0);
    let twf = ranger >= 2 || has_feat(d, "Two-Weapon Fighting") || p.gloves;
    let improved = ranger >= 6
        || has_feat(d, "Improved Two-Weapon Fighting")
        || (p.gloves && twf && (ranger >= 2 || has_feat(d, "Two-Weapon Fighting")));
    let greater = ranger >= 11 || has_feat(d, "Greater Two-Weapon Fighting");
    let sneak = (ls.get("Sneak Attack Fighter").copied().unwrap_or(0) + 1) / 2
        + (ls.get("Umbral Disciple").copied().unwrap_or(0) + 1) / 3;
    let precision =
        sneak > 0 && (p.denied_dex || p.flanking) && !p.precision_immune && !p.target_concealment;
    let mut attacks = vec![];
    let mut add = |name: &str, bonus: i32, natural: bool| {
        attacks.push(IncarnumAttack {
            weapon: name.into(),
            base_bonus: bonus,
            sneak_dice: if precision { sneak } else { 0 },
            craven_damage: if precision && has_feat(d, "Craven") {
                d.total_level()
            } else {
                0
            },
            incarnum_damage: s.effects.melee_damage
                + if natural {
                    s.effects.natural_damage_morale
                } else {
                    0
                }
                + if name == "Girallon claw" {
                    l.investments.get("Girallon Arms").copied().unwrap_or(0)
                } else {
                    0
                },
        })
    };
    if l.binds.get("Wormtail Belt").is_some_and(|c| c == "Totem") {
        return Err("Wormtail Belt stinger is the only attack permitted that round; it cannot be appended to this mixed full attack".into());
    }
    // Scenario equipment: two light manufactured weapons. Only ordinary feats/style affect these.
    let penalty = if !p.dual_wield {
        0
    } else if twf {
        -2
    } else {
        -4
    };
    for i in 0..((bab - 1).max(0) / 5 + 1) {
        add("Main hand", bab - 5 * i + penalty, false);
    }
    let off = 1 + u32::from(improved) + u32::from(greater);
    for i in 0..if p.dual_wield { off } else { 0 } {
        add(
            "Off hand",
            bab - 5 * i as i32 + if twf { -2 } else { -8 },
            false,
        );
    }
    if p.slam_available {
        add("Slam", bab - 5, true);
        if has_feat(d, "Second Slam") {
            add("Second Slam", bab - 5, true);
        }
    }
    if has_feat(d, "Jaws of Death") {
        add("Bite", bab - 5, true);
    }
    if p.dual_wield && l.binds.get("Girallon Arms").is_some_and(|c| c == "Totem") {
        // Two physical hands hold the weapons. Two remaining claws are secondary.
        let enhancement = l.investments.get("Girallon Arms").copied().unwrap_or(0) as i32;
        for _ in 0..2 {
            add("Girallon claw", bab - 5 + enhancement, true);
        }
    }
    if mongoose {
        add("Main hand (Dancing Mongoose)", bab + penalty, false);
        if p.dual_wield {
            add(
                "Off hand (Dancing Mongoose)",
                bab + if twf { -2 } else { -8 },
                false,
            );
        }
    }
    Ok(attacks)
}
impl Engine {
    /// One actor's turn only. WRT updates an ally's initiative; it never recursively replays this actor.
    pub fn replay_incarnum_turn(
        &self,
        d: &CharacterDraft,
        loadout: &IncarnumLoadout,
        p: &IncarnumTurn,
    ) -> IncarnumReplay {
        let mut out = IncarnumReplay::default();
        let mut l = loadout.clone();
        let mut s = self.incarnum_snapshot(d, &l);
        out.errors.extend(s.errors.clone());
        let training = self.martial_training(d);
        out.errors.extend(training.errors);
        let known = training.tracks.get("Warblade");
        if !p.readied.is_empty()
            && known.is_none_or(|t| {
                p.readied.len() > t.readied_capacity as usize || !p.readied.is_subset(&t.known)
            })
        {
            out.errors.push(
                "Warblade readied maneuvers must be known and within readied capacity".into(),
            );
        }
        if p.actions.len() > 64 {
            out.errors.push("Replay limit: 64 actions".into());
        }
        if p.healing_rounds_spent > s.effects.healing_rounds_per_day {
            out.errors
                .push("Fast Healing daily rounds already overspent".into());
        }
        out.healing_rounds_remaining = s
            .effects
            .healing_rounds_per_day
            .saturating_sub(p.healing_rounds_spent);
        if !out.errors.is_empty() {
            return out;
        }
        let mut budget = TurnBudget::default();
        let mut ended = false;
        let mut mongoose = false;
        let mut healed = false;
        let mut ready = p.readied.clone();
        let levels = class_levels(d);
        let bab = levels
            .iter()
            .filter_map(|(c, n)| self.pack.progressions.get(c).map(|r| r.bab(*n)))
            .sum();
        for (i, a) in p.actions.iter().enumerate() {
            let action = (|| -> Result<(), String> {
                if ended {
                    return Err("Blink Shirt ends further actions until the next turn".into());
                }
                let maneuver = match a {
                    IncarnumAction::SuddenLeap { .. } => Some("Sudden Leap"),
                    IncarnumAction::DancingMongoose => Some("Dancing Mongoose"),
                    IncarnumAction::PouncingCharge { .. } => Some("Pouncing Charge"),
                    IncarnumAction::WhiteRavenTactics { .. } => Some("White Raven Tactics"),
                    _ => None,
                };
                if let Some(n) = maneuver {
                    if !ready.contains(n) {
                        return Err(format!("{n}: not readied or already expended"));
                    }
                }
                match a {
                    IncarnumAction::Reallocate { investments } => {
                        if !budget.available(ActionCost::Swift) {
                            return Err("No swift action for essentia reallocation".into());
                        }
                        let mut next = l.clone();
                        next.investments = investments.clone();
                        let check = self.incarnum_snapshot(d, &next);
                        if !check.errors.is_empty() {
                            return Err(check.errors.join("; "));
                        }
                        l = next;
                        s = check;
                        budget.spend(ActionCost::Swift)?;
                    }
                    IncarnumAction::Blink { feet } => {
                        if s.effects.blink_distance_feet == 0
                            || *feet > s.effects.blink_distance_feet
                        {
                            return Err(
                                "Blink Shirt absent or teleport exceeds invested range".into()
                            );
                        }
                        if s.effects.blink_move_action {
                            if !budget.available(ActionCost::Move) {
                                return Err("No move action for bound Blink Shirt".into());
                            }
                            budget.spend(ActionCost::Move)?;
                        } else {
                            if !budget.available(ActionCost::Standard) {
                                return Err("No standard action for Blink Shirt".into());
                            }
                            budget.spend(ActionCost::Standard)?;
                        }
                        ended = true;
                    }
                    IncarnumAction::FullAttack | IncarnumAction::PouncingCharge { .. } => {
                        if !budget.available(ActionCost::FullRound) {
                            return Err(
                                "Full attack / Pouncing Charge requires a full-round action".into(),
                            );
                        }
                        if let IncarnumAction::PouncingCharge { feet, path_clear } = a {
                            if !path_clear || *feet < 10 || *feet > 60 {
                                return Err("Charge requires a clear path and 10-60 feet of movement for this speed-30 case".into());
                            }
                        }
                        if p.target_feet > s.effects.own_turn_reach_feet {
                            return Err("Target outside own-turn melee reach".into());
                        }
                        let mut attacks = attack_routine(d, bab, &l, &s, p, mongoose)?;
                        if matches!(a, IncarnumAction::PouncingCharge { .. }) {
                            for hit in &mut attacks {
                                hit.base_bonus += 2;
                            }
                        }
                        out.attacks.extend(attacks);
                        budget.spend(ActionCost::FullRound)?;
                        if !p.soulchilling_used_this_round && s.effects.soulchilling_strength > 0 {
                            out.soulchilling_triggers_max = 1;
                        }
                    }
                    IncarnumAction::SuddenLeap {
                        feet,
                        jump_check,
                        running_start,
                    } => {
                        if !budget.available(ActionCost::Swift) {
                            return Err("No swift action for Sudden Leap".into());
                        }
                        let max = (*jump_check).max(0) as u32 / if *running_start { 1 } else { 2 };
                        if *feet > max {
                            return Err("Sudden Leap distance exceeds supplied Jump check (double DC without running start)".into());
                        }
                        budget.spend(ActionCost::Swift)?;
                        out.log.push("Sudden Leap supplies movement, not an extra action; movement can provoke attacks of opportunity".into());
                    }
                    IncarnumAction::DancingMongoose => {
                        if !budget.available(ActionCost::Swift) {
                            return Err("No swift action for Dancing Mongoose".into());
                        }
                        // Reserve the boost's extra attacks for this turn's attack action.
                        mongoose = true;
                        budget.spend(ActionCost::Swift)?;
                    }
                    IncarnumAction::WhiteRavenTactics {
                        target_is_self,
                        feet,
                        actor_initiative,
                        ally_initiative,
                    } => {
                        if !budget.available(ActionCost::Swift) {
                            return Err("No swift action for White Raven Tactics".into());
                        }
                        if *target_is_self {
                            return Err("Self-target White Raven Tactics needs an explicit ruling; this replay models another ally only".into());
                        }
                        if *feet > 10 {
                            return Err("White Raven Tactics range is 10 feet".into());
                        }
                        let next = actor_initiative
                            .checked_sub(1)
                            .ok_or("Initiative underflow")?;
                        if *ally_initiative != next {
                            out.ally_initiative = Some(next);
                        } else {
                            out.log.push("Ally initiative is already equal to the new count; White Raven Tactics has no effect".into());
                        }
                        budget.spend(ActionCost::Swift)?;
                    }
                    IncarnumAction::FastHealingTick => {
                        if i != 0 {
                            return Err(
                                "Fast Healing ticks at the start of the turn, before other actions"
                                    .into(),
                            );
                        }
                        if healed {
                            return Err(
                                "Fast Healing cannot heal repeatedly within the same turn".into()
                            );
                        }
                        if s.effects.fast_healing == 0 || out.healing_rounds_remaining == 0 {
                            return Err("No invested Fast Healing or daily rounds exhausted".into());
                        }
                        out.healing_rounds_remaining -= 1;
                        // ECS p.23 halves supernatural healing for a Warforged; round down.
                        out.healing_this_turn = if d.race_id == "Warforged" {
                            out.log.push("ECS living construct rule: supernatural Fast Healing is halved, rounded down".into());
                            s.effects.fast_healing / 2
                        } else {
                            s.effects.fast_healing
                        };
                        healed = true;
                    }
                }
                if let Some(n) = maneuver {
                    ready.remove(n);
                }
                Ok(())
            })();
            match action {
                Ok(()) => out.log.push(format!("Action {}: {:?}", i + 1, a)),
                Err(e) => {
                    out.errors.push(format!("Action {}: {e}", i + 1));
                    break;
                }
            }
        }
        out.final_allocation = l.investments;
        out
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncarnumHit {
    pub hit: bool,
    pub actual_damage: u32,
    pub sneak_applies: bool,
    pub soulchilling_natural_roll: u32,
    pub soulchilling_fortitude_total: i32,
    pub staggering_natural_roll: u32,
    pub staggering_fortitude_total: i32,
}
#[derive(Debug, Default, Serialize)]
pub struct IncarnumDebuffs {
    pub soulchilling_saves: u32,
    pub strength_damage: u32,
    pub staggering_saves: u32,
    pub staggered: bool,
}
/// Supplied rolls are outcomes, not assumed automatic failed saves. First hit consumes Soulchilling even on a save.
pub fn resolve_incarnum_debuffs(
    s: &IncarnumSnapshot,
    hits: &[IncarnumHit],
    staggering_strike: bool,
    strength_immune: bool,
    already_used_this_round: bool,
) -> Result<IncarnumDebuffs, String> {
    if !s.errors.is_empty() {
        return Err("Cannot resolve effects from an invalid allocation".into());
    }
    let mut out = IncarnumDebuffs::default();
    for h in hits.iter().filter(|h| h.hit) {
        if !already_used_this_round
            && out.soulchilling_saves == 0
            && s.effects.soulchilling_strength > 0
        {
            if !(1..=20).contains(&h.soulchilling_natural_roll) {
                return Err("Soulchilling save needs a natural d20 roll in 1-20".into());
            }
            out.soulchilling_saves = 1;
            let saved = h.soulchilling_natural_roll == 20
                || (h.soulchilling_natural_roll != 1
                    && h.soulchilling_fortitude_total >= s.effects.soulchilling_dc);
            if !strength_immune && !saved {
                out.strength_damage = s.effects.soulchilling_strength;
            }
        }
        if staggering_strike && h.sneak_applies && h.actual_damage > 0 {
            if !(1..=20).contains(&h.staggering_natural_roll) {
                return Err("Staggering save needs a natural d20 roll in 1-20".into());
            }
            out.staggering_saves += 1;
            let saved = h.staggering_natural_roll == 20
                || (h.staggering_natural_roll != 1
                    && i64::from(h.staggering_fortitude_total) >= i64::from(h.actual_damage));
            if !saved {
                out.staggered = true;
            }
        }
    }
    Ok(out)
}
