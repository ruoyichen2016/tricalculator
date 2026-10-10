//! Rule IR execution.
//!
//! Collects active entities, expands `Grant` / `AUTO` closures, evaluates
//! `DefineVariable` and `ModifyFormula`, applies modifiers to a runtime
//! variable table, and records `CHOOSE` resolution.

use crate::draft::{CharacterDraft, DraftClassLevel};
use pcgen_core::progression::SaveKind;
use pcgen_core::CompiledRulePack;
use pcgen_core::{CastingType, SpellPowerType};
use pcgen_core::{ChoiceKind, EntityCategory};
use pcgen_core::{CompiledEntity, EffectAction, EntityId, ModifierOperation, Predicate};
use pcgen_formula::{Value as FormulaValue, VariableResolver};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedBonus {
    pub target: String,
    pub value: i32,
    pub bonus_type: Option<String>,
    pub stack_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingFormula {
    target: String,
    expression: String,
    bonus_type: Option<String>,
    stack_key: Option<String>,
    source_key: Option<String>,
}

/// A `BONUS:VAR|A,B|expr` waiting for its expression to resolve.
///
/// The names are already a list: lowering split them, so the engine never has
/// to parse a comma-separated string out of a target.
struct PendingVariable {
    names: Vec<String>,
    expression: String,
    source_key: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct EffectState {
    pub variables: BTreeMap<String, i32>,
    /// Variables explicitly changed by `BONUS:VAR` / `SetVariable`; defined
    /// variables must not overwrite these on later resolution passes.
    pub explicit_variables: BTreeSet<String>,
    /// Ability modifiers (`STR`..`CHA`) available to PCGen formulas.
    pub ability_modifiers: BTreeMap<String, i32>,
    pub external_modifiers: BTreeMap<String, i32>,
    /// Typed numeric bonuses.  Kept separate from `external_modifiers` so
    /// target-specific stacking (AC, saves, attack, skills) can be applied
    /// with bonus type / stack key semantics.
    pub typed_bonuses: Vec<TypedBonus>,
    pub active_abilities: BTreeSet<String>,
    pub choices: BTreeMap<String, String>,
    /// The chosen deity's alignment, for `PREDEITYALIGN` (PCGen).
    pub deity_alignment: Option<String>,
    /// Choice values bound to the entity that owned the `CHOOSE`.
    pub choice_bindings: BTreeMap<String, Vec<String>>,
    pub unresolved_choices: Vec<String>,
    pub diagnostics: Vec<String>,
    pub applied_effects: usize,
    pub structural_effects_skipped: usize,
    /// Base creature size (from the draft's race), used by size predicates.
    pub size: String,
    /// Number of legs, when known; defaults to biped (2).
    pub legs: u32,
    /// Race type tags (lower-cased), for `PRERACETYPE` predicates.
    pub race_types: Vec<String>,
    /// The race's `RACETYPE:` and `RACESUBTYPE:` values, for `PRERACE`.
    pub race_creature_type: Option<String>,
    pub race_subtypes: Vec<String>,
    /// Skill type tag (lowercased) to the skills carrying it, for
    /// `PRESKILL:1,TYPE.Perform=5`.  `predicate_holds` has no pack, so the map
    /// is built once here.
    pub skill_types: BTreeMap<String, Vec<String>>,
    /// Skills that are class skills for this character (lowercased), for
    /// `PRECSKILL`.  Built from the character's classes, including any class
    /// skills an archetype or substitution level swaps in.
    pub class_skills: BTreeSet<String>,
    /// Spell descriptor (lowercased) to the levels of the draft's known spells
    /// that carry it, for `PRESPELLDESCRIPTOR`.
    ///
    /// `predicate_holds` has no pack, so each known spell's descriptors are
    /// resolved here once, from `pack.spells`.
    pub spell_descriptors: BTreeMap<String, Vec<u32>>,
    /// Sub-school (lowercased) to the levels of the draft's known spells that
    /// carry it, for `PRESPELLSCHOOLSUB`.  Same shape and same reason as
    /// [`Self::spell_descriptors`].
    pub spell_subschools: BTreeMap<String, Vec<u32>>,
    /// School of magic (lowercased) to the levels of the draft's known spells
    /// that belong to it, for `PRESPELLSCHOOL`.  Same shape and same reason as
    /// [`Self::spell_descriptors`].
    pub spell_schools: BTreeMap<String, Vec<u32>>,
    /// Possessed item name (normalised) to its `TYPE:` tags, for `PREITEM`.
    ///
    /// The types are resolved the way the rest of the engine resolves an equipment
    /// name — the display name first, then a customised item's base item, then the
    /// name as written — so a `+2 scale mail` still counts as armour.
    pub item_types: BTreeMap<String, Vec<String>>,
    /// The character's reach in feet, for `PREREACH`.
    ///
    /// `ReachFacet.getReach` reads the *race's* `REACH` — whose key default is 5 —
    /// then lets any template override it, and finally adds
    /// `BONUS:COMBAT|REACH`.  Abilities are deliberately not consulted: the facet
    /// does not read them, and the wild-shape rows that carry `REACH:` in the data
    /// are templates.
    /// `None` means the character has no reach of its own, which PCGen reads as
    /// the key's default of 5 feet rather than as zero.
    pub reach_feet: Option<i32>,
    /// The largest base attack bonus any single class contributes, for `PREUATT`.
    ///
    /// Distinct from `total_bab`, which sums the classes: a Fighter 4 / Wizard 4
    /// has 6 there and 4 here, which is what `PreUnarmedAttackTester` asks for
    /// despite its name.
    pub highest_class_bab: i32,
    /// The `FACT:SpellType` strings of the character's own classes, for
    /// `PRESPELLCAST:TYPE=`.
    ///
    /// A string set rather than the typed `spell_powers`, because
    /// `PCClass.getSpellType` returns the fact verbatim and the tester compares it
    /// as text.
    pub spell_types: BTreeSet<String>,
    /// The character's own damage reduction by type (lower-cased), for `PREDR`.
    ///
    /// This is PCGen's *non-bonus* half: `BONUS:DR|<type>` arrives as an ordinary
    /// modifier on the target `DR.<type>` and is added at evaluation time, which
    /// is what `DamageReductionFacet.getDR` does.
    pub damage_reduction: BTreeMap<String, i32>,
    /// The character's objects per `OutputDB` location, by their `FACT` maps, for
    /// `PREFACT`.
    ///
    /// Only `abilities` and `race` are modelled — the two locations the data uses
    /// and the two the lowering accepts; see `Predicate::Fact`.
    pub location_facts: BTreeMap<String, Vec<BTreeMap<String, String>>>,
    /// The character's special abilities (`SA:`/`SAB:`), by key name, for
    /// `PRESA`.
    ///
    /// Collected from the initially-active entities before the effect loop, so a
    /// prerequisite anywhere in the pack sees the whole set rather than whichever
    /// entity happened to be processed first; entities granted during the loop add
    /// theirs as they are applied.
    pub special_abilities: BTreeSet<String>,
    /// The campaigns the pack was compiled from, for `PRECAMPAIGN`.
    ///
    /// `PRECAMPAIGN` asks about the *loaded sources*, not about the character, so
    /// this is a constant for a given pack — but `predicate_holds` has no pack, so
    /// it is carried here like the other precomputed facts.
    pub campaigns: Vec<pcgen_core::CampaignSource>,
    /// Ability key (normalised) to its name, category and `TYPE:` tags, for
    /// `PREABILITY`.  Built once because `predicate_holds` has no pack.
    pub ability_traits: BTreeMap<String, AbilityTraits>,
    /// Class id to what `PRECLASS` needs to know about it, built for the same
    /// reason.
    pub class_traits: BTreeMap<String, ClassTraits>,
    /// Vision entries from the race and the active templates (`VISION`), as
    /// written, e.g. `Darkvision (DarkvisionRange)`.
    pub vision: Vec<String>,
    /// Spell resistance from the race and the active templates, the highest
    /// winning.
    pub spell_resistance: Option<i32>,
    /// The character's home region, as `PREREGION` tests it: the active
    /// templates' `REGION`, followed by ` (SUBREGION)` when one is set.  PCGen's
    /// `Region.NONE` (`"None"`) stands in when no template declares a region.
    pub region: String,
    /// Total character class levels, excluding Monster (Racial) HD.
    pub character_class_levels: u32,
    /// Stable total BAB used by `PRETOTALAB` during predicate evaluation.
    pub total_bab: i32,
    /// Class progression base saves used by `PRECHECKBASE`.
    pub base_saves: BTreeMap<SaveKind, i32>,
    /// Base saves plus the save's ability modifier, used by `PRECHECK`.
    ///
    /// PCGen's `getTotalCheck` is `getBaseCheck + getTotalBonusTo("SAVE", kind)`,
    /// and the ability modifier reaches the save through a `BONUS:SAVE|Fortitude|CON`
    /// on the save's own definition (`rsrd__saves.lst`).  Bonuses applied by
    /// effects are deliberately excluded, exactly as `total_bab` excludes them:
    /// this value has to be answerable before the effect loop runs.
    pub total_saves: BTreeMap<SaveKind, i32>,
    /// Spell power types the character can cast (stage-A derived).
    pub spell_powers: BTreeSet<SpellPowerType>,
    /// Highest spell level with a slot per power type.
    pub max_spell_level: BTreeMap<SpellPowerType, u32>,
    /// Total spell slots keyed by `(power, spell level)`.
    /// Spell slots by (spell type text, level), for `PRESPELLTYPE`.
    ///
    /// Keyed by the class's `FACT:SpellType` text rather than the typed power,
    /// because `countSpellCastTypeLevel` matches that text — and because a class
    /// may carry one this codebase has no [`SpellPowerType`] for.
    pub spell_slots_by_type: BTreeMap<(String, u32), u32>,
    /// Character has a class that casts from a spellbook.
    pub has_spellbook: bool,
    /// Casting mode per class id.
    pub class_casting: BTreeMap<String, CastingType>,
    /// Inherent (race) plus applied (draft) templates, lower-cased.
    pub templates: BTreeSet<String>,
    /// Number of hands.
    pub hands: u32,
    /// Movement modes in feet (lower-cased mode -> rate).
    pub movement: BTreeMap<String, u32>,
    /// Equipped item names, type tags and wielding categories (lower-cased).
    pub equipment_values: BTreeSet<String>,
    /// Items occupying the primary hand (`PREEQUIPPRIMARY`).
    pub equipped_primary: BTreeSet<String>,
    /// Items occupying the secondary hand (`PREEQUIPSECONDARY`).
    pub equipped_secondary: BTreeSet<String>,
    /// Items equipped in both hands (`PREEQUIPBOTH`).
    pub equipped_both: BTreeSet<String>,
    /// Known weapon proficiencies (lower-cased), when derivable.
    pub weapon_proficiencies: BTreeSet<String>,
    /// Known languages (lower-cased).
    pub languages: BTreeSet<String>,
    /// Character domains (lower-cased).
    pub domains: BTreeSet<String>,
    /// Total Hit Dice for `PREHD`.
    pub hit_dice: u32,
}

impl VariableResolver for EffectState {
    fn resolve(&self, name: &str) -> Option<FormulaValue> {
        self.variables
            .get(name)
            .copied()
            .or_else(|| {
                self.ability_modifiers
                    .get(&name.to_ascii_uppercase())
                    .copied()
            })
            .map(|value| FormulaValue::Int(value as i64))
    }
}

/// Apply all effects reachable from the draft.
pub fn apply_effects(pack: &CompiledRulePack, draft: &CharacterDraft) -> EffectState {
    // Resolve base size from the draft's race so PREBASESIZE* predicates work.
    let base_size = pack
        .races
        .get(&draft.race_id)
        .map(|race| race.size.clone())
        .filter(|size| !size.trim().is_empty())
        .unwrap_or_else(|| "Medium".to_string());
    let race_types = pack
        .races
        .get(&draft.race_id)
        .map(|race| {
            race.type_tags
                .iter()
                .map(|tag| tag.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let class_traits: BTreeMap<String, ClassTraits> = pack
        .classes
        .iter()
        .map(|(id, class)| {
            (
                id.clone(),
                ClassTraits {
                    type_tags: class
                        .type_tags
                        .iter()
                        .map(|tag| tag.to_ascii_lowercase())
                        .collect(),
                    is_prestige: class.is_prestige,
                    is_monster: class.is_monster,
                    is_psionic: class.is_psionic,
                    is_spellcaster: class.spellcasting_ability.is_some(),
                    spell_power: class.spell_power,
                },
            )
        })
        .collect();
    let ability_traits: BTreeMap<String, AbilityTraits> = pack
        .entities
        .iter()
        .filter(|entity| entity.category.is_ability_like())
        .map(|entity| {
            (
                normalized(&entity.id.key),
                AbilityTraits {
                    name: entity.name.clone(),
                    category: entity.ability_category.clone().unwrap_or_default(),
                    types: entity
                        .type_tags
                        .iter()
                        .map(|tag| tag.to_ascii_lowercase())
                        .collect(),
                },
            )
        })
        .collect();
    let mut skill_types: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for definition in pack.skills.values() {
        for tag in &definition.type_tags {
            let entry = skill_types.entry(tag.to_ascii_lowercase()).or_default();
            entry.push(definition.id.clone());
            if !definition.name.is_empty() {
                entry.push(definition.name.clone());
            }
        }
    }
    // `PRECSKILL` asks whether a skill is a *class skill*, which is a property of
    // the character's classes rather than of anything spent.  It has to be known
    // before the effect loop, like vision and the base saves.
    let class_skills = character_class_skills(pack, draft, &skill_types);
    // `PRESPELLDESCRIPTOR` counts the character's known spells per descriptor, so
    // each draft spell has to be joined to its `DESCRIPTOR:` list here.
    let mut spell_descriptors: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut spell_subschools: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut spell_schools: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for spell in &draft.spells {
        let Some(definition) = find_spell(pack, &spell.name) else {
            continue;
        };
        for descriptor in &definition.descriptors {
            spell_descriptors
                .entry(descriptor.to_ascii_lowercase())
                .or_default()
                .push(spell.level);
        }
        if let Some(subschool) = definition.subschool.as_deref() {
            spell_subschools
                .entry(subschool.to_ascii_lowercase())
                .or_default()
                .push(spell.level);
        }
        if !definition.school.trim().is_empty() {
            spell_schools
                .entry(definition.school.to_ascii_lowercase())
                .or_default()
                .push(spell.level);
        }
    }
    let race_creature_type = pack
        .races
        .get(&draft.race_id)
        .and_then(|race| race.traits.race_type.clone())
        .map(|kind| kind.to_ascii_lowercase());
    let race_subtypes = pack
        .races
        .get(&draft.race_id)
        .map(|race| {
            race.traits
                .subtypes
                .iter()
                .map(|kind| kind.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let class_levels = character_class_levels(pack, draft);
    // `PRESPELLCAST:TYPE=` asks about the character's own classes' `SpellType`
    // facts, so it cannot be answered from `class_traits`, which holds every class
    // in the pack.
    let spell_types: BTreeSet<String> = crate::engine::effective_class_levels(pack, draft)
        .iter()
        .filter_map(|entry| {
            pack.classes
                .get(&entry.class_id)
                .or_else(|| {
                    pack.classes
                        .iter()
                        .find(|(key, _)| key.eq_ignore_ascii_case(&entry.class_id))
                        .map(|(_, class)| class)
                })
                .map(|class| class.spell_type.clone())
        })
        // `getSpellType` answers `None` when the class declares no spell type,
        // which is what `PRESPELLCAST:TYPE=None` asks about.
        .map(|spell_type| {
            if spell_type.is_empty() {
                "None".to_string()
            } else {
                spell_type
            }
        })
        .collect();
    let initial = initial_entities(pack, draft);
    let prereq_bab = crate::bab::derive_prereq_bab(pack, draft, &initial);
    let base_saves = crate::bab::derive_base_saves(pack, draft);
    let (spell_powers, max_spell_level, spell_slots_by_type, has_spellbook, class_casting) =
        derive_spellcasting_prereq(pack, draft);
    let race = pack.races.get(&draft.race_id);
    let mut templates: BTreeSet<String> = race
        .map(|race| race.templates.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    templates.extend(draft.templates.iter().map(|name| name.to_ascii_lowercase()));

    // Vision and spell resistance come from the race *and* the templates
    // together.  `merge_creature_traits` does this in the engine layer, but it
    // runs after `apply_effects` and `predicate_holds` cannot reach it — so
    // `PREVISION` and `PRESRGTEQ` could not be answered at all.  Neither merge
    // needs a gate evaluated, so both can be done here, before the effect loop.
    let mut vision: Vec<String> = Vec::new();
    let mut spell_resistance: Option<i32> = None;
    let mut absorb = |traits: &pcgen_core::CreatureTraits| {
        vision.extend(traits.vision.iter().cloned());
        if let Some(value) = traits.spell_resistance.as_deref().and_then(|raw| {
            raw.chars()
                .filter(char::is_ascii_digit)
                .collect::<String>()
                .parse::<i32>()
                .ok()
        }) {
            spell_resistance = Some(spell_resistance.map_or(value, |old| old.max(value)));
        }
    };
    if let Some(race) = pack.races.get(&draft.race_id) {
        absorb(&race.traits);
    }
    // Region is template-only, unlike vision and spell resistance: PCGen's
    // `RegionFacet.getTemplateRegion` walks `PCTemplate` objects and a `REGION:`
    // on a race row is an unrecognized token there.  The third-party data does
    // put one on races, so honouring it from the race would invent a region the
    // reference does not have.  Templates are reduced keeping the last, with the
    // region and subregion tracked independently.
    let mut region: Option<String> = None;
    let mut subregion: Option<String> = None;
    for name in &templates {
        if let Some(template) = pack.templates.get(name).or_else(|| {
            pack.templates
                .values()
                .find(|t| t.id.eq_ignore_ascii_case(name))
        }) {
            absorb(&template.traits);
            if template.traits.region.is_some() {
                region = template.traits.region.clone();
            }
            if template.traits.subregion.is_some() {
                subregion = template.traits.subregion.clone();
            }
        }
    }
    // `LegsFacet.getLegs` starts from the race's `LEGS` — `IntegerKey.LEGS`
    // defaults to 2 — and then walks the character's templates in order, the last
    // one stating a value winning.  A character with no race has none at all.
    let find_template = |name: &str| {
        pack.templates.get(name).or_else(|| {
            pack.templates
                .values()
                .find(|template| template.id.eq_ignore_ascii_case(name))
        })
    };
    let mut legs = race.map(|race| race.traits.legs.unwrap_or(2)).unwrap_or(0);
    for name in race
        .map(|race| race.templates.as_slice())
        .unwrap_or_default()
        .iter()
        .chain(draft.templates.iter())
    {
        if let Some(value) = find_template(name).and_then(|template| template.traits.legs) {
            legs = value;
        }
    }
    let hands = race.map(|race| race.hands).unwrap_or(2);
    let movement = race.map(|race| race.movement.clone()).unwrap_or_default();
    let mut equipment_values = BTreeSet::new();
    let mut equipped_primary = BTreeSet::new();
    let mut equipped_secondary = BTreeSet::new();
    let mut equipped_both = BTreeSet::new();
    for item in &draft.equipment {
        let normalized_item = normalized(item);
        equipment_values.insert(normalized_item.clone());
        for (id, definition) in &pack.equipment {
            if normalized(id) == normalized_item || normalized(&definition.name) == normalized_item
            {
                equipment_values.extend(definition.type_tags.iter().map(|tag| normalized(tag)));
                if let Some(wield) = &definition.wield {
                    equipment_values.insert(normalized(wield));
                }
                if definition.slot == pcgen_core::equipment::EquipSlot::Weapon
                    || definition.slot == pcgen_core::equipment::EquipSlot::Shield
                {
                    match definition
                        .wield
                        .as_deref()
                        .map(|wield| wield.to_ascii_lowercase())
                        .as_deref()
                    {
                        Some("twohanded") | Some("two-handed") | Some("both") => {
                            equipped_both.insert(normalized_item.clone());
                        }
                        Some("offhand") | Some("off-hand") | Some("secondary") => {
                            equipped_secondary.insert(normalized_item.clone());
                        }
                        _ => {
                            equipped_primary.insert(normalized_item.clone());
                        }
                    }
                }
            }
        }
    }
    // `PREITEM` matches possessed items by name and by type.  The name test reads
    // the draft's own list; the type test needs the definition, resolved the way
    // `Engine::resolve_equipment` does.
    let mut item_types: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in &draft.equipment {
        let customised = draft
            .equipment_base_items
            .get(&item.to_ascii_lowercase())
            .cloned();
        let display = draft
            .equipment_display_names
            .get(&item.to_ascii_lowercase())
            .cloned();
        let lookup = normalized(
            display
                .as_deref()
                .or(customised.as_deref())
                .unwrap_or(item.as_str())
                .trim(),
        );
        for (id, definition) in &pack.equipment {
            if normalized(id) == lookup || normalized(&definition.name) == lookup {
                item_types.insert(normalized(item), definition.type_tags.clone());
            }
        }
    }
    let mut weapon_proficiencies: BTreeSet<String> = draft
        .abilities
        .iter()
        .map(|(_, key)| normalized(key))
        .collect();
    let mut languages: BTreeSet<String> = BTreeSet::new();
    for &index in &initial {
        if let Some(entity) = pack.entities.get(index) {
            for bonus in &entity.metadata.weapon_bonus {
                seed_weapon_bonus(pack, &mut weapon_proficiencies, bonus);
            }
            for (from, to) in &entity.metadata.changed_proficiencies {
                weapon_proficiencies.insert(normalized(from));
                weapon_proficiencies.insert(normalized(to));
            }
            for language in &entity.metadata.language_bonus {
                languages.insert(normalized(language));
            }
        }
    }
    let mut domains = BTreeSet::new();
    for frame in &draft.advancement {
        for lane in &frame.lanes {
            domains.extend(lane.domains.iter().map(|domain| normalized(domain)));
        }
    }
    let mut ability_modifiers = BTreeMap::new();
    {
        let mut stats = draft.stats.clone();
        if let Some(race) = pack.races.get(&draft.race_id) {
            for (stat, adjustment) in &race.ability_adjustments {
                *stats.entry(stat.clone()).or_insert(10) += adjustment;
            }
        }
        for stat in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
            let score = stats.get(stat).copied().unwrap_or(10);
            ability_modifiers.insert(stat.to_string(), (score - 10).div_euclid(2));
        }
    }
    // `PRECHECK` asks for the total save, which is the base save plus the save's
    // ability modifier.  Built here rather than in the snapshot because
    // `predicate_holds` runs inside the effect loop and cannot see it there.
    let total_saves: BTreeMap<SaveKind, i32> =
        [SaveKind::Fortitude, SaveKind::Reflex, SaveKind::Will]
            .into_iter()
            .map(|kind| {
                let base = base_saves.get(&kind).copied().unwrap_or(0);
                (
                    kind,
                    base + crate::engine::save_ability_modifier(&ability_modifiers, kind),
                )
            })
            .collect();
    let mut state = EffectState {
        size: base_size,
        legs,
        race_types,
        race_creature_type,
        race_subtypes,
        skill_types,
        class_skills,
        spell_descriptors,
        spell_subschools,
        spell_schools,
        campaigns: pack.header.campaigns.clone(),
        ability_traits,
        class_traits,
        vision,
        spell_resistance,
        region: match (region.as_deref(), subregion.as_deref()) {
            (Some(region), Some(subregion)) => format!("{region} ({subregion})"),
            (Some(region), None) => region.to_string(),
            // PCGen's `Region.NONE`, which is what `getFullRegion` returns when
            // no template sets one.
            (None, None) => "None".to_string(),
            (None, Some(subregion)) => format!("None ({subregion})"),
        },
        character_class_levels: class_levels,
        ability_modifiers,
        total_bab: prereq_bab,
        highest_class_bab: crate::bab::max_class_bab(pack, draft),
        base_saves,
        total_saves,
        spell_powers,
        max_spell_level,
        spell_slots_by_type,
        has_spellbook,
        class_casting,
        spell_types,
        templates,
        hands,
        movement,
        equipment_values,
        item_types,
        equipped_primary,
        equipped_secondary,
        equipped_both,
        weapon_proficiencies,
        languages,
        domains,
        deity_alignment: draft
            .deity
            .as_deref()
            .and_then(|deity| pack.deities.get(deity))
            .and_then(|definition| definition.alignment.clone()),
        hit_dice: draft.total_level(),
        ..EffectState::default()
    };
    // The runtime formula vocabulary.  PCGen's `CL` / `TL` / `classlevel(...)`
    // are translated at lowering time (see `pcgen_core::formula_vocab`), so the
    // engine only has to publish the values those names refer to.  Doing it
    // here also means `CL` is resolved from the *class* rather than guessed
    // from whichever record happened to carry the formula.
    state.variables.insert(
        pcgen_core::formula_vocab::CHARACTER_LEVEL.to_string(),
        draft.total_level() as i32,
    );
    for entry in crate::engine::effective_class_levels(pack, draft) {
        state.variables.insert(
            pcgen_core::formula_vocab::class_level_variable(&entry.class_id),
            entry.level as i32,
        );
    }
    state
        .diagnostics
        .extend(crate::bab::bab_cycle_diagnostics(pack, &initial));
    let mut active: BTreeSet<usize> = BTreeSet::new();
    let mut queued: BTreeSet<usize> = BTreeSet::new();
    let mut processed_variant_effects: BTreeSet<(String, usize)> = BTreeSet::new();

    // Every `DEFINE`/`VAR` for a name, not just the last: PCGen "keeps the one
    // with the higher value" when several objects define the same variable.
    let mut variable_defs: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // A pre-pass over the initially-selected entities, so a variable a
    // predicate needs is known *before* the effect loop rather than after it.
    // A vision range is the case that matters: `VISION:Darkvision
    // (DarkvisionRange)` carries its distance in a variable, and
    // `PREVISION:1,Darkvision=120` is evaluated while grants are still being
    // applied — by the time the usual `resolve_variables` pass runs, the answer
    // has already been needed.
    for index in &initial {
        if let Some(entity) = pack.entities.get(*index) {
            for effect in &entity.effects {
                if let EffectAction::DefineVariable { name, expression } = &effect.action {
                    variable_defs
                        .entry(name.clone())
                        .or_default()
                        .push(expression.clone());
                }
            }
        }
    }
    resolve_variables(&mut state, &variable_defs);

    // Collect the special abilities of the initially-active entities before the
    // effect loop: `PRESA` is a prerequisite on feats and abilities, and the loop
    // evaluates a prerequisite when it reaches the entity carrying it, so reading
    // the set as it is built would make the answer depend on entity order.
    let mut abilities: Vec<String> = Vec::new();
    for index in &initial {
        let Some(entity) = pack.entities.get(*index) else {
            continue;
        };
        for effect in &entity.effects {
            if let EffectAction::GrantSpecialAbility { name } = &effect.action {
                if predicate_holds(effect.condition.as_ref(), draft, &state) {
                    abilities.push(name.clone());
                }
            }
        }
    }
    state.special_abilities.extend(abilities);

    // The same reasoning applies to `PREFACT`: it counts objects in an
    // `OutputDB` location, so the locations are filled before the loop rather
    // than as entities are reached.
    let mut location_facts: BTreeMap<String, Vec<BTreeMap<String, String>>> = BTreeMap::new();
    for index in &initial {
        let Some(entity) = pack.entities.get(*index) else {
            continue;
        };
        if entity.facts.is_empty() {
            continue;
        }
        if let Some(location) = fact_location(&entity.category) {
            location_facts
                .entry(location.to_string())
                .or_default()
                .push(entity.facts.clone());
        }
    }
    state.location_facts = location_facts;

    // `PREDR` reads the same kind of pre-loop snapshot: the character's own DR,
    // with each entry's own gate evaluated.
    let mut damage_reduction: BTreeMap<String, i32> = BTreeMap::new();
    for index in &initial {
        let Some(entity) = pack.entities.get(*index) else {
            continue;
        };
        for entry in &entity.traits.damage_reduction {
            if !predicate_holds(entry.condition.as_ref(), draft, &state) {
                continue;
            }
            let Some((amount, kind)) = split_damage_reduction(&entry.value) else {
                continue;
            };
            // Several `DR:` rows can name the same type; PCGen keeps one value per
            // type and the import already collapses them the same way.
            let slot = damage_reduction.entry(kind).or_insert(amount);
            *slot = (*slot).max(amount);
        }
    }
    state.damage_reduction = damage_reduction;

    // `ReachFacet.getReach`, in the same pre-loop pass: the race's `REACH` (the
    // key's default is 5) with templates overriding it.  `BONUS:COMBAT|REACH` is
    // added at evaluation time, where it is order-independent.
    let mut reach_feet = None;
    for index in &initial {
        let Some(entity) = pack.entities.get(*index) else {
            continue;
        };
        // The race sets the base — `None` here means the key's default — and a
        // template overrides it only when it carries a `REACH` of its own.
        if entity.category == pcgen_core::EntityCategory::Race
            || (entity.category == pcgen_core::EntityCategory::Template
                && entity.traits.reach_feet.is_some())
        {
            reach_feet = entity.traits.reach_feet;
        }
    }
    state.reach_feet = reach_feet;

    for index in initial {
        queued.insert(index);
    }

    let mut pending_formulas: Vec<PendingFormula> = Vec::new();
    let mut pending_variables: Vec<PendingVariable> = Vec::new();

    // Fixed-point closure over granted entities and variant effects.
    for _ in 0..16 {
        let current: Vec<usize> = queued.iter().copied().collect();
        queued.clear();
        let mut changed = false;
        for index in current {
            if !active.insert(index) {
                continue;
            }
            let Some(entity) = pack.entities.get(index) else {
                continue;
            };
            for effect in &entity.effects {
                if !predicate_holds(effect.condition.as_ref(), draft, &state) {
                    continue;
                }
                state.applied_effects += 1;
                apply_action(
                    effect,
                    entity.category.clone(),
                    Some(&entity.id),
                    pack,
                    draft,
                    &mut state,
                    &mut variable_defs,
                    &mut pending_formulas,
                    &mut pending_variables,
                    &mut active,
                    &mut queued,
                    &mut changed,
                );
            }
        }

        for frame in &draft.advancement {
            for lane in &frame.lanes {
                let mut variant_ids: Vec<&str> = Vec::new();
                if let Some(id) = lane.archetype.as_deref() {
                    variant_ids.push(id);
                }
                if let Some(id) = lane.substitution_level.as_deref() {
                    if !variant_ids
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(id))
                    {
                        variant_ids.push(id);
                    }
                }
                for variant_id in variant_ids {
                    let Some(variant) = pack.variants.get(variant_id).or_else(|| {
                        pack.variants
                            .iter()
                            .find(|(key, _)| key.eq_ignore_ascii_case(variant_id))
                            .map(|(_, variant)| variant)
                    }) else {
                        continue;
                    };
                    for (index, effect) in variant.effects.iter().enumerate() {
                        if !processed_variant_effects.insert((variant.id.clone(), index)) {
                            continue;
                        }
                        if !predicate_holds(effect.condition.as_ref(), draft, &state) {
                            continue;
                        }
                        state.applied_effects += 1;
                        apply_action(
                            effect,
                            EntityCategory::Class,
                            None,
                            pack,
                            draft,
                            &mut state,
                            &mut variable_defs,
                            &mut pending_formulas,
                            &mut pending_variables,
                            &mut active,
                            &mut queued,
                            &mut changed,
                        );
                    }
                }
            }
        }

        if !changed {
            break;
        }
    }

    // Resolve DEFINE variables first.  Then resolve variable assignments in
    // dependency passes before other effect formulas, so e.g. `MonkAC` is
    // available when `COMBAT.AC` is evaluated.
    resolve_variables(&mut state, &variable_defs);

    let effect_formulas = pending_formulas;

    for _ in 0..16 {
        if pending_variables.is_empty() {
            break;
        }
        let mut remaining = Vec::new();
        let mut progress = false;
        for pending in pending_variables.drain(..) {
            let normalized =
                normalize_formula(&pending.expression, &state, pending.source_key.as_deref());
            match pcgen_formula::evaluate(&normalized, &state) {
                Ok(value) => {
                    let value = formula_to_i32(&value);
                    for name in &pending.names {
                        state.explicit_variables.insert(name.clone());
                        *state.variables.entry(name.clone()).or_insert(0) += value;
                    }
                    progress = true;
                }
                Err(_) => remaining.push(pending),
            }
        }
        pending_variables = remaining;
        if !progress {
            break;
        }
        resolve_variables(&mut state, &variable_defs);
    }
    for pending in &pending_variables {
        state.diagnostics.push(format!(
            "unresolved variable formula `{}` for variable `{}`",
            pending.expression,
            pending.names.join(",")
        ));
    }

    for pending in effect_formulas {
        let normalized =
            normalize_formula(&pending.expression, &state, pending.source_key.as_deref());
        match pcgen_formula::evaluate(&normalized, &state) {
            Ok(value) => add_formula_target(&mut state, &pending, formula_to_i32(&value)),
            Err(_) => state.diagnostics.push(format!(
                "unresolved formula `{}` for target `{}`",
                pending.expression, pending.target
            )),
        }
    }
    state
}

#[allow(clippy::too_many_arguments)]
fn apply_action(
    effect: &pcgen_core::Effect,
    source_category: EntityCategory,
    source_entity: Option<&pcgen_core::EntityId>,
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    state: &mut EffectState,
    variable_defs: &mut BTreeMap<String, Vec<String>>,
    pending_formulas: &mut Vec<PendingFormula>,
    pending_variables: &mut Vec<PendingVariable>,
    active: &mut BTreeSet<usize>,
    queued: &mut BTreeSet<usize>,
    changed: &mut bool,
) {
    match &effect.action {
        EffectAction::DefineVariable { name, expression } => {
            variable_defs
                .entry(name.clone())
                .or_default()
                .push(expression.clone());
        }
        EffectAction::DeclareVariable { name, initial } => {
            state
                .variables
                .entry(name.clone())
                .or_insert(initial.floor() as i32);
        }
        EffectAction::SetVariable { name, value } => {
            state.variables.insert(name.clone(), value.floor() as i32);
        }
        EffectAction::Modify { modifier } => {
            if modifier.operation == ModifierOperation::Add {
                if structural_effect_skipped(
                    source_category.clone(),
                    source_entity,
                    pack,
                    &modifier.target,
                ) {
                    state.structural_effects_skipped += 1;
                } else if is_typed_bonus_target(&modifier.target) {
                    state.typed_bonuses.push(TypedBonus {
                        target: modifier.target.clone(),
                        value: modifier.value.floor() as i32,
                        bonus_type: modifier.bonus_type.clone(),
                        stack_key: modifier.stack_key.clone(),
                    });
                } else {
                    add_target(state, &modifier.target, modifier.value.floor() as i32);
                }
            }
        }
        EffectAction::ModifyVariable {
            names, expression, ..
        } => {
            pending_variables.push(PendingVariable {
                names: names.clone(),
                expression: expression.clone(),
                source_key: source_entity.map(|entity| normalized(&entity.key)),
            });
        }
        EffectAction::ModifyFormula {
            target,
            expression,
            bonus_type,
            stack_key,
            ..
        } => {
            if structural_effect_skipped(source_category.clone(), source_entity, pack, target) {
                state.structural_effects_skipped += 1;
            } else {
                pending_formulas.push(PendingFormula {
                    target: target.clone(),
                    expression: expression.clone(),
                    bonus_type: bonus_type.clone(),
                    stack_key: stack_key.clone(),
                    source_key: source_entity.map(|entity| normalized(&entity.key)),
                });
            }
        }
        EffectAction::Grant { entity } => {
            let source_key = source_entity.map(|entity| normalized(&entity.key));
            let resolved = EntityId {
                key: resolve_placeholders(&entity.key, state, source_key.as_deref()),
                ..entity.clone()
            };
            if resolved.entity_type.eq_ignore_ascii_case("weaponprof") {
                seed_weapon_bonus(pack, &mut state.weapon_proficiencies, &resolved.key);
            } else if resolved.entity_type.eq_ignore_ascii_case("lang")
                || resolved.entity_type.eq_ignore_ascii_case("language")
            {
                state.languages.insert(normalized(&resolved.key));
            }
            if let Some(target) = find_entity(pack, &resolved) {
                if !grant_allowed(&pack.entities[target], draft, state) {
                    state.diagnostics.push(format!(
                        "grant `{}` failed embedded prerequisites",
                        resolved.key
                    ));
                    return;
                }
                if !active.contains(&target) && queued.insert(target) {
                    *changed = true;
                }
                state.active_abilities.insert(resolved.key.clone());
            } else {
                state.active_abilities.insert(resolved.key.clone());
            }
        }
        EffectAction::GrantTag { tag } => {
            state.active_abilities.insert(tag.clone());
            let mut parts = tag.split('|');
            let category = parts.next().unwrap_or_default();
            if category.eq_ignore_ascii_case("WEAPONPROF") {
                for value in parts {
                    seed_weapon_bonus(pack, &mut state.weapon_proficiencies, value);
                }
            } else if category.eq_ignore_ascii_case("LANG")
                || category.eq_ignore_ascii_case("LANGBONUS")
            {
                for value in parts {
                    state.languages.insert(normalized(value));
                }
            }
        }
        EffectAction::Choose { spec } => {
            apply_choose(spec, draft, state, source_entity, pack);
        }
        // An entity granted during the loop contributes its special abilities as
        // it is applied.  The initially-active set was already collected above.
        EffectAction::GrantSpecialAbility { name } => {
            state.special_abilities.insert(name.clone());
        }
    }
}

fn apply_choose(
    spec: &str,
    draft: &CharacterDraft,
    state: &mut EffectState,
    source_entity: Option<&pcgen_core::EntityId>,
    pack: &CompiledRulePack,
) {
    let spec = spec.trim();
    if let Some(choice) = draft.choices.get(spec).cloned().or_else(|| {
        draft
            .choices
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(spec))
            .map(|(_, value)| value.clone())
    }) {
        record_choice(state, spec, choice, source_entity);
        return;
    }
    if let Some(entity) = source_entity {
        let wanted = normalized(&entity.key);
        if let Some(choice) = draft.choices.iter().find_map(|(key, value)| {
            let candidate = normalized(key);
            (candidate == wanted
                || candidate == entity.key.to_ascii_lowercase()
                || candidate.ends_with(&format!("::{wanted}")))
            .then_some(value.clone())
        }) {
            record_choice(state, spec, choice, source_entity);
            return;
        }
    }
    if spec.eq_ignore_ascii_case("NOCHOICE") {
        record_choice(state, spec, String::new(), source_entity);
        return;
    }
    if let Some(candidate) = resolve_dynamic_choice(spec, draft, pack) {
        record_choice(state, spec, candidate, source_entity);
        return;
    }
    if let Some(candidate) = concrete_choice_candidate(spec) {
        record_choice(state, spec, candidate, source_entity);
        return;
    }
    state.unresolved_choices.push(spec.to_string());
    state
        .diagnostics
        .push(format!("unresolved choice `{spec}`"));
}

fn record_choice(
    state: &mut EffectState,
    spec: &str,
    choice: String,
    source_entity: Option<&pcgen_core::EntityId>,
) {
    state.choices.insert(spec.to_string(), choice.clone());
    if let Some(entity) = source_entity {
        state
            .choice_bindings
            .entry(normalized(&entity.key))
            .or_default()
            .push(choice);
    }
}

fn resolve_dynamic_choice(
    spec: &str,
    draft: &CharacterDraft,
    pack: &CompiledRulePack,
) -> Option<String> {
    let upper = spec.trim().to_ascii_uppercase();
    if !upper.starts_with("CLASS|") {
        return None;
    }
    let filters: Vec<String> = spec
        .split('|')
        .skip(1)
        .flat_map(|part| part.split(','))
        .map(|part| part.trim().to_ascii_uppercase())
        .filter(|part| !part.is_empty())
        .collect();
    let all_classes = filters.iter().any(|filter| filter == "ALL");
    let mut candidates: Vec<String> = if all_classes {
        pack.classes
            .values()
            .map(|class| class.id.clone())
            .collect()
    } else {
        draft
            .class_levels
            .iter()
            .map(|entry| entry.class_id.clone())
            .chain(
                draft
                    .advancement
                    .iter()
                    .flat_map(|frame| frame.lanes.iter().map(|lane| lane.class_id.clone())),
            )
            .collect()
    };
    candidates.sort();
    candidates.dedup();
    candidates.retain(|class_id| {
        let Some(class) = pack.classes.get(class_id) else {
            return false;
        };
        filters.iter().all(|filter| match filter.as_str() {
            "ALL" | "PC" | "ANY" => true,
            "SPELLCASTER" => class.spellcasting_ability.is_some(),
            "SPELLCASTER.ARCANE" | "SPELLTYPE=ARCANE" => {
                matches!(class.spell_power, pcgen_core::SpellPowerType::Arcane)
            }
            "SPELLCASTER.DIVINE" | "SPELLTYPE=DIVINE" => {
                matches!(class.spell_power, pcgen_core::SpellPowerType::Divine)
            }
            "SPELLCASTER.PSIONIC" | "SPELLTYPE=PSIONIC" => {
                matches!(class.spell_power, pcgen_core::SpellPowerType::Psionic)
            }
            "TYPE=BASE" | "TYPE.BASE" => !class.is_prestige,
            "TYPE=PRESTIGE" | "TYPE.PRESTIGE" => class.is_prestige,
            "TYPE=MONSTER" | "TYPE.MONSTER" => class.is_monster,
            "TYPE=PSIONIC" | "TYPE.PSIONIC" => class.is_psionic,
            _ => true,
        })
    });
    if candidates.is_empty() {
        None
    } else {
        Some(candidates.join(","))
    }
}

fn concrete_choice_candidate(spec: &str) -> Option<String> {
    let tokens: Vec<&str> = spec
        .split('|')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.len() == 1 {
        let token = tokens[0];
        if token.eq_ignore_ascii_case("NOCHOICE") || ChoiceKind::is_keyword(token) {
            return None;
        }
        return Some(token.to_string());
    }
    for token in tokens.iter().skip(1) {
        let upper = token.to_ascii_uppercase();
        if ChoiceKind::is_keyword(token)
            || matches!(upper.as_str(), "LIST" | "%LIST" | "%CHOICE" | "SPELLCASTER")
            || upper.starts_with("TYPE=")
            || upper.starts_with("TYPE.")
            || upper.starts_with("PRE")
            || token.contains('[')
        {
            continue;
        }
        return Some((*token).to_string());
    }
    None
}

/// Internal ability-category `.MOD` records that select racial defaults.
///
/// PCGen expresses racial traits as `CATEGORY=Internal|Select Racial Traits`
/// records whose grants are gated by `PRERACE`.  Activate those selector
/// records automatically; the normal grant closure then pulls in the
/// race-specific default ability, whose own grants stay gated by `PREABILITY`.
fn internal_race_selector(entity: &CompiledEntity) -> bool {
    // PCGen models racial defaults as ability-category records whose grants
    // are gated by PRERACE.  After `.MOD` merging the selector may live on the
    // base ability entity, so key/name prefixes are not reliable here.
    entity.effects.iter().any(|effect| {
        matches!(effect.action, EffectAction::Grant { .. })
            && effect
                .condition
                .as_ref()
                .is_some_and(contains_race_predicate)
    })
}

fn contains_race_predicate(predicate: &Predicate) -> bool {
    match predicate {
        Predicate::HasEntity { entity } => entity.entity_type.eq_ignore_ascii_case("race"),
        // `PRERACE` now lowers to this. Missing it here made
        // `internal_race_selector` return false, so the racial-defaults entity
        // was never activated and a Gnome lost every racial trait — the gate
        // was correct but nothing ever reached it.
        Predicate::RaceMatches { .. } => true,
        Predicate::All(items) | Predicate::Any(items) => items.iter().any(contains_race_predicate),
        Predicate::Not(item) => contains_race_predicate(item),
        Predicate::AtLeast { items, .. } => items.iter().any(contains_race_predicate),
        _ => false,
    }
}

pub(crate) fn ability_matches_draft(draft: &CharacterDraft, entity: &CompiledEntity) -> bool {
    draft.abilities.iter().any(|(_, ability)| {
        ability.eq_ignore_ascii_case(&entity.id.key) || ability.eq_ignore_ascii_case(&entity.name)
    })
}

pub(crate) fn equipment_matches_draft(draft: &CharacterDraft, entity: &CompiledEntity) -> bool {
    draft.equipment.iter().any(|item| {
        normalized(item) == normalized(&entity.id.key)
            || normalized(item) == normalized(&entity.name)
    })
}

pub(crate) fn class_level_is_active(
    class_levels: &[DraftClassLevel],
    class_id: &str,
    level: u32,
) -> bool {
    class_levels.iter().any(|entry| {
        entry.level >= level
            && (entry.class_id.eq_ignore_ascii_case(class_id)
                || normalized(&entry.class_id) == normalized(class_id))
    })
}

fn initial_entities(pack: &CompiledRulePack, draft: &CharacterDraft) -> Vec<usize> {
    let mut indexes = BTreeSet::new();
    let class_levels = crate::engine::effective_class_levels(pack, draft);
    for (index, entity) in pack.entities.iter().enumerate() {
        let key = normalized(&entity.id.key);
        let active = match &entity.category {
            EntityCategory::Race => key == normalized(&draft.race_id),
            EntityCategory::Class => {
                // `name` is a fallback class identifier here, not just a label,
                // which makes it load-bearing: `entity.name` must keep PCGen's
                // `.MOD` / `.COPY=` suffixes.
                //
                // Stripping them looks like a harmless cosmetic cleanup — 38
                // RSRD entities are named things like `F.MOD` — but it is not.
                // A `.COPY=` record such as `Rogue.COPY=Rogue (CS)` is named
                // `Rogue.COPY=Rogue (CS)` and matched no class; stripped to
                // `Rogue`, it starts matching any character with Rogue levels.
                // That changed `skill_total` by 2 on two 3e oracle cases.
                draft.level_in_class(&entity.id.key) > 0 || draft.level_in_class(&entity.name) > 0
            }
            EntityCategory::ClassLevel => {
                entity
                    .id
                    .key
                    .split_once('#')
                    .is_some_and(|(class_id, level)| {
                        class_level_is_active(
                            &class_levels,
                            class_id,
                            level.parse::<u32>().unwrap_or(0),
                        )
                    })
            }
            EntityCategory::Equipment => equipment_matches_draft(draft, entity),
            EntityCategory::Spell | EntityCategory::Skill => {
                draft.active_states.iter().any(|state| {
                    normalized(state) == key || normalized(state) == normalized(&entity.name)
                })
            }
            EntityCategory::Template => draft.templates.iter().any(|template| {
                normalized(template) == key || normalized(template) == normalized(&entity.name)
            }),
            category if category.is_ability_like() => {
                internal_race_selector(entity) || ability_matches_draft(draft, entity)
            }
            _ => false,
        };
        if active {
            indexes.insert(index);
        }
    }
    indexes.into_iter().collect()
}

fn find_entity(pack: &CompiledRulePack, entity: &pcgen_core::EntityId) -> Option<usize> {
    pack.entities
        .iter()
        .position(|candidate| candidate.id == *entity)
        .or_else(|| {
            // A grant's `entity_type` is the PCGen ability *pool*
            // (`ABILITY:Special Ability|...`), while the entity's own
            // `ability_category` records that pool.  Prefer the entity whose
            // pool matches, so two abilities with the same key in different
            // pools resolve to the right one.
            //
            // PCGen ability `.MOD` records often carry the real effects while
            // the base key is an empty shell.  When a name/key match exists,
            // prefer the candidate with effects so `Halfling Racial Trait
            // Default` resolves to its `.MOD` implementation.
            let target_key = normalized(&entity.key);
            let mut key_fallback: Option<usize> = None;
            let mut name_fallback: Option<usize> = None;
            let mut effect_fallback: Option<usize> = None;
            for (index, candidate) in pack.entities.iter().enumerate() {
                let candidate_key = normalized(&candidate.id.key);
                let candidate_name = normalized(&candidate.name);
                let name_match = candidate_name == target_key
                    || candidate_name
                        .strip_suffix(".mod")
                        .is_some_and(|base| base == target_key || base.ends_with(&target_key));
                let key_match = candidate_key == target_key;
                if !key_match && !name_match {
                    continue;
                }
                if key_match
                    && candidate
                        .ability_category
                        .as_deref()
                        .is_some_and(|pool| pool.eq_ignore_ascii_case(&entity.entity_type))
                {
                    return Some(index);
                }
                if name_match && !candidate.effects.is_empty() {
                    return Some(index);
                }
                if key_match {
                    key_fallback.get_or_insert(index);
                } else {
                    name_fallback.get_or_insert(index);
                }
                if !candidate.effects.is_empty() {
                    effect_fallback.get_or_insert(index);
                }
            }
            key_fallback.or(name_fallback).or(effect_fallback)
        })
}

fn is_typed_bonus_target(target: &str) -> bool {
    let upper = target.to_ascii_uppercase();
    upper == "AC"
        || upper == "COMBAT.AC"
        || upper.starts_with("SAVE")
        || upper.starts_with("SKILL")
        || upper.contains("TOHIT")
        || upper.starts_with("WEAPON")
        || upper.starts_with("COMBAT.ATTACK")
        || upper.starts_with("ATTACK")
}

fn add_target(state: &mut EffectState, target: &str, value: i32) {
    *state
        .external_modifiers
        .entry(target.to_string())
        .or_insert(0) += value;
}

fn add_formula_target(state: &mut EffectState, pending: &PendingFormula, value: i32) {
    if is_typed_bonus_target(&pending.target) {
        state.typed_bonuses.push(TypedBonus {
            target: pending.target.clone(),
            value,
            bonus_type: pending.bonus_type.clone(),
            stack_key: pending.stack_key.clone(),
        });
    } else {
        add_target(state, &pending.target, value);
    }
}

/// Count character class levels, excluding Monster (Racial) classes.
///
/// When the draft only has advancement frames (no explicit class_levels), fall
/// back to `total_level()` rather than guessing which lanes are racial HD.
fn character_class_levels(pack: &CompiledRulePack, draft: &CharacterDraft) -> u32 {
    let levels = crate::engine::effective_class_levels(pack, draft);
    if levels.is_empty() {
        return draft.total_level();
    }
    levels
        .iter()
        .filter_map(|entry| {
            let class = pack.classes.get(&entry.class_id).or_else(|| {
                pack.classes
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case(&entry.class_id))
                    .map(|(_, class)| class)
            });
            match class {
                Some(class) if class.is_monster => None,
                _ => Some(entry.level),
            }
        })
        .sum()
}

/// Stage-A spellcasting state: derived only from class levels and static
/// class definitions, never from effects, so `PRESPELL*` predicates can be
/// evaluated without a fixed-point loop.
#[allow(clippy::type_complexity)]
fn derive_spellcasting_prereq(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
) -> (
    BTreeSet<SpellPowerType>,
    BTreeMap<SpellPowerType, u32>,
    BTreeMap<(String, u32), u32>,
    bool,
    BTreeMap<String, CastingType>,
) {
    let mut powers = BTreeSet::new();
    let mut max_spell_level: BTreeMap<SpellPowerType, u32> = BTreeMap::new();
    let mut slots: BTreeMap<(String, u32), u32> = BTreeMap::new();
    let mut has_spellbook = false;
    let mut class_casting = BTreeMap::new();
    for entry in crate::engine::effective_class_levels(pack, draft) {
        let Some(class) = pack.classes.get(&entry.class_id).or_else(|| {
            pack.classes
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(&entry.class_id))
                .map(|(_, class)| class)
        }) else {
            continue;
        };
        class_casting.insert(class.id.clone(), class.casting_type);
        if class.requires_spellbook {
            has_spellbook = true;
        }
        // `countSpellCastTypeLevel` walks every class and matches the
        // `FACT:SpellType` text with no spellcasting-ability filter — and skips a
        // class whose fact is unset, which is why an empty text contributes
        // nothing here.
        if !class.spell_type.is_empty() {
            if let Some(table) = class.spell_slots.get(&entry.level) {
                for (level, count) in table.iter().enumerate() {
                    if *count == 0 {
                        continue;
                    }
                    *slots
                        .entry((class.spell_type.to_ascii_lowercase(), level as u32))
                        .or_insert(0) += *count;
                }
            }
        }
        if class.spellcasting_ability.is_none() && !class.is_psionic {
            continue;
        }
        // Psionic classes often carry `FACT:SpellType|Psionic` without
        // `SPELLSTAT`; treat `is_psionic` as authoritative and default to INT.
        let power = if class.spell_power != SpellPowerType::Unknown {
            class.spell_power
        } else if class.is_psionic {
            SpellPowerType::Psionic
        } else {
            SpellPowerType::Unknown
        };
        if power != SpellPowerType::Unknown {
            powers.insert(power);
        }
        if let Some(table) = class.spell_slots.get(&entry.level) {
            for (level, count) in table.iter().enumerate() {
                if *count == 0 {
                    continue;
                }
                let level = level as u32;
                max_spell_level
                    .entry(power)
                    .and_modify(|current| *current = (*current).max(level))
                    .or_insert(level);
            }
        }
    }
    (powers, max_spell_level, slots, has_spellbook, class_casting)
}

fn variable_value(state: &EffectState, name: &str) -> i32 {
    let trimmed = name.trim();
    if trimmed.eq_ignore_ascii_case("%CHOICE") {
        return 0;
    }
    state
        .variables
        .get(trimmed)
        .copied()
        .or_else(|| {
            state
                .ability_modifiers
                .get(&trimmed.to_ascii_uppercase())
                .copied()
        })
        // PCGen's `getVariableValue` evaluates its argument, so a symbolic
        // operand is often a plain number: `PREVARGTEQ:3,EmpowerHealingLVL` is
        // "3 >= EmpowerHealingLVL".  Without this the literal read as 0 and the
        // comparison silently inverted.  A formula (`ceil((CL=Horizon
        // Walker)/10)`) still resolves to 0; see `PCGEN_SYNTAX_INVENTORY.md`
        // §5af.
        .or_else(|| trimmed.parse::<i32>().ok())
        .unwrap_or(0)
}

/// Does one `PREVISION` requirement hold?
///
/// A vision entry is `Kind` or `Kind (RangeVariable)`, and the distance lives in
/// that variable.  The two sides do not always spell the kind the same way — the
/// data has `VISION:Low-Light` but `PREVISION:1,Low-Light Vision=1` — so the
/// comparison ignores case, spaces and hyphens and accepts a containment either
/// way.
fn vision_spec_holds(spec: &pcgen_core::VisionSpec, state: &EffectState) -> bool {
    let fold = |value: &str| -> String {
        value
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    let wanted = fold(&spec.kind);
    state.vision.iter().any(|entry| {
        let (kind, range) = match entry.split_once('(') {
            Some((kind, rest)) => (kind.trim(), rest.trim_end_matches(')').trim()),
            None => (entry.trim(), ""),
        };
        let kind = fold(kind);
        if kind.is_empty() || !(kind.contains(&wanted) || wanted.contains(&kind)) {
            return false;
        }
        let Some(min) = spec.min_distance else {
            return true;
        };
        // The distance is the value of the variable named in the parentheses.
        let distance = state
            .variables
            .get(range)
            .copied()
            .or_else(|| {
                state
                    .variables
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(range))
                    .map(|(_, value)| *value)
            })
            .unwrap_or(0);
        distance >= min as i32
    })
}

/// What a `PRECLASS` needs to know about a class.
#[derive(Debug, Clone, Default)]
pub struct ClassTraits {
    pub type_tags: Vec<String>,
    pub is_prestige: bool,
    pub is_monster: bool,
    pub is_psionic: bool,
    pub is_spellcaster: bool,
    pub spell_power: pcgen_core::SpellPowerType,
}

/// Does one `PRECLASS` requirement hold?
///
/// `SPELLCASTER` sums the levels of every matching class, which is how PCGen
/// treats a multiclass caster: a Sorcerer 6 / Wizard 6 has twelve spellcaster
/// levels.
fn class_spec_holds(
    spec: &pcgen_core::ClassSpec,
    state: &EffectState,
    draft: &CharacterDraft,
) -> bool {
    let levels_in = |predicate: &dyn Fn(&ClassTraits) -> bool| -> u32 {
        state
            .class_traits
            .iter()
            .filter(|(_, traits)| predicate(traits))
            .map(|(id, _)| draft.level_in_class(id))
            .sum()
    };
    match spec {
        pcgen_core::ClassSpec::Class {
            class_id,
            min_level,
        } => draft.level_in_class(class_id) >= *min_level,
        pcgen_core::ClassSpec::Any { min_level } => draft.total_level() >= *min_level,
        pcgen_core::ClassSpec::Type {
            type_tag,
            min_level,
        } => {
            let wanted = type_tag.to_ascii_lowercase();
            levels_in(&|traits| traits.type_tags.contains(&wanted)) >= *min_level
        }
        pcgen_core::ClassSpec::Spellcaster { kind, min_level } => {
            let kind = kind.as_deref().map(str::to_ascii_uppercase);
            levels_in(&|traits| match kind.as_deref() {
                None => traits.is_spellcaster,
                Some("ARCANE") => {
                    matches!(traits.spell_power, pcgen_core::SpellPowerType::Arcane)
                }
                Some("DIVINE") => {
                    matches!(traits.spell_power, pcgen_core::SpellPowerType::Divine)
                }
                Some("PSIONIC") => {
                    matches!(traits.spell_power, pcgen_core::SpellPowerType::Psionic)
                }
                Some(_) => false,
            }) >= *min_level
        }
    }
}

/// What a `PREABILITY` needs to know about an ability.
#[derive(Debug, Clone, Default)]
pub struct AbilityTraits {
    pub name: String,
    pub category: String,
    pub types: Vec<String>,
}

/// Does one `PRESKILL` requirement hold?
///
/// A `TYPE.Text` requirement is met by any skill carrying that type, which is
/// what makes `PRESKILL:2,TYPE.Spy=2,TYPE.Spy=2` mean "two Spy skills", not "the
/// skill named TYPE.Spy".
fn skill_requirement_holds(
    spec: &pcgen_core::SkillRequirement,
    state: &EffectState,
    draft: &CharacterDraft,
) -> bool {
    if !spec.is_type {
        return skill_ranks(draft, &spec.name) >= f64::from(spec.min_ranks);
    }
    let wanted = spec.name.to_ascii_lowercase();
    state.skill_types.get(&wanted).is_some_and(|skills| {
        skills
            .iter()
            .any(|skill| skill_ranks(draft, skill) >= f64::from(spec.min_ranks))
    })
}

/// The character's ranks in one named skill, compared case-insensitively.
fn skill_ranks(draft: &CharacterDraft, skill: &str) -> f64 {
    draft
        .skills
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(skill))
        .map(|(_, ranks)| *ranks)
        .unwrap_or(0.0)
}

/// The character's ranks summed over the skills one `PRESKILLTOT` entry names.
///
/// `PreSkillTester` marks those sub-requirements `totalValues`, which both makes
/// it add `getTotalRank` instead of counting and stops it breaking at the first
/// match — so a `TYPE.Text` entry sums every skill of that type rather than
/// asking whether one of them reaches a threshold.
fn skill_total_ranks(
    spec: &pcgen_core::SkillSpec,
    state: &EffectState,
    draft: &CharacterDraft,
) -> f64 {
    if !spec.is_type {
        return skill_ranks(draft, &spec.name);
    }
    let Some(skills) = state.skill_types.get(&spec.name.to_ascii_lowercase()) else {
        return 0.0;
    };
    // `skill_types` records a skill's id *and* its name for every type tag it
    // carries, so a skill whose key and name agree appears twice.  A count of
    // matching skills does not care; a sum would double them.
    let mut seen = BTreeSet::new();
    let mut total = 0.0;
    for skill in skills {
        if seen.insert(skill.to_ascii_lowercase()) {
            total += skill_ranks(draft, skill);
        }
    }
    total
}

/// How many known spells carry one trait at or above `min_level`?
///
/// The window is the reference's own `aggregateSpellList(..., minLevel, 20)`, so
/// a spell known above 20th level would not count; that is kept rather than
/// simplified to an open-ended `>=`.
fn spell_trait_count(levels: Option<&Vec<u32>>, min_level: u32) -> u32 {
    levels
        .map(|levels| {
            levels
                .iter()
                .filter(|level| **level >= min_level && **level <= 20)
                .count() as u32
        })
        .unwrap_or(0)
}

/// `PreItemTester`'s name test: an exact match, or a prefix match when the pattern
/// carries a `%` wildcard — everything from the first `%` on is ignored, so
/// `Sword%` finds any sword and `%` finds the first item of any name.
fn item_name_matches(item: &str, pattern: &str) -> bool {
    let name = normalized(item);
    match pattern.find('%') {
        Some(index) => name.starts_with(&normalized(&pattern[..index])),
        None => name == normalized(pattern),
    }
}

/// `ReachFacet.getReach`: the base reach plus `BONUS:COMBAT|REACH`.
fn reach_value(state: &EffectState) -> i32 {
    let bonus: i32 = state
        .external_modifiers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("COMBAT.REACH"))
        .map(|(_, value)| *value)
        .sum();
    // `IntegerKey.REACH` is declared with a default of 5, and `getSafe` returns
    // that default when the race never set the key.
    state.reach_feet.unwrap_or(5) + bonus
}

/// Split a `DR:` value such as `5/Magic` into its amount and its type.
///
/// The type is lower-cased because `PREDR` and `BONUS:DR` name it in whatever
/// case the data uses.  A formula amount (`DR:5+HD/Magic`) does not parse as an
/// integer and is skipped, which leaves that type at 0 — the fail-closed
/// direction.
fn split_damage_reduction(value: &str) -> Option<(i32, String)> {
    let (amount, kind) = value.split_once('/')?;
    let amount = amount.trim().parse::<i32>().ok()?;
    let kind = kind.trim().to_ascii_lowercase();
    if kind.is_empty() {
        return None;
    }
    Some((amount, kind))
}

/// `DamageReductionFacet.getDR`: the object's own reduction plus `BONUS:DR`.
fn damage_reduction_value(state: &EffectState, kind: &str) -> i32 {
    let key = kind.trim().to_ascii_lowercase();
    let base = state.damage_reduction.get(&key).copied().unwrap_or(0);
    let target = format!("DR.{}", kind.trim());
    let bonus: i32 = state
        .external_modifiers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case(&target))
        .map(|(_, value)| *value)
        .sum();
    base + bonus
}

/// Which `OutputDB` location an entity's facts are visible in, if any.
///
/// `PreFactParser` validates the location against `OutputDB`, whose names are the
/// facet-backed iterables: `abilities` is `GrantedAbilityFacet` and `race` is
/// `RaceFacet`.  These are the two the third-party data uses and the two the
/// lowering accepts, so an entity outside them contributes nowhere.
fn fact_location(category: &pcgen_core::EntityCategory) -> Option<&'static str> {
    match category {
        pcgen_core::EntityCategory::Race => Some("race"),
        category if category.is_ability_like() => Some("abilities"),
        _ => None,
    }
}

