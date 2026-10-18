//! One metamagic transformation path for casting and supernatural applications.
use crate::{CharacterDraft, Engine};
use pcgen_core::magic::*;
use pcgen_core::{CompiledRulePack, SpellDefinition};
use serde::Serialize;

pub(crate) fn cost(pack: &CompiledRulePack, rule: SpellRule) -> Result<MetamagicCost, String> {
    pack.entities
        .iter()
        .find(|e| e.metadata.spell_rule == Some(rule))
        .and_then(|e| e.metadata.metamagic_cost)
        .ok_or_else(|| format!("Metamagic cost is unavailable for {rule:?}"))
}
/// Applies one feat in declared order. Unknown eligibility never becomes true.
pub(crate) fn transform(
    pack: &CompiledRulePack,
    spell: &SpellDefinition,
    rule: SpellRule,
    inside: bool,
    effective: &mut i32,
    duration: &mut Option<String>,
    range: SpellRange,
    multiplier: &mut u32,
) -> Result<u32, String> {
    let cost = cost(pack, rule)?.slot_increase;
    match rule {
        SpellRule::Sanctum => *effective += if inside { 1 } else { -1 },
        SpellRule::Persistent => {
            if !matches!(range, SpellRange::Personal | SpellRange::Fixed)
                || spell.profile.instantaneous != Some(false)
                || spell.profile.discharged != Some(false)
            {
                return Err("Persistent Spell requires verified personal/fixed range and a noninstantaneous, nondischarged effect".into());
            }
            *duration = Some("24 hours".into());
        }
        SpellRule::Extend => {
            if spell.profile.instantaneous != Some(false)
                || duration.as_ref().is_none_or(|d| {
                    d.to_lowercase().contains("permanent")
                        || (d.to_lowercase().contains("concentration") && !d.contains('+'))
                })
            {
                return Err("Extend requires a verified finite duration".into());
            }
            *duration = duration.as_ref().map(|d| {
                if d == "24 hours" {
                    "48 hours".into()
                } else if let Some((head, tail)) = d.split_once('+') {
                    format!("{} + 2 * ({})", head.trim(), tail.trim())
                } else {
                    format!("2 * ({d})")
                }
            });
        }
        SpellRule::Enlarge => {
            if !matches!(
                range,
                SpellRange::Close | SpellRange::Medium | SpellRange::Long
            ) {
                return Err("Enlarge requires close, medium or long range".into());
            }
            *multiplier = 2;
        }
        SpellRule::Empower | SpellRule::Maximize => {
            if spell.profile.variable_numeric_effect != Some(true) {
                return Err("Variable numeric effects are not verified for this spell".into());
            }
        }
        SpellRule::Widen => {
            if spell.profile.widenable != Some(true) {
                return Err(
                    "Widen requires a verified burst, emanation, spread, cone or cylinder".into(),
                );
            }
        }
        SpellRule::Silent | SpellRule::Still | SpellRule::Quicken => {}
        _ => return Err("This feat needs its dedicated casting parameters".into()),
    }
    Ok(cost)
}

