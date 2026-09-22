//! Lowering of raw records into the runtime `CompiledRulePack`.

use crate::patches::PatchOperation;
use crate::source::RawRecord;
use crate::typed::build_typed_definitions;
use pcgen_core::domain::{
    ClassDefinition, ClassProgression, DeityDefinition, DomainDefinition, EquipmentDefinition,
    EquipmentModifierDefinition, RaceDefinition, SkillDefinition, SpellDefinition, SpellPowerType,
};
use pcgen_core::{
    CompiledEntity, Effect, EffectAction, EffectPhase, EntityId, Modifier, ModifierOperation,
    Predicate, Provenance, Rational, RuleCoverage, TypedDefinitions,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledPackHeader {
    #[serde(default)]
    pub game_mode: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    /// Deterministic fingerprint of the pack's runtime semantics.
    #[serde(default)]
    pub rules_fingerprint: String,
    /// Upstream PCGen data revision the pack was built from, when known.
    #[serde(default)]
    pub data_ref: Option<String>,
}

/// Runtime-facing rule pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledRulePack {
    pub header: CompiledPackHeader,
    pub entities: Vec<CompiledEntity>,
    pub coverage: RuleCoverage,
    #[serde(default)]
    pub races: std::collections::BTreeMap<String, RaceDefinition>,
    #[serde(default)]
    pub classes: std::collections::BTreeMap<String, ClassDefinition>,
    #[serde(default)]
    pub skills: std::collections::BTreeMap<String, SkillDefinition>,
    #[serde(default)]
    pub spells: std::collections::BTreeMap<String, SpellDefinition>,
    #[serde(default)]
    pub equipment: std::collections::BTreeMap<String, EquipmentDefinition>,
    #[serde(default)]
    pub equipment_modifiers: std::collections::BTreeMap<String, EquipmentModifierDefinition>,
    #[serde(default)]
    pub deities: std::collections::BTreeMap<String, DeityDefinition>,
    #[serde(default)]
    pub domains: std::collections::BTreeMap<String, DomainDefinition>,
    #[serde(default)]
    pub variants: std::collections::BTreeMap<String, pcgen_core::VariantDefinition>,
    #[serde(default)]
    pub progressions: std::collections::BTreeMap<String, ClassProgression>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub coverage: RuleCoverage,
    pub patch_operations: usize,
    pub diagnostics: usize,
    pub error_diagnostics: usize,
}