/// How many of the loaded campaigns match one `PRECAMPAIGN` selector?
fn campaign_matches(
    campaigns: &[pcgen_core::CampaignSource],
    spec: &pcgen_core::CampaignSpec,
) -> u32 {
    let key_matches = |campaign: &pcgen_core::CampaignSource, key: &str| {
        campaign.identifier().eq_ignore_ascii_case(key.trim())
    };
    campaigns
        .iter()
        .filter(|campaign| match spec {
            pcgen_core::CampaignSpec::Key(key) => campaign.is_root && key_matches(campaign, key),
            pcgen_core::CampaignSpec::BookType(book_type) => {
                campaign.is_root && campaign.has_book_type(book_type)
            }
            pcgen_core::CampaignSpec::Includes(key) => key_matches(campaign, key),
            pcgen_core::CampaignSpec::IncludesBookType(book_type) => {
                campaign.has_book_type(book_type)
            }
        })
        .count() as u32
}

/// Resolve a known spell's definition by key or by display name.
fn find_spell<'a>(
    pack: &'a CompiledRulePack,
    name: &str,
) -> Option<&'a pcgen_core::SpellDefinition> {
    pack.spells.get(name).or_else(|| {
        pack.spells
            .iter()
            .find(|(key, spell)| {
                key.eq_ignore_ascii_case(name) || spell.name.eq_ignore_ascii_case(name)
            })
            .map(|(_, spell)| spell)
    })
}

