//! Bounded RAW audit: class casting, learned spells and extraordinary riders are distinct.
use crate::{CharacterDraft, Engine};
use pcgen_core::AdvancementFrame;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JokerCase {
    pub name: String,
    pub levels: Vec<String>,
    pub feats: Vec<(u32, String)>,
}
impl JokerCase {
    pub fn character(&self, through: u32) -> CharacterDraft {
        let mut d = CharacterDraft {
            name: self.name.clone(),
            race_id: "Human".into(),
            alignment: "LG".into(),
            stats: BTreeMap::from([
                ("CHA".into(), 20),
                ("WIS".into(), 14),
                ("INT".into(), 16),
                ("CON".into(), 14),
            ]),
            ..Default::default()
        };
        let mut counts = BTreeMap::new();
        for (i, c) in self.levels.iter().take(through as usize).enumerate() {
            let n = counts.entry(c).or_insert(0);
            *n += 1;
            let mut frame = AdvancementFrame::single(i as u32 + 1, c, *n, 0, 0);
            if c != "Favored Soul"
                && !(c == "Exalted Arcanist" && *n == 1)
                && !(c == "War Weaver" && *n == 1)
            {
                frame.lanes[0].advances.push("Favored Soul".into());
            }
            d.advancement.push(frame);
        }
        d
    }
}
#[derive(Debug, Serialize)]
pub struct JokerMilestone {
    pub level: u32,
    pub favored_soul_casting_level: u32,
    pub rejected_advancements: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct JokerReport {
    pub acquisition: crate::build::BuildAudit,
    pub name: String,
    pub errors: Vec<String>,
    pub milestones: Vec<JokerMilestone>,
    pub limits: Vec<String>,
}
impl Engine {
    pub fn audit_joker_case(&self, case: &JokerCase) -> JokerReport {
        let acquisition =
            self.audit_sequence(&case.character(case.levels.len() as u32), &case.feats);
        let mut errors = acquisition.errors();
        let milestones = (1..=case.levels.len() as u32)
            .map(|level| {
                let mut diagnostics = Vec::new();
                let tracks = crate::magic::casting_levels(
                    &self.pack,
                    &case.character(level),
                    &mut diagnostics,
                );
                JokerMilestone {
                    level,
                    favored_soul_casting_level: tracks.get("Favored Soul").copied().unwrap_or(0),
                    rejected_advancements: diagnostics,
                }
            })
            .collect::<Vec<_>>();
        if milestones
            .last()
            .is_some_and(|m| !m.rejected_advancements.is_empty())
        {
            errors.push("Arcane-only prestige advancements cannot target Favored Soul. Magical Training and Spell Knowledge do not change the class's divine casting type.".into());
        }
        errors.push("Spellguard converts defensive personal ARCANE spells only. A Favored Soul spell remains divine, including with Deity's Favor.".into());
        JokerReport { acquisition, name: case.name.clone(), errors, milestones, limits: vec![
            "This is the supplied Joker sequence, not a general build validator. LG and ability scores are probe assumptions, not supplied character facts.".into(),
            "Rejected levels are retained for diagnostics. Remaining casting totals are arithmetic contributions, not certification of a legal progression.".into(),
            "Missing: skill allocations, deity/church/region/order evidence, two first-level feat sources, known spell choices, two Spellguard bonus feats and retraining history.".into(),
            "Precocious Apprentice's second-level-spell termination and Sanctum early-entry interpretation require separate adjudication. Neither resolves the rank cap or divine/arcane distinction.".into(),
            "Exalted Arcanist Spell Knowledge grants two arcane spells, no slots or base-class levels; spell selection and sanctified spell access remain descriptive.".into(),
            "Favored Soul 4 retains the one-ally Deity's Favor limit even with advanced casting. Repeated uses overlap; they do not stack.".into(),
            "Outside Sanctum can lower effective level 6 to 5 for tapestry, but still costs a sixth-level slot. It grants neither slots nor spell access. Practiced Spellcaster changes caster level only.".into(),
        ] }
    }
}
