//! Lossless records -> typed definitions + progression tables.
//!
//! This is the Sprint A migration of the archive `store.rs` / `lst.rs`
//! behaviour.  It intentionally produces only the typed data the engine
//! needs today; richer tag semantics can be layered on later.

use crate::source::RawRecord;
use pcgen_core::domain::{
    AttackProgression, CastingType, ClassDefinition, ClassProgression, CompanionBonus,
    DeityDefinition, DomainDefinition, DomainSpell, EquipmentDefinition,
    EquipmentModifierDefinition, KitDefinition, KitEntry, RaceDefinition, SaveProgression,
    SkillDefinition, SpellDefinition, SpellPowerType, TemplateDefinition, TypedDefinitions,
};
use pcgen_core::{EffectPhase, Provenance, VariantDefinition};
use std::collections::BTreeMap;
use std::collections::HashMap;

/// Build typed definitions from lossless records.
pub fn build_typed_definitions(records: &[RawRecord]) -> TypedDefinitions {
    let mut typed = TypedDefinitions::default();
    let mut current_class: Option<String> = None;
    let mut current_substitution: Option<String> = None;
    let mut current_kit: Option<String> = None;

    for record in records {
        let mut record_traits = pcgen_core::CreatureTraits::default();
        parse_creature_traits(record, &mut record_traits);
        if record_traits != pcgen_core::CreatureTraits::default() {
            let key = format!(
                "{}::{}::{}",
                record.category.to_ascii_uppercase(),
                record.key,
                record.source.line
            );
            typed.creature_trait_records.insert(key, record_traits);
        }
        let category_upper = record.category.to_ascii_uppercase();
        match category_upper.as_str() {
            "CLASS" => {
                if let Some(substitution) = record.name.strip_prefix("SUBSTITUTIONCLASS:") {
                    current_substitution = Some(substitution.trim().to_string());
                    continue;
                }
                if let Some(level) = record.name.strip_prefix("SUBSTITUTIONLEVEL:") {
                    if let (Some(substitution), Ok(level)) =
                        (current_substitution.as_ref(), level.trim().parse::<u32>())
                    {
                        let variant = parse_substitution_variant(
                            record,
                            substitution,
                            level,
                            current_class.as_deref(),
                        );
                        typed.variants.insert(variant.id.clone(), variant);
                    }
                    continue;
                }
                // `SUBCLASS:` / `SUBCLASSLEVEL:` describe a specialty of the
                // *current* class.  They must not be parsed as class headers:
                // doing so moves `current_class` onto the specialty, and every
                // level row that follows is then attached to it instead of to
                // the real class (RSRD Wizard / Dragon Disciple lost all of
                // their class features that way).
                if is_class_specialty_row(&record.name) {
                    continue;
                }
                if class_level_of_name(&record.name).is_some() {
                    if let Some(class_id) = &current_class {
                        if let Some(class) = typed.classes.get_mut(class_id) {
                            parse_class_level_row(class, record);
                        }
                    }
                } else if is_class_header_row(record) && !record.name.contains(".COPY=") {
                    // A `.MOD` with a base is merged by the patch pass.  A
                    // remaining `.MOD` has no base; keep it as a typed
                    // definition so the source is not silently lost.
                    let class = parse_class(record);
                    current_class = Some(class.id.clone());
                    merge_class(&mut typed.classes, class);
                }
            }
            "RACE" => {
                let race = parse_race(record);
                typed.races.insert(race.id.clone(), race);
            }
            "SKILL" => {
                let skill = parse_skill(record);
                typed.skills.insert(skill.id.clone(), skill);
            }
            "SPELL" => {
                let spell = parse_spell(record);
                // Key typed spells by the same normalized key the lowering
                // pass uses for `EntityId`, so `psionic_effects` can join the
                // two views without duplicating normalization logic.
                typed.spells.insert(entity_key(record), spell);
            }
            "EQUIPMENT" => {
                let equipment = parse_equipment(record);
                typed.equipment.insert(equipment.id.clone(), equipment);
            }
            "EQUIPMOD" => {
                let equipment_modifier = parse_equipment_modifier(record);
                typed
                    .equipment_modifiers
                    .insert(equipment_modifier.id.clone(), equipment_modifier);
            }
            "ABILITY" => {
                // A variant (`TYPE:Archetype` / `TYPE:Substitution*`) is a class
                // variant, not a selectable ability.  Keep it only in
                // `typed.variants` so `pack.abilities` stays a true ability
                // inventory instead of double-counting every variant.
                if let Some(variant) = parse_variant(record) {
                    typed.variants.insert(variant.id.clone(), variant);
                } else {
                    let ability = parse_ability(record);
                    typed.abilities.insert(ability.id.clone(), ability);
                }
            }
            "ABILITYCATEGORY" => {
                let category = parse_ability_category(record);
                typed
                    .ability_categories
                    .insert(category.id.clone(), category);
            }
            "DEITY" => {
                let deity = parse_deity(record);
                typed.deities.insert(deity.id.clone(), deity);
            }
            "DOMAIN" => {
                let domain = parse_domain(record);
                typed.domains.insert(domain.id.clone(), domain);
            }
            "KIT" => parse_kit_record(record, &mut current_kit, &mut typed.kits),
            "ALIGNMENT" => {
                typed.alignments.insert(
                    record.key.clone(),
                    pcgen_core::AlignmentDefinition {
                        id: record.key.clone(),
                        name: record.name.clone(),
                        valid_for_deity: tag(record, "VALIDFORDEITY").map(parse_yes_no),
                        valid_for_follower: tag(record, "VALIDFORFOLLOWER").map(parse_yes_no),
                    },
                );
            }
            "SIZE" => {
                let mut size = pcgen_core::SizeDefinition {
                    id: record.key.clone(),
                    name: record.name.clone(),
                    size_number: tag(record, "SIZENUM").map(str::to_string),
                };
                if !size.id.is_empty() {
                    // `.MOD` rows (e.g. `M.MOD`) carry the SIZENUM; keep the
                    // base key so lookups stay stable.
                    if let Some(base) = size.id.strip_suffix(".MOD") {
                        size.id = base.to_string();
                    }
                    typed.sizes.insert(size.id.clone(), size);
                }
            }
            "STAT" => {
                if let Some(key) = tag(record, "KEY") {
                    typed.stats.insert(
                        key.to_string(),
                        pcgen_core::StatDefinition {
                            id: key.to_string(),
                            name: record.name.clone(),
                            modifier_formula: tag(record, "STATMOD").map(str::to_string),
                        },
                    );
                }
            }
            "COMPANIONMOD" => {
                if let Some(companion) = parse_companion_mod(record) {
                    typed
                        .companion_mods
                        .entry(companion.id.clone())
                        .or_default()
                        .push(companion);
                }
            }
            "TEMPLATE" => {
                let template = parse_template(record);
                typed.templates.insert(template.id.clone(), template);
            }
            _ => {}
        }
    }

    typed.progressions = typed
        .classes
        .values()
        .map(|class| (class.id.clone(), build_class_progression(class)))
        .collect();
    typed
}