/// Lower records into the runtime IR.
///
/// The first cut preserves legacy tags and creates stable IDs.  Tag-specific
/// lowering (`BONUS`, `PRE*`, `ABILITY`, ...) plugs into this function during
/// migration; unknown data is never silently discarded.
pub fn lower_records(
    game_mode: Option<String>,
    sources: Vec<String>,
    records: &[RawRecord],
) -> (CompiledRulePack, ImportReport) {
    let ruleset = game_mode.as_deref().unwrap_or("unknown");
    let mut coverage = RuleCoverage::default();
    let mut entities = Vec::new();
    let mut current_class: Option<String> = None;

    for record in records {
        let category = record.category.to_ascii_uppercase();
        let is_class_header = category == "CLASS"
            && record.name.trim().parse::<u32>().is_err()
            && !record.name.contains(".MOD")
            && !record.name.contains(".COPY=");
        if is_class_header {
            current_class = Some(record.key.clone());
        }
        let (effective_category, effective_key) =
            if category == "CLASS" && record.name.trim().parse::<u32>().is_ok() {
                let level = record.name.trim();
                (
                    "CLASS_LEVEL".to_string(),
                    format!("{}#{level}", current_class.as_deref().unwrap_or("?")),
                )
            } else {
                (record.category.clone(), record.key.clone())
            };

        let id = EntityId::from_pcgen(
            ruleset,
            record.source.source_id.as_str(),
            &effective_category,
            &effective_key,
        );
        // `CATEGORY:` on ABILITY / ABILITYCATEGORY declares the ability pool
        // and is consumed here rather than reported as legacy.
        let ability_category =
            if matches!(effective_category.as_str(), "ABILITY" | "ABILITYCATEGORY") {
                record
                    .tags
                    .iter()
                    .find(|tag| tag.name.eq_ignore_ascii_case("CATEGORY"))
                    .map(|tag| tag.value.trim().to_string())
                    .filter(|value| !value.is_empty())
            } else {
                None
            };

        let legacy_tags: Vec<String> = record
            .tags
            .iter()
            .filter(|tag| {
                !known_tag(&tag.name) && !consumed_legacy_tag(&effective_category, &tag.name)
            })
            .map(|tag| format!("{}:{}", tag.name, tag.value))
            .collect();
        for tag in record.tags.iter().filter(|tag| {
            !known_tag(&tag.name) && !consumed_legacy_tag(&effective_category, &tag.name)
        }) {
            coverage.record_legacy_tag(&tag.name);
        }
        let provenance = Provenance::new(
            record.source.path.display().to_string(),
            format!("PCGen {} definition", category),
        )
        .with_line(record.source.line);
        let phase = phase_for_category(&effective_category);

        let mut effects = Vec::new();
        let mut requirements = Vec::new();
        let mut unsupported_tags = Vec::new();
        for tag in &record.tags {
            let upper = tag.name.to_ascii_uppercase();
            if upper == "BONUS" {
                match lower_bonus(tag, phase, &provenance) {
                    Some(effect) => effects.push(effect),
                    None => unsupported_tags.push(format!("BONUS:{}", tag.value)),
                }
            } else if upper == "DEFINE" || upper == "VAR" {
                match lower_define(tag) {
                    Some(effect) => effects.push(effect),
                    None => unsupported_tags.push(format!("{}:{}", tag.name, tag.value)),
                }
            } else if upper == "ABILITY" || upper == "AUTO" {
                match lower_grant(record, tag) {
                    Some(effect) => effects.push(effect),
                    None => unsupported_tags.push(format!("{}:{}", tag.name, tag.value)),
                }
            } else if upper == "CHOOSE" {
                match lower_choose(tag) {
                    Some(effect) => effects.push(effect),
                    None => unsupported_tags.push(format!("CHOOSE:{}", tag.value)),
                }
            } else if crate::tags::is_predicate_tag(&upper) {
                match parse_predicate(&format!("{}:{}", tag.name, tag.value), "") {
                    Some(predicate) => requirements.push(predicate),
                    None => unsupported_tags.push(format!("{}:{}", tag.name, tag.value)),
                }
            }
        }
        let known = record
            .tags
            .iter()
            .filter(|tag| known_tag(&tag.name))
            .count();
        for entry in &unsupported_tags {
            let tag_name = entry.split(':').next().unwrap_or(entry.as_str());
            coverage.record_unsupported_tag(tag_name);
        }
        coverage.record_entity(
            &effective_category,
            effects.len(),
            legacy_tags.len(),
            unsupported_tags.len(),
        );
        coverage.record_tags(&effective_category, record.tags.len(), known);
        coverage.record_effect_actions(&effective_category, &effects);
        entities.push(CompiledEntity {
            id,
            category: pcgen_core::EntityCategory::from_pcgen(&effective_category),
            name: record.name.clone(),
            effects,
            requirements,
            legacy_tags,
            unsupported_tags,
            source: record.source.path.display().to_string(),
            ability_category,
        });
    }

    let typed = build_typed_definitions(records);
    let typed_digest = typed_digest(&typed);
    let rules_fingerprint = pcgen_core::rules_fingerprint(&pcgen_core::RuleFingerprintInput {
        game_mode: game_mode.as_deref().unwrap_or("unknown"),
        sources: &sources,
        entities: &entities,
        progressions: &typed.progressions,
        variants: &typed.variants,
        typed_digest: &typed_digest,
    });
    let pack = CompiledRulePack {
        header: CompiledPackHeader {
            game_mode,
            sources,
            rules_fingerprint,
            data_ref: None,
        },
        entities,
        coverage: coverage.clone(),
        races: typed.races,
        classes: typed.classes,
        skills: typed.skills,
        spells: typed.spells,
        equipment: typed.equipment,
        equipment_modifiers: typed.equipment_modifiers,
        deities: typed.deities,
        domains: typed.domains,
        variants: typed.variants,
        progressions: typed.progressions,
    };
    let report = ImportReport {
        coverage,
        patch_operations: 0,
        diagnostics: 0,
        error_diagnostics: 0,
    };
    (pack, report)
}

