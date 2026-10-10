//! Lowering of raw records into the runtime `CompiledRulePack`.

pub use pcgen_core::pack::{CompiledPackHeader, CompiledRulePack, ImportReport};

use crate::patches::PatchOperation;
use crate::source::{RawRecord, RawTag};
use crate::typed::{build_typed_definitions, parse_creature_traits};
use pcgen_core::domain::SpellPowerType;
use pcgen_core::GrantMode;
use pcgen_core::{
    CompiledEntity, Effect, EffectAction, EffectPhase, EntityId, Modifier, ModifierOperation,
    Predicate, Provenance, Rational, RuleCoverage, TemporaryValue, TypedDefinitions,
};
use std::collections::BTreeSet;

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
        // `SUBCLASS:` / `SUBCLASSLEVEL:` rows sit between a class header and its
        // level rows and refine the current class.  They are not headers of
        // their own: treating them as such moved `current_class` onto the
        // specialty, so every following level row was keyed
        // `<specialty>#<level>` instead of `<class>#<level>` and its class
        // features never activated for the class.  The RSRD Wizard, whose eight
        // schools precede its level rows, lost every class feature that way.
        let is_class_header = crate::typed::is_class_header_row(record)
            && !record.name.contains(".MOD")
            && !record.name.contains(".COPY=");
        if is_class_header {
            current_class = Some(record.key.clone());
        }
        let (effective_category, effective_key) =
            if category == "CLASS" && crate::typed::class_level_of_name(&record.name).is_some() {
                let level = record.name.trim().split(':').next().unwrap_or("").trim();
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

        // `TYPE:Archetype` / `TYPE:Substitution*` marks an ABILITY as a
        // variant; `parse_variant` then consumes its `APPLIESTO`, `REPLACES`
        // and `GRANTS` tags.  Those must not also be reported as legacy.
        let variant_record = effective_category.eq_ignore_ascii_case("ABILITY")
            && crate::typed::is_variant_record(record);
        let is_consumed = |tag_name: &str| {
            consumed_legacy_tag(&effective_category, tag_name)
                || (variant_record
                    && matches!(
                        tag_name.to_ascii_uppercase().as_str(),
                        "APPLIESTO" | "REPLACES" | "GRANTS"
                    ))
        };

        let legacy_tags: Vec<String> = record
            .tags
            .iter()
            .filter(|tag| !known_tag(&tag.name) && !is_consumed(&tag.name))
            .map(|tag| format!("{}:{}", tag.name, tag.value))
            .collect();
        for tag in record
            .tags
            .iter()
            .filter(|tag| !known_tag(&tag.name) && !is_consumed(&tag.name))
        {
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
        let mut temporary_value = None;
        for tag in &record.tags {
            let upper = tag.name.to_ascii_uppercase();
            if upper == "BONUS" {
                match lower_bonus(tag, phase, &provenance, current_class.as_deref()) {
                    Some(effect) => {
                        if condition_has_unsupported_predicate(effect.condition.as_ref()) {
                            unsupported_tags.push(format!("BONUS:{}", tag.value));
                        }
                        effects.push(effect);
                    }
                    None => unsupported_tags.push(format!("BONUS:{}", tag.value)),
                }
            } else if upper == "TEMPBONUS" {
                match lower_temp_bonus(record, tag, phase, &provenance, current_class.as_deref()) {
                    Some(effect) => {
                        if condition_has_unsupported_predicate(effect.condition.as_ref()) {
                            unsupported_tags.push(format!("TEMPBONUS:{}", tag.value));
                        }
                        effects.push(effect);
                    }
                    None => unsupported_tags.push(format!("TEMPBONUS:{}", tag.value)),
                }
            } else if upper == "TEMPVALUE" {
                match parse_temporary_value(tag) {
                    Some(value) => temporary_value = Some(value),
                    None => unsupported_tags.push(format!("TEMPVALUE:{}", tag.value)),
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
            } else if upper == "SA" || upper == "SAB" {
                match lower_special_ability(tag, phase, &provenance, &mut effects) {
                    Some(()) => {}
                    None => unsupported_tags.push(format!("{}:{}", tag.name, tag.value)),
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
        let mut traits = pcgen_core::CreatureTraits::default();
        parse_creature_traits(record, &mut traits);
        let metadata = parse_entity_metadata(record);
        entities.push(CompiledEntity {
            id,
            category: pcgen_core::EntityCategory::from_pcgen(&effective_category),
            name: record.name.clone(),
            type_tags: record
                .tags
                .iter()
                .filter(|tag| tag.name.eq_ignore_ascii_case("TYPE"))
                .flat_map(|tag| tag.value.split(['.', ',', '|']))
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect(),
            effects,
            requirements,
            legacy_tags,
            unsupported_tags,
            source: record.source.path.display().to_string(),
            ability_category,
            temporary_value,
            facts: parse_facts(record),
            traits,
            metadata,
        });
    }

    merge_ability_mod_entities(&mut entities);

    let typed = build_typed_definitions(records);
    let typed_digest = typed_digest(&typed);
    let rules_fingerprint = pcgen_core::rules_fingerprint(&pcgen_core::RuleFingerprintInput {
        game_mode: game_mode.as_deref().unwrap_or("unknown"),
        sources: &sources,
        entities: &entities,
        progressions: &typed.progressions,
        classes: &typed.classes,
        variants: &typed.variants,
        typed_digest: &typed_digest,
    });
    let spell_effects = entities
        .iter()
        .filter(|entity| {
            matches!(
                entity.category,
                pcgen_core::EntityCategory::Spell | pcgen_core::EntityCategory::Domain
            )
        })
        .map(|entity| entity.effects.len())
        .sum();
    let psionic_effects = entities
        .iter()
        .filter(|entity| {
            entity.category == pcgen_core::EntityCategory::Spell && spell_is_psionic(&typed, entity)
        })
        .map(|entity| entity.effects.len())
        .sum();
    let pack = CompiledRulePack {
        header: CompiledPackHeader {
            game_mode,
            sources,
            rules_fingerprint,
            data_ref: None,
            // Filled in by the `.pcc` loader, which is the only place that knows
            // the include graph; lowering sees records, not campaigns.
            campaigns: Vec::new(),
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
        kits: typed.kits,
        templates: typed.templates,
        abilities: typed.abilities,
        ability_categories: typed.ability_categories,
        creature_trait_records: typed.creature_trait_records,
        companion_mods: typed.companion_mods,
        sizes: typed.sizes,
        stats: typed.stats,
        alignments: typed.alignments,
        progressions: typed.progressions,
    };
    let report = ImportReport {
        coverage,
        patch_operations: 0,
        diagnostics: 0,
        error_diagnostics: 0,
        spell_effects,
        psionic_effects,
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
    owner: Option<&str>,
) -> Option<Effect> {
    lower_bonus_effect(&tag.value, phase, provenance, owner)
}

fn temp_state_names(record: &RawRecord) -> Vec<String> {
    let mut states = Vec::new();
    for raw in [record.key.as_str(), record.name.as_str()] {
        let value = raw.trim();
        let base = value.strip_suffix(".MOD").unwrap_or(value).trim();
        let normalized = normalized_key(base);
        if !normalized.is_empty() && !states.contains(&normalized) {
            states.push(normalized);
        }
    }
    states
}

fn equipped_condition(qualifier: &str) -> Option<Predicate> {
    let mut any_groups = Vec::new();
    for group in qualifier.split(';') {
        let values: Vec<String> = group
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(normalized_key)
            .collect();
        if values.is_empty() {
            continue;
        }
        let mut all = Vec::new();
        for value in values {
            all.push(Predicate::Equipped {
                slot: "any".to_string(),
                values: vec![value],
            });
        }
        any_groups.push(if all.len() == 1 {
            all.pop().expect("one predicate")
        } else {
            Predicate::All(all)
        });
    }
    match any_groups.len() {
        0 => None,
        1 => any_groups.pop(),
        _ => Some(Predicate::Any(any_groups)),
    }
}

fn combine_conditions(
    active: Option<Predicate>,
    scope: Option<Predicate>,
    existing: Option<Predicate>,
) -> Option<Predicate> {
    let mut all = Vec::new();
    if let Some(predicate) = active {
        all.push(predicate);
    }
    if let Some(predicate) = scope {
        all.push(predicate);
    }
    if let Some(predicate) = existing {
        all.push(predicate);
    }
    match all.len() {
        0 => None,
        1 => all.pop(),
        _ => Some(Predicate::All(all)),
    }
}

fn lower_temp_bonus(
    record: &RawRecord,
    tag: &crate::source::RawTag,
    phase: EffectPhase,
    provenance: &Provenance,
    owner: Option<&str>,
) -> Option<Effect> {
    let (scope, body) = tag.value.split_once('|')?;
    let scope = scope.trim().to_ascii_uppercase();
    if body.trim().is_empty() {
        return None;
    }
    let active = Predicate::ActiveState {
        states: temp_state_names(record),
    };
    let (bonus_body, scope_condition) = if scope == "EQ" {
        let (qualifier, rest) = body.split_once('|')?;
        (rest, equipped_condition(qualifier))
    } else {
        (body, None)
    };
    let mut effect = lower_bonus_effect(bonus_body, phase, provenance, owner)?;
    effect.condition = combine_conditions(Some(active), scope_condition, effect.condition);
    Some(effect)
}

fn parse_temporary_value(tag: &crate::source::RawTag) -> Option<TemporaryValue> {
    let mut value = TemporaryValue::default();
    let mut saw_any = false;
    for part in tag.value.split('|') {
        let Some((key, raw)) = part.split_once('=') else {
            continue;
        };
        match key.trim().to_ascii_uppercase().as_str() {
            "MIN" => {
                value.min = Some(raw.trim().parse::<i32>().ok()?);
                saw_any = true;
            }
            "MAX" => {
                value.max = Some(raw.trim().parse::<i32>().ok()?);
                saw_any = true;
            }
            "TITLE" => {
                value.title = Some(raw.trim().to_string());
                saw_any = true;
            }
            _ => {}
        }
    }
    saw_any.then_some(value)
}

/// Lower a raw `BONUS` value (without the `BONUS:` prefix) into an effect.
pub fn lower_bonus_effect(
    value: &str,
    phase: EffectPhase,
    provenance: &Provenance,
    owner: Option<&str>,
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
    // PCGen's formula dialect is translated here, where the owning class is
    // known, so the engine never has to interpret `CL` / `TL` /
    // `classlevel(...)`.  `classlevel("APPLIEDAS=NONEPIC")` in particular means
    // "this class's level", which is exactly the information that is available
    // now and was guesswork at runtime.
    let raw_value =
        pcgen_core::formula_vocab::normalize_pcgen_formula(parts.get(value_index)?, owner);
    let mut bonus_type = None;
    let mut stack_key = None;
    let mut conditions = Vec::new();
    for part in parts.iter().skip(value_index + 1) {
        if let Some(type_name) = part.strip_prefix("TYPE=") {
            // Preserve the full `TYPE=` expression so `.STACK` policies are
            // visible to the engine's stacking layer; category matching uses
            // `contains`, not equality.
            bonus_type = Some(type_name.trim().to_ascii_lowercase());
        } else if let Some(stack) = part.strip_prefix("STACK=") {
            stack_key = Some(if stack.eq_ignore_ascii_case("YES") || stack == "1" {
                "stack".to_string()
            } else {
                "nonstack".to_string()
            });
        } else if crate::tags::is_predicate_tag(part) {
            // Keep an unreadable gate as a first-class condition instead of
            // dropping the whole bonus.  The engine treats `Unsupported` as
            // satisfied and the raw text is reported through
            // `CompiledEntity::unsupported_tags` by the caller.
            conditions.push(
                parse_predicate(part, "").unwrap_or_else(|| Predicate::Unsupported {
                    raw: part.to_string(),
                }),
            );
        }
    }
    let condition = match conditions.len() {
        0 => None,
        1 => conditions.pop(),
        _ => Some(Predicate::All(conditions)),
    };

    // `BONUS:VAR|A,B|expr` becomes an explicit variable assignment.  The name
    // list is split here, where the tag is still in front of us.
    if first.eq_ignore_ascii_case("VAR") {
        let names: Vec<String> = parts
            .get(1)?
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect();
        if names.is_empty() {
            return None;
        }
        return Some(Effect {
            phase,
            condition: condition.clone(),
            action: EffectAction::ModifyVariable {
                names,
                expression: raw_value,
                condition,
                source: provenance.clone(),
            },
            source: provenance.clone(),
        });
    }

    if let Ok(value) = raw_value.parse::<i32>() {
        let modifier = Modifier {
            target,
            operation: ModifierOperation::Add,
            value: Rational::from(value),
            bonus_type: bonus_type.clone(),
            stack_key: stack_key.clone(),
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
                stack_key,
                condition,
                source: provenance.clone(),
            },
            source: provenance.clone(),
        })
    } else {
        None
    }
}

/// Does this condition carry an unreadable predicate that must stay in the
/// pack's unsupported report even though the effect itself is retained?
fn condition_has_unsupported_predicate(condition: Option<&Predicate>) -> bool {
    condition.is_some_and(predicate_has_unsupported)
}

fn predicate_has_unsupported(predicate: &Predicate) -> bool {
    match predicate {
        Predicate::Unsupported { .. } => true,
        Predicate::All(items) | Predicate::Any(items) | Predicate::AtLeast { items, .. } => {
            items.iter().any(predicate_has_unsupported)
        }
        Predicate::Not(item) => predicate_has_unsupported(item),
        _ => false,
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

/// The pieces of a PCGen `ABILITY` grant row.
///
/// `lower_grant` and the class-level grant list both need these, and used to
/// derive them separately from the same string — the duplicate parse that put
/// raw rows on `ClassDefinition::level_abilities` *and* structured effects on
/// the `ClassLevel` entities.
pub(crate) struct GrantShape<'a> {
    /// Ability pool, e.g. `Wizard Class Feature`; `None` when the row names
    /// only a feature.
    pub pool: Option<&'a str>,
    pub feature: &'a str,
    pub mode: GrantMode,
    /// Replacement anchors from a `!PREABILITY:...TYPE.X` guard.
    pub anchors: Vec<String>,
    /// Pool those anchors are looked up in, e.g. `ACF`.
    pub anchor_category: Option<String>,
    /// Every other predicate, verbatim.
    pub guards: Vec<&'a str>,
}

/// Split a PCGen `ABILITY` grant row.
///
/// Returns `None` when the row names no feature, which is the one case that
/// cannot be represented.
pub(crate) fn parse_grant_shape(value: &str) -> Option<GrantShape<'_>> {
    let parts: Vec<&str> = value.split('|').map(str::trim).collect();
    let mut mode = GrantMode::Automatic;
    for part in &parts {
        if part.eq_ignore_ascii_case("VIRTUAL") || part.eq_ignore_ascii_case("NORMAL") {
            mode = GrantMode::Selectable;
        }
    }
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
    let feature = *meaningful.last()?;
    let pool = (meaningful.len() > 1).then(|| meaningful[0]);

    let mut anchors = Vec::new();
    let mut anchor_category = None;
    let mut guards = Vec::new();
    for part in parts
        .iter()
        .copied()
        .filter(|part| crate::tags::is_predicate_tag(part))
    {
        let body = part.split_once(':').map(|(_, body)| body).unwrap_or("");
        let mut found = Vec::new();
        for piece in body.split(',').map(str::trim) {
            if let Some(anchor) = piece.strip_prefix("TYPE.") {
                found.push(anchor.to_string());
            }
            if let Some(category) = piece.strip_prefix("CATEGORY=") {
                anchor_category = Some(category.to_string());
            }
        }
        if found.is_empty() {
            guards.push(part);
        } else {
            anchors.extend(found);
        }
    }

    Some(GrantShape {
        pool,
        feature,
        mode,
        anchors,
        anchor_category,
        guards,
    })
}

fn lower_grant(record: &RawRecord, tag: &crate::source::RawTag) -> Option<Effect> {
    let parts: Vec<&str> = tag.value.split('|').map(str::trim).collect();
    let first = *parts.first()?;
    if first.eq_ignore_ascii_case("WEAPONPROF")
        || first.eq_ignore_ascii_case("ARMORPROF")
        || first.eq_ignore_ascii_case("LANG")
        || first.eq_ignore_ascii_case("LANGBONUS")
    {
        // A predicate in the row is the grant's gate, not one of its values.
        // `AUTO:WEAPONPROF|TYPE=Martial|PREHD:MAX=0` used to fold both into the
        // tag, so `TYPE=Martial` and `PREHD:MAX=0` were inserted as weapon
        // proficiency *names* and the HD gate was never evaluated — it granted
        // martial proficiency to a 12th-level character.
        let mut conditions = Vec::new();
        let mut all_parsed = true;
        for part in parts
            .iter()
            .skip(1)
            .filter(|part| crate::tags::is_predicate_tag(part))
        {
            match parse_predicate(part, "") {
                Some(predicate) => conditions.push(predicate),
                None => all_parsed = false,
            }
        }
        let body = parts
            .iter()
            .skip(1)
            .copied()
            .filter(|part| !part.is_empty() && !crate::tags::is_predicate_tag(part))
            .collect::<Vec<_>>()
            .join("|");
        let action = if body.is_empty() {
            first.to_ascii_uppercase()
        } else {
            format!("{}|{}", first.to_ascii_uppercase(), body)
        };
        // An unparseable gate must not become an ungated grant, so keep the
        // previous conservative behaviour of leaving the condition unknown.
        let condition = if !all_parsed || conditions.is_empty() {
            None
        } else if conditions.len() == 1 {
            conditions.pop()
        } else {
            Some(Predicate::All(conditions))
        };
        return Some(Effect {
            phase: EffectPhase::Global,
            condition,
            action: EffectAction::GrantTag { tag: action },
            source: Provenance::new(
                record.source.path.display().to_string(),
                "AUTO proficiency lowering",
            ),
        });
    }
    // One parse, shared with the class-level grant list.
    let shape = parse_grant_shape(&tag.value)?;
    let key = shape.feature.to_string();
    let category = shape.pool.unwrap_or("Ability").to_string();
    let mut conditions = Vec::new();
    let mut all_conditions_parsed = true;
    for part in parts
        .iter()
        .copied()
        .filter(|part| crate::tags::is_predicate_tag(part))
    {
        match parse_predicate(part, "") {
            Some(predicate) => conditions.push(predicate),
            // Preserve the previous lowering behavior for unsupported
            // embedded predicates: keep the grant rather than dropping it
            // from the lowered set.  The unsupported predicate is still
            // visible through the source record and can be tightened later.
            None => all_conditions_parsed = false,
        }
    }
    let condition = if !all_conditions_parsed {
        None
    } else {
        match conditions.len() {
            0 => None,
            1 => conditions.pop(),
            _ => Some(Predicate::All(conditions)),
        }
    };
    Some(Effect {
        phase: EffectPhase::Global,
        condition,
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

/// `TYPE=X` or `TYPE.X`, case-insensitively — both spellings appear in the data.
/// Classify one `PRECLASS`-family entry's name at the given level.
///
/// `PRECLASS` and `PRECLASSLEVELMAX` share the vocabulary — a class name,
/// `TYPE.X` / `TYPE=X`, `SPELLCASTER[.kind]` and `ANY` — and differ only in what
/// the level means.
fn class_spec_from(name: &str, level: u32) -> Option<pcgen_core::ClassSpec> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let upper = name.to_ascii_uppercase();
    if upper == "ANY" {
        return Some(pcgen_core::ClassSpec::Any { min_level: level });
    }
    if let Some(rest) = upper.strip_prefix("SPELLCASTER") {
        // `SPELLCASTER` alone, or `SPELLCASTER.Arcane`.
        let kind = rest.trim_start_matches(['.', '=']).trim();
        return Some(pcgen_core::ClassSpec::Spellcaster {
            kind: (!kind.is_empty()).then(|| kind.to_string()),
            min_level: level,
        });
    }
    if let Some(kind) = strip_type_prefix(name) {
        return Some(pcgen_core::ClassSpec::Type {
            type_tag: kind.to_string(),
            min_level: level,
        });
    }
    Some(pcgen_core::ClassSpec::Class {
        class_id: name.to_string(),
        min_level: level,
    })
}

fn strip_type_prefix(value: &str) -> Option<&str> {
    let lower = value.to_ascii_lowercase();
    for prefix in ["type=", "type."] {
        if lower.starts_with(prefix) {
            let kind = value[prefix.len()..].trim();
            if !kind.is_empty() {
                return Some(kind);
            }
        }
    }
    None
}

/// One value of a `PRERACE` list.
fn parse_race_spec(value: &str) -> pcgen_core::RaceSpec {
    let trimmed = value.trim();
    for (prefix, make) in [
        (
            "TYPE=",
            pcgen_core::RaceSpec::TypeTag as fn(String) -> pcgen_core::RaceSpec,
        ),
        ("RACETYPE=", pcgen_core::RaceSpec::CreatureType),
        ("RACESUBTYPE=", pcgen_core::RaceSpec::Subtype),
    ] {
        if trimmed.len() > prefix.len() && trimmed[..prefix.len()].eq_ignore_ascii_case(prefix) {
            return make(trimmed[prefix.len()..].trim().to_string());
        }
    }
    if trimmed.contains('%') {
        return pcgen_core::RaceSpec::Pattern(trimmed.to_string());
    }
    pcgen_core::RaceSpec::Name(trimmed.to_string())
}

/// Parse the `x,y,y` body of `PREWEAPONPROF` / `PREWIELD` / `PREDOMAIN`.
///
/// PCGen's tag reference for `PREWEAPONPROF` lists `Text` (a proficiency name)
/// and `TYPE.Text` (a proficiency *type*) as valid values. A type used to be
/// dropped here, which emptied the list and made the caller discard the whole
/// predicate — so `PREWEAPONPROF:1,TYPE=Martial` required nothing at all.
///
/// `TYPE=` and `TYPE.` are both normalised to the `type.<kind>` marker that
/// `seed_weapon_bonus` writes, so the two sides agree.
fn parse_qualified_names(body: &str) -> Vec<String> {
    body.split([',', '|'])
        .map(str::trim)
        .filter(|part| !part.is_empty() && part.parse::<u32>().is_err())
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            match lower
                .strip_prefix("type=")
                .or_else(|| lower.strip_prefix("type."))
            {
                Some(kind) => format!("type.{}", kind.trim()),
                None => lower,
            }
        })
        .collect()
}

/// Parse `ADDDOMAINS:Travel|Sun` or `Sun[PREDIETY:Ra]|Law`.
///
/// PCGen's tag reference: `ADDDOMAINS:x|x`, where each `x` is a domain name and
/// "it can contain PRExxx tags, in brackets ([PRExxx]), immediately after the
/// domain name". So `|` separates domains and `[...]` qualifies the one before
/// it — reading the whole value as a single domain produced a domain literally
/// named `Travel|Sun`, which matches nothing.
pub(crate) fn parse_added_domains(value: &str) -> Vec<pcgen_core::AddedDomain> {
    value
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(parse_one_added_domain)
        .collect()
}

fn parse_one_added_domain(value: &str) -> pcgen_core::AddedDomain {
    let value = value.trim();
    let (domain, condition) = match value.split_once('[') {
        Some((domain, condition)) => (domain.trim(), condition.trim().trim_end_matches(']').trim()),
        None => (value, ""),
    };
    pcgen_core::AddedDomain {
        domain: domain.to_string(),
        condition: (!condition.is_empty())
            .then(|| parse_predicate(condition, ""))
            .flatten(),
    }
}

/// Parse `Ex Paladin|11|10|1`.
///
/// `w|x|y|z` per PCGen's tag reference: the donating class, the level it must
/// reach before anything may be exchanged, the most levels that may be donated,
/// and the lowest it may be reduced to.
pub(crate) fn parse_exchange_level(value: &str) -> Option<pcgen_core::ExchangeLevel> {
    let parts: Vec<&str> = value.split('|').map(str::trim).collect();
    let donating_class = parts.first()?.to_string();
    if donating_class.is_empty() {
        return None;
    }
    let number =
        |index: usize| -> u32 { parts.get(index).and_then(|p| p.parse().ok()).unwrap_or(0) };
    Some(pcgen_core::ExchangeLevel {
        donating_class,
        minimum_level: number(1),
        maximum_levels: number(2),
        floor_level: number(3),
    })
}

/// Parse `Claw,...,*2,1d8|Bite,...,*1,1d8` into one entry per attack.
///
/// `|` separates attacks within one `NATURALATTACKS` tag, and `,` separates the
/// fields of each.  Reading the whole value as a single attack let the second
/// attack's `*1` and `1d8` overwrite the first's, so a bear came out with one
/// claw at a single die instead of two claws and a bite.
pub(crate) fn parse_natural_attacks(value: &str) -> Vec<pcgen_core::NaturalAttack> {
    value
        .split('|')
        .filter(|part| !part.trim().is_empty())
        .map(parse_one_natural_attack)
        .collect()
}

fn parse_one_natural_attack(value: &str) -> pcgen_core::NaturalAttack {
    let fields: Vec<&str> = value.split(',').map(str::trim).collect();
    let name = fields.first().copied().unwrap_or_default().to_string();
    let mut types = Vec::new();
    let mut count = 1;
    let mut damage = String::new();
    let mut special_property = None;
    for field in fields.iter().skip(1) {
        if let Some(description) = field
            .strip_prefix("SPROP=")
            .or_else(|| field.strip_prefix("sprop="))
        {
            special_property = Some(description.trim().to_string());
        } else if let Some(value) = field.strip_prefix('*') {
            count = value.parse().unwrap_or(1);
        } else if field.chars().next().is_some_and(|c| c.is_ascii_digit()) && field.contains('d') {
            damage = (*field).to_string();
        } else if !field.is_empty() {
            types.push((*field).to_string());
        }
    }
    pcgen_core::NaturalAttack {
        name,
        count,
        damage,
        types,
        special_property,
    }
}

/// Parse `SCHOOL.Abjuration|PREVARGTEQ:ProhibitAbjuration,1`.
///
/// Returns `None` for a target we do not model, rather than keeping the raw
/// string: an entry nobody can act on is worse than an absent one.
pub(crate) fn parse_prohibited_spell(value: &str) -> Option<pcgen_core::ProhibitedSpell> {
    let mut parts = value.split('|').map(str::trim).filter(|p| !p.is_empty());
    let target = parts.next()?;
    // Alignment, descriptor, school and subschool names are period-delimited;
    // spell names are comma-delimited.
    let dotted = |rest: &str| -> Vec<String> {
        rest.split('.')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    };
    let listed = |rest: &str| -> Vec<String> {
        rest.split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    };
    let target = match target.split_once('.') {
        Some(("ALIGNMENT", rest)) => pcgen_core::ProhibitionTarget::Alignment(dotted(rest)),
        Some(("DESCRIPTOR", rest)) => pcgen_core::ProhibitionTarget::Descriptor(dotted(rest)),
        Some(("SUBSCHOOL", rest)) => pcgen_core::ProhibitionTarget::Subschool(dotted(rest)),
        Some(("SPELL", rest)) => pcgen_core::ProhibitionTarget::Spell(listed(rest)),
        Some(("SCHOOL", rest)) => pcgen_core::ProhibitionTarget::School(rest.trim().to_string()),
        _ => return None,
    };
    let condition = parts.find_map(|part| parse_predicate(part, ""));
    Some(pcgen_core::ProhibitedSpell { target, condition })
}

/// Split `5/Magic|PRELEVEL:MIN=4` into a value and its gate.
///
/// The gate was never evaluated while this was a `String`, so a level-1
/// Celestial reported both of its entries and exposed the raw PCGen text.
pub(crate) fn parse_damage_reduction(value: &str) -> pcgen_core::DamageReduction {
    let mut parts = value.split('|').map(str::trim);
    let reduction = parts.next().unwrap_or_default();
    let condition = parts
        .filter(|part| !part.is_empty())
        .find_map(|part| parse_predicate(part, ""));
    pcgen_core::DamageReduction {
        value: reduction.to_string(),
        condition,
    }
}

/// The record's PCGen `FACT:Name|Value` pairs, for `PREFACT`.
///
/// A fact with no value (`FACT:Name|`) is kept as an empty string, which is what
/// the data means by it.  Later tags win, matching PCGen's `set` on the object.
fn parse_facts(record: &RawRecord) -> std::collections::BTreeMap<String, String> {
    let mut facts = std::collections::BTreeMap::new();
    for tag in &record.tags {
        if !tag.name.eq_ignore_ascii_case("FACT") {
            continue;
        }
        let Some((name, value)) = tag.value.split_once('|') else {
            continue;
        };
        let name = name.trim();
        if !name.is_empty() {
            facts.insert(name.to_string(), value.trim().to_string());
        }
    }
    facts
}

/// Lower an `SA:`/`SAB:` tag into a granted special ability.
///
/// The tag's shape is `<key name>|<display text>|<PRE...>`, where the tokens after
/// the key name are display text until one "looks like a prerequisite" — PCGen's
/// `looksLikeAPrerequisite` is just a `PRE` prefix test — and the rest are
/// prerequisites for the ability.  `SabLst` puts the first token into
/// `new SpecialAbility(firstToken)`, and `PreSpecialAbilityTester` matches
/// `PRESA` against that key name with `startsWith`, which is why the display text
/// and the `%` placeholders do not have to be modelled here.
///
/// `.CLEAR` and `.CLEAR.Text` are per-object list edits, so they are applied to
/// the effects collected for *this* record, in tag order.
fn lower_special_ability(
    tag: &RawTag,
    phase: EffectPhase,
    provenance: &Provenance,
    effects: &mut Vec<Effect>,
) -> Option<()> {
    let mut parts = tag.value.split('|').map(str::trim);
    let head = parts.next().unwrap_or_default();
    let upper = head.to_ascii_uppercase();
    if upper == ".CLEAR" {
        effects.retain(|effect| !matches!(effect.action, EffectAction::GrantSpecialAbility { .. }));
        return Some(());
    }
    // `SAB:.CLEAR.Text` removes the ability whose name matches the text up to its
    // first open parenthesis, and only within this object.  The prefix is checked
    // through `get` so a value that is not `.CLEAR.` cannot split a character.
    let cleared = head
        .get(..7)
        .filter(|prefix| prefix.eq_ignore_ascii_case(".CLEAR."))
        .map(|_| clear_key(&head[7..]));
    if let Some(wanted) = cleared {
        effects.retain(|effect| match &effect.action {
            EffectAction::GrantSpecialAbility { name } => clear_key(name) != wanted,
            _ => true,
        });
        return Some(());
    }
    if head.is_empty() {
        return None;
    }
    let mut conditions = Vec::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if !part.to_ascii_uppercase().starts_with("PRE") {
            // Display text.
            continue;
        }
        if let Some(predicate) = parse_predicate(part, "") {
            conditions.push(predicate);
        }
    }
    let condition = match conditions.len() {
        0 => None,
        1 => conditions.pop(),
        _ => Some(Predicate::All(conditions)),
    };
    effects.push(Effect {
        phase,
        condition,
        action: EffectAction::GrantSpecialAbility {
            name: head.to_string(),
        },
        source: provenance.clone(),
    });
    Some(())
}

/// The text `.CLEAR.Text` compares against: everything before the first `(`.
fn clear_key(text: &str) -> String {
    text.split('(')
        .next()
        .unwrap_or(text)
        .trim()
        .to_ascii_lowercase()
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
    // `PRETEXT:x` is display-only.  PCGen's reference: "This tag in itself does
    // not disqualify anything but alerts the player to the additional
    // pre-requisites."  It is therefore vacuously true, and reading it as an
    // unknown tag discarded every effect it gated — 7,358 occurrences across the
    // third-party packs, mostly `TEMPBONUS:...|PRETEXT:...`.
    if tag_upper == "PRETEXT" {
        return Some(Predicate::All(Vec::new()));
    }
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
        // PRECLASS:x,y=z,y=z — at least x of the listed class requirements.
        // Only the first pair was read and the count was ignored.
        // PREVISION:x,y=z,y=z — at least x of the listed vision types, each at
        // the stated distance (`ANY` for any distance).
        "PREVISION" => {
            let min_count = parts
                .iter()
                .find(|part| !part.trim().is_empty())
                .and_then(|part| part.trim().parse::<u32>().ok())
                .unwrap_or(1);
            let specs: Vec<pcgen_core::VisionSpec> = parts
                .iter()
                .skip_while(|part| part.trim().parse::<u32>().is_ok() || part.trim().is_empty())
                .filter_map(|part| {
                    let (kind, distance) = part.split_once('=')?;
                    let kind = kind.trim();
                    if kind.is_empty() {
                        return None;
                    }
                    Some(pcgen_core::VisionSpec {
                        kind: kind.to_string(),
                        min_distance: if distance.trim().eq_ignore_ascii_case("any") {
                            None
                        } else {
                            Some(distance.trim().parse::<u32>().ok()?)
                        },
                    })
                })
                .collect();
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::Vision { specs, min_count })
        }
        // PRESRGTEQ:10 is "spell resistance of 10 or greater"; PRESREQ:10 is
        // "exactly 10".
        "PRESRGTEQ" | "PRESREQ" | "PRESRLTEQ" => {
            let value = body
                .split(|ch: char| !ch.is_ascii_digit())
                .find(|part| !part.is_empty())
                .and_then(|part| part.parse::<i32>().ok())?;
            Some(match tag_upper.as_str() {
                "PRESRGTEQ" => Predicate::SpellResistance {
                    min: Some(value),
                    max: None,
                },
                "PRESRLTEQ" => Predicate::SpellResistance {
                    min: None,
                    max: Some(value),
                },
                _ => Predicate::SpellResistance {
                    min: Some(value),
                    max: Some(value),
                },
            })
        }
        // PREGENDER:M — the gender must start with the text, case sensitively.
        "PREGENDER" => {
            let prefix = parts
                .iter()
                .map(|part| part.trim())
                .find(|part| !part.is_empty() && part.parse::<u32>().is_err())?
                .to_string();
            Some(Predicate::Gender { prefix })
        }
        "PRECLASS" => {
            let min_count = parts
                .iter()
                .find(|part| !part.trim().is_empty())
                .and_then(|part| part.trim().parse::<u32>().ok())
                .unwrap_or(1);
            let mut specs: Vec<pcgen_core::ClassSpec> = Vec::new();
            for part in parts
                .iter()
                .skip_while(|part| part.trim().parse::<u32>().is_ok() || part.trim().is_empty())
            {
                let trimmed = part.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // The reference splits on every `=` and takes the value from the
                // last segment. `PRECLASS:1,Bard` has no `=` at all: it is valid,
                // and the level falls back to the count, which is how the Babylon 5
                // and Star Wars templates are written.
                let (name, min_level) = match trimmed.rsplit_once('=') {
                    Some((name, value)) => match value.trim().parse::<u32>() {
                        Ok(level) => (name.trim().to_string(), level),
                        // `=9th`, `=1st+ level.`: the reference reads the whole
                        // part as the key and the level as the count, yielding a
                        // key that can never name a class. Flag it rather than
                        // transcribe the nonsense.
                        Err(_) => return None,
                    },
                    None => (trimmed.to_string(), min_count),
                };
                specs.push(class_spec_from(&name, min_level)?);
            }
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::ClassRequirement { specs, min_count })
        }
        // PRECLASSLEVELMAX:x,y=z — at most `z` levels in each listed class.
        //
        // There is no tester for this tag: `PreClassLevelMaxParser` rewrites it
        // into the *inverse* of `PRECLASS`, changing the kind to `class`, setting
        // the operator to `LT` and adding one to the operand — so
        // `PRECLASSLEVELMAX:Barbarian=1` is `!PRECLASS:Barbarian=2` — and clearing
        // `countMultiples` so each entry contributes 0 or 1 to the group's count.
        // Modelling it as the negated `ClassRequirement` at `z + 1` reproduces
        // that without a second tester.
        "PRECLASSLEVELMAX" => {
            let min_count = parts
                .iter()
                .find(|part| !part.trim().is_empty())
                .and_then(|part| part.trim().parse::<u32>().ok())
                .unwrap_or(1);
            let mut items = Vec::new();
            for part in parts
                .iter()
                .skip_while(|part| part.trim().parse::<u32>().is_ok() || part.trim().is_empty())
            {
                let trimmed = part.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // The level is required here.  A pair without one is not modelled:
                // the parser's own comment and its generic list parser disagree
                // about what the operand would be, and the third-party data always
                // states one, so guessing would be worse than staying visible.
                let (name, level) = trimmed.rsplit_once('=')?;
                let level = level.trim().parse::<u32>().ok()?;
                items.push(Predicate::Not(Box::new(Predicate::ClassRequirement {
                    specs: vec![class_spec_from(name, level.saturating_add(1))?],
                    min_count: 1,
                })));
            }
            if items.is_empty() {
                return None;
            }
            Some(Predicate::AtLeast {
                n: min_count,
                items,
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
            let Ok(value) = value.parse::<i32>() else {
                // Symbolic form such as `PREVARGT:%CHOICE,STR`.
                return match tag_upper.as_str() {
                    "PREVARGT" | "PREVARGTEQ" | "PREVARLT" | "PREVARLTEQ" => {
                        Some(Predicate::VariableCompare {
                            left: name,
                            right: value,
                            greater: matches!(tag_upper.as_str(), "PREVARGT" | "PREVARGTEQ"),
                            // The `GT`/`GTEQ` and `LT`/`LTEQ` pairs differ only in
                            // the boundary, and this path used to lose it.
                            strict: matches!(tag_upper.as_str(), "PREVARGT" | "PREVARLT"),
                        })
                    }
                    "PREVAREQ" | "PREVARNEQ" => Some(Predicate::VariableEquals {
                        left: name,
                        right: value,
                        equals: tag_upper == "PREVAREQ",
                    }),
                    _ => None,
                };
            };
            let (min, max) = match tag_upper.as_str() {
                "PREVARGTEQ" => (Some(value), None),
                "PREVARLTEQ" => (None, Some(value)),
                // The strict forms become the inclusive bound one step in,
                // which is exact for the integer variables this engine models
                // (`x > 5` is `x >= 6`).  PCGen's own variables are floats, where
                // the two are not the same, so this shift is only sound because
                // `EffectState::variables` is an integer map.
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
        "PREEQUIP" | "PREEQUIPPRIMARY" | "PREEQUIPSECONDARY" | "PREEQUIPBOTH" => {
            let slot = match tag_upper.as_str() {
                "PREEQUIPPRIMARY" => "primary",
                "PREEQUIPSECONDARY" => "secondary",
                "PREEQUIPBOTH" => "both",
                _ => "any",
            };
            let values: Vec<String> = parts
                .iter()
                .skip(1)
                .map(|part| part.trim())
                .filter(|part| !part.is_empty() && part.parse::<u32>().is_err())
                .map(|part| part.to_ascii_lowercase())
                .collect();
            if values.is_empty() {
                None
            } else {
                Some(Predicate::Equipped {
                    slot: slot.to_string(),
                    values,
                })
            }
        }
        "PREDEITY" => {
            let min_count = parts.first()?.trim().parse::<u32>().ok()?;
            let names = parts
                .iter()
                .skip(1)
                .map(|part| {
                    part.trim()
                        .trim_end_matches(']')
                        .trim()
                        .to_ascii_lowercase()
                })
                .filter(|part| !part.is_empty() && part.parse::<u32>().is_err())
                .collect::<std::collections::BTreeSet<_>>();
            if names.is_empty() {
                return None;
            }
            Some(Predicate::Deity {
                names,
                min_count: min_count.max(1),
            })
        }
        "PREWEAPONPROF" | "PREWIELD" | "PREDOMAIN" => {
            let values = parse_qualified_names(body);
            if values.is_empty() {
                return None;
            }
            match tag_upper.as_str() {
                "PREWEAPONPROF" => Some(Predicate::WeaponProf { names: values }),
                "PREWIELD" => Some(Predicate::Wield { categories: values }),
                _ => Some(Predicate::Domain { names: values }),
            }
        }
        "PREHD" => {
            let mut min: Option<u32> = None;
            let mut max: Option<u32> = None;
            for part in parts
                .iter()
                .map(|part| part.trim())
                .filter(|part| !part.is_empty())
            {
                if let Some((key, value)) = part.split_once('=') {
                    let value = value.trim().parse::<u32>().ok()?;
                    match key.trim().to_ascii_uppercase().as_str() {
                        "MIN" => min = Some(value),
                        "MAX" => max = Some(value),
                        _ => return None,
                    }
                } else {
                    min = Some(part.parse::<u32>().ok()?);
                }
            }
            if min.is_none() && max.is_none() {
                None
            } else {
                Some(Predicate::HitDice { min, max })
            }
        }
        "PRESPELL" => {
            let names: Vec<String> = parts
                .iter()
                .skip(1)
                .map(|part| part.trim())
                .filter(|part| !part.is_empty() && part.parse::<u32>().is_err())
                .map(str::to_string)
                .collect();
            if names.is_empty() {
                None
            } else {
                Some(Predicate::KnownSpell { names })
            }
        }
        // PREDR:count,Type=Value,Type=Value — at least `count` of the listed
        // damage reductions hold.
        //
        // `PreDamageReductionParser` tokenises each pair on `=` *or* `.`, so
        // `Good=10` and `Good.10` are the same, and a pair with no value defaults
        // to 0.  A value that is not an integer — including one with a `|PRE`
        // suffix, which the reference's `Integer.parseInt` also rejects — leaves
        // the whole tag unsupported rather than being guessed at.
        "PREDR" => {
            let (count_str, rest) = body.split_once(',')?;
            let min_count = count_str.trim().parse::<u32>().ok()?;
            let specs: Vec<pcgen_core::DamageReductionSpec> = rest
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    let (kind, value) = match (part.find('='), part.find('.')) {
                        (Some(eq), Some(dot)) if dot < eq => (&part[..dot], &part[dot + 1..]),
                        (Some(eq), _) => (&part[..eq], &part[eq + 1..]),
                        (None, Some(dot)) => (&part[..dot], &part[dot + 1..]),
                        (None, None) => (part, ""),
                    };
                    let kind = kind.trim();
                    if kind.is_empty() {
                        return None;
                    }
                    let value = value.trim();
                    let min = if value.is_empty() {
                        0
                    } else {
                        value.parse::<i32>().ok()?
                    };
                    Some(pcgen_core::DamageReductionSpec {
                        kind: kind.to_string(),
                        min,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::DamageReduction {
                specs,
                min_count: min_count.max(1),
            })
        }
        // PREFACT:count,location,Fact=Value[,Fact=Value...] — at least `count`
        // objects in `location` have the named fact set to the given value.
        //
        // Only the locations the runtime models are lowered: `PreFactParser`
        // checks the location against `OutputDB`, whose names are facet-backed
        // iterables, and claiming a location we cannot enumerate would turn a
        // prerequisite into a silent "no".  Anything else stays unsupported so it
        // is visible.  A value containing `|` is malformed — the parser throws on
        // it — and is likewise left unsupported rather than truncated.
        "PREFACT" | "PREFACTSET" => {
            let (count_str, rest) = body.split_once(',')?;
            let min_count = count_str.trim().parse::<u32>().ok()?;
            let (location, specs_str) = rest.split_once(',')?;
            let location = location.trim().to_ascii_lowercase();
            if location != "abilities" && location != "race" {
                return None;
            }
            let specs: Vec<pcgen_core::FactSpec> = specs_str
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    let (name, value) = part.split_once('=')?;
                    let name = name.trim();
                    if name.is_empty() || value.contains('|') {
                        return None;
                    }
                    Some(pcgen_core::FactSpec {
                        name: name.to_string(),
                        value: value.trim().to_string(),
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::Fact {
                location,
                specs,
                min_count: min_count.max(1),
            })
        }
        // PRESA:count,name,name — the character has special abilities whose names
        // start with these texts.  `PreSpecialAbilityTester` counts, per listed
        // text, how many of the character's abilities start with it, and the list
        // wrapper sums those counts.
        "PRESA" => {
            let (count_str, rest) = body.split_once(',')?;
            let min_count = count_str.trim().parse::<u32>().ok()?;
            let prefixes: Vec<String> = rest
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect();
            if prefixes.is_empty() {
                return None;
            }
            Some(Predicate::SpecialAbility {
                prefixes,
                min_count: min_count.max(1),
            })
        }
        // PRECHARACTERTYPE:count,type,type — the character's type is one of them.
        //
        // `PreCharactertypeParser` is a list parser, so the leading number is the
        // count; `PreCharactertypeTester` compares the character type with each
        // listed key and the wrapper sums the matches.
        "PRECHARACTERTYPE" => {
            let min_count = parts
                .iter()
                .find(|part| !part.trim().is_empty())
                .and_then(|part| part.trim().parse::<u32>().ok())
                .unwrap_or(1);
            let types: Vec<String> = parts
                .iter()
                .skip_while(|part| part.trim().parse::<u32>().is_ok() || part.trim().is_empty())
                .map(|part| part.trim())
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect();
            if types.is_empty() {
                return None;
            }
            Some(Predicate::CharacterType { types, min_count })
        }
        // PRECAMPAIGN:count,y,y — at least `count` of the listed data sets are
        // loaded.
        //
        // The selectors are not `name=value` pairs in the usual sense: PCGen's
        // list parser sees `BOOKTYPE=Core Rules`, fails to read `Core Rules` as a
        // number, and keeps the whole `BOOKTYPE=Core Rules` as the key with the
        // count as the operand.  So the prefix is the selector kind and the rest
        // is its argument.
        "PRECAMPAIGN" => {
            let (count_str, rest) = body.split_once(',')?;
            let min_count = count_str.trim().parse::<u32>().ok()?;
            let specs: Vec<pcgen_core::CampaignSpec> = rest
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    // Longest prefix first: `INCLUDES=` is not a prefix of
                    // `INCLUDESBOOKTYPE=`, but reading it first is clearer.
                    for (prefix, build) in [
                        (
                            "INCLUDESBOOKTYPE=",
                            pcgen_core::CampaignSpec::IncludesBookType
                                as fn(String) -> pcgen_core::CampaignSpec,
                        ),
                        ("INCLUDES=", pcgen_core::CampaignSpec::Includes),
                        ("BOOKTYPE=", pcgen_core::CampaignSpec::BookType),
                    ] {
                        if let Some(value) = part
                            .get(..prefix.len())
                            .filter(|head| head.eq_ignore_ascii_case(prefix))
                            .map(|_| part[prefix.len()..].trim())
                            .filter(|value| !value.is_empty())
                        {
                            return build(value.to_string());
                        }
                    }
                    pcgen_core::CampaignSpec::Key(part.to_string())
                })
                .collect();
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::Campaign { specs, min_count })
        }
        // PRESPELLSCHOOL:count,school=level — at least `count` known spells are
        // of the listed schools at or above the stated levels.
        //
        // Three things were wrong here.  The count was skipped and the entries
        // were joined with `all()`, so "at least x" became "every one of them";
        // the school was matched against the spell's *name* (see §5z), not its
        // `SCHOOL:`; and the value was taken from the first `=` rather than the
        // last.  `PreSpellSchoolParser` is the same list parser as
        // `PRESPELLSCHOOLSUB`, so this now lowers to the same shape.
        "PRESPELLSCHOOL" => {
            let (count_str, rest) = body.split_once(',')?;
            let count = count_str.trim().parse::<u32>().ok()?;
            let specs: Vec<pcgen_core::SpellSchoolSpec> = rest
                .split(',')
                .filter_map(|pair| {
                    let (school, level) = pair.rsplit_once('=')?;
                    let school = school.trim();
                    if school.is_empty() {
                        return None;
                    }
                    Some(pcgen_core::SpellSchoolSpec {
                        school: school.to_string(),
                        min_level: level.trim().parse::<u32>().ok()?,
                    })
                })
                .collect();
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::SpellSchool {
                specs,
                count: count.max(1),
            })
        }
        "PRESUBCLASS" | "PRELANG" => {
            let values: Vec<String> = parts
                .iter()
                .skip(1)
                .map(|part| part.trim())
                .filter(|part| !part.is_empty() && part.parse::<u32>().is_err())
                .map(str::to_string)
                .collect();
            if values.is_empty() {
                return None;
            }
            if tag_upper == "PRESUBCLASS" {
                Some(Predicate::Subclass { names: values })
            } else {
                Some(Predicate::Language { names: values })
            }
        }
        "PREARMORTYPE" | "PREPROFWITHARMOR" | "PREPROFWITHSHIELD" => {
            let values: Vec<String> = parts
                .iter()
                .skip(1)
                .map(|part| part.trim())
                .filter(|part| !part.is_empty() && part.parse::<u32>().is_err())
                .map(|part| part.to_ascii_lowercase())
                .collect();
            if values.is_empty() {
                return None;
            }
            match tag_upper.as_str() {
                "PREARMORTYPE" => Some(Predicate::ArmorType { types: values }),
                "PREPROFWITHARMOR" => Some(Predicate::ArmorProf { categories: values }),
                _ => Some(Predicate::ShieldProf { categories: values }),
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
        // PREABILITY:x,CATEGORY=y,z — at least x of the wanted abilities are
        // held and none of the bracketed exclusions are.
        "PREABILITY" | "PREFEAT" => {
            let category = if tag_upper == "PREFEAT" {
                "Feat".to_string()
            } else {
                parts
                    .iter()
                    .find_map(|part| {
                        let trimmed = part.trim();
                        let (name, value) = trimmed.split_once('=')?;
                        name.eq_ignore_ascii_case("CATEGORY")
                            .then(|| value.trim().to_string())
                    })
                    .unwrap_or_else(|| "Ability".to_string())
            };
            let min_count = parts
                .iter()
                .find(|part| !part.trim().is_empty())
                .and_then(|part| part.trim().parse::<u32>().ok())
                .unwrap_or(1);
            let mut wanted = Vec::new();
            let mut excluded = Vec::new();
            for part in parts
                .iter()
                .skip_while(|part| part.trim().parse::<u32>().is_ok() || part.trim().is_empty())
            {
                let trimmed = part.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // `CHECKMULT` is a modifier flag, not an ability name.
                if trimmed.eq_ignore_ascii_case("CHECKMULT") {
                    continue;
                }
                // `CATEGORY=x` / `CATEGORY.x` is the qualifier already read above.
                let upper = trimmed.to_ascii_uppercase();
                if upper.starts_with("CATEGORY=") || upper.starts_with("CATEGORY.") {
                    continue;
                }
                // `TYPE=X` and `TYPE.X` name an ability type.
                if let Some(kind) = strip_type_prefix(trimmed) {
                    wanted.push(pcgen_core::AbilityRef::Type(kind.to_string()));
                    continue;
                }
                // `[Name]` is an exclusion, per the reference's examples.
                if let Some(name) = trimmed.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                    if !name.trim().is_empty() {
                        excluded.push(name.trim().to_string());
                    }
                    continue;
                }
                // An ability name may itself contain `=`: `Weapon Focus (TYPE=Bow)`
                // is one name, and the reference splits it into key `Weapon Focus`
                // plus sub-key `TYPE=Bow`, then resolves that sub-key against the
                // ability's association list and the weapon proficiency's types.
                // That is not modelled here, so the text is kept verbatim: it will
                // fail to match rather than silently match any `Weapon Focus`.
                wanted.push(pcgen_core::AbilityRef::Name(trimmed.to_string()));
            }
            if wanted.is_empty() && excluded.is_empty() {
                return None;
            }
            Some(Predicate::AbilityRequirement {
                category,
                wanted,
                excluded,
                min_count,
            })
        }
        // PRESKILL:x,y=z,y=z — at least x of the listed skills have z ranks.
        // Only the last pair was read, and `TYPE.` was not understood.
        // PREUATT:x — the largest base attack bonus any single class contributes.
        //
        // The reference calls it an "unarmed attack bonus" and the tester is named
        // `PreUnarmedAttackTester`, but the body is `Math.max` over each class's own
        // `baseAttackBonus`, so it is a per-class maximum rather than the character's
        // total.  The operand is read with `Integer.parseInt`, so a non-numeric one
        // is malformed.
        "PREUATT" => {
            let min = body.trim().parse::<i32>().ok()?;
            Some(Predicate::HighestClassBab { min })
        }
        // PRESKILLTOT:Spot,Listen,Search=30 — the character's total ranks across
        // the listed skills.
        //
        // `PreSkillTotalParser` splits the formula on commas and reads a `=value`
        // off any token carrying one — the *last* such token wins, and a formula
        // with none leaves the total at 0 — then marks every sub-requirement
        // `totalValues`, which makes `PreSkillTester` add the ranks of the matching
        // skills instead of stopping at the first.  A value that is not a number is
        // malformed: the reference's `Integer.parseInt` throws on it, and the
        // reference splits on the *first* `=`, so a skill name containing one
        // cannot be written here.
        "PRESKILLTOT" => {
            let mut total = 0i32;
            let mut specs: Vec<pcgen_core::SkillSpec> = Vec::new();
            for part in parts.iter().map(|part| part.trim()) {
                if part.is_empty() {
                    continue;
                }
                let (name, value) = match part.split_once('=') {
                    Some((name, value)) => (name.trim(), Some(value.trim())),
                    None => (part, None),
                };
                if let Some(value) = value {
                    total = value.parse::<i32>().ok()?;
                }
                // An empty name is kept: the reference accepts `PRESKILLTOT:=1`
                // and it simply matches no skill, which is a truthful answer and
                // better than calling the tag unknown.
                let (is_type, name) = match strip_type_prefix(name) {
                    Some(kind) => (true, kind.to_string()),
                    None => (false, name.to_string()),
                };
                specs.push(pcgen_core::SkillSpec { name, is_type });
            }
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::SkillTotal { specs, total })
        }
        "PRESKILL" => {
            let min_count = parts
                .iter()
                .find(|part| !part.trim().is_empty())
                .and_then(|part| part.trim().parse::<u32>().ok())
                .unwrap_or(1);
            let mut specs: Vec<pcgen_core::SkillRequirement> = Vec::new();
            for part in parts
                .iter()
                .skip_while(|part| part.trim().parse::<u32>().is_ok() || part.trim().is_empty())
            {
                let trimmed = part.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // The reference splits the part on every `=` and takes the value
                // from the *last* segment, so a skill name may itself contain `=`:
                // `Knowledge (TYPE=Other)=3` is three ranks in the skill named
                // `Knowledge (TYPE=Other)`. When that last segment is not a number
                // the whole part is the key and the value falls back to the count,
                // which is how `TYPE=Knowledge` (three ranks in a Knowledge-type
                // skill) is written.
                let (name, min_ranks) = match trimmed.rsplit_once('=') {
                    Some((name, value)) => match value.trim().parse::<u32>() {
                        Ok(ranks) => (name.trim().to_string(), ranks),
                        Err(_) => (trimmed.to_string(), min_count),
                    },
                    // `PRESKILL` requires a value; a bare name is malformed.
                    None => return None,
                };
                if name.is_empty() {
                    return None;
                }
                let (is_type, name) = match strip_type_prefix(&name) {
                    Some(kind) => (true, kind.to_string()),
                    None => (false, name),
                };
                specs.push(pcgen_core::SkillRequirement {
                    name,
                    is_type,
                    min_ranks,
                });
            }
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::SkillRanks { specs, min_count })
        }
        // PRECSKILL:x,y,y — at least x of the listed skills are class skills.
        //
        // Unlike `PRESKILL` there is no `=z`: the value is just a skill selector,
        // and the question is whether the character's classes make it a class
        // skill.  `PreCSkillParser` is a list parser, so the first part is the
        // count and the rest are names or `TYPE.Text`.
        "PRECSKILL" => {
            let min_count = parts
                .iter()
                .find(|part| !part.trim().is_empty())
                .and_then(|part| part.trim().parse::<u32>().ok())
                .unwrap_or(1);
            let specs: Vec<pcgen_core::SkillSpec> = parts
                .iter()
                .skip_while(|part| part.trim().parse::<u32>().is_ok() || part.trim().is_empty())
                .map(|part| part.trim())
                .filter(|part| !part.is_empty())
                .map(|part| match strip_type_prefix(part) {
                    Some(kind) => pcgen_core::SkillSpec {
                        name: kind.to_string(),
                        is_type: true,
                    },
                    None => pcgen_core::SkillSpec {
                        name: part.to_string(),
                        is_type: false,
                    },
                })
                .collect();
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::ClassSkill { specs, min_count })
        }
        // PRERACE:x,y,y — at least x of the listed specs must describe the race.
        // The reference allows a name, the `%` wildcard, `TYPE=`, `RACETYPE=`
        // and `RACESUBTYPE=`. Only names were handled, so 1,141 of the RSRD's
        // 2,131 values could never match — including 929 bare `%`, which the
        // reference defines as matching any race.
        "PRERACE" => {
            let min_count = parts
                .iter()
                .find(|part| !part.trim().is_empty())
                .and_then(|part| part.trim().parse::<u32>().ok())
                .unwrap_or(1);
            let specs: Vec<pcgen_core::RaceSpec> = parts
                .iter()
                .skip_while(|part| part.trim().parse::<u32>().is_ok() || part.trim().is_empty())
                .map(|part| part.trim().trim_matches(['[', ']']).trim())
                .filter(|part| !part.is_empty())
                .map(parse_race_spec)
                .collect();
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::RaceMatches { specs, min_count })
        }
        "PREDEITYALIGN" => {
            let allowed = parts
                .iter()
                .filter(|part| !part.is_empty() && part.parse::<i32>().is_err())
                .map(|part| (*part).to_string())
                .collect::<Vec<_>>();
            if allowed.is_empty() {
                None
            } else {
                Some(Predicate::DeityAlignment { allowed })
            }
        }
        // PREALIGN:LG,NG,CG — the character's alignment must be one of these.
        //
        // `PreAlignParser` does no validation at all: it sets the key to the raw
        // text and `PreAlignTester` looks that key up among the game mode's
        // alignments.  Numbers are therefore *not* a different syntax to be
        // filtered out — the Book of Erotic Fantasy writes `PREALIGN:0,1,3,4,6,7`
        // and no vendored alignment file defines a numeric key (the `SORTKEY`s
        // are `1a`..`3c`, and nothing looks those up).  Such a requirement can
        // never match, which is the correct outcome; discarding it, as filtering
        // the numbers out did, left those prestige classes with no alignment
        // restriction at all.
        "PREALIGN" => {
            let allowed = parts
                .iter()
                .map(|part| part.trim())
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            if allowed.is_empty() {
                None
            } else {
                Some(Predicate::Alignment { allowed })
            }
        }
        // PRELEVEL:MIN=n,MAX=m — total character level bounds.  The old
        // implementation read the first number and always treated it as a
        // minimum, so `PRELEVEL:MAX=10` became "at least 10th level" and
        // excluded exactly the characters it was meant to allow.
        "PRELEVEL" => {
            let body = body.strip_prefix("Q:").unwrap_or(body).trim();
            // Historical data and this workspace's own fixtures use the bare
            // form `PRELEVEL:3`; PCGen treats that as a minimum.
            if let Ok(min) = body.parse::<u32>() {
                return Some(Predicate::CharacterLevel {
                    min: Some(min),
                    max: None,
                });
            }
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
                Some(Predicate::CharacterLevel { min, max })
            }
        }
        // PRELEVELMAX:x — maximum *class* level, excluding racial/monster HD.
        // The separate predicate already models that distinction.
        "PRELEVELMAX" => {
            let body = body.strip_prefix("Q:").unwrap_or(body).trim();
            let max = body.parse::<u32>().ok()?;
            Some(Predicate::CharacterClassLevel {
                min: None,
                max: Some(max),
            })
        }
        "PREMULT" => parse_premult(body),
        // `PREATT` is documented as a minimum base attack bonus.  PCGen also
        // has `PRETOTALAB`; both lower to the same runtime predicate.
        "PRETOTALAB" | "PREATT" => {
            let body = body.strip_prefix("Q:").unwrap_or(body).trim();
            let min = body.parse::<u32>().ok()?;
            Some(Predicate::TotalBab { min })
        }
        // PRECHECK:n,Save=value / PRECHECKBASE:n,Save=value — at least `n` of
        // the listed saves meet their minimum.
        //
        // `PreCheckParser` handles both kinds as a list parser: the leading
        // number is the count, and each remaining part is split on the *last*
        // `=` (see §5t) into a save name and its minimum.  The old lowering read
        // only the first `=` part and dropped the count, so
        // `PRECHECKBASE:2,Fortitude=5,Reflex=3` became "Fortitude >= 5" — a
        // stricter test than the reference's "either".
        "PRECHECK" | "PRECHECKBASE" => {
            let base = tag_upper == "PRECHECKBASE";
            let min_count = parts
                .iter()
                .find(|part| !part.trim().is_empty())
                .and_then(|part| part.trim().parse::<u32>().ok())
                .unwrap_or(1);
            let specs: Vec<pcgen_core::SaveSpec> = parts
                .iter()
                .skip_while(|part| part.trim().parse::<u32>().is_ok() || part.trim().is_empty())
                .filter_map(|part| {
                    let (save, value) = part.trim().rsplit_once('=')?;
                    let save = save.trim();
                    if save.is_empty() {
                        return None;
                    }
                    Some(pcgen_core::SaveSpec {
                        save: save.to_string(),
                        min: value.trim().parse::<i32>().ok()?,
                    })
                })
                .collect();
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::Check {
                base,
                specs,
                min_count,
            })
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
        "PRELEGSGTEQ" | "PRELEGSLTEQ" => {
            let count = body
                .split(|ch: char| !ch.is_ascii_digit())
                .find(|part| !part.is_empty())
                .and_then(|part| part.parse::<u32>().ok())?;
            Some(if tag_upper == "PRELEGSLTEQ" {
                Predicate::LegsAtMost { count }
            } else {
                Predicate::LegsAtLeast { count }
            })
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
        // PREREGION:Varisia — the character's home region starts with the given
        // text, case-insensitively.
        //
        // The whole body is one prefix, commas included: it is a *prefix* test,
        // not a list.  PCGen's `AbstractPrerequisiteSimpleParser` sets the key
        // to the entire formula, and the data depends on that --
        // `PREREGION:Elorii Nations,Dwarven Enclaves` is written against a
        // region literally named that, and matches nothing else.
        "PREREGION" => {
            let body = body.strip_prefix("Q:").unwrap_or(body).trim();
            if body.is_empty() {
                return None;
            }
            Some(Predicate::Region {
                prefix: body.to_string(),
            })
        }
        // PRESPELLDESCRIPTOR:count,descriptor=level — at least `count` known
        // spells carry the listed descriptors at or above the stated levels.
        //
        // `PreSpellDescriptorParser` is a list parser: the leading number is the
        // count, and each `y=z` pair is a sub-requirement.  Its tester returns
        // the *number* of known spells matching its one descriptor, and the list
        // wrapper sums the sub-requirements, so the pairs add up rather than
        // being alternatives.  The documented deprecated spelling (`y,x,z`) is
        // rejected by the reference parser too — `Integer.parseInt(elements[0])`
        // throws on a descriptor — so it is not supported here either.
        "PRESPELLDESCRIPTOR" => {
            let (count_str, rest) = body.split_once(',')?;
            let count = count_str.trim().parse::<u32>().ok()?;
            let specs: Vec<pcgen_core::SpellDescriptorSpec> = rest
                .split(',')
                .filter_map(|pair| {
                    // The value is the *last* `=` segment, as everywhere else.
                    let (descriptor, level) = pair.rsplit_once('=')?;
                    let descriptor = descriptor.trim();
                    if descriptor.is_empty() {
                        return None;
                    }
                    Some(pcgen_core::SpellDescriptorSpec {
                        descriptor: descriptor.to_string(),
                        min_level: level.trim().parse::<u32>().ok()?,
                    })
                })
                .collect();
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::SpellDescriptor {
                specs,
                count: count.max(1),
            })
        }
        // PRESPELLSCHOOLSUB:count,subschool=level — the same list shape and the
        // same summing count as `PRESPELLDESCRIPTOR`; only the spell property
        // being matched differs (`PreSpellSchoolSubTester` fills the sub-school
        // slot of `aggregateSpellList` where the descriptor tester fills the
        // descriptor slot).
        "PRESPELLSCHOOLSUB" => {
            let (count_str, rest) = body.split_once(',')?;
            let count = count_str.trim().parse::<u32>().ok()?;
            let specs: Vec<pcgen_core::SpellSchoolSubSpec> = rest
                .split(',')
                .filter_map(|pair| {
                    let (subschool, level) = pair.rsplit_once('=')?;
                    let subschool = subschool.trim();
                    if subschool.is_empty() {
                        return None;
                    }
                    Some(pcgen_core::SpellSchoolSubSpec {
                        subschool: subschool.to_string(),
                        min_level: level.trim().parse::<u32>().ok()?,
                    })
                })
                .collect();
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::SpellSchoolSub {
                specs,
                count: count.max(1),
            })
        }
        // PRESPELLTYPE:count,type=level — can cast `count` spells of `type`
        // at spell level `level` or higher.
        "PRESPELLTYPE" => {
            let (count_str, rest) = body.split_once(',')?;
            let count = count_str.trim().parse::<u32>().ok()?;
            let specs: Vec<pcgen_core::SpellTypeSpec> = rest
                .split(',')
                .filter_map(|pair| {
                    let (power_str, level_str) = pair.split_once('=')?;
                    let power = if power_str.trim().eq_ignore_ascii_case("any") {
                        None
                    } else {
                        Some(SpellPowerType::from_pcgen(power_str)?)
                    };
                    Some(pcgen_core::SpellTypeSpec {
                        power,
                        min_level: level_str.trim().parse::<u32>().ok()?,
                    })
                })
                .collect();
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::SpellType {
                specs,
                count: count.max(1),
            })
        }
        // PRESPELLBOOK:YES — requires a spellbook.
        "PRESPELLBOOK" => match body.trim().to_ascii_uppercase().as_str() {
            "YES" => Some(Predicate::HasSpellbook),
            "NO" => Some(Predicate::Not(Box::new(Predicate::HasSpellbook))),
            _ => None,
        },
        // PRESPELLCAST:MEMORIZE=Y|N,TYPE=<spell type> — one sub-requirement per
        // comma token, in a group whose operand is the default 1, so two tokens
        // mean "at least one of them holds".
        //
        // `PreSpellCastParser` splits on commas and builds either a
        // `spellcast.memorize` or a `spellcast.type` sub-requirement; any other
        // token makes it throw.  The type is compared as *text* against
        // `PCClass.getSpellType`, which returns the class's `FACT:SpellType`
        // verbatim (or `None`), so a value like `Arcane=1` names no class and is
        // false rather than unreadable.
        //
        // The previous lowering read only the first token of either kind, so
        // `PRESPELLCAST:TYPE=Arcane,TYPE=Divine` — nine occurrences in the
        // third-party data — tested Arcane alone instead of "either".
        "PRESPELLCAST" => {
            let mut items = Vec::new();
            for token in parts
                .iter()
                .map(|part| part.trim())
                .filter(|part| !part.is_empty())
            {
                let upper = token.to_ascii_uppercase();
                if upper.starts_with("MEMORIZE") {
                    // `MEMORIZE=Y` is a prepared caster; the reference reads
                    // anything not starting with `N` as `Y`, so `MEMORIZE=Yes`
                    // lands on the same side.
                    let value = token.split_once('=')?.1.trim().to_ascii_uppercase();
                    items.push(Predicate::CastingTypeIs {
                        prepared: !value.starts_with('N'),
                    });
                } else if upper.starts_with("TYPE") {
                    // `token.substring(5)`, so `TYPE=X` and `TYPE.X` both yield
                    // `X`; a token shorter than five characters throws in the
                    // reference and stays unsupported here.
                    items.push(Predicate::HasSpellType {
                        spell_type: token.get(5..)?.trim().to_string(),
                    });
                } else {
                    return None;
                }
            }
            match items.len() {
                0 => None,
                1 => items.pop(),
                // The parser drops the wrapper only for a single sub-requirement;
                // with more it keeps the group, whose operator is GTEQ and whose
                // operand is the default 1.
                _ => Some(Predicate::AtLeast { n: 1, items }),
            }
        }
        // PREITEM:count,name,TYPE=type — the character possesses at least `count`
        // of the listed items.
        //
        // `PreItemParser` is a list parser, so the leading number is the count and
        // each remaining entry is one item test.  An entry is a `TYPE=` (or
        // `TYPE.`) prefix followed by a dot-separated type list, or a name that
        // may contain a `%` wildcard.
        "PREITEM" => {
            let (count_str, rest) = body.split_once(',')?;
            let min_count = count_str.trim().parse::<u32>().ok()?;
            let specs: Vec<pcgen_core::ItemSpec> = rest
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    let types = part
                        .get(..5)
                        .filter(|head| {
                            head.eq_ignore_ascii_case("TYPE=") || head.eq_ignore_ascii_case("TYPE.")
                        })
                        .map(|_| part[5..].trim());
                    match types {
                        Some(types) => {
                            let types: Vec<String> = types
                                .split('.')
                                .map(str::trim)
                                .filter(|kind| !kind.is_empty())
                                .map(str::to_string)
                                .collect();
                            if types.is_empty() {
                                return None;
                            }
                            Some(pcgen_core::ItemSpec::Types(types))
                        }
                        None => Some(pcgen_core::ItemSpec::Name(part.to_string())),
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            if specs.is_empty() {
                return None;
            }
            Some(Predicate::Item {
                specs,
                min_count: min_count.max(1),
            })
        }
        // PREREACH and its six operator kinds.  `PreReachParser` reads the
        // comparison out of the tag name (`kind.substring(5)`, empty meaning
        // `gteq`) and `PreReachTester` compares the character's reach with the
        // operand, so the strict and equal kinds compose from the two directions
        // exactly as the size family does.
        "PREREACH" | "PREREACHEQ" | "PREREACHGT" | "PREREACHGTEQ" | "PREREACHLT"
        | "PREREACHLTEQ" | "PREREACHNEQ" => {
            let feet = body.trim().parse::<i32>().ok()?;
            let at_least = Predicate::ReachAtLeast { feet };
            let at_most = Predicate::ReachAtMost { feet };
            match tag_upper.as_str() {
                "PREREACH" | "PREREACHGTEQ" => Some(at_least),
                "PREREACHLTEQ" => Some(at_most),
                "PREREACHEQ" => Some(Predicate::All(vec![at_least, at_most])),
                "PREREACHGT" => Some(Predicate::All(vec![
                    at_least,
                    Predicate::Not(Box::new(at_most)),
                ])),
                "PREREACHLT" => Some(Predicate::All(vec![
                    at_most,
                    Predicate::Not(Box::new(at_least)),
                ])),
                _ => Some(Predicate::Not(Box::new(Predicate::All(vec![
                    at_least, at_most,
                ])))),
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
        "PREHANDSLTEQ" => Some(Predicate::HandsAtMost {
            count: body
                .split(|ch: char| !ch.is_ascii_digit())
                .find(|part| !part.is_empty())
                .and_then(|part| part.parse::<u32>().ok())?,
        }),
        // Strictly more than n hands: at least n + 1.
        "PREHANDSGT" => Some(Predicate::HandsAtLeast {
            count: body
                .split(|ch: char| !ch.is_ascii_digit())
                .find(|part| !part.is_empty())
                .and_then(|part| part.parse::<u32>().ok())?
                .saturating_add(1),
        }),
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
        // PREMOVE:x,Mode=rate,Mode2=rate2[|TYPE=...] — at least `x` of the
        // listed movement modes meet their minimum rate.
        "PREMOVE" => {
            let body = body.split('|').next()?.trim();
            let mut fields = body
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty());
            let min_count = fields
                .next()
                .and_then(|part| part.parse::<u32>().ok())
                .unwrap_or(1)
                .max(1);
            let mut specs = Vec::new();
            for movement in fields {
                let (mode, rate) = movement.split_once('=')?;
                let mode = mode.trim().to_ascii_lowercase();
                let feet = rate.trim().parse::<u32>().ok()?;
                if mode.is_empty() {
                    return None;
                }
                specs.push(pcgen_core::MovementSpec {
                    mode,
                    feet: feet.max(1),
                });
            }
            match specs.len() {
                0 => None,
                1 if min_count == 1 => {
                    let spec = specs.into_iter().next().expect("one spec");
                    Some(Predicate::MovementAtLeast {
                        mode: spec.mode,
                        feet: spec.feet,
                    })
                }
                _ => Some(Predicate::MovementCount { min_count, specs }),
            }
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
/// Merge PCGen ability `.MOD` records into their base entity.
///
/// The lowering pass is line-oriented, so a base ability header and its
/// following `.MOD` lines become separate entities.  In PCGen the `.MOD`
/// records modify the base ability; merging their effects makes the base key
/// resolve to the real implementation (e.g. `Halfling Racial Trait Default`).
fn merge_ability_mod_entities(entities: &mut [CompiledEntity]) {
    let mut merges: Vec<(usize, usize)> = Vec::new();
    for (mod_index, entity) in entities.iter().enumerate() {
        let Some(base_name) = entity.name.trim().strip_suffix(".MOD") else {
            continue;
        };
        let base_name = base_name.rsplit('|').next().unwrap_or(base_name).trim();
        let wanted = normalized_key(base_name);
        if let Some(base_index) = entities.iter().enumerate().find_map(|(index, candidate)| {
            if index == mod_index {
                return None;
            }
            let candidate_key = normalized_key(&candidate.id.key);
            let candidate_name = normalized_key(&candidate.name);
            (candidate_key == wanted || candidate_name == wanted).then_some(index)
        }) {
            merges.push((mod_index, base_index));
        }
    }
    for (mod_index, base_index) in merges {
        let effects = std::mem::take(&mut entities[mod_index].effects);
        let requirements = std::mem::take(&mut entities[mod_index].requirements);
        let legacy = std::mem::take(&mut entities[mod_index].legacy_tags);
        let unsupported = std::mem::take(&mut entities[mod_index].unsupported_tags);
        entities[base_index].effects.extend(effects);
        entities[base_index].requirements.extend(requirements);
        entities[base_index].legacy_tags.extend(legacy);
        entities[base_index].unsupported_tags.extend(unsupported);
    }
}

fn normalized_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}

fn spell_is_psionic(typed: &TypedDefinitions, entity: &CompiledEntity) -> bool {
    typed
        .spells
        .get(&entity.id.key)
        .or_else(|| {
            typed
                .spells
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(&entity.id.key))
                .map(|(_, spell)| spell)
        })
        .is_some_and(|spell| spell.is_psionic)
}

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
    for key in typed.kits.keys() {
        parts.push(format!("kit:{key}"));
    }
    for key in typed.templates.keys() {
        parts.push(format!("template:{key}"));
    }
    for key in typed.abilities.keys() {
        parts.push(format!("ability:{key}"));
    }
    for key in typed.ability_categories.keys() {
        parts.push(format!("ability-category:{key}"));
    }
    for key in typed.creature_trait_records.keys() {
        parts.push(format!("trait-record:{key}"));
    }
    for (key, rows) in &typed.companion_mods {
        parts.push(format!("companion-mod:{key}:{}", rows.len()));
        for row in rows {
            parts.push(format!("companion-row:{row:?}"));
        }
    }
    for key in typed.sizes.keys() {
        parts.push(format!("size:{key}"));
    }
    for key in typed.stats.keys() {
        parts.push(format!("stat:{key}"));
    }
    for key in typed.alignments.keys() {
        parts.push(format!("alignment:{key}"));
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

fn parse_entity_metadata(record: &RawRecord) -> pcgen_core::EntityMetadata {
    let mut metadata = pcgen_core::EntityMetadata::default();
    for tag in &record.tags {
        let upper = tag.name.to_ascii_uppercase();
        let value = tag.value.trim();
        match upper.as_str() {
            "ALTCRITMULT" => metadata.alternate_weapon.crit_mult = Some(value.to_string()),
            "ALTCRITRANGE" => {
                metadata.alternate_weapon.crit_range = value.parse::<i32>().ok();
            }
            "ALTDAMAGE" => metadata.alternate_weapon.damage = Some(value.to_string()),
            "ALTTYPE" => extend_metadata_list(&mut metadata.alternate_weapon.types, value),
            "ALTEQMOD" => {
                extend_metadata_list(&mut metadata.alternate_weapon.equipment_mods, value);
            }
            "BASEQTY" => metadata.alternate_weapon.base_quantity = Some(value.to_string()),
            "WEAPONBONUS" => metadata.weapon_bonus.push(value.to_string()),
            "CHANGEPROF" => {
                for entry in value
                    .split([',', '|'])
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    if let Some((from, to)) = entry.rsplit_once('=') {
                        metadata
                            .changed_proficiencies
                            .insert(from.trim().to_string(), to.trim().to_string());
                    }
                }
            }
            "ARMORTYPE" => extend_metadata_list(&mut metadata.armor_types, value),
            "ADDDOMAINS" => metadata.added_domains.extend(parse_added_domains(value)),
            "PROHIBITCOST" => metadata.prohibited_cost = value.parse::<i32>().ok(),
            "DONOTADD" => extend_metadata_list(&mut metadata.do_not_add, value),
            "VARIANTS" => extend_metadata_list(&mut metadata.spell_variants, value),
            "LANGBONUS" => extend_metadata_list(&mut metadata.language_bonus, value),
            _ => {}
        }
    }
    metadata
}

fn extend_metadata_list(target: &mut Vec<String>, value: &str) {
    target.extend(
        value
            .split([',', '|'])
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string),
    );
}