fn merge_class(classes: &mut BTreeMap<String, ClassDefinition>, class: ClassDefinition) {
    match classes.get_mut(&class.id) {
        Some(existing) => {
            // PCGen classes are often continued across header rows; merge
            // non-default fields instead of creating a duplicate.
            if existing.hit_die == "d8" && class.hit_die != "d8" {
                existing.hit_die = class.hit_die;
            }
            if class.is_prestige {
                existing.is_prestige = true;
            }
            if class.is_psionic {
                existing.is_psionic = true;
            }
            if class.skill_points_per_level != 2 {
                existing.skill_points_per_level = class.skill_points_per_level;
            }
            existing.class_skills.extend(class.class_skills);
            if class.spellcasting_ability.is_some() {
                existing.spellcasting_ability = class.spellcasting_ability;
            }
            if existing.casting_type == CastingType::Unknown
                && class.casting_type != CastingType::Unknown
            {
                existing.casting_type = class.casting_type;
            }
            existing.templates.extend(class.templates);
            existing.level_bonuses.extend(class.level_bonuses);
            existing.level_abilities.extend(class.level_abilities);
            existing
                .level_prerequisites
                .extend(class.level_prerequisites);
            existing.prohibited_spells.extend(class.prohibited_spells);
            existing.known_spells_from_specialty = existing
                .known_spells_from_specialty
                .max(class.known_spells_from_specialty);
            existing.spell_slots.extend(class.spell_slots);
            existing.spells_known.extend(class.spells_known);
            existing.weapon_bonus.extend(class.weapon_bonus);
            existing.added_domains.extend(class.added_domains);
            existing
                .level_do_not_add
                .extend(class.level_do_not_add.clone());
            existing
                .level_spellcaster_advance
                .extend(class.level_spellcaster_advance.clone());
            if class.prohibited_cost.is_some() {
                existing.prohibited_cost = class.prohibited_cost;
            }
            if class.subclass_choice.is_some() {
                existing.subclass_choice = class.subclass_choice;
            }
            if !class.spell_list.is_empty() {
                existing.spell_list = class.spell_list;
            }
        }
        None => {
            classes.insert(class.id.clone(), class);
        }
    }
}

/// Normalized key matching `EntityId::from_pcgen` for raw records.
pub(crate) fn entity_key(record: &RawRecord) -> String {
    let raw = record.key.trim();
    if raw.contains(':') || raw.contains('|') {
        raw.to_string()
    } else {
        raw.to_ascii_lowercase().replace(' ', "_")
    }
}

fn parse_race(record: &RawRecord) -> RaceDefinition {
    let mut race = RaceDefinition::new(record.key.clone(), record.name.clone());
    if let Some(size) = tag(record, "SIZE") {
        race.size = expand_size(size).to_string();
    }
    for move_value in tags(record, "MOVE") {
        let parts: Vec<&str> = move_value.split(',').map(str::trim).collect();
        if parts.len().is_multiple_of(2) && !parts.is_empty() {
            for pair in parts.chunks(2) {
                let Ok(rate) = pair[1].parse::<u32>() else {
                    continue;
                };
                let mode = pair[0].to_ascii_lowercase();
                if mode.is_empty() {
                    continue;
                }
                race.movement.insert(mode.clone(), rate);
                if mode == "walk" {
                    race.speed_feet = rate as i32;
                }
            }
        } else if let Some(speed) = parts.iter().find_map(|part| part.parse::<i32>().ok()) {
            race.speed_feet = speed;
        }
    }
    race.hands = tag(record, "HANDS")
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(2);
    for bonus in tags(record, "BONUS") {
        let parts: Vec<&str> = bonus.split('|').map(str::trim).collect();
        if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("STAT") {
            if let Ok(value) = parts[2].parse::<i32>() {
                for stat in parts[1].split(',') {
                    race.ability_adjustments
                        .insert(stat.trim().to_uppercase(), value);
                }
            }
        }
    }
    race.display_name = tag(record, "OUTPUTNAME").unwrap_or(&race.name).to_string();
    race.level_adjustment = tag(record, "LEVELADJUSTMENT")
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0);
    race.serves_as = tag(record, "SERVESAS").and_then(|value| {
        value
            .split('|')
            .find(|part| !part.is_empty() && !part.eq_ignore_ascii_case("RACE"))
            .map(str::to_string)
    });
    race.favored_class = tag(record, "FAVOREDCLASS").map(str::to_string);
    race.extra_skill_points_per_level =
        tag(record, "XTRASKILLPTSPERLVL").and_then(|value| value.trim().parse().ok());
    race.stat_minimums = parse_stat_minimums(record);
    race.skill_multiplier = tag(record, "SKILLMULT").and_then(|value| value.trim().parse().ok());
    for changed in tags(record, "CHANGEPROF") {
        for entry in changed.split(',').map(str::trim) {
            if let Some((from, to)) = entry.rsplit_once('=') {
                race.changed_proficiencies
                    .insert(from.trim().to_string(), to.trim().to_string());
            }
        }
    }
    race.type_tags = type_tags(record);
    parse_creature_traits(record, &mut race.traits);
    for template in tags(record, "TEMPLATE") {
        for name in template.split('|').map(str::trim) {
            if !name.is_empty() && !name.to_ascii_uppercase().starts_with("CHOOSE:") {
                race.templates.push(name.to_string());
            }
        }
    }
    race
}

/// True for `SUBCLASS:` / `SUBCLASSLEVEL:` rows, which refine the current class
/// rather than declaring a new one.
///
/// These appear *inside* a class block in the RSRD (e.g. the eight Wizard
/// schools precede the Wizard level progression).  They carry no class header
/// of their own, so treating them as headers corrupts `current_class` and
/// silently detaches every following level row.
/// True when a `CLASS` record is a class *header* rather than a level row or a
/// refinement of the class already in scope.
///
/// Both lowering passes have to decide this, and each previously carried its
/// own copy of the test.  That is precisely how the RSRD Wizard lost every
/// class feature: one copy learned to skip `SUBCLASS:` / `SUBCLASSLEVEL:` rows
/// and the other did not, so the cursor moved onto the specialty and the level
/// rows were keyed after it.  Keeping the decision in one place is what stops
/// the two from drifting apart again.
///
/// Callers layer their own `.MOD` / `.COPY=` policy on top, because the two
/// passes differ there deliberately: `rulepack` ignores such rows as headers,
/// while the typed pass keeps a stray `.MOD` (one with no base to merge into)
/// so its source is not silently dropped.
/// The class level a record names, if it is a level row.
///
/// Level rows are written `1`, but the RSRD also uses `14:REPEATLEVEL:4` to
/// mean "level 14, repeating every four levels".  That form was previously
/// taken for a class header, which invented classes called `14:REPEATLEVEL:4`
/// and detached the row's own effects from any class.
///
/// Returns `None` for anything that is not a level row, including the
/// `SUBCLASSLEVEL:` / `SUBSTITUTIONLEVEL:` refinements, whose first field also
/// starts with digits-free text.
pub fn class_level_of_name(name: &str) -> Option<u32> {
    let name = name.trim();
    if let Ok(level) = name.parse::<u32>() {
        return Some(level);
    }
    // `N:REPEATLEVEL:M`
    let (head, tail) = name.split_once(':')?;
    if !tail.to_ascii_uppercase().starts_with("REPEATLEVEL:") {
        return None;
    }
    head.trim().parse::<u32>().ok()
}

/// True for a `.lst` file's own metadata header.
///
/// The first line of a data file is `SOURCELONG:...\tSOURCESHORT:...`, which is
/// file-scoped provenance rather than a record.  With no other field to name
/// it, the line's name became `SOURCELONG:Revised (v.3.5) ...`, and a class
/// was invented under that name.
pub fn is_file_metadata_row(record: &RawRecord) -> bool {
    let upper = record.name.trim().to_ascii_uppercase();
    ["SOURCELONG:", "SOURCESHORT:", "SOURCEWEB:", "SOURCEDATE:"]
        .iter()
        .any(|prefix| upper.starts_with(prefix))
}

pub fn is_class_header_row(record: &RawRecord) -> bool {
    if !record.category.eq_ignore_ascii_case("CLASS") {
        return false;
    }
    let name = record.name.trim();
    !name.is_empty()
        && class_level_of_name(name).is_none()
        && !name.starts_with("SUBSTITUTIONCLASS:")
        && !name.starts_with("SUBSTITUTIONLEVEL:")
        && !is_class_specialty_row(name)
        && !is_file_metadata_row(record)
}

pub fn is_class_specialty_row(name: &str) -> bool {
    let name = name.trim();
    let upper = name.to_ascii_uppercase();
    upper.starts_with("SUBCLASS:")
        || upper.starts_with("SUBCLASSLEVEL:")
        || upper.starts_with("SUBCLASS.")
}