/// Map a PCGen save name to its kind.  The abbreviations are what the data
/// uses in `PRECHECK`-style gates as well as the full names.
fn save_kind(name: &str) -> Option<SaveKind> {
    match name.trim().to_ascii_lowercase().as_str() {
        "fort" | "fortitude" => Some(SaveKind::Fortitude),
        "ref" | "reflex" => Some(SaveKind::Reflex),
        "will" => Some(SaveKind::Will),
        _ => None,
    }
}

/// Does one `PRECSKILL` selector name a class skill for this character?
fn class_skill_spec_holds(spec: &pcgen_core::SkillSpec, state: &EffectState) -> bool {
    let wanted = spec.name.trim().to_ascii_lowercase();
    if wanted.is_empty() {
        return false;
    }
    if !spec.is_type {
        return state.class_skills.contains(&wanted);
    }
    state.skill_types.get(&wanted).is_some_and(|skills| {
        skills
            .iter()
            .any(|skill| state.class_skills.contains(&skill.to_ascii_lowercase()))
    })
}

/// Does one `PRERACE` value describe this character's race?
fn race_spec_matches(
    spec: &pcgen_core::RaceSpec,
    state: &EffectState,
    draft: &CharacterDraft,
) -> bool {
    let race = draft.race_id.as_str();
    let lower = |value: &str| value.trim().to_ascii_lowercase();
    match spec {
        pcgen_core::RaceSpec::Name(name) => lower(race) == lower(name),
        // `Elf%` is a prefix match; a bare `%` matches any race.
        pcgen_core::RaceSpec::Pattern(pattern) => {
            let prefix = lower(pattern).trim_end_matches('%').to_string();
            prefix.is_empty() || lower(race).starts_with(&prefix)
        }
        pcgen_core::RaceSpec::TypeTag(tag) => state
            .race_types
            .iter()
            .any(|candidate| candidate == &lower(tag)),
        pcgen_core::RaceSpec::CreatureType(kind) => state
            .race_creature_type
            .as_deref()
            .is_some_and(|candidate| candidate == lower(kind)),
        pcgen_core::RaceSpec::Subtype(kind) => state
            .race_subtypes
            .iter()
            .any(|candidate| candidate == &lower(kind)),
    }
}