/// True if `tag_name` is consumed during lowering for `category`, so it should
/// not also be reported as a legacy (un-lowered) tag.
fn consumed_legacy_tag(category: &str, tag_name: &str) -> bool {
    let name = tag_name.to_ascii_uppercase();
    // These tags are captured generically into `CompiledEntity.metadata` for
    // every category by `parse_entity_metadata` above.
    if matches!(
        name.as_str(),
        "ALTCRITMULT"
            | "ALTCRITRANGE"
            | "ALTDAMAGE"
            | "ALTEQMOD"
            | "ALTTYPE"
            | "ARMORTYPE"
            | "BASEQTY"
            | "CHANGEPROF"
            | "WEAPONBONUS"
            | "ADDDOMAINS"
            | "PROHIBITCOST"
            | "DONOTADD"
            | "VARIANTS"
            | "STACK"
            | "ABILITYLIST"
            | "POOL"
            | "CHOICE"
            | "ATTACKCYCLE"
            | "MOVECLONE"
            | "REACH"
    ) {
        return true;
    }
    if pcgen_core::tagclass::is_kit_category(category)
        && pcgen_core::tagclass::classify_for_category(category, &name)
            == pcgen_core::tagclass::TagClass::Allowlisted
    {
        return true;
    }
    match category {
        "ABILITY" | "ABILITYCATEGORY" => matches!(
            name.as_str(),
            "CATEGORY"
                | "DEFINESTAT"
                | "SPELLLEVEL"
                | "ADDSPELLLEVEL"
                | "UDAM"
                | "UMULT"
                | "NATURALATTACKS"
                | "VISION"
                | "DR"
                | "SR"
        ),
        "DEITY" => matches!(
            name.as_str(),
            "DOMAINS" | "VALIDFORDEITY" | "VALIDFORFOLLOWER" | "DEITYWEAP"
        ),
        "ALIGNMENT" => matches!(name.as_str(), "VALIDFORDEITY" | "VALIDFORFOLLOWER"),
        "SPELL" => name == "DOMAINS",
        "DOMAIN" => name == "SPELLLEVEL",
        // Region is a *template* token.  Most of the third-party packs that write
        // `REGION:` on a RACE row are wrong about that, and PCGen ignores it
        // there; only templates may claim the tag, so a misplaced one stays
        // visible as legacy instead of looking consumed.
        "RACE" | "TEMPLATE" => {
            (category == "TEMPLATE" && matches!(name.as_str(), "REGION" | "SUBREGION"))
                || matches!(
                    name.as_str(),
                    "REACH"
                        | "REACHMULT"
                        | "FACE"
                        | "RACESUBTYPE"
                        | "NATURALATTACKS"
                        | "UDAM"
                        | "UMULT"
                        | "DR"
                        | "SR"
                        | "CR"
                        | "VISION"
                        | "MOVECLONE"
                        | "UNENCUMBEREDMOVE"
                        | "ATTACKCYCLE"
                        | "HITDIE"
                        | "ADDLEVEL"
                        | "DEFINESTAT"
                        | "XTRASKILLPTSPERLVL"
                        | "SKILLMULT"
                )
        }
        "CLASS" | "CLASS_LEVEL" => matches!(
            name.as_str(),
            "UDAM"
                | "UMULT"
                | "DR"
                | "SR"
                | "SPELLLEVEL"
                | "PROHIBITSPELL"
                | "KNOWNSPELLSFROMSPECIALTY"
                | "SPELLBOOK"
                | "SKILLMULT"
                | "LEVELSPERFEAT"
                | "HITDIE"
                | "EXCLASS"
                | "ADDLEVEL"
                | "EXCHANGELEVEL"
        ),
        "SIZE" => name == "SIZENUM",
        "STAT" => name == "STATMOD",
        "EQUIPMENT" => matches!(
            name.as_str(),
            "DR" | "SR"
                | "REACHMULT"
                | "CRITMULT"
                | "CRITRANGE"
                | "WIELD"
                | "BASEITEM"
                | "CONTAINS"
                | "SLOTS"
                | "LOCATION"
        ),
        "EQUIPMOD" => matches!(
            name.as_str(),
            "DR" | "SR" | "PLUS" | "ITYPE" | "REPLACES" | "CHARGES"
        ),
        "COMPANIONMOD" => matches!(name.as_str(), "SR" | "DEFINESTAT"),
        _ => false,
    }
}