fn parse_class(record: &RawRecord) -> ClassDefinition {
    let class_name = record.name.trim().trim_end_matches(".MOD").to_string();
    let mut class = ClassDefinition::new(record.key.clone(), class_name);
    let hit_die = tag(record, "HD").unwrap_or("8");
    class.hit_die = if hit_die.starts_with('d') {
        hit_die.to_string()
    } else {
        format!("d{hit_die}")
    };
    if let Some(bab) = tag(record, "BAB") {
        class.attack_progression = attack_progression_from_text(bab);
    }
    class.max_level = tag(record, "MAXLEVEL")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(20);
    class.skill_points_per_level = tag(record, "STARTSKILLPTS")
        .or_else(|| tag(record, "SKILLPTS"))
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(2);
    class.spellcasting_ability = tag(record, "SPELLSTAT").map(str::to_string);
    if let Some(memorize) = tag(record, "MEMORIZE") {
        class.casting_type = casting_type_from_text(memorize);
    } else if class.spellcasting_ability.is_some() {
        class.casting_type = CastingType::Prepared;
    }
    // FACT:SpellType|Arcane/Divine/Psionic declares the class's magic source.
    class.spell_power = tags(record, "FACT")
        .find_map(|value| value.split_once('|').map(|(_, power)| power))
        .and_then(SpellPowerType::from_pcgen)
        .unwrap_or_default();
    class.requires_spellbook =
        tag(record, "SPELLBOOK").is_some_and(|value| value.eq_ignore_ascii_case("YES"));
    class.type_tags = type_tags(record);
    class.serves_as = tag(record, "SERVESAS").and_then(|value| {
        value
            .split('|')
            .find(|part| !part.is_empty() && !part.eq_ignore_ascii_case("CLASS"))
            .map(str::to_string)
    });
    class.is_prestige = type_tags(record)
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case("prestige"));
    class.is_psionic = type_tags(record)
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case("psionic"))
        || tags(record, "FACT").any(|value| value.eq_ignore_ascii_case("SpellType|Psionic"));
    class.is_monster = type_tags(record)
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case("monster"))
        || tags(record, "FACT").any(|value| value.eq_ignore_ascii_case("ClassType|Monster"));
    class.gestalt_slot_cost = tag(record, "GESTALTCOST").and_then(parse_slot_cost);
    for template in tags(record, "TEMPLATE") {
        for name in template.split('|').map(str::trim) {
            if !name.is_empty() && !name.to_ascii_uppercase().starts_with("CHOOSE:") {
                class.templates.push(name.to_string());
            }
        }
    }
    for skill_list in tags(record, "CSKILL") {
        class.class_skills.extend(
            skill_list
                .split([',', '|'])
                .map(|skill| skill.trim().trim_start_matches("TYPE=").to_string())
                .filter(|skill| !skill.is_empty()),
        );
    }
    class.skill_multiplier = tag(record, "SKILLMULT").and_then(|value| value.trim().parse().ok());
    class.levels_per_feat =
        tag(record, "LEVELSPERFEAT").and_then(|value| value.trim().parse().ok());
    class.hit_die_override = tag(record, "HITDIE").map(normalize_hit_die);
    class.excluded_class = tag(record, "EXCLASS").map(str::to_string);
    class.exchange_level =
        tag(record, "EXCHANGELEVEL").and_then(crate::rulepack::parse_exchange_level);
    for add in tags(record, "ADDLEVEL") {
        let mut parts = add.split('|').map(str::trim);
        if let Some(class_name) = parts.next().filter(|name| !name.is_empty()) {
            let levels = parts
                .next()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(1);
            class.add_levels.insert(class_name.to_string(), levels);
        }
    }
    class.weapon_bonus = tags(record, "WEAPONBONUS").map(str::to_string).collect();
    class.added_domains = tags(record, "ADDDOMAINS")
        .flat_map(crate::rulepack::parse_added_domains)
        .collect();
    class.prohibited_cost = tag(record, "PROHIBITCOST").and_then(|value| value.parse().ok());
    class.subclass_choice = tag(record, "CHOICE").map(str::to_string);
    class.prohibited_spells = tags(record, "PROHIBITSPELL")
        .filter_map(crate::rulepack::parse_prohibited_spell)
        .collect();
    class.known_spells_from_specialty = tag(record, "KNOWNSPELLSFROMSPECIALTY")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    for bonus in tags(record, "BONUS") {
        // The owning class is this record's own key, which is what lets `CL`
        // and `classlevel("APPLIEDAS=NONEPIC")` be resolved here rather than by
        // the engine.
        apply_class_bonus(&mut class, bonus, &record.key);
    }
    class
}

