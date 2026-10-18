//! Full-level native class features, independent of any example build.
use crate::{CharacterDraft, Engine};
use pcgen_core::magic::*;
use pcgen_core::CompiledRulePack;
use serde::Serialize;
pub(crate) fn validate_class_choices(pack: &CompiledRulePack, d: &CharacterDraft) -> Vec<String> {
    let mut errors = vec![];
    for level in crate::engine::effective_class_levels(pack, d) {
        if let Some(m) = crate::magic::class_magic(pack, &level.class_id) {
            for rule in m.choices.iter().filter(|r| r.at_level <= level.level) {
                let chosen = d
                    .choices
                    .get(&rule.key)
                    .and_then(|v| rule.options.iter().find(|o| &o.value == v));
                match chosen {
                    None=>errors.push(format!("{}: choose {}",level.class_id,rule.label)),
                    Some(o) if !o.alignments.is_empty()&&!o.alignments.iter().any(|a|a.eq_ignore_ascii_case(&d.alignment))=>errors.push(format!("{}: alignment incompatible with {}; class powers unavailable until totem is changed by the RAW retraining ritual",level.class_id,o.value)),
                    _=>{}
                }
            }
        }
    }
    errors
}

pub(crate) fn improved_cost(pack: &CompiledRulePack, draft: &CharacterDraft, cost: u32) -> u32 {
    let reduced = crate::engine::effective_class_levels(pack, draft)
        .iter()
        .any(|l| {
            crate::magic::class_magic(pack, &l.class_id)
                .is_some_and(|m| m.improved_metamagic.is_some_and(|n| l.level >= n))
        });
    if reduced && cost > 0 {
        cost.saturating_sub(1).max(1)
    } else {
        cost
    }
}
pub(crate) fn instant_uses(pack: &CompiledRulePack, draft: &CharacterDraft) -> u32 {
    crate::engine::effective_class_levels(pack, draft)
        .iter()
        .filter_map(|l| {
            crate::magic::class_magic(pack, &l.class_id).and_then(|m| {
                m.instant_metamagic
                    .iter()
                    .filter(|t| t.level <= l.level)
                    .max_by_key(|t| t.level)
                    .map(|t| t.uses)
            })
        })
        .max()
        .unwrap_or(0)
}
pub(crate) fn sudden_granted(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    rule: SpellRule,
) -> bool {
    if pack.entities.iter().any(|e| {
        e.metadata.sudden_rule == Some(rule) && crate::effects::ability_matches_draft(draft, e)
    }) {
        return true;
    }
    crate::engine::effective_class_levels(pack, draft)
        .iter()
        .any(|l| {
            crate::magic::class_magic(pack, &l.class_id).is_some_and(|m| {
                m.sudden_grants
                    .iter()
                    .any(|g| l.level >= g.level && g.rule == rule)
            })
        })
}
/// Returns selected additional spells only after validating their acquisition level.
pub(crate) fn learned_spells(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    track: &str,
) -> Result<Vec<(String, u32)>, String> {
    let mut result = vec![];
    for class in crate::engine::effective_class_levels(pack, draft) {
        if !class.class_id.eq_ignore_ascii_case(track) {
            continue;
        }
        let Some(m) = crate::magic::class_magic(pack, track) else {
            continue;
        };
        let Some(learning) = &m.advanced_learning else {
            continue;
        };
        for level in &learning.levels {
            let key = format!("{track}.AdvancedLearning.{level}");
            let Some(name) = draft.choices.get(&key) else {
                continue;
            };
            if *level > class.level {
                return Err(format!(
                    "Advanced Learning at level {level} is not acquired"
                ));
            }
            let spell = pack
                .spells
                .values()
                .find(|s| s.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| format!("Unknown Advanced Learning spell: {name}"))?;
            if spell
                .class_levels
                .keys()
                .any(|id| id.eq_ignore_ascii_case(track))
            {
                return Err(
                    "Advanced Learning must add a spell outside the existing class list".into(),
                );
            }
            let wizard_level = spell
                .class_levels
                .iter()
                .filter(|(id, _)| {
                    learning
                        .lists
                        .iter()
                        .any(|list| list.eq_ignore_ascii_case(id))
                })
                .map(|(_, n)| *n)
                .min()
                .ok_or("Advanced Learning spell is not on an eligible source list")?;
            if !learning
                .schools
                .iter()
                .any(|school| school.eq_ignore_ascii_case(&spell.school))
            {
                return Err(format!(
                    "Advanced Learning requires one of these schools: {}",
                    learning.schools.join(", ")
                ));
            }
            let acquired = draft
                .advancement
                .iter()
                .position(|f| {
                    f.lanes
                        .iter()
                        .any(|l| l.class_id.eq_ignore_ascii_case(track) && l.class_level == *level)
                })
                .ok_or("Advanced Learning acquisition frame is unavailable")?;
            let mut before = draft.clone();
            before.advancement.truncate(acquired + 1);
            let mut errors = vec![];
            let progression = crate::magic::casting_levels(pack, &before, &mut errors);
            if !errors.is_empty() {
                return Err(errors.join("; "));
            }
            let cl = progression
                .get(track)
                .ok_or("Learning casting progression unavailable")?;
            let definition = pack
                .classes
                .values()
                .find(|c| c.id.eq_ignore_ascii_case(track))
                .ok_or("Learning class unavailable")?;
            let maximum = definition
                .spell_slots
                .get(cl)
                .and_then(|r| r.len().checked_sub(1))
                .ok_or("Learning spell levels unavailable")?;
            if wizard_level as usize > maximum {
                return Err(format!(
                    "{name} exceeds the spell level known when Advanced Learning was acquired"
                ));
            }
            if result.iter().any(|(n, _)| n == &spell.name) {
                return Err("Advanced Learning cannot add the same spell twice".into());
            }
            result.push((spell.name.clone(), wizard_level));
        }
    }
    Ok(result)
}
#[derive(Debug, Serialize)]
pub struct FeatureSummary {
    pub class_id: String,
    pub level: u32,
    pub features: Vec<FeatureLevel>,
    pub choices: Vec<String>,
}
impl Engine {
    pub fn class_features(&self, draft: &CharacterDraft) -> Vec<FeatureSummary> {
        crate::engine::effective_class_levels(&self.pack, draft)
            .iter()
            .filter_map(|l| {
                let m = crate::magic::class_magic(&self.pack, &l.class_id)?;
                Some(FeatureSummary {
                    class_id: l.class_id.clone(),
                    level: l.level,
                    features: m
                        .progression
                        .iter()
                        .filter(|f| f.level <= l.level)
                        .cloned()
                        .collect(),
                    choices: m
                        .advanced_learning
                        .iter()
                        .flat_map(|r| r.levels.iter())
                        .filter(|n| **n <= l.level)
                        .map(|n| format!("{}.AdvancedLearning.{n}", l.class_id))
                        .collect(),
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SuddenReplacement {
    pub pool: String,
    pub class_id: String,
    pub class_level: u32,
    pub character_level: u32,
    pub granted_feat: String,
    pub available: bool,
    pub selected: Option<String>,
}
pub(crate) fn sudden_pool(class: &str, level: u32) -> String {
    format!("{}_sudden_{level}", class.to_lowercase().replace(' ', "_"))
}
fn same_feat(a: &str, b: &str) -> bool {
    a.trim().to_ascii_lowercase().replace(' ', "_")
        == b.trim().to_ascii_lowercase().replace(' ', "_")
}
fn selected_level(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    pool: &str,
    name: &str,
) -> Option<u32> {
    for class in &pack.entities {
        if let Some(m) = &class.metadata.class_magic {
            for g in &m.sudden_grants {
                if pool.eq_ignore_ascii_case(&sudden_pool(&class.name, g.level)) {
                    return draft
                        .advancement
                        .iter()
                        .find(|f| {
                            f.lanes.iter().any(|l| {
                                l.class_id.eq_ignore_ascii_case(&class.name)
                                    && l.class_level == g.level
                            })
                        })
                        .map(|f| f.character_level);
                }
            }
        }
    }
    draft
        .feat_acquired_at
        .iter()
        .find(|(n, _)| same_feat(n, name))
        .map(|(_, n)| *n)
}
pub(crate) fn sudden_slots(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
) -> Result<Vec<SuddenReplacement>, String> {
    let mut slots = vec![];
    for class in crate::engine::effective_class_levels(pack, draft) {
        let Some(m) = crate::magic::class_magic(pack, &class.class_id) else {
            continue;
        };
        for grant in m.sudden_grants.iter().filter(|g| g.level <= class.level) {
            let at = draft
                .advancement
                .iter()
                .find(|f| {
                    f.lanes.iter().any(|l| {
                        l.class_id.eq_ignore_ascii_case(&class.class_id)
                            && l.class_level == grant.level
                    })
                })
                .ok_or("Sudden grant acquisition frame unavailable")?
                .character_level;
            let pool = sudden_pool(&class.class_id, grant.level);
            let prior: Vec<_> = draft
                .abilities
                .iter()
                .filter(|(_, n)| same_feat(n, &grant.feat))
                .collect();
            let mut available = false;
            for (source, name) in prior {
                let acquired=selected_level(pack,draft,source,name).ok_or_else(||format!("Record when {name} was acquired before resolving its duplicate class grant"))?;
                if acquired == 0 || acquired >= at {
                    return Err(format!("{name} must have been acquired before character level {at}; it cannot be selected again after its automatic grant"));
                }
                available = true;
            }
            let selected: Vec<_> = draft
                .abilities
                .iter()
                .filter(|(p, _)| p.eq_ignore_ascii_case(&pool))
                .map(|(_, n)| n.clone())
                .collect();
            if selected.len() > 1 || (!available && !selected.is_empty()) {
                return Err(format!(
                    "{pool}: no replacement slot available for this selection"
                ));
            }
            if let Some(name) = selected.first() {
                let feat = pack
                    .entities
                    .iter()
                    .find(|e| {
                        e.name.eq_ignore_ascii_case(name) || e.id.key.eq_ignore_ascii_case(name)
                    })
                    .ok_or_else(|| format!("Unknown replacement feat {name}"))?;
                if !feat
                    .type_tags
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case("Metamagic"))
                {
                    return Err(format!("{name} is not a metamagic feat"));
                }
                if same_feat(name, &grant.feat) {
                    return Err("Select a different metamagic feat for the replacement".into());
                }
                if draft.abilities.iter().any(|(p, n)| {
                    p != &pool
                        && same_feat(n, name)
                        && selected_level(pack, draft, p, n).is_some_and(|l| l <= at)
                }) && !pack
                    .abilities
                    .get(&feat.id.key)
                    .is_some_and(|a| a.multi == Some(true))
                {
                    return Err(format!("{name} is already possessed"));
                }
            }
            slots.push(SuddenReplacement {
                pool,
                class_id: class.class_id.clone(),
                class_level: grant.level,
                character_level: at,
                granted_feat: grant.feat.clone(),
                available,
                selected: selected.first().cloned(),
            });
        }
    }
    Ok(slots)
}
impl Engine {
    pub fn sudden_replacements(
        &self,
        draft: &CharacterDraft,
    ) -> Result<Vec<SuddenReplacement>, String> {
        sudden_slots(&self.pack, draft)
    }
}
pub(crate) fn validate_replacements(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
) -> Result<(), String> {
    for slot in sudden_slots(pack, draft)? {
        let Some(name) = slot.selected else {
            continue;
        };
        let mut before = draft.clone();
        before
            .advancement
            .retain(|f| f.character_level <= slot.character_level);
        before.abilities.retain(|(pool, name)| {
            pool != &slot.pool
                && selected_level(pack, draft, pool, name)
                    .is_some_and(|at| at <= slot.character_level)
        });
        let state = crate::apply_effects(pack, &before);
        if !state.diagnostics.is_empty() {
            return Err(state.diagnostics.join("; "));
        }
        let feat = pack
            .entities
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(&name) || e.id.key.eq_ignore_ascii_case(&name))
            .ok_or("Replacement feat unavailable")?;
        if feat
            .requirements
            .iter()
            .any(|p| crate::effects::predicate_result(Some(p), &before, &state) != Some(true))
        {
            return Err(format!("{name}: prerequisites not established at character level {}; record prerequisite feat acquisition levels",slot.character_level));
        }
    }
    Ok(())
}

pub(crate) fn validate_sudden_feats(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
) -> Result<(), String> {
    for (pool, name) in &draft.abilities {
        let Some(feat) = pack.entities.iter().find(|e| {
            e.metadata.sudden_rule.is_some()
                && (e.name.eq_ignore_ascii_case(name) || e.id.key.eq_ignore_ascii_case(name))
        }) else {
            continue;
        };
        let mut before = draft.clone();
        if let Some(at) = selected_level(pack, draft, pool, name) {
            if at == 0 || at > draft.total_level() {
                return Err(format!("Invalid acquisition level for {name}"));
            }
            before.advancement.retain(|f| f.character_level <= at);
            before.abilities.retain(|(p, n)| {
                !same_feat(n, name)
                    && selected_level(pack, draft, p, n).is_some_and(|level| level <= at)
            });
        } else {
            before.abilities.retain(|(_, n)| !same_feat(n, name));
        }
        let state = crate::apply_effects(pack, &before);
        if !state.diagnostics.is_empty() {
            return Err(state.diagnostics.join("; "));
        }
        if feat
            .requirements
            .iter()
            .any(|p| crate::effects::predicate_result(Some(p), &before, &state) != Some(true))
        {
            return Err(format!(
                "{name}: prerequisites are unmet or unavailable before selection"
            ));
        }
    }
    Ok(())
}

impl Engine {
    pub fn class_proficiencies(&self, draft: &CharacterDraft) -> Vec<ProficiencyGrant> {
        crate::engine::effective_class_levels(&self.pack, draft)
            .iter()
            .flat_map(|l| {
                crate::magic::class_magic(&self.pack, &l.class_id)
                    .into_iter()
                    .flat_map(|m| m.proficiencies.iter())
                    .filter(|g| g.level <= l.level)
                    .cloned()
            })
            .collect()
    }
}

#[derive(Debug, Serialize)]
pub struct ResolvedClassFeature {
    pub class_id: String,
    pub name: String,
    pub values: std::collections::BTreeMap<String, i32>,
    pub daily_uses: Option<u32>,
    pub resolution: Option<pcgen_core::magic::FeatureResolution>,
}
pub(crate) fn resolve_feature_values(
    pack: &CompiledRulePack,
    d: &CharacterDraft,
    stats: &std::collections::BTreeMap<String, i32>,
) -> Result<Vec<ResolvedClassFeature>, String> {
    let mut out = vec![];
    for l in crate::engine::effective_class_levels(pack, d) {
        let Some(m) = crate::magic::class_magic(pack, &l.class_id) else {
            continue;
        };
        let mut vars = std::collections::HashMap::from([(
            "CLASSLEVEL".into(),
            (l.level as i32 - crate::necromancy::negative_levels(&d.class_play, "self")).max(0),
        )]);
        for (key, score) in stats {
            vars.insert(key.clone(), (score - 10).div_euclid(2));
        }
        for f in m.feature_values.iter().filter(|f| f.at_level <= l.level) {
            let eval = |formula: &str| {
                pcgen_formula::Formula::parse(formula)
                    .and_then(|f| f.eval_i32(&vars))
                    .map_err(|e| format!("{} / {} / {formula}: {e}", l.class_id, f.name))
            };
            let values = f
                .values
                .iter()
                .map(|(k, v)| eval(v).map(|n| (k.clone(), n)))
                .collect::<Result<_, _>>()?;
            let daily_uses = f
                .daily_uses
                .as_ref()
                .map(|n| {
                    eval(n).and_then(|n| {
                        u32::try_from(n).map_err(|_| "Negative resource allowance".into())
                    })
                })
                .transpose()?;
            out.push(ResolvedClassFeature {
                class_id: l.class_id.clone(),
                name: f.name.clone(),
                values,
                daily_uses,
                resolution: f.resolution.clone(),
            });
        }
    }
    Ok(out)
}
impl Engine {
    pub fn class_feature_values(
        &self,
        d: &CharacterDraft,
    ) -> Result<Vec<ResolvedClassFeature>, String> {
        let snapshot = self.evaluate(d);
        if !snapshot.is_available() {
            return Err(snapshot.diagnostics.join("; "));
        }
        resolve_feature_values(&self.pack, d, &snapshot.stats)
    }
}
