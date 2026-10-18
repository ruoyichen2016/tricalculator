//! Martial advancement and bounded action replay. No final attack count is accepted as input.
use crate::{CharacterDraft, Engine};
use pcgen_core::actions::{ActionCost, TurnBudget};
use pcgen_core::martial::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default, Serialize)]
pub struct TrainingAudit {
    pub tracks: BTreeMap<String, Track>,
    pub errors: Vec<String>,
}
#[derive(Debug, Default, Serialize)]
pub struct Track {
    pub initiator_level: u32,
    pub known_capacity: u32,
    pub readied_capacity: u32,
    pub stance_capacity: u32,
    pub granted: u32,
    pub known: BTreeSet<String>,
    pub stances: BTreeSet<String>,
}
fn levels(d: &CharacterDraft, through: u32) -> BTreeMap<String, u32> {
    let mut result = BTreeMap::new();
    for frame in d
        .advancement
        .iter()
        .filter(|f| f.character_level <= through)
    {
        for lane in &frame.lanes {
            result.insert(lane.class_id.clone(), lane.class_level);
        }
    }
    result
}
impl Engine {
    fn martial_class(&self, name: &str) -> Option<&MartialClass> {
        crate::magic::class_magic(&self.pack, name)?
            .martial
            .as_ref()
    }
    pub fn initiator_level(
        &self,
        d: &CharacterDraft,
        track: &str,
        through: u32,
    ) -> Result<u32, String> {
        let ls = levels(d, through);
        let own = *ls
            .get(track)
            .ok_or("Initiator track has not been acquired")?;
        if self.martial_class(track).is_none_or(|m| m.prestige) {
            return Err("Select a base martial class".into());
        }
        let prestige: u32 = ls
            .iter()
            .filter(|(c, _)| self.martial_class(c).is_some_and(|m| m.prestige))
            .map(|(_, n)| n)
            .sum();
        let total: u32 = ls.values().sum();
        if d.is_gestalt() {
            return Err("Martial initiator levels require an explicit gestalt ruling".into());
        }
        Ok(own + prestige + (total - own - prestige) / 2)
    }
    pub fn martial_training(&self, d: &CharacterDraft) -> TrainingAudit {
        let mut out = TrainingAudit::default();
        let ls = levels(d, d.total_level());
        for (c, n) in &ls {
            if let Some(m) = self.martial_class(c).filter(|m| !m.prestige) {
                if let Some(row) = m.at_level(*n) {
                    out.tracks.insert(
                        c.clone(),
                        Track {
                            initiator_level: self
                                .initiator_level(d, c, d.total_level())
                                .unwrap_or(0),
                            known_capacity: row.known,
                            readied_capacity: row.readied,
                            stance_capacity: row.stances,
                            granted: row.granted,
                            ..Default::default()
                        },
                    );
                } else {
                    out.errors
                        .push(format!("{c}: class level {n} has no progression"));
                }
            }
        }
        if d.is_gestalt() && !out.tracks.is_empty() {
            out.errors
                .push("Martial gestalt advancement needs an explicit ruling".into());
        }
        let mut taken: BTreeMap<(u32, String, bool), u32> = BTreeMap::new();
        let mut swaps = BTreeSet::new();
        let mut rows = d
            .martial_training
            .acquisitions
            .iter()
            .enumerate()
            .collect::<Vec<_>>();
        rows.sort_by_key(|(i, a)| (a.character_level, *i));
        for (_, a) in rows {
            let result = (|| -> Result<(), String> {
                let frame = d
                    .advancement
                    .iter()
                    .find(|f| f.character_level == a.character_level)
                    .ok_or("No acquisition frame")?;
                let lane = frame
                    .lanes
                    .iter()
                    .find(|l| l.class_id == a.source_class)
                    .ok_or("Acquisition source did not advance at this level")?;
                let rule = self
                    .martial_class(&a.source_class)
                    .ok_or("Source cannot grant maneuvers")?;
                let row = rule
                    .at_level(lane.class_level)
                    .ok_or("Missing martial class row")?;
                let m = self
                    .pack
                    .maneuvers
                    .get(&a.maneuver)
                    .ok_or("Unknown maneuver")?;
                if !rule.disciplines.contains(&m.discipline) {
                    return Err("Discipline not offered by acquisition source".into());
                }
                if !rule.prestige && a.track != a.source_class {
                    return Err("Base class grants cannot be assigned to another class".into());
                }
                let il = self.initiator_level(d, &a.track, a.character_level)?;
                if m.level > (il + 1) / 2 {
                    return Err(format!(
                        "Maneuver level {} needs IL {}; current IL {il}",
                        m.level,
                        2 * m.level - 1
                    ));
                }
                if !rule.prestige && lane.class_level == 1 && m.stance && m.level != 1 {
                    return Err("First base-class stance must be level 1".into());
                }
                let count = out
                    .tracks
                    .values()
                    .flat_map(|t| t.known.iter().chain(&t.stances))
                    .collect::<BTreeSet<_>>()
                    .iter()
                    .filter(|n| {
                        self.pack
                            .maneuvers
                            .get(**n)
                            .is_some_and(|p| p.discipline == m.discipline)
                    })
                    .count();
                if count < m.prerequisite_count as usize {
                    return Err(format!(
                        "Requires {} prior {} maneuvers/stances",
                        m.prerequisite_count, m.discipline
                    ));
                }
                let t = out
                    .tracks
                    .get_mut(&a.track)
                    .ok_or("Unknown receiving track")?;
                if t.known.contains(&a.maneuver) || t.stances.contains(&a.maneuver) {
                    return Err("Duplicate acquisition in this track".into());
                }
                if let Some(old) = &a.replaces {
                    if rule.prestige
                        || m.stance
                        || lane.class_level < 4
                        || lane.class_level % 2 != 0
                        || !t.known.contains(old)
                        || !swaps.insert((a.character_level, a.track.clone()))
                    {
                        return Err("Illegal maneuver exchange".into());
                    }
                    t.known.remove(old);
                } else {
                    let prior = rule.at_level(lane.class_level - 1);
                    let award = if m.stance { row.stances } else { row.known }
                        - if rule.prestige {
                            0
                        } else {
                            prior.map_or(0, |p| if m.stance { p.stances } else { p.known })
                        };
                    let used = taken
                        .entry((a.character_level, a.source_class.clone(), m.stance))
                        .or_default();
                    if *used >= award {
                        return Err("No remaining acquisition award at this level".into());
                    }
                    *used += 1;
                    if rule.prestige {
                        if m.stance {
                            t.stance_capacity += 1;
                        } else {
                            t.known_capacity += 1;
                        }
                    }
                }
                if m.stance {
                    t.stances.insert(a.maneuver.clone());
                } else {
                    t.known.insert(a.maneuver.clone());
                }
                Ok(())
            })();
            if let Err(e) = result {
                out.errors
                    .push(format!("Level {} / {}: {e}", a.character_level, a.maneuver));
            }
        }
        for (level, track) in &d.martial_training.readied_advances {
            let valid = d
                .advancement
                .iter()
                .find(|f| f.character_level == *level)
                .is_some_and(|f| {
                    f.lanes.iter().any(|l| {
                        self.martial_class(&l.class_id)
                            .filter(|m| m.prestige)
                            .and_then(|m| m.at_level(l.class_level))
                            .is_some_and(|r| r.readied == 1)
                    })
                });
            if valid {
                if let Some(t) = out.tracks.get_mut(track) {
                    t.readied_capacity += 1;
                    if track == "Crusader" {
                        t.granted += 1;
                    }
                    continue;
                }
            }
            out.errors.push(format!(
                "Level {level}: invalid additional readied maneuver assignment"
            ));
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum MartialAction {
    Impetus,
    Recovery { track: String, maneuver: String },
    Initiate { track: String, maneuver: String },
    Belt { charges: u32 },
    FullAttack,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnPlan {
    pub readied: BTreeMap<String, BTreeSet<String>>,
    pub granted: BTreeSet<String>,
    pub stance: String,
    pub actions: Vec<MartialAction>,
    pub nightstick: bool,
    pub haste: bool,
    pub denied_dex: bool,
    pub flanking: bool,
    pub precision_immune: bool,
    pub fire_resistance: u32,
    pub fire_immune: bool,
    pub deadly_precision_mic: bool,
    pub weapon_enhancement: u32,
    pub teleport_destination_clear: bool,
    pub teleport_distance_feet: u32,
    pub destination_visible: bool,
    pub destination_line_of_effect: bool,
    pub target_visible_before: bool,
    pub target_in_reach_after: bool,
    /// Empty means the ordinary supernatural standard-action rule.
    pub free_impetus_ruling: Option<String>,
}
#[derive(Debug, Default, Serialize)]
pub struct TurnReplay {
    pub errors: Vec<String>,
    pub log: Vec<String>,
    pub turning_remaining: u32,
    pub full_attacks: u32,
    pub main_attacks: u32,
    pub off_attacks: u32,
    pub conditional: bool,
    pub main_attack_bonuses: Vec<i32>,
    pub off_attack_bonuses: Vec<i32>,
    pub hit_damage_min: u64,
    pub hit_damage_max: u64,
    pub all_hit_average: Option<f64>,
    pub damage_notes: Vec<String>,
}
impl Engine {
    pub fn replay_martial_turn(&self, d: &CharacterDraft, p: &TurnPlan) -> TurnReplay {
        let audit = self.martial_training(d);
        let mut out = TurnReplay {
            errors: audit.errors,
            ..Default::default()
        };
        let ls = levels(d, d.total_level());
        let has = |name: &str| {
            d.abilities
                .iter()
                .any(|(_, n)| n.eq_ignore_ascii_case(name))
        };
        let cha = (d.stats.get("CHA").copied().unwrap_or(10) - 10).div_euclid(2);
        let extra = d
            .abilities
            .iter()
            .filter(|(_, n)| n.eq_ignore_ascii_case("Extra Turning"))
            .count() as i32;
        let domain = u32::from(d.domains.iter().any(|n| n.eq_ignore_ascii_case("Undeath")));
        let total = if ls.get("Cleric").copied().unwrap_or(0) > 0 {
            (3 + cha + 4 * extra).max(0) as u32 + 4 * domain + if p.nightstick { 4 } else { 0 }
        } else {
            0
        };
        out.turning_remaining = total.saturating_sub(d.class_play.resources.turning_spent);
        if d.class_play.resources.turning_spent > total {
            out.errors
                .push("Spent turning attempts exceed daily capacity".into());
        }
        for (track, names) in &p.readied {
            match audit.tracks.get(track) {
                Some(t)
                    if names.len() <= t.readied_capacity as usize && names.is_subset(&t.known) => {}
                _ => out
                    .errors
                    .push(format!("{track}: invalid readied selection")),
            }
        }
        if let Some(t) = audit.tracks.get("Crusader") {
            if p.granted.len() != t.granted as usize
                || !p
                    .granted
                    .is_subset(p.readied.get("Crusader").unwrap_or(&BTreeSet::new()))
            {
                out.errors.push(
                    "Crusader requires a supplied random granted subset of the correct size".into(),
                );
            }
        } else if !p.granted.is_empty() {
            out.errors.push("No Crusader track grants maneuvers".into());
        }
        if !audit.tracks.values().any(|t| t.stances.contains(&p.stance)) {
            out.errors.push("Starting stance is not known".into());
        }
        out.conditional = p
            .free_impetus_ruling
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty());
        if p.free_impetus_ruling
            .as_ref()
            .is_some_and(|s| s.trim().is_empty())
        {
            out.errors
                .push("A free-action Impetus ruling needs a reason".into());
        }
        if p.weapon_enhancement > 5 {
            out.errors
                .push("Non-epic weapon enhancement must be at most +5".into());
        }
        if p.actions.len() > 256 {
            out.errors.push("Turn plan exceeds 256 actions".into());
        }
        if !out.errors.is_empty() {
            return out;
        }
        let bab: i32 = ls
            .iter()
            .map(|(c, n)| self.pack.progressions.get(c).map_or(0, |p| p.bab(*n)))
            .sum();
        let main = if bab > 0 {
            ((bab - 1) / 5 + 1).min(4) as u32
        } else {
            1
        };
        let off = if has("Greater Two-Weapon Fighting") && bab >= 11 {
            3
        } else if has("Improved Two-Weapon Fighting") && bab >= 6 {
            2
        } else if has("Two-Weapon Fighting") {
            1
        } else {
            0
        };
        let dex = (d.stats.get("DEX").copied().unwrap_or(10) - 10).div_euclid(2);
        let strength = (d.stats.get("STR").copied().unwrap_or(10) - 10).div_euclid(2);
        let shadow_stance = self
            .pack
            .maneuvers
            .get(&p.stance)
            .is_some_and(|s| s.discipline == "Shadow Hand");
        let discipline = if shadow_stance {
            3
        } else if audit.tracks.values().any(|t| {
            t.known
                .iter()
                .any(|n| self.pack.maneuvers[n].discipline == "Shadow Hand")
        }) {
            1
        } else {
            0
        };
        let attack = bab
            + if has("Weapon Finesse") { dex } else { strength }
            + p.weapon_enhancement as i32
            + discipline
            + i32::from(has("Weapon Focus (Short Sword)"))
            + i32::from(p.haste)
            - if off > 0 { 2 } else { 0 };
        out.main_attack_bonuses = (0..main).map(|i| attack - 5 * i as i32).collect();
        if p.haste {
            out.main_attack_bonuses.insert(0, attack);
        }
        out.off_attack_bonuses = (0..off).map(|i| attack - 5 * i as i32).collect();
        let mut budget = TurnBudget::default();
        let mut belt = 3u32;
        let mut inferno_level = 0u32;
        let mut expected = 0.0;
        let mut expended = BTreeSet::new();
        let mut granted = p.granted.clone();
        let rkv = ls.get("Ruby Knight Vindicator").copied().unwrap_or(0);
        for (i, a) in p.actions.iter().enumerate() {
            let result = (|| -> Result<u32, String> {
                match a {
                    MartialAction::Impetus => {
                        if rkv < 7 {
                            return Err("Divine Impetus requires RKV 7".into());
                        }
                        if out.turning_remaining == 0 {
                            return Err("No turning attempts remain".into());
                        }
                        if !out.conditional {
                            if !budget.available(ActionCost::Standard) {
                                return Err("Divine Impetus requires a standard action; free activation is a house ruling".into());
                            }
                            budget.spend(ActionCost::Standard)?;
                        }
                        out.turning_remaining -= 1;
                        budget.grant(ActionCost::Swift)?;
                        Ok(0)
                    }
                    MartialAction::Recovery { track, maneuver } => {
                        if rkv < 2 {
                            return Err("Divine Recovery requires RKV 2".into());
                        }
                        if !budget.available(ActionCost::Swift) || out.turning_remaining == 0 {
                            return Err(
                                "Recovery needs one swift action and one turning attempt".into()
                            );
                        }
                        if !expended.remove(&(track.clone(), maneuver.clone())) {
                            return Err("Maneuver is not expended in this track".into());
                        }
                        budget.spend(ActionCost::Swift)?;
                        out.turning_remaining -= 1;
                        if track == "Crusader" {
                            granted.insert(maneuver.clone());
                        }
                        Ok(0)
                    }
                    MartialAction::Belt { charges } => {
                        if !(1..=3).contains(charges)
                            || *charges > belt
                            || !budget.available(ActionCost::Swift)
                        {
                            return Err(
                                "Belt requires a swift action and sufficient charges (1-3)".into(),
                            );
                        }
                        // This case spends the immediate full-round award next, not later in the round.
                        if *charges == 3
                            && !matches!(p.actions.get(i + 1), Some(MartialAction::FullAttack))
                        {
                            return Err("Belt full-round award must be used immediately".into());
                        }
                        budget.spend(ActionCost::Swift)?;
                        belt -= charges;
                        match charges {
                            1 => budget.grant(ActionCost::Move)?,
                            2 => budget.grant(ActionCost::Standard)?,
                            _ => budget.grant(ActionCost::FullRound)?,
                        };
                        Ok(0)
                    }
                    MartialAction::FullAttack => {
                        budget.spend(ActionCost::FullRound)?;
                        Ok(1)
                    }
                    MartialAction::Initiate { track, maneuver } => {
                        let m = self
                            .pack
                            .maneuvers
                            .get(maneuver)
                            .ok_or("Unknown maneuver")?;
                        if !p.readied.get(track).is_some_and(|s| s.contains(maneuver))
                            || expended.contains(&(track.clone(), maneuver.clone()))
                        {
                            return Err("Maneuver is not ready or is already expended".into());
                        }
                        if track == "Crusader" && !granted.contains(maneuver) {
                            return Err("Crusader maneuver is withheld".into());
                        }
                        let action = if m.action.contains("swift") {
                            ActionCost::Swift
                        } else if m.action.contains("standard") {
                            ActionCost::Standard
                        } else if m.action.contains("move") {
                            ActionCost::Move
                        } else {
                            return Err(
                                "This maneuver action is not supported by the turn replay".into()
                            );
                        };
                        if !budget.available(action) {
                            return Err(format!("No {} available", m.action));
                        }
                        if m.teleportation
                            && !["Shadow Jaunt", "Shadow Stride", "Shadow Blink"]
                                .contains(&maneuver.as_str())
                        {
                            return Err("Teleport effect requires its own resolver".into());
                        }
                        if m.teleportation && p.teleport_distance_feet > 50 {
                            return Err("Shadow teleport range is 50 feet".into());
                        }
                        if !m.teleportation && maneuver != "Inferno Blade" {
                            return Err("Turn replay supports teleport maneuvers and Inferno Blade; other effects need their own resolver".into());
                        }
                        if m.teleportation
                            && (!p.destination_visible || !p.destination_line_of_effect)
                        {
                            return Err(
                                "Teleport destination requires line of sight and line of effect"
                                    .into(),
                            );
                        }
                        if maneuver == "Inferno Blade" {
                            inferno_level = audit
                                .tracks
                                .get(track)
                                .ok_or("Unknown Inferno Blade track")?
                                .initiator_level;
                        }
                        budget.spend(action)?;
                        expended.insert((track.clone(), maneuver.clone()));
                        if m.teleportation
                            && p.teleport_destination_clear
                            && p.target_visible_before
                            && p.target_in_reach_after
                            && ls.get("Crinti Shadow Marauder").copied().unwrap_or(0) >= 5
                        {
                            Ok(1)
                        } else {
                            Ok(0)
                        }
                    }
                }
            })();
            match result {
                Ok(n) => {
                    out.full_attacks += n;
                    out.main_attacks += n * (main + u32::from(p.haste));
                    out.off_attacks += n * off;
                    if n > 0 {
                        let str_mod =
                            (d.stats.get("STR").copied().unwrap_or(10) - 10).div_euclid(2);
                        let dex_mod =
                            (d.stats.get("DEX").copied().unwrap_or(10) - 10).div_euclid(2);
                        let shadow = self
                            .pack
                            .maneuvers
                            .get(&p.stance)
                            .is_some_and(|s| s.discipline == "Shadow Hand");
                        let sneak = p.stance == "Assassin's Stance"
                            && (p.denied_dex || p.flanking)
                            && !p.precision_immune;
                        let sudden = p.denied_dex
                            && !p.precision_immune
                            && ls.get("Crinti Shadow Marauder").copied().unwrap_or(0) >= 2;
                        let dice = 1
                            + if sneak { 2 } else { 0 }
                            + if sudden {
                                ls["Crinti Shadow Marauder"] / 2
                            } else {
                                0
                            }
                            + if p.deadly_precision_mic {
                                u32::from(sneak || sudden)
                            } else {
                                if sneak {
                                    2
                                } else {
                                    0
                                }
                            };
                        // The scenario has +5 weapons. Martial Discipline adds to attack, never damage.
                        let common = p.weapon_enhancement as i32
                            + if shadow && has("Shadow Blade") {
                                dex_mod
                            } else {
                                0
                            }
                            + if sneak && has("Craven") {
                                d.total_level() as i32
                            } else {
                                0
                            };
                        let fire = if inferno_level == 0 || p.fire_immune {
                            (0, 0, 0.0)
                        } else {
                            let min = (3 + inferno_level).saturating_sub(p.fire_resistance);
                            let max = (18 + inferno_level).saturating_sub(p.fire_resistance);
                            let mean = (1..=6)
                                .flat_map(|a| {
                                    (1..=6).flat_map(move |b| {
                                        (1..=6).map(move |c| {
                                            (a + b + c + inferno_level)
                                                .saturating_sub(p.fire_resistance)
                                                as f64
                                        })
                                    })
                                })
                                .sum::<f64>()
                                / 216.0;
                            (min, max, mean)
                        };
                        for (count, strength) in [
                            (n * (main + u32::from(p.haste)), str_mod),
                            (n * off, if str_mod > 0 { str_mod / 2 } else { str_mod }),
                        ] {
                            let flat = common + strength;
                            out.hit_damage_min += count as u64 * (dice as i32 + flat).max(1) as u64
                                + count as u64 * fire.0 as u64;
                            out.hit_damage_max += count as u64
                                * (6 * dice as i32 + flat).max(1) as u64
                                + count as u64 * fire.1 as u64;
                            expected += count as f64 * (3.5 * dice as f64 + flat as f64 + fire.2);
                        }
                    }
                    out.log.push(format!(
                        "{}: {:?}; full attacks +{n}; turn attempts {}",
                        i + 1,
                        a,
                        out.turning_remaining
                    ));
                }
                Err(e) => {
                    out.errors.push(format!("Action {}: {e}", i + 1));
                    break;
                }
            }
        }
        if out.errors.is_empty() {
            out.all_hit_average = Some(expected);
        }
        out.damage_notes.push("Every attack assumed to hit; no criticals or physical DR. Precision assumes visible, reachable vitals. Teleportation does not itself deny Dexterity. Gear: two +5 martial-discipline short swords.".into());
        out
    }
}