fn parse_class_level_row(class: &mut ClassDefinition, record: &RawRecord) {
    let Ok(level) = record.name.trim().parse::<u32>() else {
        return;
    };
    for tag in &record.tags {
        let name = tag.name.to_ascii_uppercase();
        match name.as_str() {
            "CAST" => {
                class
                    .spell_slots
                    .insert(level, parse_spell_table(&tag.value));
            }
            "KNOWN" => {
                class
                    .spells_known
                    .insert(level, parse_spell_table(&tag.value));
            }
            "SPELLLEVEL" => {}
            "SPELLLIST" => {
                class.spell_list = tag
                    .value
                    .split('|')
                    .map(str::trim)
                    .filter(|entry| !entry.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "ABILITY" => {
                // Structured, through the same parse the grant effects use.
                if let Some(shape) = crate::rulepack::parse_grant_shape(&tag.value) {
                    class
                        .level_abilities
                        .entry(level)
                        .or_default()
                        .push(pcgen_core::ClassGrant {
                            pool: shape.pool.map(str::to_string),
                            feature: shape.feature.to_string(),
                            mode: shape.mode,
                            unless_replaced: shape.anchors,
                            unless_category: shape.anchor_category,
                            guards: shape.guards.iter().map(|g| (*g).to_string()).collect(),
                        });
                }
            }
            "TEMPLATE" => {
                for template in tag.value.split('|').map(str::trim) {
                    if !template.is_empty()
                        && !template.to_ascii_uppercase().starts_with("CHOOSE:")
                        && !class.templates.iter().any(|existing| existing == template)
                    {
                        class.templates.push(template.to_string());
                    }
                }
            }
            "BONUS" => {
                class
                    .level_bonuses
                    .entry(level)
                    .or_default()
                    .push(format!("BONUS:{}", tag.value));
            }
            "DONOTADD" => {
                class.level_do_not_add.entry(level).or_default().extend(
                    tag.value
                        .split('|')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string),
                );
            }
            "ADD" => {
                class
                    .level_spellcaster_advance
                    .entry(level)
                    .or_default()
                    .push(tag.value.clone());
            }
            _ if crate::tags::is_predicate_tag(&name) => {
                class
                    .level_prerequisites
                    .entry(level)
                    .or_default()
                    .push(format!("{}:{}", tag.name, tag.value));
            }
            _ => {}
        }
    }
}

fn parse_template(record: &RawRecord) -> TemplateDefinition {
    let mut template = TemplateDefinition::new(record.key.clone(), record.name.clone());
    template.type_tags = type_tags(record);
    template.hit_die_override = tag(record, "HITDIE").map(normalize_hit_die);
    template.stat_minimums = parse_stat_minimums(record);
    for add in tags(record, "ADDLEVEL") {
        let mut parts = add.split('|').map(str::trim);
        if let Some(class_name) = parts.next().filter(|name| !name.is_empty()) {
            let levels = parts
                .next()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(1);
            template.add_levels.insert(class_name.to_string(), levels);
        }
    }
    parse_creature_traits(record, &mut template.traits);
    template
}

/// Collect `DEFINESTAT:MINVALUE|STAT|n` entries into a stat-minimum map.
fn parse_stat_minimums(record: &RawRecord) -> BTreeMap<String, i32> {
    let mut minimums = BTreeMap::new();
    for value in tags(record, "DEFINESTAT") {
        let parts: Vec<&str> = value.split('|').map(str::trim).collect();
        if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("MINVALUE") {
            if let Ok(min) = parts[2].parse::<i32>() {
                minimums.insert(parts[1].to_ascii_uppercase(), min);
            }
        }
    }
    minimums
}

/// Parse a PCGen `YES`/`NO` style flag.
fn parse_yes_no(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("YES") || value.trim() == "1"
}

/// Normalize a `HITDIE:` value to `dN` form.
fn normalize_hit_die(value: &str) -> String {
    let value = value.trim();
    if value.starts_with('d') || value.starts_with('D') {
        format!("d{}", value.trim_start_matches(['d', 'D']))
    } else {
        format!("d{value}")
    }
}

pub(crate) fn parse_creature_traits(record: &RawRecord, traits: &mut pcgen_core::CreatureTraits) {
    traits.reach_feet = tag(record, "REACH").and_then(|value| value.parse().ok());
    traits.reach_mult = tag(record, "REACHMULT").and_then(|value| value.parse().ok());
    traits.face = tag(record, "FACE").map(str::to_string);
    traits.race_type = tag(record, "RACETYPE").map(str::to_string);
    traits.subtypes = split_tag_list(record, "RACESUBTYPE");
    traits.natural_attacks = tags(record, "NATURALATTACKS")
        .flat_map(crate::rulepack::parse_natural_attacks)
        .collect();
    traits.unarmed_damage = tags(record, "UDAM").map(str::to_string).collect();
    traits.unarmed_mult = tag(record, "UMULT").and_then(|value| value.parse().ok());
    traits.damage_reduction = tags(record, "DR")
        .map(crate::rulepack::parse_damage_reduction)
        .collect();
    traits.spell_resistance = tag(record, "SR").map(normalize_formula_text);
    traits.challenge_rating = tag(record, "CR").map(str::to_string);
    traits.vision = tags(record, "VISION").map(str::to_string).collect();
    traits.move_clone = tags(record, "MOVECLONE").map(str::to_string).collect();
    traits.unencumbered_move = tags(record, "UNENCUMBEREDMOVE")
        .map(str::to_string)
        .collect();
    traits.attack_cycle = tags(record, "ATTACKCYCLE").map(str::to_string).collect();
}

/// Parse a PCGen `FOLLOWER:` companion modifier row.
fn parse_companion_mod(record: &RawRecord) -> Option<pcgen_core::CompanionModDefinition> {
    let (prefix, body) = record.key.split_once(':')?;
    if !prefix.eq_ignore_ascii_case("FOLLOWER") {
        return None;
    }
    let (master_variable, master_level) = body.split_once('=')?;
    let master_variable = master_variable.trim().to_string();
    let master_level = master_level.trim().parse::<u32>().ok()?;
    let mut companion = pcgen_core::CompanionModDefinition::new(
        body.trim().to_string(),
        tag(record, "TYPE").unwrap_or(&master_variable).to_string(),
    );
    companion.master_variable = master_variable;
    companion.master_level = master_level;
    companion.hit_dice_bonus = tag(record, "HD")
        .and_then(|value| value.trim().parse::<i32>().ok())
        .unwrap_or(0);
    companion.spell_resistance = tag(record, "SR").map(normalize_formula_text);
    parse_creature_traits(record, &mut companion.traits);
    for ability in tags(record, "ABILITY") {
        let parts: Vec<&str> = ability.split('|').map(str::trim).collect();
        if let Some(name) = parts.last().filter(|name| !name.is_empty()) {
            if !name.to_ascii_uppercase().starts_with("TYPE=") {
                companion.abilities.push((*name).to_string());
            }
        }
    }
    // Preserve row conditions.  Several FOLLOWER rows share the same
    // `(type, level)` id but select different companion races via `PRERACE`;
    // they must not overwrite each other.
    for tag in &record.tags {
        let upper = tag.name.to_ascii_uppercase();
        if crate::tags::is_predicate_tag(&upper) {
            companion
                .requirements
                .push(format!("{}:{}", tag.name, tag.value));
        }
    }
    for bonus in tags(record, "BONUS") {
        if let Some(parsed) = parse_companion_bonus(bonus) {
            companion.bonuses.push(parsed);
        }
    }
    Some(companion)
}

/// Parse one `BONUS:` row on a companion modifier into a structured bonus.
///
/// Companion bonuses commonly carry formulas over `mastervar("X")` and are
/// evaluated later against the master level, so the raw formula is preserved.
fn parse_companion_bonus(value: &str) -> Option<CompanionBonus> {
    let parts: Vec<&str> = value.split('|').map(str::trim).collect();
    let first = *parts.first()?;
    if first.is_empty() {
        return None;
    }
    let (target, value_index) =
        if parts.len() >= 3 && parts[1].parse::<i32>().is_err() && !parts[1].is_empty() {
            (format!("{}.{}", first, parts[1].to_ascii_uppercase()), 2)
        } else {
            (first.to_ascii_uppercase(), 1)
        };
    let formula = normalize_formula_text(parts.get(value_index)?);
    let mut bonus_type = None;
    let mut stack_key = None;
    for part in parts.iter().skip(value_index + 1) {
        if let Some(value) = part.strip_prefix("TYPE=") {
            bonus_type = Some(value.trim().to_string());
        } else if let Some(value) = part.strip_prefix("STACK=") {
            stack_key = Some(value.trim().to_string());
        }
    }
    Some(CompanionBonus {
        target,
        formula,
        bonus_type,
        stack_key,
    })
}

/// Rewrite a PCGen formula into the runtime vocabulary.
///
/// Companion and creature formulas carry `mastervar("X")` and used to be
/// normalised by a regex inside the engine, at evaluation time.  Doing it here
/// keeps the engine from reading PCGen syntax at all.
fn normalize_formula_text(value: &str) -> String {
    pcgen_core::formula_vocab::normalize_pcgen_formula(value, None)
}

const KIT_ENTRY_TARGETS: &[&str] = &[
    "GEAR", "RACE", "CLASS", "STAT", "SKILL", "ABILITY", "TEMPLATE", "KIT", "ALIGN", "NAME",
    "TABLE", "SELECT", "QTY", "OPTION", "LOOKUP", "VALUES", "EQUIPBUY", "APPLY", "LOCATION",
];

fn parse_kit_record(
    record: &RawRecord,
    current_kit: &mut Option<String>,
    kits: &mut BTreeMap<String, KitDefinition>,
) {
    let raw_key = record.key.trim();
    if let Some(name) = raw_key.strip_prefix("STARTPACK:") {
        let name = name.trim();
        let kit = parse_kit_header(name, record, "STARTPACK");
        kits.insert(name.to_string(), kit);
        *current_kit = Some(name.to_string());
        return;
    }
    if let Some(name) = raw_key.strip_prefix("TABLE:") {
        let name = name.trim();
        let kit = parse_kit_header(name, record, "TABLE");
        kits.insert(name.to_string(), kit);
        return;
    }
    let Some(kit_id) = current_kit.as_deref() else {
        return;
    };
    let Some(entry) = parse_kit_entry(record) else {
        return;
    };
    let kit = kits
        .entry(kit_id.to_string())
        .or_insert_with(|| KitDefinition {
            id: kit_id.to_string(),
            name: kit_id.to_string(),
            kind: "STARTPACK".to_string(),
            ..KitDefinition::default()
        });
    kit.entries.push(entry);
}

fn parse_kit_header(name: &str, record: &RawRecord, kind: &str) -> KitDefinition {
    let mut kit = KitDefinition {
        id: name.to_string(),
        name: name.to_string(),
        kind: kind.to_string(),
        ..KitDefinition::default()
    };
    for tag in &record.tags {
        let upper = tag.name.to_ascii_uppercase();
        match upper.as_str() {
            "TYPE" => {
                kit.type_tags.extend(
                    tag.value
                        .split(['.', '|'])
                        .map(str::trim)
                        .filter(|part| !part.is_empty())
                        .map(str::to_string),
                );
            }
            "VISIBLE" => kit.visible = Some(tag.value.clone()),
            "APPLY" => kit.apply_mode = Some(tag.value.clone()),
            "EQUIPBUY" => kit.equip_buy = Some(tag.value.clone()),
            "SELECT" => kit.select_expression = Some(tag.value.clone()),
            _ if crate::tags::is_predicate_tag(&upper) => {
                kit.conditions.push(format!("{}:{}", tag.name, tag.value));
            }
            _ => {}
        }
    }
    kit
}

fn parse_kit_entry(record: &RawRecord) -> Option<KitEntry> {
    let raw_key = record.key.trim();
    let mut target: Option<String> = None;
    let mut key: Option<String> = None;

    for candidate in KIT_ENTRY_TARGETS {
        let prefix = format!("{candidate}:");
        if let Some(rest) = raw_key.strip_prefix(&prefix) {
            target = Some((*candidate).to_string());
            key = Some(rest.trim().to_string());
            break;
        }
    }

    if target.is_none() {
        for tag in &record.tags {
            let upper = tag.name.to_ascii_uppercase();
            if KIT_ENTRY_TARGETS.contains(&upper.as_str()) {
                target = Some(upper);
                key = Some(tag.value.clone());
                break;
            }
        }
    }

    let target = target?;
    let mut entry = KitEntry {
        target: target.clone(),
        key: key.unwrap_or_else(|| record.name.clone()),
        ..KitEntry::default()
    };

    for tag in &record.tags {
        let upper = tag.name.to_ascii_uppercase();
        if upper == target {
            continue;
        }
        match upper.as_str() {
            "QTY" | "RANK" | "LEVEL" => entry.quantity = Some(tag.value.clone()),
            "OPTION" => entry.option = Some(tag.value.clone()),
            "LOOKUP" => entry.lookup = Some(tag.value.clone()),
            "VALUES" => entry.values = Some(tag.value.clone()),
            _ if crate::tags::is_predicate_tag(&upper) => {
                entry.conditions.push(format!("{}:{}", tag.name, tag.value));
            }
            _ => {
                entry.extra.insert(upper, tag.value.clone());
            }
        }
    }
    Some(entry)
}

fn parse_skill(record: &RawRecord) -> SkillDefinition {
    let mut skill = SkillDefinition::new(record.key.clone(), record.name.clone());
    skill.type_tags = split_tag_list(record, "TYPE");
    skill.key_stat = tag(record, "KEYSTAT").map(|value| value.to_uppercase());
    skill.armor_check =
        tag(record, "ACHECK").is_some_and(|value| value.eq_ignore_ascii_case("YES"));
    skill.trained_only =
        tag(record, "USEUNTRAINED").is_some_and(|value| value.eq_ignore_ascii_case("NO"));
    skill
}

fn parse_spell(record: &RawRecord) -> SpellDefinition {
    let mut spell = SpellDefinition::new(record.key.clone(), record.name.clone());
    spell.school = tag(record, "SCHOOL")
        .or_else(|| tag(record, "SCHOOLS"))
        .unwrap_or("")
        .to_string();
    spell.subschool = tag(record, "SUBSCHOOL").map(str::to_string);
    spell.descriptors = tags(record, "DESCRIPTOR")
        .flat_map(|value| value.split('.'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    if let Some(level) = tag(record, "LEVEL").and_then(|value| value.parse().ok()) {
        spell.level = level;
    }
    for class_levels in tags(record, "CLASSES") {
        for entry in class_levels.split('|') {
            let Some((classes, level)) = entry.split_once('=') else {
                continue;
            };
            let Ok(level) = level.trim().parse::<u32>() else {
                continue;
            };
            for class in classes.split(',') {
                let class = class.trim();
                if !class.is_empty() {
                    spell.class_levels.insert(class.to_string(), level);
                }
            }
        }
    }
    for domain in tags(record, "DOMAINS") {
        spell.domains.extend(
            domain
                .split([',', '|'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        );
    }
    spell.is_psionic = type_tags(record)
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case("psionic"));
    for variants in tags(record, "VARIANTS") {
        spell.variants.extend(
            variants
                .split('|')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        );
    }
    spell
}

fn parse_equipment(record: &RawRecord) -> EquipmentDefinition {
    let mut equipment = EquipmentDefinition::new(record.key.clone(), record.name.clone());
    equipment.cost_gp = tag(record, "COST").and_then(parse_number).unwrap_or(0.0);
    equipment.weight_lb = tag(record, "WT").and_then(parse_number).unwrap_or(0.0);
    equipment.max_dex_bonus = tag(record, "MAXDEX").and_then(|value| value.parse().ok());
    equipment.armor_check_penalty = tag(record, "ACCHECK")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    equipment.type_tags = type_tags(record);
    equipment.damage = tag(record, "DAMAGE").map(str::to_string);
    equipment.crit_mult = tag(record, "CRITMULT").map(str::to_string);
    equipment.crit_range = tag(record, "CRITRANGE").and_then(|value| value.parse().ok());
    equipment.wield = tag(record, "WIELD").map(str::to_string);
    equipment.base_item = tag(record, "BASEITEM").map(str::to_string);
    equipment.contains = tags(record, "CONTAINS").map(str::to_string).collect();
    equipment.slots = tag(record, "SLOTS").and_then(|value| value.parse().ok());
    equipment.location = tag(record, "LOCATION").map(str::to_string);
    equipment.proficiency = tag(record, "PROFICIENCY").map(str::to_string);
    equipment.alternate_damage = tag(record, "ALTDAMAGE").map(str::to_string);
    equipment.alternate_crit_mult = tag(record, "ALTCRITMULT").map(str::to_string);
    equipment.alternate_crit_range =
        tag(record, "ALTCRITRANGE").and_then(|value| value.parse().ok());
    equipment.alternate_types = split_tag_list(record, "ALTTYPE");
    equipment.alternate_equipment_mods = split_tag_list(record, "ALTEQMOD");
    equipment.base_quantity = tag(record, "BASEQTY").map(str::to_string);
    equipment.slot = pcgen_core::equipment::EquipSlot::from_type_tags(&equipment.type_tags);
    for bonus in tags(record, "BONUS") {
        let parts: Vec<&str> = bonus.split('|').map(str::trim).collect();
        if parts.len() >= 3
            && parts[0].eq_ignore_ascii_case("COMBAT")
            && parts[1].eq_ignore_ascii_case("AC")
        {
            equipment.armor_bonus += parts[2].parse::<i32>().unwrap_or(0);
        }
    }
    equipment
}

fn parse_deity(record: &RawRecord) -> DeityDefinition {
    let mut deity = DeityDefinition::new(record.key.clone(), record.name.clone());
    deity.alignment = tag(record, "ALIGN").map(str::to_string);
    deity.favored_weapon = tag(record, "DEITYWEAP").map(str::to_string);
    deity.pantheon = tag(record, "PANTHEON").map(str::to_string);
    deity.valid_for_deity = tag(record, "VALIDFORDEITY").map(parse_yes_no);
    deity.valid_for_follower = tag(record, "VALIDFORFOLLOWER").map(parse_yes_no);
    for domain in tags(record, "DOMAINS") {
        deity.domains.extend(
            domain
                .split([',', '|'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        );
    }
    deity
}

fn parse_domain(record: &RawRecord) -> DomainDefinition {
    let mut domain = DomainDefinition::new(record.key.clone(), record.name.clone());
    domain.granted_power = tag(record, "DESC").map(str::to_string);
    for spell in tags(record, "SPELLLEVEL") {
        let parts: Vec<&str> = spell.split('|').map(str::trim).collect();
        let mut pending_level: Option<u32> = None;
        for part in parts {
            if let Some((_, level)) = part.split_once('=') {
                pending_level = level.parse::<u32>().ok();
            } else if !part.is_empty() {
                domain.spell_list.push(part.to_string());
                if let Some(level) = pending_level {
                    domain.domain_spells.push(DomainSpell {
                        level,
                        name: part.to_string(),
                    });
                }
            }
        }
    }
    domain
}

fn ability_grants(record: &RawRecord) -> Vec<String> {
    let mut grants = Vec::new();
    for granted in tags(record, "ABILITY").chain(tags(record, "AUTO")) {
        if let Some(key) = granted.split('|').map(str::trim).rfind(|part| {
            !part.is_empty()
                && !part.eq_ignore_ascii_case("AUTOMATIC")
                && !part.eq_ignore_ascii_case("VIRTUAL")
                && !part.eq_ignore_ascii_case("NORMAL")
                && !part.starts_with("TYPE=")
                && !crate::tags::is_predicate_tag(part)
        }) {
            grants.push(key.to_string());
        }
    }
    grants
}

fn sample_class_skills(record: &RawRecord) -> Vec<String> {
    let mut skills = Vec::new();
    for skill in tags(record, "CSKILL") {
        skills.extend(
            skill
                .split([',', '|'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        );
    }
    skills
}

fn parse_substitution_variant(
    record: &RawRecord,
    substitution: &str,
    level: u32,
    class_id: Option<&str>,
) -> VariantDefinition {
    let mut variant = VariantDefinition::new(
        format!("{substitution} ~ level {level}"),
        substitution.to_string(),
    );
    variant.applies_to = class_id.map(str::to_string);
    for replaces in tags(record, "REPLACES") {
        variant.replaces.extend(
            replaces
                .split([',', '|'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        );
    }
    variant.grants = ability_grants(record);
    variant.class_skills = sample_class_skills(record);
    variant.bonus_tags = tags(record, "BONUS")
        .map(|value| format!("BONUS:{value}"))
        .collect();
    let source = Provenance::new(
        record.source.path.display().to_string(),
        format!("substitution level {level}"),
    )
    .with_line(record.source.line);
    variant.effects = tags(record, "BONUS")
        .filter_map(|value| {
            crate::rulepack::lower_bonus_effect(value, EffectPhase::Class, &source, class_id)
        })
        .collect();
    variant.spellcaster_advance =
        tags(record, "ADD").any(|value| value.to_ascii_uppercase().contains("SPELLCASTER"));
    variant
}

/// Split a `GRANTS:` value into grant entries, keeping each feature together
/// with its `PRECLASS:` guard.
///
/// `|` is overloaded: it separates a feature from its guard
/// (`Weapon Training|PRECLASS:1,Fighter=5`) *and* separates multiple grants
/// (`A|B`).  A comma likewise separates the guard's own count from its
/// `Class=level` pairs.  A fragment is therefore treated as the start of a new
/// grant only when it is neither a predicate guard nor part of one.
fn split_grant_entries(value: &str) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    for fragment in value.split('|') {
        let fragment = fragment.trim();
        if fragment.is_empty() {
            continue;
        }
        let starts_guard = crate::tags::is_predicate_tag(fragment);
        match entries.last_mut() {
            // `PRECLASS:1,Fighter=5` arrives as its own `|` fragment; append it
            // to the feature it guards.
            Some(current) if starts_guard => {
                current.push('|');
                current.push_str(fragment);
            }
            // A bare `Fighter=5` continuation of a guard already being built.
            Some(current)
                if current
                    .rsplit('|')
                    .next()
                    .is_some_and(crate::tags::is_predicate_tag)
                    && !fragment.contains('|') =>
            {
                current.push(',');
                current.push_str(fragment);
            }
            _ => entries.push(fragment.to_string()),
        }
    }
    entries
}

fn parse_ability(record: &RawRecord) -> pcgen_core::AbilityDefinition {
    pcgen_core::AbilityDefinition {
        id: entity_key(record),
        name: record.name.clone(),
        category: tag(record, "CATEGORY").map(str::to_string),
        type_tags: type_tags(record),
        stack: tag(record, "STACK").map(parse_yes_no),
        multi: tag(record, "MULT").map(parse_yes_no),
        choose: tag(record, "CHOOSE").map(str::to_string),
    }
}

fn parse_ability_category(record: &RawRecord) -> pcgen_core::AbilityCategoryDefinition {
    let mut ability_list = Vec::new();
    for list in tags(record, "ABILITYLIST") {
        ability_list.extend(
            list.split('|')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        );
    }
    pcgen_core::AbilityCategoryDefinition {
        id: entity_key(record),
        name: record.name.clone(),
        category: tag(record, "CATEGORY").map(str::to_string),
        type_tags: type_tags(record),
        ability_list,
        pool: tag(record, "POOL").map(str::to_string),
    }
}

/// True when an `ABILITY` record declares a class variant.
///
/// Two dialects express the same idea:
///
/// * Pathfinder names it *archetype* and marks the record with
///   `TYPE:Archetype.<Class>Archetype`.
/// * 3.5e names it *ACF* (alternate class feature) and marks the record with
///   `CATEGORY:ACF`, carrying the displaced feature names as type tags
///   (`TYPE:Totem Barbarian.SpecialQuality.BarbarianRage...`).  The RSRD class
///   rows guard the replaceable feature with the matching
///   `!PREABILITY:1,CATEGORY=ACF,TYPE.BarbarianRage`, so the type-tag anchors
///   line up on both sides.
///
/// `TYPE:Substitution*` covers PCGen's substitution levels.
pub(crate) fn is_variant_record(record: &RawRecord) -> bool {
    if type_tags(record).iter().any(|tag| {
        let tag = tag.to_ascii_lowercase();
        tag.contains("archetype") || tag.contains("substitution")
    }) {
        return true;
    }
    // 3.5e ACF: the pool is declared in `CATEGORY:` rather than `TYPE:`.
    is_acf_record(record)
}

fn parse_variant(record: &RawRecord) -> Option<VariantDefinition> {
    if !is_variant_record(record) {
        return None;
    }
    let mut variant = VariantDefinition::new(record.key.clone(), record.name.clone());
    variant.applies_to = tag(record, "APPLIESTO")
        .or_else(|| tag(record, "CLASS"))
        .map(str::to_string);
    for replaces in tags(record, "REPLACES") {
        variant.replaces.extend(
            replaces
                .split([',', '|'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        );
    }
    for grants in tags(record, "GRANTS") {
        variant.grants.extend(split_grant_entries(grants));
    }
    for skill in tags(record, "CSKILL") {
        variant.class_skills.extend(
            skill
                .split([',', '|'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        );
    }
    // Native PCGen archetypes usually express granted abilities through
    // ABILITY/AUTO tags rather than an explicit GRANTS tag.
    variant.grants.extend(ability_grants(record));
    if is_acf_record(record) {
        apply_acf_conventions(record, &mut variant);
    }
    Some(variant)
}

/// True when the record's pool is the 3.5e alternate-class-feature pool.
///
/// This is the one definition of "is an ACF"; [`is_variant_record`] builds on
/// it so the two can never disagree about what the ACF pool is.
fn is_acf_record(record: &RawRecord) -> bool {
    tag(record, "CATEGORY").is_some_and(|value| value.trim().eq_ignore_ascii_case("ACF"))
}

/// Fill in the fields a 3.5e ACF expresses through convention rather than
/// dedicated tags.
///
/// An ACF record looks like:
///
/// ```text
/// Ape
/// KEY:Totem Barbarian ~ Ape
/// CATEGORY:ACF
/// TYPE:Totem Barbarian.SpecialQuality.BarbarianUncannyDodge.BarbarianTrapSense...
/// ABILITY:Barbarian Class Feature|AUTOMATIC|Ape Totem ~ Climb (Half)|PREVARGTEQ:BarbarianLVL,1
/// ```
///
/// * the first type tag names the ACF group (`Totem Barbarian`), whose first
///   word is the class it applies to;
/// * tags shaped `<Class><Feature>` are the displaced class features — the same
///   anchors the RSRD class rows guard with `!PREABILITY:1,CATEGORY=ACF,TYPE.X`,
///   so they are addressed as `TYPE=` targets;
/// * grants carry their level in a `PREVARGTEQ:<Class>LVL,<n>` guard.
fn apply_acf_conventions(record: &RawRecord, variant: &mut VariantDefinition) {
    // `TYPE:` mixes three kinds of tag:
    //   descriptors     - the pool itself (`ACF`, `Internal`) or quality words
    //   feature anchors - `<Class><Feature>` namespaced by the owning class
    //                     (`ClericTurnUndead`), which is exactly what the RSRD
    //                     class row guards with
    //                     `!PREABILITY:1,CATEGORY=ACF,TYPE.ClericTurnUndead`
    //   group tags      - a named ACF family (`Totem Barbarian`)
    let descriptors = [
        "acf",
        "internal",
        "specialquality",
        "specialattack",
        "extraordinary",
        "supernatural",
        "spelllike",
        "classfeature",
        "classfeatures",
    ];
    let type_tags = type_tags(record);
    // The owning class, most authoritative source first:
    //   `PREVARGTEQ:BarbarianLVL,5` names the class outright (`Barbarian`);
    //   `SORTKEY:Cleric` is PCGen's own sort hint;
    //   `KEY:Cleric ~ Smite Evil` gives an owner, which for an ACF family is a
    //   group (`Totem Barbarian`) rather than the class itself.
    // The class matters: the displaced-feature anchors are namespaced by it
    // (`BarbarianUncannyDodge`), so a group name would match nothing.
    let class_hint = level_variable_class(record)
        .or_else(|| tag(record, "SORTKEY").map(str::to_string))
        .or_else(|| {
            let owner = record.key.split_once('~').map(|(owner, _)| owner.trim())?;
            // `Totem Barbarian` -> `Barbarian`: a trailing word that is itself a
            // known class prefix of some anchor tag.
            let last = owner.split_whitespace().last().unwrap_or(owner);
            let has_anchor = type_tags.iter().any(|tag| {
                let lower = tag.to_ascii_lowercase();
                let probe = last.to_ascii_lowercase();
                lower.starts_with(&probe) && lower.len() > probe.len() && !tag.contains(' ')
            });
            Some(if has_anchor {
                last.to_string()
            } else {
                owner.to_string()
            })
        })
        .or_else(|| {
            type_tags
                .iter()
                .find(|tag| !descriptors.contains(&tag.to_ascii_lowercase().as_str()))
                .and_then(|group| group.split_whitespace().next())
                .map(str::to_string)
        });

    let owner = class_hint.unwrap_or_default();
    if !owner.is_empty() {
        let owner_lower = owner.to_ascii_lowercase();
        for tag in &type_tags {
            let lower = tag.to_ascii_lowercase();
            if descriptors.contains(&lower.as_str()) || tag.contains(' ') {
                continue;
            }
            if lower.starts_with(&owner_lower) && lower.len() > owner_lower.len() {
                let target = format!("TYPE={tag}");
                if !variant.replaces.iter().any(|existing| existing == &target) {
                    variant.replaces.push(target);
                }
            }
        }
    }

    // `APPLIESTO`/`CLASS` are absent on ACF records; `ACF` is a pool name, not
    // a class.
    let needs_class = variant.applies_to.as_deref().is_none_or(|value| {
        value.eq_ignore_ascii_case("ACF") || value.eq_ignore_ascii_case("Internal")
    });
    if needs_class && !owner.is_empty() {
        variant.applies_to = Some(owner.clone());
    }

    // ACF grants carry their level in the tag itself; replace the plain
    // ability names collected earlier with the level-aware form.
    let leveled = acf_leveled_grants(record, &owner);
    if !leveled.is_empty() {
        variant.grants = leveled;
    }
}

/// The class named by a `PREVARGTEQ:<Class>LVL,<n>` guard, if any.
///
/// ACF records gate their grants on the owning class's level variable, which is
/// the most reliable statement of which class the variant belongs to.
fn level_variable_class(record: &RawRecord) -> Option<String> {
    for granted in tags(record, "ABILITY").chain(tags(record, "AUTO")) {
        for part in granted.split('|') {
            let Some(rest) = part.trim().strip_prefix("PREVARGTEQ:") else {
                continue;
            };
            let Some((variable, _)) = rest.split_once(',') else {
                continue;
            };
            let variable = variable.trim();
            let Some(class) = variable.strip_suffix("LVL") else {
                continue;
            };
            if !class.is_empty() {
                return Some(class.to_string());
            }
        }
    }
    None
}

/// Rebuild `ABILITY:` grants with the level taken from `PREVARGTEQ:<Class>LVL,<n>`.
fn acf_leveled_grants(record: &RawRecord, class_id: &str) -> Vec<String> {
    let mut grants = Vec::new();
    for granted in tags(record, "ABILITY").chain(tags(record, "AUTO")) {
        let parts: Vec<&str> = granted.split('|').map(str::trim).collect();
        let Some(name) = parts.iter().rev().find(|part| {
            !part.is_empty()
                && !part.eq_ignore_ascii_case("AUTOMATIC")
                && !part.eq_ignore_ascii_case("VIRTUAL")
                && !part.eq_ignore_ascii_case("NORMAL")
                && !part.starts_with("TYPE=")
                && !crate::tags::is_predicate_tag(part)
        }) else {
            continue;
        };
        let level = parts.iter().find_map(|part| {
            let rest = part.strip_prefix("PREVARGTEQ:")?;
            let (variable, value) = rest.split_once(',')?;
            if variable.trim().to_ascii_uppercase().ends_with("LVL") {
                value.trim().parse::<u32>().ok()
            } else {
                None
            }
        });
        grants.push(match level {
            Some(level) if !class_id.is_empty() => {
                format!("{name}|PRECLASS:1,{class_id}={level}")
            }
            Some(level) => format!("{name}|PRECLASS:1,{level}"),
            None => (*name).to_string(),
        });
    }
    grants
}

fn parse_equipment_modifier(record: &RawRecord) -> EquipmentModifierDefinition {
    // PCGen equipmods often declare a stable `KEY:` (used by `.pcg`
    // `EQMOD=` references); prefer it over the display key.
    let key = tag(record, "KEY").unwrap_or(&record.key);
    let mut modifier = EquipmentModifierDefinition::new(key.to_string(), record.name.clone());
    modifier.cost_gp = tag(record, "COST").and_then(parse_number).unwrap_or(0.0);
    modifier.weight_lb = tag(record, "WT").and_then(parse_number).unwrap_or(0.0);
    modifier.type_tags = type_tags(record);
    modifier.plus = tag(record, "PLUS").and_then(|value| value.parse().ok());
    modifier.item_types = split_tag_list(record, "ITYPE");
    modifier.replaces = split_tag_list(record, "REPLACES");
    modifier.charges = tag(record, "CHARGES").map(str::to_string);
    modifier.armor_types = split_tag_list(record, "ARMORTYPE");
    for bonus in tags(record, "BONUS") {
        let parts: Vec<&str> = bonus.split('|').map(str::trim).collect();
        if parts.len() >= 3
            && parts[0].eq_ignore_ascii_case("COMBAT")
            && parts[1].eq_ignore_ascii_case("AC")
        {
            modifier.armor_bonus += parts[2].parse::<i32>().unwrap_or(0);
        }
    }
    parse_creature_traits(record, &mut modifier.traits);
    modifier
}

/// Split a dot/comma separated tag (e.g. `ITYPE`, `REPLACES`) into a list.
fn split_tag_list(record: &RawRecord, name: &str) -> Vec<String> {
    tags(record, name)
        .flat_map(|value| value.split(['.', ',', '|']))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn build_class_progression(class: &ClassDefinition) -> ClassProgression {
    let mut progression = ClassProgression::from_class(class);
    let max_level = class.max_level.max(1);
    if let Some(formula) = &class.base_attack_formula {
        progression.base_attack_bonus = (1..=max_level)
            .map(|level| evaluate_class_formula(formula, level, &class.id))
            .collect();
    }
    if let Some(formula) = &class.fortitude_formula {
        progression.fortitude = (1..=max_level)
            .map(|level| evaluate_class_formula(formula, level, &class.id))
            .collect();
    }
    if let Some(formula) = &class.reflex_formula {
        progression.reflex = (1..=max_level)
            .map(|level| evaluate_class_formula(formula, level, &class.id))
            .collect();
    }
    if let Some(formula) = &class.will_formula {
        progression.will = (1..=max_level)
            .map(|level| evaluate_class_formula(formula, level, &class.id))
            .collect();
    }
    progression
}

/// Evaluate a class progression formula at one class level.
///
/// The PCGen dialect is translated first (see
/// [`pcgen_core::formula_vocab`]), so this is plain arithmetic over the
/// runtime vocabulary.  The class level stands in for both `ClassLevel_*` and
/// `CharacterLevel`, which is what the previous PCGen-token version did: at
/// this point the class level is the only level in scope.
fn evaluate_class_formula(formula: &str, level: u32, class_id: &str) -> i32 {
    let normalized = pcgen_core::formula_vocab::normalize_pcgen_formula(formula, Some(class_id));
    let mut variables: HashMap<String, i64> = HashMap::new();
    variables.insert(
        pcgen_core::formula_vocab::class_level_variable(class_id),
        i64::from(level),
    );
    variables.insert(
        pcgen_core::formula_vocab::CHARACTER_LEVEL.to_string(),
        i64::from(level),
    );
    // `var(...)` references are not resolvable while the tables are built; the
    // previous version substituted zero for them.
    pcgen_formula::evaluate_f64(&normalized, &variables)
        .map(|value| value as i32)
        .unwrap_or(0)
}

fn apply_class_bonus(class: &mut ClassDefinition, bonus: &str, owner: &str) {
    let parts: Vec<&str> = bonus.split('|').map(str::trim).collect();
    if parts.len() < 3 {
        return;
    }
    if parts[0].eq_ignore_ascii_case("COMBAT") && parts[1].eq_ignore_ascii_case("BASEAB") {
        class.attack_progression = attack_progression_from_text(parts[2]);
        class.base_attack_formula = Some(pcgen_core::formula_vocab::normalize_pcgen_formula(
            parts[2],
            Some(owner),
        ));
    } else if parts[0].eq_ignore_ascii_case("SAVE") {
        let progression = save_progression_from_text(parts[2]);
        let formula = pcgen_core::formula_vocab::normalize_pcgen_formula(parts[2], Some(owner));
        for target in parts[1].split(',') {
            let target = target.trim().to_ascii_lowercase();
            if target.contains("fortitude") || target.eq_ignore_ascii_case("all") {
                class.fortitude = progression;
                class.fortitude_formula = Some(formula.clone());
            }
            if target.contains("reflex") || target.eq_ignore_ascii_case("all") {
                class.reflex = progression;
                class.reflex_formula = Some(formula.clone());
            }
            if target.contains("will") || target.eq_ignore_ascii_case("all") {
                class.will = progression;
                class.will_formula = Some(formula.clone());
            }
        }
    }
}

fn attack_progression_from_text(text: &str) -> AttackProgression {
    let normalized = text.trim().to_ascii_lowercase();
    if normalized == "none" || normalized == "0" {
        AttackProgression::None
    } else if normalized.contains("3/4") || normalized == "moderate" {
        AttackProgression::ThreeQuarter
    } else if normalized.contains("/2") || normalized == "bad" || normalized == "half" {
        AttackProgression::Half
    } else {
        AttackProgression::Full
    }
}

fn save_progression_from_text(text: &str) -> SaveProgression {
    if text.contains("/2+2") || text.contains("+2") {
        SaveProgression::Good
    } else {
        SaveProgression::Poor
    }
}

fn parse_slot_cost(value: &str) -> Option<pcgen_core::PrestigeSlotCost> {
    match value.trim().to_ascii_uppercase().as_str() {
        "SINGLE" | "SINGLELANE" | "SINGLE_LANE" => Some(pcgen_core::PrestigeSlotCost::SingleLane),
        "DUAL" | "DUALLANE" | "DUAL_LANE" => Some(pcgen_core::PrestigeSlotCost::DualLane),
        "BOTH" | "BOTHLANES" | "BOTH_LANES" => Some(pcgen_core::PrestigeSlotCost::BothLanes),
        _ => None,
    }
}

fn casting_type_from_text(text: &str) -> CastingType {
    if text.trim().eq_ignore_ascii_case("NO") {
        CastingType::Spontaneous
    } else if text.trim().eq_ignore_ascii_case("YES") {
        CastingType::Prepared
    } else {
        CastingType::Unknown
    }
}

fn parse_spell_table(value: &str) -> Vec<u32> {
    value
        .split(',')
        .map(str::trim)
        .map(|entry| {
            entry
                .split('+')
                .next()
                .unwrap_or(entry)
                .parse::<u32>()
                .unwrap_or(0)
        })
        .collect()
}

fn parse_number(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

fn expand_size(value: &str) -> &'static str {
    match value.trim().to_ascii_uppercase().as_str() {
        "F" => "Fine",
        "D" => "Diminutive",
        "T" => "Tiny",
        "S" => "Small",
        "M" => "Medium",
        "L" => "Large",
        "H" => "Huge",
        "G" => "Gargantuan",
        "C" => "Colossal",
        _ => "Medium",
    }
}

fn type_tags(record: &RawRecord) -> Vec<String> {
    tags(record, "TYPE")
        .flat_map(|value| value.split('.'))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn tag<'a>(record: &'a RawRecord, name: &str) -> Option<&'a str> {
    record
        .tags
        .iter()
        .find(|tag| tag.name.eq_ignore_ascii_case(name))
        .map(|tag| tag.value.as_str())
}

fn tags<'a>(record: &'a RawRecord, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    record
        .tags
        .iter()
        .filter(move |tag| tag.name.eq_ignore_ascii_case(name))
        .map(|tag| tag.value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{RawTag, SourceRef};

    fn record(category: &str, key: &str, tags: &[(&str, &str)]) -> RawRecord {
        let source = SourceRef::new("/tmp/test.lst", 1, "test.pcc");
        RawRecord {
            category: category.to_string(),
            key: key.to_string(),
            name: key.to_string(),
            tags: tags
                .iter()
                .map(|(name, value)| RawTag {
                    name: (*name).to_string(),
                    value: (*value).to_string(),
                    source: source.clone(),
                })
                .collect(),
            source,
            raw: String::new(),
        }
    }

    #[test]
    fn parses_class_progression_and_spell_table() {
        let header = record("CLASS", "Fighter", &[("HD", "10"), ("BAB", "GOOD")]);
        let level = record("CLASS", "2", &[("CAST", "1,2")]);
        let typed = build_typed_definitions(&[header, level]);
        let class = typed.classes.get("Fighter").expect("class");
        assert_eq!(class.hit_die, "d10");
        assert_eq!(class.spell_slots.get(&2), Some(&vec![1, 2]));
        let progression = typed.progressions.get("Fighter").expect("progression");
        assert_eq!(progression.bab(2), 2);
    }

    #[test]
    fn parses_native_archetype_grants() {
        let archetype = record(
            "ABILITY",
            "Champion Archetype ~ Improved Critical",
            &[
                ("TYPE", "Archetype Ability.Fighter.Class Feature"),
                ("ABILITY", "Class Feature|AUTOMATIC|Improved Critical"),
            ],
        );
        let typed = build_typed_definitions(&[archetype]);
        let variant = typed
            .variants
            .get("Champion Archetype ~ Improved Critical")
            .expect("variant");
        assert!(variant
            .grants
            .iter()
            .any(|grant| grant == "Improved Critical"));
    }

    #[test]
    fn parses_substitution_levels() {
        let records = vec![
            record("CLASS", "Elemental Theurge", &[("HD", "4")]),
            record(
                "CLASS",
                "SUBSTITUTIONCLASS:Lesser Elemental Theurge Option",
                &[],
            ),
            record(
                "CLASS",
                "SUBSTITUTIONLEVEL:2",
                &[
                    ("ABILITY", "Special Ability|AUTOMATIC|Domain Access"),
                    ("BONUS", "VAR|DomainAccess|CL-1"),
                    ("ADD", "SPELLCASTER|ANY"),
                ],
            ),
        ];
        let typed = build_typed_definitions(&records);
        let variant = typed
            .variants
            .get("Lesser Elemental Theurge Option ~ level 2")
            .expect("substitution variant");
        assert_eq!(variant.applies_to.as_deref(), Some("Elemental Theurge"));
        assert!(variant.grants.iter().any(|grant| grant == "Domain Access"));
    }

    #[test]
    fn parses_psionic_spell_class_lists() {
        let spell = record(
            "SPELL",
            "Mind Thrust",
            &[("TYPE", "Psionic"), ("CLASSES", "Psion,Wilder=1")],
        );
        let typed = build_typed_definitions(&[spell]);
        let spell = typed.spells.get("mind_thrust").expect("spell");
        assert!(spell.is_psionic);
        assert_eq!(spell.class_levels.get("Psion"), Some(&1));
        assert_eq!(spell.class_levels.get("Wilder"), Some(&1));
    }
}