fn phase_for_category(category: &str) -> EffectPhase {
    match category.to_ascii_uppercase().as_str() {
        "RACE" => EffectPhase::Race,
        "CLASS" | "CLASS_LEVEL" => EffectPhase::Class,
        "TEMPLATE" => EffectPhase::Template,
        "ABILITY" | "FEAT" => EffectPhase::Feat,
        "EQUIPMENT" | "EQUIPMOD" => EffectPhase::Equipment,
        "DEITY" | "DOMAIN" => EffectPhase::Faith,
        _ => EffectPhase::Global,
    }
}

fn lower_bonus(
    tag: &crate::source::RawTag,
    phase: EffectPhase,
    provenance: &Provenance,
) -> Option<Effect> {
    lower_bonus_effect(&tag.value, phase, provenance)
}

/// Lower a raw `BONUS` value (without the `BONUS:` prefix) into an effect.
pub fn lower_bonus_effect(
    value: &str,
    phase: EffectPhase,
    provenance: &Provenance,
) -> Option<Effect> {
    let parts: Vec<&str> = value.split('|').map(str::trim).collect();
    let first = *parts.first()?;
    if first.is_empty() {
        return None;
    }
    let (target, value_index) =
        if parts.len() >= 3 && parts[1].parse::<i32>().is_err() && !parts[1].is_empty() {
            (format!("{first}.{}", parts[1]), 2)
        } else {
            (first.to_string(), 1)
        };
    let raw_value = parts.get(value_index)?.to_string();
    let mut bonus_type = None;
    let mut conditions = Vec::new();
    for part in parts.iter().skip(value_index + 1) {
        if let Some(type_name) = part.strip_prefix("TYPE=") {
            bonus_type = Some(
                type_name
                    .split('.')
                    .next()
                    .unwrap_or(type_name)
                    .to_ascii_lowercase(),
            );
        } else if crate::tags::is_predicate_tag(part) {
            // PRERULE gates the whole bonus; parse_predicate handles it.
            conditions.push(parse_predicate(part, "")?);
        }
    }
    let condition = match conditions.len() {
        0 => None,
        1 => conditions.pop(),
        _ => Some(Predicate::All(conditions)),
    };

    if let Ok(value) = raw_value.parse::<i32>() {
        let modifier = Modifier {
            target,
            operation: ModifierOperation::Add,
            value: Rational::from(value),
            bonus_type: bonus_type.clone(),
            stack_key: None,
            condition: condition.clone(),
            source: provenance.clone(),
        };
        Some(Effect {
            phase,
            condition,
            action: EffectAction::Modify { modifier },
            source: provenance.clone(),
        })
    } else if !raw_value.is_empty() {
        Some(Effect {
            phase,
            condition: condition.clone(),
            action: EffectAction::ModifyFormula {
                target,
                expression: raw_value,
                bonus_type,
                stack_key: None,
                condition,
                source: provenance.clone(),
            },
            source: provenance.clone(),
        })
    } else {
        None
    }
}

fn lower_define(tag: &crate::source::RawTag) -> Option<Effect> {
    let (name, expression) = tag.value.split_once('|')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let source = Provenance::new("pcgen", "DEFINE/VAR lowering");
    Some(Effect {
        phase: EffectPhase::Global,
        condition: None,
        action: EffectAction::DefineVariable {
            name: name.to_string(),
            expression: expression.trim().to_string(),
        },
        source,
    })
}

fn lower_grant(record: &RawRecord, tag: &crate::source::RawTag) -> Option<Effect> {
    let parts: Vec<&str> = tag.value.split('|').map(str::trim).collect();
    let meaningful: Vec<&str> = parts
        .iter()
        .copied()
        .filter(|part| {
            !part.is_empty()
                && !part.eq_ignore_ascii_case("AUTOMATIC")
                && !part.eq_ignore_ascii_case("VIRTUAL")
                && !part.eq_ignore_ascii_case("NORMAL")
                && !part.starts_with("TYPE=")
                && !crate::tags::is_predicate_tag(part)
        })
        .collect();
    let key = meaningful.last()?.to_string();
    let category = if meaningful.len() > 1 {
        meaningful[0].to_string()
    } else {
        "Ability".to_string()
    };
    Some(Effect {
        phase: EffectPhase::Global,
        condition: None,
        action: EffectAction::Grant {
            entity: EntityId::new("pcgen", record.source.source_id.clone(), category, key),
        },
        source: Provenance::new(
            record.source.path.display().to_string(),
            "ABILITY/AUTO lowering",
        ),
    })
}

