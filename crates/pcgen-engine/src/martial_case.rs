//! Reproducible case inputs and audit; the forum's totals are never used as expectations.
use crate::{martial::*, CharacterDraft, Engine};
use pcgen_core::{martial::*, AdvancementFrame};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MartialCase {
    pub name: String,
    pub levels: Vec<String>,
    pub alignment: String,
    pub deity: String,
    pub domains: Vec<String>,
    pub stats: BTreeMap<String, i32>,
    pub feats: Vec<(u32, String)>,
    pub training: MartialTraining,
    pub plan: TurnPlan,
}
#[derive(Debug, Serialize)]
pub struct MartialCaseReport {
    pub acquisition: crate::build::BuildAudit,
    pub name: String,
    pub class_levels: BTreeMap<String, u32>,
    pub base_attack_bonus: i32,
    pub base_saves: BTreeMap<String, i32>,
    pub training: TrainingAudit,
    pub build_errors: Vec<String>,
    pub unresolved: Vec<String>,
    pub strict: TurnReplay,
    pub resource_probe: TurnReplay,
    pub action_rules_probe: TurnReplay,
    pub resource_probe_assumptions: Vec<String>,
}
impl MartialCase {
    pub fn character(&self) -> CharacterDraft {
        let mut d = CharacterDraft {
            name: self.name.clone(),
            race_id: "Human".into(),
            alignment: self.alignment.clone(),
            deity: Some(self.deity.clone()),
            domains: self.domains.clone(),
            stats: self.stats.clone(),
            martial_training: self.training.clone(),
            ..Default::default()
        };
        let mut ls = BTreeMap::new();
        for (i, c) in self.levels.iter().enumerate() {
            let n = ls.entry(c.clone()).or_insert(0);
            *n += 1;
            d.advancement
                .push(AdvancementFrame::single(i as u32 + 1, c, *n, 0, 0));
        }
        d.abilities = self
            .feats
            .iter()
            .map(|(_, n)| ("FEAT".into(), n.clone()))
            .collect();
        d
    }
}
impl Engine {
    pub fn audit_martial_case(&self, c: &MartialCase) -> MartialCaseReport {
        let d = c.character();
        let mut counts = BTreeMap::<String, u32>::new();
        let mut bab = 0;
        let acquisition = self.audit_sequence(&d, &c.feats);
        let mut errors = acquisition.errors();
        for name in &c.levels {
            let n = counts.entry(name.clone()).or_default();
            *n += 1;
            if let Some(p) = self.pack.progressions.get(name) {
                bab += p.bab(*n) - p.bab(*n - 1);
            } else {
                errors.push(format!("Unknown class {name}"));
            }
        }
        if c.deity == "Wee Jas" {
            if !["LN", "LE", "LG"].contains(&c.alignment.as_str()) {
                errors.push("Wee Jas cleric alignment must be within one step of LN; True Neutral is not permitted for a non-neutral deity".into());
            }
            for domain in &c.domains {
                if !["Death", "Law", "Magic"].contains(&domain.as_str()) {
                    errors.push(format!(
                        "Wee Jas does not grant {domain} in the standard deity entry"
                    ));
                }
            }
        }
        let mut saves = BTreeMap::new();
        for (name, n) in &counts {
            if let Some(p) = self.pack.progressions.get(name) {
                for (key, kind) in [
                    ("Fortitude", pcgen_core::SaveKind::Fortitude),
                    ("Reflex", pcgen_core::SaveKind::Reflex),
                    ("Will", pcgen_core::SaveKind::Will),
                ] {
                    *saves.entry(key.into()).or_insert(0) += p.save(*n, kind);
                }
            }
        }
        let training = self.martial_training(&d);
        // This separate probe deliberately removes the illegal level-8 choice.
        // It does not certify the class entry, deity, feats or equipment history.
        let mut probe = d.clone();
        probe
            .martial_training
            .acquisitions
            .retain(|a| a.maneuver != "Divine Surge, Greater");
        let mut plan = c.plan.clone();
        plan.free_impetus_ruling = Some(
            "Counterfactual resource probe: allow unlimited free-action Divine Impetus".into(),
        );
        let action_rules_probe = self.replay_martial_turn(&probe, &c.plan);
        let mut strict = self.replay_martial_turn(&d, &c.plan);
        strict.errors.extend(errors.clone());
        if !strict.errors.is_empty() {
            strict.all_hit_average = None;
        }
        MartialCaseReport{acquisition,name:c.name.clone(),class_levels:counts,base_attack_bonus:bab,base_saves:saves,training,build_errors:errors,unresolved:vec![
            "No per-level skill spending, base ability purchase or item acquisition history was supplied; entry skills and feat ability thresholds are not certified.".into(),
            "Human Crinti entry needs Dambrath region and noble sanction; not established by the post.".into(),
            "HP 154 cannot be verified without level-by-level HP inputs.".into(),
            "Bracers are not light armor: the literal Swordsage AC bonus condition is not met. An unarmored allowance requires a ruling.".into(),
            "Belt of Magnificence and Battle is a custom combined item requiring approval; Belt of Battle also grants +2 initiative.".into(),
            "Craven qualification through a stance and the favored campaign's deity/domain adaptations need explicit adjudication.".into(),
        ],strict,action_rules_probe,resource_probe:self.replay_martial_turn(&probe,&plan),resource_probe_assumptions:vec!["Remove the illegal Greater Divine Surge selection; all attack feats assumed available.".into(),"Allow free Divine Impetus and the claimed Undeath domain; this is not a legal-build result.".into(),"One Nightstick only, prepared teleport maneuvers, pre-existing Assassin's Stance, activated Boots of Speed, and the chosen target conditions.".into()]}
    }
}
