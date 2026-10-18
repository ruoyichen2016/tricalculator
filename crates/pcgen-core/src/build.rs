//! Ordered acquisition inputs. Derived projections carry no ledger and cannot recurse.
use crate::{CharacterDraft, DraftSpell};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Satisfied,
    Failed,
    MissingEvidence,
    NeedsRuling,
    NotImplemented,
}
impl DecisionStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Satisfied => "Satisfied",
            Self::Failed => "Failed",
            Self::MissingEvidence => "Missing Evidence",
            Self::NeedsRuling => "Needs Ruling",
            Self::NotImplemented => "Not Implemented",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleDecision {
    pub level: u32,
    pub subject: String,
    pub status: DecisionStatus,
    pub explanation: String,
    pub source: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildHistory {
    pub initial_stats: BTreeMap<String, i32>,
    pub initial_alignment: String,
    /// A declaration of recorded input completeness, not an endorsement of legality.
    pub recorded_through: u32,
    pub events: Vec<BuildEvent>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildEvent {
    pub level: u32,
    pub action: Acquisition,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Acquisition {
    Skill { name: String, ranks: f64 },
    Feat { name: String },
    RetrainFeat { old: String, new: String },
    Spell { spell: DraftSpell },
    Choice { key: String, value: String },
    Evidence { statement: String },
    AbilityIncrease { ability: String },
}
impl Acquisition {
    pub fn label(&self) -> String {
        match self {
            Self::Skill { name, ranks } => format!("{name}: +{ranks} ranks"),
            Self::Feat { name } => format!("Feat: {name}"),
            Self::RetrainFeat { old, new } => format!("Retrain {old} → {new}"),
            Self::Spell { spell } => format!(
                "Learn {} ({})",
                spell.name,
                spell.class_id.as_deref().unwrap_or("no track")
            ),
            Self::Choice { key, value } => format!("{key}: {value}"),
            Self::Evidence { statement } => format!("Evidence: {statement}"),
            Self::AbilityIncrease { ability } => format!("{ability} +1"),
        }
    }
}
impl CharacterDraft {
    /// Materialize only recorded acquisitions, never final-state skills/feats at an earlier level.
    pub fn build_at(&self, level: u32, event_limit: usize) -> Result<Self, String> {
        let h = self
            .build_history
            .as_ref()
            .ok_or("Acquisition history is missing")?;
        let mut d = self.clone();
        d.build_history = None;
        d.preparations.clear();
        d.advancement.retain(|f| f.character_level <= level);
        d.stats = h.initial_stats.clone();
        d.alignment = h.initial_alignment.clone();
        d.skills.clear();
        d.spells.clear();
        d.abilities.clear();
        d.choices.clear();
        d.active_states.clear();
        d.feat_acquired_at.clear();
        d.martial_training
            .acquisitions
            .retain(|a| a.character_level <= level);
        // Equipment and encounter state must never prove historical prerequisites.
        d.equipment.clear();
        d.equipment_modifiers.clear();
        d.equipment_base_items.clear();
        d.equip_slots.clear();
        d.class_play = Default::default();
        let mut previous = 0;
        for e in h.events.iter().take(event_limit) {
            if e.level == 0 || e.level < previous || e.level > self.total_level() {
                return Err(
                    "Acquisition events must be ordered and belong to recorded character levels"
                        .into(),
                );
            }
            previous = e.level;
            if e.level > level {
                continue;
            }
            match &e.action {
                Acquisition::Skill { name, ranks } => {
                    if !ranks.is_finite() || *ranks <= 0.0 {
                        return Err("Skill rank increments must be finite and positive".into());
                    }
                    let total = d.skills.entry(name.clone()).or_default();
                    *total += ranks;
                    if !total.is_finite() {
                        return Err("Skill rank total overflow".into());
                    }
                }
                Acquisition::Feat { name } => {
                    d.abilities.push(("FEAT".into(), name.clone()));
                    d.feat_acquired_at.insert(name.clone(), e.level);
                }
                Acquisition::RetrainFeat { old, new } => {
                    let i = d
                        .abilities
                        .iter()
                        .position(|(c, n)| c == "FEAT" && n == old)
                        .ok_or("Retrained feat was not previously acquired")?;
                    d.abilities[i].1 = new.clone();
                    d.feat_acquired_at.remove(old);
                    d.feat_acquired_at.insert(new.clone(), e.level);
                }
                Acquisition::Spell { spell } => d.spells.push(spell.clone()),
                Acquisition::Choice { key, value } => {
                    d.choices.insert(key.clone(), value.clone());
                }
                Acquisition::Evidence { statement } => d.active_states.push(statement.clone()),
                Acquisition::AbilityIncrease { ability } => {
                    let score = d.stats.get_mut(ability).ok_or("Unknown ability score")?;
                    *score = score.checked_add(1).ok_or("Ability score overflow")?;
                }
            }
        }
        Ok(d)
    }
    /// Final projection preserves current equipment and encounter state, separately from acquisitions.
    pub fn materialized(&self) -> Result<Self, String> {
        let mut d = match &self.build_history {
            Some(h) => self.build_at(self.total_level(), h.events.len())?,
            None => self.clone(),
        };
        d.preparations = self.preparations.clone();
        d.class_play = self.class_play.clone();
        d.equipment = self.equipment.clone();
        d.equipment_modifiers = self.equipment_modifiers.clone();
        d.equipment_base_items = self.equipment_base_items.clone();
        d.equip_slots = self.equip_slots.clone();
        Ok(d)
    }
    pub fn refresh_build_projection(&mut self) -> Result<(), String> {
        let history = self.build_history.clone();
        let preparations = self.preparations.clone();
        let mut d = self.materialized()?;
        d.build_history = history;
        d.preparations = preparations;
        *self = d;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageState {
    Recorded,
    Implemented,
    Partial,
    NotImplemented,
    #[default]
    NotAssessed,
}
impl CoverageState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Recorded => "Recorded",
            Self::Implemented => "Implemented",
            Self::Partial => "Partial",
            Self::NotImplemented => "Not Implemented",
            Self::NotAssessed => "Not Assessed",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureCoverage {
    pub data: CoverageState,
    pub qualification: CoverageState,
    pub execution: CoverageState,
    #[serde(default)]
    pub note: String,
}
impl Default for FeatureCoverage {
    fn default() -> Self {
        Self {
            data: CoverageState::Recorded,
            qualification: CoverageState::NotAssessed,
            execution: CoverageState::NotAssessed,
            note: String::new(),
        }
    }
}