fn lower_choose(tag: &crate::source::RawTag) -> Option<Effect> {
    let spec = tag.value.trim();
    if spec.is_empty() {
        return None;
    }
    Some(Effect {
        phase: EffectPhase::Global,
        condition: None,
        action: EffectAction::Choose {
            spec: spec.to_string(),
        },
        source: Provenance::new("pcgen", "CHOOSE lowering"),
    })
}

fn parse_predicate(raw: &str, _value: &str) -> Option<Predicate> {
    let raw = raw.trim();
    // A leading `!` negates the whole predicate (`!PRETYPE`, `!PRERACE`,
    // ...).  Negation inside `PREMULT` groups is handled by `parse_group`,
    // but the negated form also appears as a standalone token, which used to
    // fall through as an unknown tag.
    if let Some(inner) = raw.strip_prefix('!') {
        let inner = inner.trim();
        if inner.is_empty() {
            return None;
        }
        return Some(Predicate::Not(Box::new(parse_predicate(inner, _value)?)));
    }
    let (tag, body) = raw.split_once(':')?;
    let tag_upper = tag.to_ascii_uppercase();
    let parts: Vec<&str> = body.split(',').map(str::trim).collect();
    match tag_upper.as_str() {
        "PRESTAT" => {
            let part = parts.iter().skip(1).find(|part| part.contains('='))?;
            let (stat, score) = part.split_once('=')?;
            Some(Predicate::Stat {
                stat: stat.trim().to_ascii_uppercase(),
                min: score.parse::<i32>().ok(),
                max: None,
            })
        }
        "PRECLASS" => {
            let part = parts.iter().skip(1).find(|part| part.contains('='))?;
            let (class_id, level) = part.split_once('=')?;
            Some(Predicate::ClassLevel {
                class_id: class_id.trim().to_string(),
                min: level.parse::<u32>().ok()?,
                max: None,
            })
        }
        "PREVARGT" | "PREVARLT" | "PREVARGTEQ" | "PREVARLTEQ" | "PREVAREQ" | "PREVARNEQ" => {
            let (name, value) = if let Some((name, value)) = body.split_once('=') {
                (name.trim().to_string(), value.trim().to_string())
            } else {
                let mut fields = body.split(',');
                (
                    fields.next()?.trim().to_string(),
                    fields.next()?.trim().to_string(),
                )
            };
            let value = value.parse::<i32>().ok()?;
            let (min, max) = match tag_upper.as_str() {
                "PREVARGTEQ" => (Some(value), None),
                "PREVARLTEQ" => (None, Some(value)),
                // Strict forms are approximated as >= / <= (off-by-one is
                // acceptable until the predicate gains exclusive bounds).
                "PREVARGT" => (Some(value + 1), None),
                "PREVARLT" => (None, Some(value - 1)),
                _ => (Some(value), Some(value)),
            };
            let predicate = Predicate::Variable { name, min, max };
            if tag_upper == "PREVARNEQ" {
                Some(Predicate::Not(Box::new(predicate)))
            } else {
                Some(predicate)
            }
        }
        // PREPCLEVEL counts character class levels only; monster / racial HD
        // are excluded.  PCGen syntax: PREPCLEVEL:MIN=n,MAX=m (either part
        // optional) and an optional Q: qualifier override.
        "PREPCLEVEL" => {
            let body = body.strip_prefix("Q:").unwrap_or(body);
            let mut min: Option<u32> = None;
            let mut max: Option<u32> = None;
            for part in body
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
            {
                let (key, value) = part.split_once('=')?;
                let value = value.trim().parse::<u32>().ok()?;
                match key.trim().to_ascii_uppercase().as_str() {
                    "MIN" => min = Some(value),
                    "MAX" => max = Some(value),
                    _ => return None,
                }
            }
            if min.is_none() && max.is_none() {
                None
            } else {
                Some(Predicate::CharacterClassLevel { min, max })
            }
        }
        "PREABILITY" | "PREFEAT" => {
            let category = if tag_upper == "PREFEAT" {
                "Feat".to_string()
            } else {
                parts
                    .iter()
                    .find_map(|part| part.strip_prefix("CATEGORY=").map(str::to_string))
                    .unwrap_or_else(|| "Ability".to_string())
            };
            let key = parts
                .iter()
                .rev()
                .find(|part| !part.contains('='))
                .map(|part| (*part).to_string())?;
            Some(Predicate::HasEntity {
                entity: EntityId::new("pcgen", "ability", category, key),
            })
        }
        "PRESKILL" => {
            let part = parts.iter().rev().find(|part| part.contains('='))?;
            let (skill, level) = part.split_once('=')?;
            Some(Predicate::SkillRank {
                skill: skill.trim().to_string(),
                min: level.parse::<u32>().ok()?,
            })
        }
        "PRERACE" => {
            let key = parts
                .iter()
                .rev()
                .find(|part| !part.is_empty() && part.parse::<u32>().is_err())
                .map(|part| (*part).to_string())?;
            Some(Predicate::HasEntity {
                entity: EntityId::new("pcgen", "race", "Race", key),
            })
        }
        "PREALIGN" => {
            let allowed = parts
                .iter()
                .filter(|part| !part.is_empty() && part.parse::<i32>().is_err())
                .map(|part| (*part).to_string())
                .collect::<Vec<_>>();
            if allowed.is_empty() {
                None
            } else {
                Some(Predicate::Alignment { allowed })
            }
        }
        "PRELEVEL" => {
            let min = body
                .split(|ch: char| !ch.is_ascii_digit())
                .find(|part| !part.is_empty())
                .and_then(|part| part.parse::<u32>().ok())?;
            Some(Predicate::CharacterLevel { min })
        }
        "PREMULT" => parse_premult(body),
        "PRETOTALAB" => {
            let body = body.strip_prefix("Q:").unwrap_or(body).trim();
            let min = body.parse::<u32>().ok()?;
            Some(Predicate::TotalBab { min })
        }
        "PRETYPE" => {
            let tag = parts
                .iter()
                .rev()
                .find(|part| !part.is_empty() && part.parse::<i32>().is_err())
                .map(|part| (*part).to_string())?;
            Some(Predicate::HasTag { tag })
        }
        // Base creature size comparisons (PCGen PREBASESIZEx).
        "PREBASESIZEEQ" => {
            let size = parts
                .iter()
                .rev()
                .find(|part| !part.is_empty())?
                .to_string();
            Some(Predicate::All(vec![
                Predicate::SizeAtLeast { size: size.clone() },
                Predicate::SizeAtMost { size },
            ]))
        }
        "PREBASESIZELT" | "PREBASESIZELTEQ" => {
            let size = parts
                .iter()
                .rev()
                .find(|part| !part.is_empty())?
                .to_string();
            if tag_upper == "PREBASESIZELTEQ" {
                Some(Predicate::SizeAtMost { size })
            } else {
                // Strictly smaller: <= AND NOT ==.
                Some(Predicate::All(vec![
                    Predicate::SizeAtMost { size: size.clone() },
                    Predicate::Not(Box::new(Predicate::SizeAtLeast { size })),
                ]))
            }
        }
        "PRELEGSGTEQ" => {
            let count = body
                .split(|ch: char| !ch.is_ascii_digit())
                .find(|part| !part.is_empty())
                .and_then(|part| part.parse::<u32>().ok())?;
            Some(Predicate::LegsAtLeast { count })
        }
        // PRERULE:1,SYS_DWS_RACIAL[,SYS_DWS_EXTRA] — game-mode rule toggles.
        // We lower to a rule-enabled predicate per rule name.
        "PRERULE" => {
            let rules: Vec<Predicate> = parts
                .iter()
                .filter(|part| !part.is_empty() && part.parse::<i32>().is_err())
                .map(|rule| Predicate::RuleEnabled {
                    rule: (*rule).to_string(),
                })
                .collect();
            match rules.len() {
                0 => None,
                1 => rules.into_iter().next(),
                _ => Some(Predicate::All(rules)),
            }
        }
        // PREAGESET:1,Middle Age — age category gate.
        "PREAGESET" => {
            let category = parts
                .iter()
                .rev()
                .find(|part| !part.is_empty() && part.parse::<i32>().is_err())
                .map(|part| (*part).to_string())?;
            Some(Predicate::AgeCategory { category })
        }
        // PRERACETYPE:Humanoid — race type tag match.
        "PRERACETYPE" => {
            let type_tag = parts
                .iter()
                .rev()
                .find(|part| !part.is_empty() && part.parse::<i32>().is_err())
                .map(|part| (*part).to_string())?;
            Some(Predicate::RaceType { type_tag })
        }
        // PRESPELLTYPE:count,type=level — can cast `count` spells of `type`
        // at spell level `level` or higher.
        "PRESPELLTYPE" => {
            let (count_str, spec) = body.split_once(',')?;
            let count = count_str.trim().parse::<u32>().ok()?;
            let (power_str, level_str) = spec.split_once('=')?;
            let power = SpellPowerType::from_pcgen(power_str)?;
            let min_level = level_str.trim().parse::<u32>().ok()?;
            Some(Predicate::SpellType {
                power,
                min_level,
                count: count.max(1),
            })
        }
        // PRESPELLBOOK:YES — requires a spellbook.
        "PRESPELLBOOK" => match body.trim().to_ascii_uppercase().as_str() {
            "YES" => Some(Predicate::HasSpellbook),
            "NO" => Some(Predicate::Not(Box::new(Predicate::HasSpellbook))),
            _ => None,
        },
        // PRESPELLCAST:MEMORIZE=Y|N — prepared vs spontaneous caster.
        "PRESPELLCAST" => {
            if let Some(part) = parts
                .iter()
                .find(|part| part.to_ascii_uppercase().starts_with("MEMORIZE="))
            {
                let value = part.split_once('=')?.1.trim();
                let prepared = value.eq_ignore_ascii_case("Y") || value.eq_ignore_ascii_case("YES");
                Some(Predicate::CastingTypeIs { prepared })
            } else {
                let part = parts
                    .iter()
                    .find(|part| part.to_ascii_uppercase().starts_with("TYPE="))?;
                let power = SpellPowerType::from_pcgen(part.split_once('=')?.1)?;
                Some(Predicate::HasSpellPower { power })
            }
        }
        // PRESIZEEQ / PRESIZEGTEQ use the creature's current size.
        "PRESIZEEQ" => {
            let size = parts.iter().rev().find(|part| !part.is_empty())?;
            let size = pcgen_core::normalize_size(size);
            Some(Predicate::All(vec![
                Predicate::SizeAtLeast { size: size.clone() },
                Predicate::SizeAtMost { size },
            ]))
        }
        "PRESIZEGTEQ" => {
            let size = parts.iter().rev().find(|part| !part.is_empty())?;
            Some(Predicate::SizeAtLeast {
                size: pcgen_core::normalize_size(size),
            })
        }
        "PRESIZELT" | "PRESIZELTEQ" => {
            let size = parts.iter().rev().find(|part| !part.is_empty())?;
            let size = pcgen_core::normalize_size(size);
            if tag_upper == "PRESIZELTEQ" {
                Some(Predicate::SizeAtMost { size })
            } else {
                Some(Predicate::All(vec![
                    Predicate::SizeAtMost { size: size.clone() },
                    Predicate::Not(Box::new(Predicate::SizeAtLeast { size })),
                ]))
            }
        }
        "PRESIZEGT" => {
            let size = parts.iter().rev().find(|part| !part.is_empty())?;
            let size = pcgen_core::normalize_size(size);
            Some(Predicate::All(vec![
                Predicate::SizeAtLeast { size: size.clone() },
                Predicate::Not(Box::new(Predicate::SizeAtMost { size })),
            ]))
        }
        // PRETEMPLATE:n,A,B — character has at least n of the listed templates.
        "PRETEMPLATE" => {
            let min_count = parts.first()?.trim().parse::<u32>().ok()?;
            let names = parts
                .iter()
                .skip(1)
                .map(|part| part.trim().trim_end_matches(']').trim())
                .filter(|part| !part.is_empty() && part.parse::<i32>().is_err())
                .map(|part| part.to_ascii_lowercase())
                .collect::<std::collections::BTreeSet<_>>();
            if names.is_empty() {
                return None;
            }
            Some(Predicate::TemplateCount {
                names,
                min_count: min_count.max(1),
            })
        }
        // PREHANDSGTEQ:n / PREHANDSEQ:n — hand count.
        "PREHANDSGTEQ" => Some(Predicate::HandsAtLeast {
            count: body.trim().parse::<u32>().ok()?,
        }),
        "PREHANDSEQ" => {
            let count = body.trim().parse::<u32>().ok()?;
            Some(Predicate::All(vec![
                Predicate::HandsAtLeast { count },
                Predicate::HandsAtMost { count },
            ]))
        }
        // PREMOVE:n,Mode=rate[|TYPE=...] — has movement mode `Mode`.
        "PREMOVE" => {
            let spec = body.split('|').next()?.trim();
            let movement = spec.split_once(',')?.1;
            let (mode, rate) = movement.split_once('=')?;
            Some(Predicate::MovementAtLeast {
                mode: mode.trim().to_ascii_lowercase(),
                feet: rate.trim().parse::<u32>().ok()?.max(1),
            })
        }
        _ => None,
    }
}