#[derive(Debug, Serialize)]
pub struct MetamagicResolution {
    pub mode: MetamagicMode,
    pub spell: String,
    pub base_spell_level: u32,
    pub modified_spell_level: u32,
    pub spellcraft_dc: u32,
    pub spellcraft_modifier: i32,
    pub die_result: u32,
    pub take_ten: bool,
    pub check_total: i32,
    pub success: bool,
    pub uses_per_day: u32,
    pub uses_spent_by_attempt: u32,
    pub uses_remaining_after_attempt: u32,
    pub additional_spell_slots: u32,
    pub action: String,
    pub provokes_attack_of_opportunity: bool,
    pub original_duration: Option<String>,
    pub resulting_duration: Option<String>,
    pub resulting_effective_level: i32,
    pub range_multiplier: u32,
    pub applied: Vec<SpellRule>,
    pub source: String,
}
impl Engine {
    pub fn metamagic_preview(
        &self,
        draft: &CharacterDraft,
        request: &MetamagicRequest,
    ) -> Result<MetamagicResolution, String> {
        let levels = crate::engine::effective_class_levels(&self.pack, draft);
        let feature = levels
            .iter()
            .find_map(|l| {
                crate::magic::class_magic(&self.pack, &l.class_id).and_then(|m| {
                    m.metamagic_features
                        .iter()
                        .find(|f| f.mode == request.mode && l.level >= f.at_level)
                })
            })
            .ok_or("This metamagic feature has not been acquired")?;
        if feature.excluded.contains(&request.apply) {
            return Err("This feature excludes the selected metamagic feat".into());
        }
        match request.mode {
            MetamagicMode::Cooperative if !request.adjacent || !request.willing_ally || !request.readied_action => return Err("Cooperative Metamagic requires a readied action, an adjacent willing allied caster, and their spell being cast".into()),
            MetamagicMode::Effect if !request.ongoing_effect || !(request.adjacent || request.within_effect) => return Err("Metamagic Effect requires an existing ongoing effect and adjacency or presence within it".into()),
            _=>{}
        }
        let spell = self
            .pack
            .spells
            .values()
            .find(|s| {
                s.name.eq_ignore_ascii_case(&request.spell)
                    || s.id.eq_ignore_ascii_case(&request.spell)
            })
            .ok_or("Unknown spell")?;
        let base = *spell
            .class_levels
            .iter()
            .find(|(c, _)| c.eq_ignore_ascii_case(&request.spell_list))
            .map(|(_, l)| l)
            .ok_or("Spell has no verified level on the original caster's list")?;
        if request.mode == MetamagicMode::Effect && spell.profile.instantaneous != Some(false) {
            return Err(
                "Metamagic Effect requires a verified noninstantaneous spell effect".into(),
            );
        }
        if request.mode == MetamagicMode::Effect && cost(&self.pack, request.apply)?.casting_only {
            return Err("A casting-only feat cannot change an existing effect".into());
        }
        let feat = self
            .pack
            .entities
            .iter()
            .find(|e| {
                e.metadata.spell_rule == Some(request.apply)
                    && crate::effects::ability_matches_draft(draft, e)
            })
            .ok_or("The incantatrix does not possess the applied metamagic feat")?;
        let state = crate::apply_effects(&self.pack, draft);
        if !state.diagnostics.is_empty() {
            return Err(state.diagnostics.join("; "));
        }
        if feat
            .requirements
            .iter()
            .any(|p| crate::effects::predicate_result(Some(p), draft, &state) != Some(true))
        {
            return Err("Applied metamagic prerequisites are unmet or unavailable".into());
        }
        let snapshot = self.evaluate(draft);
        if !snapshot.is_available() {
            return Err(snapshot.diagnostics.join("; "));
        }
        if draft
            .skills
            .iter()
            .find(|(s, _)| s.eq_ignore_ascii_case("Spellcraft"))
            .is_none_or(|(_, r)| *r < 1.0)
        {
            return Err("Spellcraft is trained only; at least one rank is required".into());
        }
        let modifier = *snapshot
            .skills
            .iter()
            .find(|(s, _)| s.eq_ignore_ascii_case("Spellcraft"))
            .map(|(_, v)| v)
            .ok_or("Spellcraft modifier is unavailable")?;
        let ability = *snapshot
            .stats
            .get(&feature.uses_ability)
            .ok_or("Daily-use ability score is unavailable")?;
        let uses = (feature.uses_base as i32 + (ability - 10).div_euclid(2)).max(0) as u32;
        if request.spent >= uses {
            return Err("No daily uses remain for this feature".into());
        }
        let die = match request.roll {
            Some(n) if (1..=20).contains(&n) => n,
            Some(_) => return Err("A skill roll must be between 1 and 20".into()),
            None if request.threatened || request.distracted => {
                return Err("Taking 10 is unavailable while threatened or distracted".into())
            }
            None => 10,
        };
        let mut modified = base;
        let mut effective = base as i32;
        let mut duration = spell.profile.duration.clone();
        let mut multiplier = 1;
        let mut applied = Vec::new();
        for rule in &request.existing {
            if applied.contains(rule) {
                return Err("Duplicate existing metamagic feat".into());
            }
            applied.push(*rule);
            let adjustment = transform(
                &self.pack,
                spell,
                *rule,
                request.inside_sanctum,
                &mut effective,
                &mut duration,
                spell.profile.range,
                &mut multiplier,
            )?;
            if request.mode == MetamagicMode::Cooperative || !cost(&self.pack, *rule)?.casting_only
            {
                modified = modified
                    .checked_add(crate::class_features::improved_cost(
                        &self.pack, draft, adjustment,
                    ))
                    .ok_or("Modified spell level overflow")?;
            }
        }
        if applied.contains(&request.apply) {
            return Err("The spell already has this metamagic feat".into());
        }
        let original_duration = duration.clone();
        modified = modified
            .checked_add(crate::class_features::improved_cost(
                &self.pack,
                draft,
                transform(
                    &self.pack,
                    spell,
                    request.apply,
                    request.inside_sanctum,
                    &mut effective,
                    &mut duration,
                    spell.profile.range,
                    &mut multiplier,
                )?,
            ))
            .ok_or("Modified spell level overflow")?;
        // Spellcraft uses slot adjustment, never Sanctum's effective spell level.
        let dc = feature
            .dc_per_spell_level
            .checked_mul(modified)
            .and_then(|x| x.checked_add(feature.dc_base))
            .ok_or("Spellcraft DC overflow")?;
        let total = modifier
            .checked_add(die as i32)
            .ok_or("Spellcraft total overflow")?;
        let success = i64::from(total) >= i64::from(dc); // Natural 20 is not an automatic skill success.
        if success {
            applied.push(request.apply);
        }
        Ok(MetamagicResolution {
            mode: request.mode,
            spell: spell.name.clone(),
            base_spell_level: base,
            modified_spell_level: modified,
            spellcraft_dc: dc,
            spellcraft_modifier: modifier,
            die_result: die,
            take_ten: request.roll.is_none(),
            check_total: total,
            success,
            uses_per_day: uses,
            uses_spent_by_attempt: 1,
            uses_remaining_after_attempt: uses - request.spent - 1,
            additional_spell_slots: 0,
            action: feature.action.clone(),
            provokes_attack_of_opportunity: true,
            original_duration: original_duration.clone(),
            resulting_duration: if success { duration } else { original_duration },
            resulting_effective_level: if success {
                effective
            } else {
                base as i32
                    + if request.existing.contains(&SpellRule::Sanctum) {
                        if request.inside_sanctum {
                            1
                        } else {
                            -1
                        }
                    } else {
                        0
                    }
            },
            range_multiplier: if success {
                multiplier
            } else {
                if request.existing.contains(&SpellRule::Enlarge) {
                    2
                } else {
                    1
                }
            },
            applied,
            source: feature.source.clone(),
        })
    }
}