fn entity_is_active(state: &EffectState, draft: &CharacterDraft, entity: &EntityId) -> bool {
    match entity.entity_type.to_ascii_lowercase().as_str() {
        "race" => normalized(&draft.race_id) == normalized(&entity.key),
        "class" => draft.level_in_class(&entity.key) > 0,
        _ => state
            .active_abilities
            .iter()
            .any(|ability| ability.eq_ignore_ascii_case(&entity.key)),
    }
}

pub(crate) fn predicate_holds(
    predicate: Option<&Predicate>,
    draft: &CharacterDraft,
    state: &EffectState,
) -> bool {
    let Some(predicate) = predicate else {
        return true;
    };
    match predicate {
        Predicate::Always => true,
        // An unreadable gate is kept in the IR for reporting.  Applying the
        // effect is an intentional over-approximation: the alternative was to
        // discard the whole effect when lowering, which made the loss silent.
        Predicate::Unsupported { .. } => true,
        Predicate::ActiveState { states } => states.iter().any(|state| {
            draft
                .active_states
                .iter()
                .any(|active| normalized(active) == normalized(state))
        }),
        Predicate::All(items) => items
            .iter()
            .all(|item| predicate_holds(Some(item), draft, state)),
        Predicate::Any(items) => items
            .iter()
            .any(|item| predicate_holds(Some(item), draft, state)),
        Predicate::Not(item) => !predicate_holds(Some(item), draft, state),
        Predicate::Stat { stat, min, max } => {
            let value = draft.stats.get(stat).copied().unwrap_or(10);
            min.is_none_or(|min| value >= min) && max.is_none_or(|max| value <= max)
        }
        Predicate::HasEntity { entity } => entity_is_active(state, draft, entity),
        Predicate::Variable { name, min, max } => {
            let value = state.variables.get(name).copied().unwrap_or(0);
            min.is_none_or(|min| value >= min) && max.is_none_or(|max| value <= max)
        }
        Predicate::SkillRanks { specs, min_count } => {
            let matches = specs
                .iter()
                .filter(|spec| skill_requirement_holds(spec, state, draft))
                .count() as u32;
            matches >= (*min_count).max(1)
        }
        // `PreCSkillTester` asks `character.isClassSkill(skill)` per listed
        // selector and compares the count.  A `TYPE.` selector is satisfied when
        // any skill of that type is a class skill.
        Predicate::ClassSkill { specs, min_count } => {
            let matches = specs
                .iter()
                .filter(|spec| class_skill_spec_holds(spec, state))
                .count() as u32;
            matches >= (*min_count).max(1)
        }
        // `PreAlignTester.alignMatches` first compares the character's alignment
        // with the named one, and then has one special case: the key `Deity`
        // matches when the character's alignment equals their *deity's*.  Both
        // halves matter — every game mode also defines a real alignment whose
        // key is `Deity` (`Deity's`), so the literal comparison comes first.
        Predicate::Alignment { allowed } => allowed.iter().any(|alignment| {
            if alignment.eq_ignore_ascii_case(&draft.alignment) {
                return true;
            }
            alignment.eq_ignore_ascii_case("Deity")
                && state
                    .deity_alignment
                    .as_deref()
                    .is_some_and(|deity| deity.eq_ignore_ascii_case(&draft.alignment))
        }),
        Predicate::DeityAlignment { allowed } => {
            state.deity_alignment.as_deref().is_some_and(|deity| {
                allowed
                    .iter()
                    .any(|alignment| alignment.eq_ignore_ascii_case(deity))
            })
        }
        Predicate::CharacterLevel { min, max } => {
            let level = draft.total_level();
            min.is_none_or(|min| level >= min) && max.is_none_or(|max| level <= max)
        }
        Predicate::CharacterClassLevel { min, max } => {
            let value = state.character_class_levels;
            min.is_none_or(|min| value >= min) && max.is_none_or(|max| value <= max)
        }
        Predicate::TotalBab { min } => state.total_bab >= *min as i32,
        // `PreUnarmedAttackTester` compares the largest per-class base attack
        // bonus, not the summed one — see `HighestClassBab`.
        Predicate::HighestClassBab { min } => state.highest_class_bab >= *min,
        Predicate::BaseSave { save, min } => save_kind(save)
            .is_some_and(|kind| state.base_saves.get(&kind).copied().unwrap_or(0) >= *min),
        // `PreCheckTester` counts how many of the listed saves clear their
        // minimum and compares that count with the operand.
        Predicate::Check {
            base,
            specs,
            min_count,
        } => {
            let saves = if *base {
                &state.base_saves
            } else {
                &state.total_saves
            };
            let met = specs
                .iter()
                .filter(|spec| {
                    save_kind(&spec.save)
                        .is_some_and(|kind| saves.get(&kind).copied().unwrap_or(0) >= spec.min)
                })
                .count() as u32;
            met >= (*min_count).max(1)
        }
        Predicate::Gender { prefix } => draft.gender.starts_with(prefix.as_str()),
        // `PreItemTester` counts, per listed entry, how many of the character's
        // possessed items match, and the list wrapper sums those counts.  A name
        // entry breaks at its first match, so it contributes at most one; a `TYPE`
        // entry has no such break and contributes every matching item.
        Predicate::Item { specs, min_count } => {
            let available: u32 = specs
                .iter()
                .map(|spec| match spec {
                    pcgen_core::ItemSpec::Name(pattern) => u32::from(
                        draft
                            .equipment
                            .iter()
                            .any(|item| item_name_matches(item, pattern)),
                    ),
                    pcgen_core::ItemSpec::Types(types) => draft
                        .equipment
                        .iter()
                        .filter(|item| {
                            let found = state.item_types.get(&normalized(item));
                            types.iter().all(|kind| {
                                found.is_some_and(|tags| {
                                    tags.iter().any(|tag| tag.eq_ignore_ascii_case(kind.trim()))
                                })
                            })
                        })
                        .count() as u32,
                })
                .sum();
            available >= (*min_count).max(1)
        }
        Predicate::ReachAtLeast { feet } => reach_value(state) >= *feet,
        Predicate::ReachAtMost { feet } => reach_value(state) <= *feet,
        // `PreSkillTotalParser` marks every listed skill `totalValues`, so
        // `PreSkillTester` adds each matching skill's ranks and the group compares
        // that total with the operand.  A formula with no `=` leaves the operand at
        // 0, which is why `PRESKILLTOT:Spot,Listen` always holds.
        Predicate::SkillTotal { specs, total } => {
            let ranks: f64 = specs
                .iter()
                .map(|spec| skill_total_ranks(spec, state, draft))
                .sum();
            ranks >= f64::from(*total)
        }
        // `PreDamageReductionTester` compares the character's reduction for one
        // type with the pair's value; its sub-prerequisites are not
        // `countMultiples`, so each contributes 0 or 1 and the wrapper counts how
        // many of the listed reductions hold.
        Predicate::DamageReduction { specs, min_count } => {
            let matched = specs
                .iter()
                .filter(|spec| damage_reduction_value(state, &spec.kind) >= spec.min)
                .count() as u32;
            matched >= (*min_count).max(1)
        }
        // `PreFactTester` walks the objects in one `OutputDB` location and counts
        // those whose fact equals the target value; the list wrapper sums the
        // counts.  Values are compared case-insensitively: 215 of the 217 fact
        // names the third-party data tests are declared `DATAFORMAT:BOOLEAN`,
        // where PCGen's typed parse makes `true` and `True` the same value.
        Predicate::Fact {
            location,
            specs,
            min_count,
        } => {
            let objects = state.location_facts.get(location).map(Vec::as_slice);
            let matched = specs
                .iter()
                .map(|spec| {
                    objects
                        .unwrap_or_default()
                        .iter()
                        .filter(|facts| {
                            facts
                                .get(&spec.name)
                                .is_some_and(|value| value.eq_ignore_ascii_case(spec.value.trim()))
                        })
                        .count() as u32
                })
                .sum::<u32>();
            matched >= (*min_count).max(1)
        }
        // `PreSpecialAbilityTester` counts, for each listed text, the character's
        // abilities whose key name *starts with* it, and the list wrapper sums the
        // counts.  `startsWith` is load-bearing: an `SAB`'s key name is its first
        // `|`-separated token, so `Sneak Attack +%d%` is found by `Sneak Attack`.
        Predicate::SpecialAbility {
            prefixes,
            min_count,
        } => {
            let matched = prefixes
                .iter()
                .map(|prefix| {
                    let prefix = prefix.trim().to_ascii_lowercase();
                    state
                        .special_abilities
                        .iter()
                        .filter(|ability| ability.to_ascii_lowercase().starts_with(&prefix))
                        .count() as u32
                })
                .sum::<u32>();
            matched >= (*min_count).max(1)
        }
        // `PreCharactertypeTester` compares the character's type with each listed
        // key; the list wrapper sums the matches, and a character has exactly one
        // type.
        Predicate::CharacterType { types, min_count } => {
            let matched = types
                .iter()
                .filter(|kind| {
                    kind.trim()
                        .eq_ignore_ascii_case(draft.character_type.trim())
                })
                .count() as u32;
            matched >= (*min_count).max(1)
        }
        // `PreRegionTester` upper-cases both sides and asks `startsWith`.  The
        // region is a prefix rather than an equality because the publishers
        // write `PREREGION:Varisia` for a region named `Varisia (Magnimar)`.
        Predicate::Region { prefix } => state
            .region
            .to_ascii_uppercase()
            .starts_with(&prefix.to_ascii_uppercase()),
        Predicate::SpellResistance { min, max } => {
            let value = state.spell_resistance.unwrap_or(0);
            min.is_none_or(|min| value >= min) && max.is_none_or(|max| value <= max)
        }
        Predicate::Vision { specs, min_count } => {
            let met = specs
                .iter()
                .filter(|spec| vision_spec_holds(spec, state))
                .count() as u32;
            met >= (*min_count).max(1)
        }
        Predicate::ClassRequirement { specs, min_count } => {
            let met = specs
                .iter()
                .filter(|spec| class_spec_holds(spec, state, draft))
                .count() as u32;
            met >= (*min_count).max(1)
        }
        Predicate::AbilityRequirement {
            category,
            wanted,
            excluded,
            min_count,
        } => {
            // The active abilities in the required category, with whatever the
            // pack knows about each.
            let in_category: Vec<(&String, Option<&AbilityTraits>)> = state
                .active_abilities
                .iter()
                .filter(|key| {
                    category.eq_ignore_ascii_case("Ability")
                        || state
                            .ability_traits
                            .get(&normalized(key))
                            .is_some_and(|traits| traits.category.eq_ignore_ascii_case(category))
                })
                .map(|key| (key, state.ability_traits.get(&normalized(key))))
                .collect();
            let named = |name: &str| {
                in_category.iter().any(|(key, traits)| {
                    key.eq_ignore_ascii_case(name)
                        || traits.is_some_and(|traits| traits.name.eq_ignore_ascii_case(name))
                })
            };
            // `[Name]` is an exclusion: every one of them must be absent.
            if excluded.iter().any(|name| named(name)) {
                return false;
            }
            // Nothing wanted means only the exclusions mattered.
            if wanted.is_empty() {
                return true;
            }
            // The count is the number of *held abilities* that match any of the
            // wanted refs, not the number of refs satisfied: the reference's
            // `PREABILITY:3,Category=Feat,TYPE.Metamagic` lists one type and
            // asks for three feats carrying it.
            let matches = in_category
                .iter()
                .filter(|(key, traits)| {
                    wanted.iter().any(|wanted| match wanted {
                        pcgen_core::AbilityRef::Name(name) => {
                            key.eq_ignore_ascii_case(name)
                                || traits
                                    .is_some_and(|traits| traits.name.eq_ignore_ascii_case(name))
                        }
                        pcgen_core::AbilityRef::Type(kind) => {
                            let kind = kind.to_ascii_lowercase();
                            traits.is_some_and(|traits| traits.types.contains(&kind))
                        }
                    })
                })
                .count() as u32;
            matches >= (*min_count).max(1)
        }
        Predicate::HasTag { tag } => state
            .active_abilities
            .iter()
            .any(|ability| ability.eq_ignore_ascii_case(tag)),
        Predicate::SizeAtLeast { size } => pcgen_core::size_at_least(&state.size, size),
        Predicate::SizeAtMost { size } => pcgen_core::size_at_most(&state.size, size),
        Predicate::LegsAtLeast { count } => state.legs >= *count,
        Predicate::LegsAtMost { count } => state.legs <= *count,
        Predicate::RuleEnabled { rule } => rule_enabled(rule, draft),
        Predicate::AtLeast { n, items } => {
            items
                .iter()
                .filter(|item| predicate_holds(Some(item), draft, state))
                .count() as u32
                >= *n
        }
        Predicate::RaceType { type_tag } => state
            .race_types
            .iter()
            .any(|candidate| candidate == &type_tag.to_ascii_lowercase()),
        Predicate::RaceMatches { specs, min_count } => {
            let matches = specs
                .iter()
                .filter(|spec| race_spec_matches(spec, state, draft))
                .count() as u32;
            matches >= (*min_count).max(1)
        }
        Predicate::AgeCategory { category } => draft
            .choices
            .get("AGE.CATEGORY")
            .is_some_and(|value| value.eq_ignore_ascii_case(category)),
        // `PreSpellTypeTester` asks `countSpellCastTypeLevel` for the listed type
        // at the listed level and compares the answer with 1; the list's
        // sub-requirements count multiples, so the wrapper sums those counts and
        // compares them with the operand.  Each pair is therefore counted on its
        // own — two pairs naming the same type count its slots twice, which is
        // what the reference does.
        Predicate::SpellType { specs, count } => {
            let available: u32 = specs
                .iter()
                .map(|spec| {
                    state
                        .spell_slots_by_type
                        .iter()
                        .filter(|((spell_type, level), _)| {
                            spec.spell_type.as_deref().is_none_or(|wanted| {
                                // `countSpellCastTypeLevel` treats `Any` as the
                                // wildcard and compares case-insensitively.
                                wanted.eq_ignore_ascii_case("any")
                                    || wanted.eq_ignore_ascii_case(spell_type)
                            }) && *level >= spec.min_level
                        })
                        .map(|(_, slots)| *slots)
                        .sum::<u32>()
                })
                .sum();
            available >= (*count).max(1)
        }
        // `PreSpellDescriptorTester` counts the known spells matching its one
        // descriptor at or above the required level, and the list wrapper adds
        // those counts up before comparing with the operand.  The level window
        // is the reference's: `aggregateSpellList(..., requiredLevel, 20)`.
        Predicate::SpellDescriptor { specs, count } => {
            let available: u32 = specs
                .iter()
                .map(|spec| {
                    spell_trait_count(
                        state
                            .spell_descriptors
                            .get(&spec.descriptor.to_ascii_lowercase()),
                        spec.min_level,
                    )
                })
                .sum();
            available >= (*count).max(1)
        }
        // `PreSpellSchoolSubTester` is the descriptor tester with the
        // sub-school slot of `aggregateSpellList` filled in, so the pairs sum
        // here as well.
        Predicate::SpellSchoolSub { specs, count } => {
            let available: u32 = specs
                .iter()
                .map(|spec| {
                    spell_trait_count(
                        state
                            .spell_subschools
                            .get(&spec.subschool.to_ascii_lowercase()),
                        spec.min_level,
                    )
                })
                .sum();
            available >= (*count).max(1)
        }
        Predicate::HasSpellbook => state.has_spellbook,
        // `PreSpellCastTester` counts the classes whose `getSpellType` equals the
        // value, case-insensitively, and compares the count with the default
        // operand of 1 — so this is "some class has this spell type".
        Predicate::HasSpellType { spell_type } => state
            .spell_types
            .iter()
            .any(|known| known.eq_ignore_ascii_case(spell_type.trim())),
        // `PreCampaignTester` counts matching *campaigns* and compares the count
        // with the operand, summing across the listed selectors.  A plain name
        // and `BOOKTYPE=` count only the selected (root) source; `INCLUDES=` and
        // `INCLUDESBOOKTYPE=` also count what it includes.
        Predicate::Campaign { specs, min_count } => {
            let available: u32 = specs
                .iter()
                .map(|spec| campaign_matches(&state.campaigns, spec))
                .sum();
            available >= (*min_count).max(1)
        }
        Predicate::TemplateCount { names, min_count } => {
            names
                .iter()
                .filter(|name| state.templates.contains(*name))
                .count() as u32
                >= *min_count
        }
        Predicate::HandsAtLeast { count } => state.hands >= *count,
        Predicate::HandsAtMost { count } => state.hands <= *count,
        Predicate::MovementAtLeast { mode, feet } => {
            state.movement.get(mode).is_some_and(|rate| *rate >= *feet)
        }
        Predicate::MovementCount { min_count, specs } => {
            specs
                .iter()
                .filter(|spec| {
                    state
                        .movement
                        .get(&spec.mode)
                        .is_some_and(|rate| *rate >= spec.feet)
                })
                .count() as u32
                >= *min_count
        }
        Predicate::Equipped { slot, values } => {
            let matches = |item: &String, value: &str| item == value || item.contains(value);
            let in_scope = |set: &BTreeSet<String>| {
                values
                    .iter()
                    .any(|value| set.iter().any(|item| matches(item, value)))
            };
            match slot.as_str() {
                "primary" => in_scope(&state.equipped_primary) || in_scope(&state.equipped_both),
                "secondary" => {
                    in_scope(&state.equipped_secondary) || in_scope(&state.equipped_both)
                }
                "both" => in_scope(&state.equipped_both),
                _ => in_scope(&state.equipment_values),
            }
        }
        Predicate::WeaponProf { names } => names.iter().any(|name| {
            state
                .weapon_proficiencies
                .iter()
                .any(|prof| prof == name || prof.contains(name))
        }),
        Predicate::Wield { categories } => categories.iter().any(|category| {
            state
                .equipment_values
                .iter()
                .any(|item| item == category || item.contains(category))
        }),
        Predicate::Deity { .. } => {
            // Temporary: deity prerequisites are disabled until the deity
            // selection/faith pipeline is fully adapted to the new UI.
            true
        }
        Predicate::Domain { names } => names.iter().any(|name| {
            state
                .domains
                .iter()
                .any(|domain| domain == name || domain.contains(name))
        }),
        Predicate::HitDice { min, max } => {
            min.is_none_or(|min| state.hit_dice >= min)
                && max.is_none_or(|max| state.hit_dice <= max)
        }
        Predicate::VariableCompare {
            left,
            right,
            greater,
            strict,
        } => {
            let left = variable_value(state, left);
            let right = variable_value(state, right);
            match (*greater, *strict) {
                (true, true) => left > right,
                (true, false) => left >= right,
                (false, true) => left < right,
                (false, false) => left <= right,
            }
        }
        Predicate::VariableEquals {
            left,
            right,
            equals,
        } => {
            let left = variable_value(state, left);
            let right = variable_value(state, right);
            if *equals {
                left == right
            } else {
                left != right
            }
        }
        Predicate::ArmorType { types } => types.iter().any(|armor_type| {
            state
                .equipment_values
                .iter()
                .any(|item| item.contains(armor_type))
        }),
        Predicate::ArmorProf { categories } => categories.iter().any(|category| {
            state
                .equipment_values
                .iter()
                .any(|item| item.contains(category))
        }),
        Predicate::ShieldProf { categories } => categories.iter().any(|category| {
            state
                .equipment_values
                .iter()
                .any(|item| item.contains(category))
        }),
        Predicate::KnownSpell { names } => names.iter().any(|name| {
            draft
                .spells
                .iter()
                .any(|spell| spell.name.eq_ignore_ascii_case(name))
        }),
        Predicate::Subclass { names } => names.iter().any(|name| {
            draft
                .advancement
                .iter()
                .flat_map(|frame| frame.lanes.iter())
                .any(|lane| {
                    lane.subclass
                        .as_deref()
                        .or(lane.substitution_level.as_deref())
                        .is_some_and(|subclass| subclass.eq_ignore_ascii_case(name))
                })
                || draft
                    .class_subclasses
                    .values()
                    .any(|subclass| subclass.eq_ignore_ascii_case(name))
        }),
        Predicate::Language { names } => names.iter().any(|name| {
            state
                .languages
                .iter()
                .any(|language| language == &normalized(name))
        }),
        // `PreSpellSchoolTester` fills the school slot of `aggregateSpellList`,
        // so this is the sub-school tester one field over: the pairs sum and the
        // count is a total.
        Predicate::SpellSchool { specs, count } => {
            let available: u32 = specs
                .iter()
                .map(|spec| {
                    spell_trait_count(
                        state.spell_schools.get(&spec.school.to_ascii_lowercase()),
                        spec.min_level,
                    )
                })
                .sum();
            available >= (*count).max(1)
        }
        // The pre-`SpellSchool` shape, still read from old packs.  It never
        // stored the count, so its entries stay "all of them" — but the matching
        // is the corrected one, against the spell's `SCHOOL:`.  The old test
        // compared the school name with the spell's *name*, which no spell but a
        // handful happens to contain; preserving that would be preserving a bug.
        Predicate::SpellSchoolLevel { entries } => entries.iter().all(|(school, level)| {
            spell_trait_count(
                state.spell_schools.get(&school.to_ascii_lowercase()),
                *level,
            ) > 0
        }),
        Predicate::CastingTypeIs { prepared } => state.class_casting.values().any(|mode| {
            if *prepared {
                *mode == CastingType::Prepared
            } else {
                matches!(mode, CastingType::Spontaneous | CastingType::Innate)
            }
        }),
    }
}