fn known_tag(name: &str) -> bool {
    crate::tags::is_known_tag(&name.to_ascii_uppercase())
}

pub fn patch_count(operations: &[PatchOperation]) -> usize {
    operations.len()
}

/// Collect distinct categories present in a pack.
pub fn categories(pack: &CompiledRulePack) -> BTreeSet<String> {
    pack.entities
        .iter()
        .map(|entity| entity.category.as_str().to_string())
        .collect()
}

/// Digest of typed definitions that are not already covered by entities or
/// progressions.  Keeping it coarse avoids coupling `pcgen-core` to every
/// typed field while still invalidating a fingerprint when definitions change.
fn typed_digest(typed: &TypedDefinitions) -> String {
    let mut parts = Vec::new();
    for key in typed.races.keys() {
        parts.push(format!("race:{key}"));
    }
    for key in typed.classes.keys() {
        parts.push(format!("class:{key}"));
    }
    for key in typed.skills.keys() {
        parts.push(format!("skill:{key}"));
    }
    for key in typed.spells.keys() {
        parts.push(format!("spell:{key}"));
    }
    for key in typed.equipment.keys() {
        parts.push(format!("equipment:{key}"));
    }
    for key in typed.equipment_modifiers.keys() {
        parts.push(format!("equipment_modifier:{key}"));
    }
    for key in typed.deities.keys() {
        parts.push(format!("deity:{key}"));
    }
    for key in typed.domains.keys() {
        parts.push(format!("domain:{key}"));
    }
    parts.push(format!("counts:{}", parts.len()));
    pcgen_core::hash_parts(&parts)
}

