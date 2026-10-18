//! Original forum input and an explicitly limited repair probe; neither certifies missing history.
use crate::{incarnum::*, martial::TrainingAudit, CharacterDraft, Engine};
use pcgen_core::{martial::MartialTraining, AdvancementFrame};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncarnumCase {
    pub name: String,
    pub levels: Vec<String>,
    pub feats: Vec<(u32, String)>,
    pub training: MartialTraining,
    pub probe_stats: BTreeMap<String, i32>,
    pub probe_alignment: String,
    pub loadout: IncarnumLoadout,
    pub turn: IncarnumTurn,
}
#[derive(Debug, Serialize)]
pub struct IncarnumMilestone {
    pub level: u32,
    pub bab: i32,
    pub essentia: u32,
    pub capacity: u32,
    pub sneak_dice: u32,
    pub warblade_il: Option<u32>,
    pub ordinary_attacks: usize,
}
#[derive(Debug, Serialize)]
pub struct IncarnumCaseReport {
    pub acquisition: crate::build::BuildAudit,
    pub name: String,
    pub milestones: Vec<IncarnumMilestone>,
    pub original_training: TrainingAudit,
    pub original_allocation: IncarnumSnapshot,
    pub build_errors: Vec<String>,
    pub assumptions: Vec<String>,
    pub repaired_training: TrainingAudit,
    pub allocation_probe: IncarnumSnapshot,
    pub turn_probe: IncarnumReplay,
}
impl IncarnumCase {
    pub fn character(&self, through: u32) -> CharacterDraft {
        let mut d = CharacterDraft {
            name: self.name.clone(),
            race_id: "Warforged".into(),
            alignment: self.probe_alignment.clone(),
            stats: self.probe_stats.clone(),
            ..Default::default()
        };
        let mut counts = BTreeMap::new();
        for (i, c) in self.levels.iter().take(through as usize).enumerate() {
            let n = counts.entry(c).or_insert(0);
            *n += 1;
            d.advancement
                .push(AdvancementFrame::single(i as u32 + 1, c, *n, 0, 0));
        }
        d.abilities = self
            .feats
            .iter()
            .filter(|(n, _)| *n <= through)
            .map(|(_, f)| ("FEAT".into(), f.clone()))
            .collect();
        d.martial_training = self.training.clone();
        d.martial_training
            .acquisitions
            .retain(|a| a.character_level <= through);
        d
    }
    /// Keep Leading The Charge, Sudden Leap, Pouncing Charge and WRT; replace Mongoose with a prerequisite seed.
    pub fn probe_character(&self, through: u32) -> CharacterDraft {
        let mut d = self.character(through);
        let mut bonus = false;
        d.abilities.retain(|(_, n)| {
            if n == "Bonus Essentia" {
                let keep = !bonus;
                bonus = true;
                keep
            } else {
                true
            }
        });
        for a in &mut d.martial_training.acquisitions {
            if a.maneuver == "Dancing Mongoose" {
                a.maneuver = "Wolf Fang Strike".into();
            }
        }
        d.martial_training.acquisitions.sort_by_key(|a| {
            (
                a.character_level,
                match a.maneuver.as_str() {
                    "Leading The Charge" => 0,
                    "Wolf Fang Strike" => 1,
                    "Sudden Leap" => 2,
                    _ => 3,
                },
            )
        });
        d
    }
}
impl Engine {
    pub fn audit_incarnum_case(&self, c: &IncarnumCase) -> IncarnumCaseReport {
        let original = c.character(c.levels.len() as u32);
        let probe = c.probe_character(c.levels.len() as u32);
        let original_allocation = self.incarnum_snapshot(&original, &c.loadout);
        let original_training = self.martial_training(&original);
        let acquisition = self.audit_sequence(&original, &c.feats);
        let mut errors = acquisition.errors();
        errors.extend(original_allocation.errors.clone());
        errors.extend(original_training.errors.clone());
        let mut milestones = vec![];
        for level in 1..=c.levels.len() as u32 {
            let d = c.probe_character(level);
            let ls = class_levels(&d);
            let bab = ls
                .iter()
                .filter_map(|(n, l)| self.pack.progressions.get(n).map(|p| p.bab(*l)))
                .sum();
            let allocation = self.incarnum_snapshot(&d, &IncarnumLoadout::default());
            let sneak = (ls.get("Sneak Attack Fighter").copied().unwrap_or(0) + 1) / 2
                + (ls.get("Umbral Disciple").copied().unwrap_or(0) + 1) / 3;
            // Attack arithmetic at the milestone does not require initiating any maneuver.
            let mut attacks_d = d.clone();
            attacks_d.martial_training = Default::default();
            let p = IncarnumTurn {
                actions: vec![IncarnumAction::FullAttack],
                readied: Default::default(),
                gloves: level >= 12,
                dual_wield: level >= 2,
                slam_available: true,
                target_feet: 5,
                denied_dex: true,
                flanking: false,
                precision_immune: false,
                target_concealment: false,
                healing_rounds_spent: 0,
                soulchilling_used_this_round: false,
            };
            let replay = self.replay_incarnum_turn(&attacks_d, &IncarnumLoadout::default(), &p);
            milestones.push(IncarnumMilestone {
                level,
                bab,
                essentia: allocation.pool,
                capacity: allocation.capacity,
                sneak_dice: sneak,
                warblade_il: self.initiator_level(&d, "Warblade", level).ok(),
                ordinary_attacks: replay.attacks.len(),
            });
        }
        IncarnumCaseReport{acquisition,name:c.name.clone(),milestones,original_training,original_allocation,build_errors:errors,
            assumptions:vec![
                "The post supplies no ability scores, alignment or skill-spending history. Probe CON 14 and NG are explicit assumptions, not recovered facts; prestige entry and feat prerequisites are not certified.".into(),
                "The repair removes the second Bonus Essentia and replaces Dancing Mongoose with Wolf Fang Strike, learned before Sudden Leap and Pouncing Charge. Level 18's replacement feat remains unchosen.".into(),
                "Two light manufactured weapons, medium Warforged, speed 30, active Ranger TWF style and Gloves from level 12; slam availability is explicitly selectable. No shield is used by this full-attack probe.".into(),
                "Kiss extends reach only on your own turn. Target distance is distance at the attack, not charge movement; flanking must use the adjacent-square rule.".into(),
                "Natural attacks, actual hit damage and saving throws need explicit adjudication/rolls. Attack rows are opportunities, not automatic hits or automatic sneak attacks.".into(),
                "Only the four named soulmelds and this action chain execute here. Other class features retain complete level descriptions but are not all encounter resolvers.".into(),
            ],repaired_training:self.martial_training(&probe),allocation_probe:self.incarnum_snapshot(&probe,&c.loadout),turn_probe:self.replay_incarnum_turn(&probe,&c.loadout,&c.turn)}
    }
}