/// PCGen game-mode rules (`PRERULE`) default to *on*; characters may disable
/// them explicitly via the draft's choice map (`RULE:<name> = off`).
fn rule_enabled(rule: &str, draft: &CharacterDraft) -> bool {
    !matches!(
        draft
            .choices
            .get(&format!("RULE:{rule}"))
            .map(String::as_str),
        Some("off") | Some("false") | Some("0") | Some("no")
    )
}

fn resolve_variables(state: &mut EffectState, definitions: &BTreeMap<String, Vec<String>>) {
    if definitions.is_empty() {
        return;
    }
    let names: Vec<String> = definitions.keys().cloned().collect();
    let index: BTreeMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect();
    let mut adjacency = vec![Vec::new(); names.len()];
    for (position, name) in names.iter().enumerate() {
        // Every expression for the name contributes dependencies.
        let expression = definitions
            .get(name)
            .map(|expressions| expressions.join("+"))
            .unwrap_or_default();
        for dependency in variable_dependencies(&expression) {
            if let Some(&dependency_index) = index.get(dependency.as_str()) {
                // dependency -> dependent
                adjacency[dependency_index].push(position);
            }
        }
    }
    let Some(order) = pcgen_core::topological_sort(&adjacency) else {
        let cyclic: Vec<&str> = names
            .iter()
            .filter(|name| {
                let expression = definitions
                    .get(*name)
                    .map(|expressions| expressions.join("+"))
                    .unwrap_or_default();
                !variable_dependencies(&expression).is_empty()
            })
            .map(String::as_str)
            .collect();
        state
            .diagnostics
            .push(format!("variable dependency cycle involving {cyclic:?}"));
        return;
    };

    for position in order {
        let name = &names[position];
        if state.explicit_variables.contains(name) {
            continue;
        }
        // The reference: "If it finds more than one, it keeps the one with the
        // higher value." Expressions that fail are reported and skipped rather
        // than discarding the whole name.
        let mut best: Option<i32> = None;
        for expression in definitions.get(name).map(Vec::as_slice).unwrap_or_default() {
            let normalized = normalize_formula(expression, state, None);
            match pcgen_formula::evaluate(&normalized, state) {
                Ok(value) => {
                    let value = formula_to_i32(&value);
                    best = Some(best.map_or(value, |old| old.max(value)));
                }
                Err(error) => state.diagnostics.push(format!(
                    "variable `{name}` definition `{expression}` failed: {}",
                    error.message
                )),
            }
        }
        if let Some(value) = best {
            state.variables.insert(name.clone(), value);
        }
    }
}