/// Parse a `PREMULT:` body of the form `N,[pred],[pred],...`.
///
/// Semantics: at least `N` of the bracketed sub-predicates must hold.  Groups
/// may nest (`[PREMULT:2,[..],[..]]`) and may be negated (`[!PRETYPE:1,X]`).
fn parse_premult(body: &str) -> Option<Predicate> {
    let (n_str, rest) = body.split_once(',')?;
    let n: u32 = n_str.trim().parse().ok()?;
    let groups = split_groups(rest)?;
    let items: Vec<Predicate> = groups
        .iter()
        .map(|group| parse_group(group))
        .collect::<Option<Vec<_>>>()?;
    if items.is_empty() {
        return None;
    }
    if n as usize >= items.len() {
        Some(Predicate::All(items))
    } else {
        Some(Predicate::AtLeast { n, items })
    }
}

/// Parse one `[..]` group body into a predicate, handling `!` negation.
fn parse_group(group: &str) -> Option<Predicate> {
    let trimmed = group.trim();
    let (negated, inner) = match trimmed.strip_prefix('!') {
        Some(rest) => (true, rest.trim()),
        None => (false, trimmed),
    };
    let predicate = parse_predicate(inner, "")?;
    if negated {
        Some(Predicate::Not(Box::new(predicate)))
    } else {
        Some(predicate)
    }
}

/// Split `[a],[b,[c]],[d]` into `["a", "b,[c]", "d"]`, respecting nesting.
fn split_groups(input: &str) -> Option<Vec<String>> {
    let mut groups = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for ch in input.chars() {
        match ch {
            '[' => {
                if depth > 0 {
                    current.push(ch);
                }
                depth += 1;
            }
            ']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    groups.push(current.trim().to_string());
                    current.clear();
                } else {
                    current.push(ch);
                }
            }
            _ if depth > 0 => current.push(ch),
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    Some(groups)
}

/// True if `tag_name` is consumed during lowering for `category`, so it should
/// not also be reported as a legacy (un-lowered) tag.
fn consumed_legacy_tag(category: &str, tag_name: &str) -> bool {
    let name = tag_name.to_ascii_uppercase();
    match category {
        "ABILITY" | "ABILITYCATEGORY" => name == "CATEGORY",
        // Equipment modifier metadata captured by the typed model.
        "EQUIPMOD" => matches!(name.as_str(), "PLUS" | "ITYPE" | "REPLACES"),
        // Weapon/armor metadata captured by the typed model.
        "EQUIPMENT" => matches!(name.as_str(), "CRITMULT" | "CRITRANGE" | "WIELD"),
        _ => false,
    }
}