fn variable_dependencies(expression: &str) -> BTreeSet<String> {
    let normalized = dependency_expression(expression);
    pcgen_formula::Formula::parse(&normalized)
        .map(|formula| formula.variables().into_iter().collect())
        .unwrap_or_default()
}

/// Rewrite an expression so it can be parsed for its variable references.
///
/// The PCGen dialect is already gone by this point (lowering translated it),
/// so only the placeholders need neutralising.
fn dependency_expression(expression: &str) -> String {
    expression.replace("%CHOICE%", "0").replace("%LIST%", "0")
}

/// Prepare an expression for evaluation.
///
/// PCGen's dialect is translated at lowering time
/// ([`pcgen_core::formula_vocab`]), so `CL`, `TL`, `classlevel(...)` and
/// `var(...)` no longer appear here.  What remains is the choice placeholders,
/// which depend on what the player selected at this character's own table.
fn normalize_formula(expression: &str, state: &EffectState, source_key: Option<&str>) -> String {
    let mut normalized = expression.to_string();
    let bound = source_key
        .and_then(|key| state.choice_bindings.get(key))
        .filter(|values| !values.is_empty());
    if let Some(first) = bound
        .and_then(|values| values.first().cloned())
        .or_else(|| state.choices.values().next().cloned())
    {
        normalized = normalized
            .replace("%CHOICE%", &first)
            .replace("%choice%", &first);
    }
    if let Some(list) = bound.map(|values| values.join(",")).or_else(|| {
        (!state.choices.is_empty()).then(|| {
            state
                .choices
                .values()
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        })
    }) {
        normalized = normalized.replace("%LIST%", &list).replace("%list%", &list);
    }
    normalized
}

fn formula_to_i32(value: &FormulaValue) -> i32 {
    match value {
        FormulaValue::Int(value) => *value as i32,
        FormulaValue::Float(value) => *value as i32,
        FormulaValue::Bool(value) => *value as i32,
        FormulaValue::String(_) => 0,
    }
}

fn seed_weapon_bonus(pack: &CompiledRulePack, proficiencies: &mut BTreeSet<String>, value: &str) {
    let token = normalized(value);
    if token.is_empty() {
        return;
    }
    if let Some(kind) = token
        .strip_prefix("type=")
        .or_else(|| token.strip_prefix("type."))
    {
        let kind = kind.trim().to_string();
        proficiencies.insert(format!("type.{kind}"));
        for definition in pack.equipment.values() {
            if definition
                .type_tags
                .iter()
                .any(|tag| normalized(tag) == kind)
            {
                proficiencies.insert(normalized(&definition.name));
                proficiencies.insert(normalized(&definition.id));
            }
        }
    } else {
        proficiencies.insert(token);
    }
}

fn normalized(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}

fn resolve_placeholders(value: &str, state: &EffectState, source_key: Option<&str>) -> String {
    let bound = source_key
        .and_then(|key| state.choice_bindings.get(key))
        .filter(|values| !values.is_empty());
    let first = bound
        .and_then(|values| values.first().cloned())
        .or_else(|| state.choices.values().next().cloned())
        .unwrap_or_default();
    let list = bound.map(|values| values.join(",")).unwrap_or_else(|| {
        state
            .choices
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join(",")
    });
    value
        .replace("%CHOICE%", &first)
        .replace("%choice%", &first)
        .replace("%LIST%", &list)
        .replace("%list%", &list)
}

fn grant_allowed(entity: &CompiledEntity, draft: &CharacterDraft, state: &EffectState) -> bool {
    entity
        .requirements
        .iter()
        .all(|predicate| predicate_holds(Some(predicate), draft, state))
}

fn structural_effect_skipped(
    category: EntityCategory,
    source_entity: Option<&pcgen_core::EntityId>,
    pack: &CompiledRulePack,
    target: &str,
) -> bool {
    let target = target.to_ascii_uppercase();
    match category {
        EntityCategory::Race => target.starts_with("STAT."),
        EntityCategory::Class | EntityCategory::ClassLevel => {
            if target.starts_with("STAT.") || target == "HP" {
                return true;
            }
            // Class BASEAB/BASE.SAVE is normally represented by the typed
            // progression table, so skip the raw BONUS to avoid double
            // counting.  Monster classes (Giant, Monstrous humanoid, ...)
            // have *no* progression table: their BAB/saves come only from
            // these formula BONUS tags, so they must be applied.
            let has_progression = source_entity
                .and_then(|id| class_progression_for(pack, &id.key))
                .is_some_and(class_progression_is_populated);
            (target.contains("BASEAB") || target.contains("BASE.")) && has_progression
        }
        // Equipment armor/shield AC is already represented by
        // EquipmentDefinition.armor_bonus; do not apply the raw BONUS twice.
        EntityCategory::Equipment => target.contains("AC") || target.contains("ARMOR"),
        _ => false,
    }
}

/// A progression is "populated" when it actually has table data.  Monster
/// classes can have an entry with empty vectors, which must be treated as
/// absent so their formula BONUS tags are applied instead.
fn class_progression_for<'a>(
    pack: &'a CompiledRulePack,
    entity_key: &str,
) -> Option<&'a pcgen_core::ClassProgression> {
    // Class-level entities carry keys such as `charlie_class#1`; the
    // progression is keyed by the class id only.
    let class_key = entity_key.split('#').next().unwrap_or(entity_key);
    let wanted = normalized(class_key);
    pack.progressions.get(class_key).or_else(|| {
        pack.progressions
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(class_key) || normalized(key) == wanted)
            .map(|(_, progression)| progression)
    })
}

/// The skills that count as class skills for this character (lowercased).
///
/// PCGen's `PlayerCharacter.isClassSkill(Skill)` returns true when *any* of the
/// character's classes lists the skill, and `SkillCostFacet` fills those lists
/// from the class `CSKILL:` tags — which is why this walks the effective class
/// levels rather than the character's spent ranks.
///
/// `CSKILL:TYPE=Craft` was lowered to the bare text `Craft`, losing the marker
/// that said "this is a type", so an entry that names a known skill type also
/// stands for every skill of that type.  Keeping the bare entry as well is
/// correct: a skill really can be named `Craft`.
///
/// Not modelled: the *global* `CSKILL:` / `CCSKILL:` that PCGen also accepts on
/// templates, races and abilities, and the `SERVESAS` skill imitators.
fn character_class_skills(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    skill_types: &BTreeMap<String, Vec<String>>,
) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    let add = |entry: &str, names: &mut BTreeSet<String>| {
        // Class rows arrive with `TYPE=` already stripped, variant rows without,
        // so normalise both spellings here.
        let lowered = entry.trim().to_ascii_lowercase();
        let lower = lowered
            .strip_prefix("type=")
            .unwrap_or(&lowered)
            .trim()
            .to_string();
        if lower.is_empty() {
            return;
        }
        if let Some(skills) = skill_types.get(&lower) {
            names.extend(skills.iter().map(|skill| skill.to_ascii_lowercase()));
        }
        names.insert(lower);
    };
    for level in crate::engine::effective_class_levels(pack, draft) {
        match class_progression_for(pack, &level.class_id) {
            Some(progression) => {
                for skill in &progression.class_skills {
                    add(skill, &mut names);
                }
            }
            None => {
                if let Some(class) = pack.classes.get(&level.class_id).or_else(|| {
                    pack.classes
                        .iter()
                        .find(|(key, _)| key.eq_ignore_ascii_case(&level.class_id))
                        .map(|(_, class)| class)
                }) {
                    for skill in &class.class_skills {
                        add(skill, &mut names);
                    }
                }
            }
        }
    }
    // An archetype or substitution level can swap a class skill in; PCGen's
    // `class_skills_for_lane` does the same, and only for variants that apply to
    // the lane's own class.
    for frame in &draft.advancement {
        for lane in &frame.lanes {
            for id in [
                lane.archetype.as_deref(),
                lane.substitution_level.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                let Some(variant) = find_variant(pack, id) else {
                    continue;
                };
                if variant
                    .applies_to
                    .as_deref()
                    .is_none_or(|applies| applies.eq_ignore_ascii_case(&lane.class_id))
                {
                    for skill in &variant.class_skills {
                        add(skill, &mut names);
                    }
                }
            }
        }
    }
    names
}

fn find_variant<'a>(
    pack: &'a CompiledRulePack,
    id: &str,
) -> Option<&'a pcgen_core::VariantDefinition> {
    pack.variants.get(id).or_else(|| {
        pack.variants
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(id))
            .map(|(_, variant)| variant)
    })
}

fn class_progression_is_populated(progression: &pcgen_core::ClassProgression) -> bool {
    !progression.base_attack_bonus.is_empty()
        || !progression.fortitude.is_empty()
        || !progression.reflex.is_empty()
        || !progression.will.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_base_save_predicate_still_evaluates() {
        // `PRECHECKBASE` used to lower to `BaseSave`.  Packs written then still
        // have to answer, so the variant is kept alongside `Check`.
        let mut state = EffectState::default();
        state.base_saves.insert(SaveKind::Will, 3);
        let draft = CharacterDraft::default();
        assert!(predicate_holds(
            Some(&Predicate::BaseSave {
                save: "Will".to_string(),
                min: 2,
            }),
            &draft,
            &state
        ));
        assert!(!predicate_holds(
            Some(&Predicate::BaseSave {
                save: "Will".to_string(),
                min: 4,
            }),
            &draft,
            &state
        ));
    }
}
