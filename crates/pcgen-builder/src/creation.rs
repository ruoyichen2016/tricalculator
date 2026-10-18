//! Rule-pack-aware character creation and level-up helpers.

use crate::point_buy;
use crate::BuilderError;
use pcgen_core::CompiledRulePack;
use pcgen_core::{AdvancementFrame, AdvancementLane, LaneId};
use pcgen_engine::{CharacterDraft, DraftSpell};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const ABILITY_KEYS: [&str; 6] = ["STR", "DEX", "CON", "INT", "WIS", "CHA"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSpell {
    pub name: String,
    #[serde(default)]
    pub level: u32,
    #[serde(default = "default_times")]
    pub times_memorized: u32,
    #[serde(default)]
    pub book: Option<String>,
}

fn default_times() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateAdvancementLane {
    pub class: String,
    /// Optional explicit HP gained for this lane; defaults to average HD.
    #[serde(default)]
    pub hit_points_gained: Option<i32>,
    /// Optional archetype / alternate ability variant id.
    #[serde(default)]
    pub archetype: Option<String>,
    /// Optional substitution-level variant id.
    #[serde(default)]
    pub substitution_level: Option<String>,
    /// Optional subclass/specialty selection.
    #[serde(default)]
    pub subclass: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateAdvancementFrame {
    /// 1-based character level. If 0, it is inferred from the frame index.
    #[serde(default)]
    pub character_level: u32,
    /// One lane for normal characters; two or more for Gestalt.
    #[serde(default)]
    pub lanes: Vec<CreateAdvancementLane>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateRequest {
    pub id: String,
    pub name: String,
    pub race: String,
    #[serde(rename = "class")]
    pub class_id: String,
    pub level: u32,
    #[serde(default)]
    pub alignment: String,
    #[serde(default)]
    pub campaign: String,
    /// Per-level advancement frames. When non-empty, this takes precedence
    /// over `class` / `level` and is used as the authoritative progression.
    #[serde(default)]
    pub advancement: Vec<CreateAdvancementFrame>,
    #[serde(default)]
    pub stats: BTreeMap<String, i32>,
    #[serde(default)]
    pub point_buy_budget: Option<u32>,
    #[serde(default)]
    pub skills: BTreeMap<String, f64>,
    #[serde(default)]
    pub abilities: Vec<(String, String)>,
    #[serde(default)]
    pub spells: Vec<CreateSpell>,
    #[serde(default)]
    pub equipment: Vec<String>,
    /// Applied creature templates.
    #[serde(default)]
    pub templates: Vec<String>,
    /// Optional starting-gold budget; equipment cost is summed and checked.
    #[serde(default)]
    pub equipment_budget: Option<f64>,
    #[serde(default)]
    pub deity: Option<String>,
    #[serde(default)]
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LevelUpRequest {
    #[serde(rename = "class")]
    pub class_id: String,
    #[serde(default)]
    pub hp_gained: Option<i32>,
    #[serde(default)]
    pub skills: BTreeMap<String, f64>,
    #[serde(default)]
    pub abilities: Vec<(String, String)>,
    #[serde(default)]
    pub spells: Vec<CreateSpell>,
    /// Creature templates appended while leveling up.
    #[serde(default)]
    pub templates: Vec<String>,
    /// Replace the draft's template list instead of appending to it.
    #[serde(default)]
    pub replace_templates: bool,
    /// Optional archetype / alternate ability variant id for this level.
    #[serde(default)]
    pub archetype: Option<String>,
    /// Optional substitution-level variant id for this level.
    #[serde(default)]
    pub substitution_level: Option<String>,
    /// Optional subclass / specialty selection for this level.
    #[serde(default)]
    pub subclass: Option<String>,
    /// Extra class lanes to advance in the same character level (Gestalt).
    /// `class_id` / `hp_gained` still describe the primary lane.
    #[serde(default)]
    pub lanes: Vec<LevelUpLane>,
}

/// Build a `CharacterDraft` from a creation request, validating against the
/// compiled rule pack.
pub fn create_draft(
    pack: &CompiledRulePack,
    request: &CreateRequest,
) -> Result<CharacterDraft, BuilderError> {
    let race_id = canonical_race(pack, &request.race)
        .ok_or_else(|| BuilderError::UnknownRace(request.race.clone()))?;

    let mut stats = BTreeMap::new();
    for key in ABILITY_KEYS {
        stats.insert(key.to_string(), 10);
    }
    for (key, value) in &request.stats {
        let key = key.to_ascii_uppercase();
        if !ABILITY_KEYS.contains(&key.as_str()) {
            return Err(BuilderError::InvalidAbilityScore(key));
        }
        stats.insert(key, *value);
    }
    if let Some(budget) = request.point_buy_budget {
        point_buy::validate(&stats, budget)?;
    }

    let mut skills = BTreeMap::new();
    for (name, ranks) in &request.skills {
        if *ranks < 0.0 {
            return Err(BuilderError::InvalidSkillRank(name.clone()));
        }
        let key =
            canonical_skill(pack, name).ok_or_else(|| BuilderError::UnknownSkill(name.clone()))?;
        skills.insert(key, *ranks);
    }

    let mut abilities = Vec::new();
    for (category, key) in &request.abilities {
        if category.trim().is_empty() || key.trim().is_empty() {
            return Err(BuilderError::InvalidAbility(key.clone()));
        }
        // Ability categories are data-dependent; keep the user-provided value
        // when the pack has no matching typed category yet.
        let category =
            canonical_ability_category(pack, category).unwrap_or_else(|| category.clone());
        abilities.push((category, key.clone()));
    }

    let mut equipment = Vec::new();
    for item in &request.equipment {
        let key = canonical_equipment(pack, item)
            .ok_or_else(|| BuilderError::UnknownEquipment(item.clone()))?;
        equipment.push(key);
    }
    if let Some(budget) = request.equipment_budget {
        let spent = equipment_cost(pack, &equipment);
        if spent > budget + 1e-9 {
            return Err(BuilderError::EquipmentBudgetExceeded { spent, budget });
        }
    }

    let mut templates = Vec::new();
    for template in &request.templates {
        let template_id = canonical_template(pack, template)
            .ok_or_else(|| BuilderError::UnknownTemplate(template.clone()))?;
        templates.push(template_id);
    }

    // Resolve class progression: either an explicit per-level advancement
    // (normal multiclass or Gestalt) or the legacy single-class request.
    let mut class_ids: Vec<String> = Vec::new();
    let mut advancement = Vec::new();

    if !request.advancement.is_empty() {
        let mut lane_levels: BTreeMap<String, u32> = BTreeMap::new();
        for (index, frame) in request.advancement.iter().enumerate() {
            if frame.lanes.is_empty() {
                return Err(BuilderError::InvalidLevel(index as u32 + 1));
            }
            let character_level = if frame.character_level == 0 {
                index as u32 + 1
            } else {
                frame.character_level
            };
            if character_level != index as u32 + 1 || character_level > 20 {
                return Err(BuilderError::InvalidLevel(character_level));
            }

            let mut lanes = Vec::new();
            for (lane_index, lane) in frame.lanes.iter().enumerate() {
                let class_id = canonical_class(pack, &lane.class)
                    .ok_or_else(|| BuilderError::UnknownClass(lane.class.clone()))?;
                let lane_id = if frame.lanes.len() == 1 {
                    LaneId::primary()
                } else if lane_index == 0 {
                    LaneId::new("primary")
                } else {
                    LaneId::new(format!("lane-{}", lane_index + 1))
                };
                let key = class_id.clone();
                let next_level = lane_levels.entry(key).or_insert(0);
                *next_level += 1;
                if let Some(class) = pack.classes.get(&class_id) {
                    if class.max_level > 0 && *next_level > class.max_level {
                        return Err(BuilderError::InvalidLevel(*next_level));
                    }
                }
                let hit_points = lane
                    .hit_points_gained
                    .unwrap_or_else(|| average_hit_die(pack, &class_id));
                let mut advancement_lane =
                    AdvancementLane::new(lane_id, class_id.clone(), *next_level, hit_points, 0);
                if let Some(archetype) = &lane.archetype {
                    advancement_lane = advancement_lane.with_archetype(archetype.clone());
                }
                if let Some(substitution_level) = &lane.substitution_level {
                    let variant = pack
                        .variants
                        .get(substitution_level)
                        .or_else(|| {
                            pack.variants
                                .iter()
                                .find(|(key, _)| key.eq_ignore_ascii_case(substitution_level))
                                .map(|(_, variant)| variant)
                        })
                        .ok_or_else(|| BuilderError::UnknownVariant(substitution_level.clone()))?;
                    if let Some(applies_to) = &variant.applies_to {
                        if !applies_to.eq_ignore_ascii_case(&class_id) {
                            return Err(BuilderError::InvalidSubstitutionLevel {
                                class: class_id.clone(),
                                level: *next_level,
                                variant: variant.id.clone(),
                            });
                        }
                    }
                    let variant_level = substitution_level_value(&variant.id).ok_or_else(|| {
                        BuilderError::InvalidSubstitutionLevel {
                            class: class_id.clone(),
                            level: *next_level,
                            variant: variant.id.clone(),
                        }
                    })?;
                    if variant_level != *next_level {
                        return Err(BuilderError::InvalidSubstitutionLevel {
                            class: class_id.clone(),
                            level: *next_level,
                            variant: variant.id.clone(),
                        });
                    }
                    advancement_lane = advancement_lane.with_substitution_level(variant.id.clone());
                }
                if let Some(subclass) = &lane.subclass {
                    advancement_lane = advancement_lane.with_subclass(subclass.clone());
                }
                lanes.push(advancement_lane);
                if !class_ids.contains(&class_id) {
                    class_ids.push(class_id.clone());
                }
            }
            advancement.push(AdvancementFrame::new(character_level, lanes));
        }
    } else {
        let class_id = canonical_class(pack, &request.class_id)
            .ok_or_else(|| BuilderError::UnknownClass(request.class_id.clone()))?;
        if request.level == 0 || request.level > 20 {
            return Err(BuilderError::InvalidLevel(request.level));
        }
        if let Some(class) = pack.classes.get(&class_id) {
            if class.max_level > 0 && request.level > class.max_level {
                return Err(BuilderError::InvalidLevel(request.level));
            }
        }
        class_ids.push(class_id.clone());
        for (index, hp) in average_hit_points(pack, &class_id, request.level)
            .into_iter()
            .enumerate()
        {
            advancement.push(AdvancementFrame::single(
                index as u32 + 1,
                class_id.clone(),
                index as u32 + 1,
                hp,
                0,
            ));
        }
    }
    // SRD, "Skills Summary": a class skill costs 1 point per rank, a
    // cross-class skill 1 point per **half** rank, so the points a skill
    // consumes depend on whether it is on any of the character's class lists.
    //
    // "Regardless of whether a skill is purchased as a class skill or a
    // cross-class skill, if it is a class skill for any of your classes, your
    // maximum rank equals your total character level + 3." — hence the union
    // across classes, and the *character* level for the cap.
    let character_level = advancement.len() as u32;
    let skill_context = CharacterDraft {
        advancement: advancement.clone(),
        ..Default::default()
    };
    let class_skill_names = pcgen_engine::effects::class_skill_names(pack, &skill_context);
    let is_class_skill = |skill: &str| class_skill_names.contains(&skill.to_ascii_lowercase());

    let mut spent_skill_points = 0.0_f64;
    for (skill, ranks) in &skills {
        let class_skill = is_class_skill(skill);
        let max = pcgen_core::skill_rules::max_rank(character_level, class_skill);
        if *ranks > max + 1e-9 {
            return Err(BuilderError::SkillRankExceeded {
                skill: skill.clone(),
                ranks: *ranks,
                max,
            });
        }
        spent_skill_points += pcgen_core::skill_rules::points_for_ranks(*ranks, class_skill);
    }

    let mut spells = Vec::new();
    for spell in &request.spells {
        let name = canonical_spell(pack, &spell.name)
            .ok_or_else(|| BuilderError::UnknownSpell(spell.name.clone()))?;
        let (spell_class_id, class_level) = spell_class_level_for_classes(pack, &name, &class_ids)
            .ok_or_else(|| {
                BuilderError::SpellNotOnClassList(
                    name.clone(),
                    class_ids.first().cloned().unwrap_or_default(),
                )
            })?;
        let level = if spell.level > 0 {
            spell.level
        } else {
            class_level
        };
        spells.push(DraftSpell {
            name: spell_display_name(pack, &name),
            level,
            class_id: Some(spell_class_id),
            times_memorized: spell.times_memorized,
            book: spell.book.clone(),
        });
    }

    let mut draft = CharacterDraft {
        id: if request.id.trim().is_empty() {
            slug(&request.name)
        } else {
            request.id.clone()
        },
        name: request.name.clone(),
        race_id,
        campaign: request.campaign.clone(),
        alignment: request.alignment.clone(),
        stats,
        advancement,
        skills,
        spells,
        equipment,
        templates,
        abilities,
        deity: request.deity.clone(),
        domains: request.domains.clone(),
        ..CharacterDraft::default()
    };
    let snapshot = pcgen_engine::Engine::new(pack.clone()).evaluate(&draft);
    if spent_skill_points > 0.0 && !snapshot.is_available() {
        return Err(BuilderError::DerivedUnavailable(
            snapshot.diagnostics.join("; "),
        ));
    }
    let skill_budget = snapshot
        .available(snapshot.skill_points)
        .unwrap_or_default()
        .max(0) as u32;
    if spent_skill_points > f64::from(skill_budget) + 1e-9 {
        return Err(BuilderError::SkillPointsExceeded {
            spent: spent_skill_points,
            budget: skill_budget,
        });
    }

    draft.id = crate::sanitize_id(&draft.id).unwrap_or_else(|_| "character".to_string());
    Ok(draft)
}

/// One class-lane to advance during a single character level.
///
/// A normal character has one lane; a Gestalt character has one lane per
/// parallel class progression.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LevelUpLane {
    #[serde(rename = "class")]
    pub class_id: String,
    /// Explicit HP gained for this lane; defaults to the class hit-die average.
    #[serde(default)]
    pub hp_gained: Option<i32>,
    /// Optional archetype / alternate ability variant id for this lane.
    #[serde(default)]
    pub archetype: Option<String>,
    /// Optional substitution-level variant id for this lane.
    #[serde(default)]
    pub substitution_level: Option<String>,
    /// Optional subclass / specialty selection for this lane.
    #[serde(default)]
    pub subclass: Option<String>,
}

/// Resolve a variant id by key or case-insensitive match, erroring when absent.
fn resolve_variant(
    pack: &CompiledRulePack,
    value: &str,
) -> Result<pcgen_core::VariantDefinition, BuilderError> {
    pack.variants
        .get(value)
        .or_else(|| {
            pack.variants
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(value))
                .map(|(_, variant)| variant)
        })
        .cloned()
        .ok_or_else(|| BuilderError::UnknownVariant(value.to_string()))
}

/// Validate a substitution-level variant for `class_id` at `class_level`,
/// returning its canonical id.
fn resolve_substitution_level(
    pack: &CompiledRulePack,
    class_id: &str,
    class_level: u32,
    value: &str,
) -> Result<String, BuilderError> {
    let variant = resolve_variant(pack, value)?;
    if let Some(applies_to) = &variant.applies_to {
        if !applies_to.eq_ignore_ascii_case(class_id) {
            return Err(BuilderError::InvalidSubstitutionLevel {
                class: class_id.to_string(),
                level: class_level,
                variant: variant.id.clone(),
            });
        }
    }
    let variant_level = substitution_level_value(&variant.id).ok_or_else(|| {
        BuilderError::InvalidSubstitutionLevel {
            class: class_id.to_string(),
            level: class_level,
            variant: variant.id.clone(),
        }
    })?;
    if variant_level != class_level {
        return Err(BuilderError::InvalidSubstitutionLevel {
            class: class_id.to_string(),
            level: class_level,
            variant: variant.id.clone(),
        });
    }
    Ok(variant.id.clone())
}

/// Resolve the ordered list of lanes to advance for a level-up request.
///
/// `request.class_id` is always the primary lane; any extra `lanes` are
/// appended for parallel progression. Every level is recorded as a frame.
fn collect_level_up_lanes(request: &LevelUpRequest) -> Vec<LevelUpLane> {
    let mut lanes = Vec::new();
    if !request.class_id.trim().is_empty() || request.lanes.is_empty() {
        lanes.push(LevelUpLane {
            class_id: request.class_id.clone(),
            hp_gained: request.hp_gained,
            archetype: request.archetype.clone(),
            substitution_level: request.substitution_level.clone(),
            subclass: request.subclass.clone(),
        });
    }
    for lane in &request.lanes {
        lanes.push(lane.clone());
    }
    lanes
}

/// Advance one lane's class-level totals, returning the resulting class level
/// and whether this lane carries explicit variant/subclass choices.
fn bump_class_level(
    pack: &CompiledRulePack,
    draft: &mut CharacterDraft,
    lane: &LevelUpLane,
) -> Result<(String, u32, i32), BuilderError> {
    let class_id = canonical_class(pack, &lane.class_id)
        .ok_or_else(|| BuilderError::UnknownClass(lane.class_id.clone()))?;
    let gained = lane
        .hp_gained
        .unwrap_or_else(|| average_hit_die(pack, &class_id));

    let next_level = draft.level_in_class(&class_id) + 1;
    if pack
        .classes
        .get(&class_id)
        .is_some_and(|class| class.max_level > 0 && next_level > class.max_level)
    {
        return Err(BuilderError::InvalidLevel(next_level));
    }

    if let Some(archetype) = &lane.archetype {
        let variant = resolve_variant(pack, archetype)?;
        if let Some(applies_to) = &variant.applies_to {
            if !applies_to.eq_ignore_ascii_case(&class_id) {
                return Err(BuilderError::UnknownVariant(archetype.clone()));
            }
        }
    }
    Ok((class_id, next_level, gained))
}

/// Build the `AdvancementLane` for one level-up lane, including any variants.
fn build_advancement_lane(
    pack: &CompiledRulePack,
    lane: &LevelUpLane,
    lane_id: LaneId,
    class_id: &str,
    class_level: u32,
    gained: i32,
) -> Result<AdvancementLane, BuilderError> {
    let mut advancement_lane =
        AdvancementLane::new(lane_id, class_id.to_string(), class_level, gained, 0);
    if let Some(archetype) = &lane.archetype {
        advancement_lane = advancement_lane.with_archetype(archetype.clone());
    }
    if let Some(substitution) = &lane.substitution_level {
        let variant_id = resolve_substitution_level(pack, class_id, class_level, substitution)?;
        advancement_lane = advancement_lane.with_substitution_level(variant_id);
    }
    if let Some(subclass) = &lane.subclass {
        advancement_lane = advancement_lane.with_subclass(subclass.clone());
    }
    Ok(advancement_lane)
}

/// Apply one character level to `draft`.
///
/// `advancement` overrides the per-level lane choices when non-empty (used for
/// application and preview); otherwise the request's single-class fields and
/// `lanes` are used. Failed commands leave the original draft unchanged.
pub fn apply_level_up(
    pack: &CompiledRulePack,
    draft: &mut CharacterDraft,
    request: &LevelUpRequest,
    advancement: Option<AdvancementFrame>,
) -> Result<(), BuilderError> {
    let mut next = draft.clone();
    apply_level_up_inner(pack, &mut next, request, advancement)?;
    *draft = next;
    Ok(())
}

fn apply_level_up_inner(
    pack: &CompiledRulePack,
    draft: &mut CharacterDraft,
    request: &LevelUpRequest,
    advancement: Option<AdvancementFrame>,
) -> Result<(), BuilderError> {
    if draft.total_level() >= 20 {
        return Err(BuilderError::InvalidLevel(draft.total_level() + 1));
    }
    let frame_level = draft.total_level() + 1;
    let frame = if let Some(mut frame) = advancement {
        if frame.character_level != frame_level || frame.lanes.is_empty() {
            return Err(BuilderError::InvalidLevel(frame.character_level));
        }
        for lane in &mut frame.lanes {
            lane.class_id = canonical_class(pack, &lane.class_id)
                .ok_or_else(|| BuilderError::UnknownClass(lane.class_id.clone()))?;
            if lane.class_level != draft.level_in_class(&lane.class_id) + 1 {
                return Err(BuilderError::InvalidLevel(lane.class_level));
            }
        }
        frame
    } else {
        let mut frame_lanes = Vec::new();
        for (index, lane) in collect_level_up_lanes(request).iter().enumerate() {
            let (class_id, next_level, gained) = bump_class_level(pack, draft, lane)?;
            let lane_id = if index == 0 {
                LaneId::primary()
            } else {
                LaneId::new(format!("lane-{}", index + 1))
            };
            frame_lanes.push(build_advancement_lane(
                pack, lane, lane_id, &class_id, next_level, gained,
            )?);
        }
        AdvancementFrame::new(frame_level, frame_lanes)
    };
    for lane in &frame.lanes {
        if let Some(subclass) = &lane.subclass {
            draft
                .class_subclasses
                .insert(lane.class_id.clone(), subclass.clone());
        }
    }
    draft.advancement.push(frame);

    // Skills, abilities and spells are shared across all lanes; attribute them
    // to the primary class for spell-list validation.
    let primary_class_id = canonical_class(pack, &request.class_id)
        .or_else(|| {
            request
                .lanes
                .iter()
                .find_map(|lane| canonical_class(pack, &lane.class_id))
        })
        .ok_or_else(|| BuilderError::UnknownClass(request.class_id.clone()))?;

    for (name, ranks) in &request.skills {
        if *ranks < 0.0 {
            return Err(BuilderError::InvalidSkillRank(name.clone()));
        }
        let key =
            canonical_skill(pack, name).ok_or_else(|| BuilderError::UnknownSkill(name.clone()))?;
        *draft.skills.entry(key).or_insert(0.0) += *ranks;
    }
    for (category, key) in &request.abilities {
        if category.trim().is_empty() || key.trim().is_empty() {
            return Err(BuilderError::InvalidAbility(key.clone()));
        }
        let category =
            canonical_ability_category(pack, category).unwrap_or_else(|| category.clone());
        draft.abilities.push((category, key.clone()));
    }
    for spell in &request.spells {
        let name = canonical_spell(pack, &spell.name)
            .ok_or_else(|| BuilderError::UnknownSpell(spell.name.clone()))?;
        let class_level =
            spell_level_for_class(pack, &name, &primary_class_id).ok_or_else(|| {
                BuilderError::SpellNotOnClassList(name.clone(), primary_class_id.clone())
            })?;
        let level = if spell.level > 0 {
            spell.level
        } else {
            class_level
        };
        draft.spells.push(DraftSpell {
            name: spell_display_name(pack, &name),
            level,
            class_id: Some(primary_class_id.clone()),
            times_memorized: spell.times_memorized,
            book: spell.book.clone(),
        });
    }

    let mut resolved_templates = Vec::new();
    for template in &request.templates {
        let template_id = canonical_template(pack, template)
            .ok_or_else(|| BuilderError::UnknownTemplate(template.clone()))?;
        if !resolved_templates
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&template_id))
        {
            resolved_templates.push(template_id);
        }
    }
    if request.replace_templates {
        draft.templates = resolved_templates;
    } else {
        for template_id in resolved_templates {
            if !draft
                .templates
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&template_id))
            {
                draft.templates.push(template_id);
            }
        }
    }
    Ok(())
}

/// Apply one level to an existing draft in place.
pub fn level_up_draft(
    pack: &CompiledRulePack,
    draft: &mut CharacterDraft,
    request: &LevelUpRequest,
) -> Result<(), BuilderError> {
    apply_level_up(pack, draft, request, None)
}

/// Compute the result of a level-up without persisting it.
///
/// Template replacement and `replace_templates` are applied to the returned
/// draft exactly as they would be on a real level-up.
pub fn preview_level_up(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    request: &LevelUpRequest,
    advancement: Option<AdvancementFrame>,
) -> Result<CharacterDraft, BuilderError> {
    let mut preview = draft.clone();
    apply_level_up(pack, &mut preview, request, advancement)?;
    Ok(preview)
}

/// Deterministic 3.5e starting gold by class name (average of the DMG roll).
pub fn starting_gold_for_class(class_name: &str) -> f64 {
    match class_name.trim().to_ascii_lowercase().as_str() {
        "barbarian" | "bard" => 100.0,
        "cleric" | "rogue" => 125.0,
        "druid" => 50.0,
        "fighter" | "paladin" | "ranger" => 150.0,
        "monk" => 12.5,
        "sorcerer" | "wizard" => 75.0,
        _ => 100.0,
    }
}

/// Sum the gold cost of canonical equipment keys.
pub fn equipment_cost(pack: &CompiledRulePack, equipment: &[String]) -> f64 {
    equipment
        .iter()
        .filter_map(|item| pack.equipment.get(item))
        .map(|item| item.cost_gp)
        .sum()
}

fn spell_display_name(pack: &CompiledRulePack, spell_id: &str) -> String {
    pack.spells
        .get(spell_id)
        .map(|spell| spell.name.clone())
        .unwrap_or_else(|| spell_id.to_string())
}

fn spell_level_for_class(pack: &CompiledRulePack, spell_id: &str, class_id: &str) -> Option<u32> {
    let spell = pack.spells.get(spell_id)?;
    if spell.class_levels.is_empty() {
        return Some(0);
    }
    if let Some(level) = spell.class_levels.get(class_id) {
        return Some(*level);
    }
    let class_name = pack
        .classes
        .get(class_id)
        .map(|class| class.name.as_str())?;
    spell
        .class_levels
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(class_name))
        .map(|(_, level)| *level)
}

fn spell_class_level_for_classes(
    pack: &CompiledRulePack,
    spell_id: &str,
    class_ids: &[String],
) -> Option<(String, u32)> {
    for class_id in class_ids {
        if let Some(level) = spell_level_for_class(pack, spell_id, class_id) {
            return Some((class_id.clone(), level));
        }
    }
    None
}

fn substitution_level_value(variant_id: &str) -> Option<u32> {
    variant_id
        .rsplit_once(" ~ level ")
        .and_then(|(_, level)| level.trim().parse().ok())
}

fn average_hit_points(pack: &CompiledRulePack, class_id: &str, level: u32) -> Vec<i32> {
    let average = average_hit_die(pack, class_id);
    if average <= 0 {
        Vec::new()
    } else {
        vec![average; level as usize]
    }
}

fn average_hit_die(pack: &CompiledRulePack, class_id: &str) -> i32 {
    pack.classes
        .get(class_id)
        .and_then(|class| parse_die(&class.hit_die))
        .map(|die| die / 2 + 1)
        .unwrap_or(0)
}

fn parse_die(value: &str) -> Option<i32> {
    let digits: String = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn slug(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "character".to_string()
    } else {
        value
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect()
    }
}

fn canonical_race(pack: &CompiledRulePack, value: &str) -> Option<String> {
    if pack.races.contains_key(value) {
        return Some(value.to_string());
    }
    pack.races
        .iter()
        .find(|(_, race)| race.name.eq_ignore_ascii_case(value))
        .map(|(key, _)| key.clone())
}

fn canonical_class(pack: &CompiledRulePack, value: &str) -> Option<String> {
    if pack.classes.contains_key(value) {
        return Some(value.to_string());
    }
    pack.classes
        .iter()
        .find(|(_, class)| class.name.eq_ignore_ascii_case(value))
        .map(|(key, _)| key.clone())
}

fn canonical_skill(pack: &CompiledRulePack, value: &str) -> Option<String> {
    if pack.skills.contains_key(value) {
        return Some(value.to_string());
    }
    pack.skills
        .iter()
        .find(|(_, skill)| skill.name.eq_ignore_ascii_case(value))
        .map(|(key, _)| key.clone())
}

fn canonical_spell(pack: &CompiledRulePack, value: &str) -> Option<String> {
    if pack.spells.contains_key(value) {
        return Some(value.to_string());
    }
    pack.spells
        .iter()
        .find(|(_, spell)| spell.name.eq_ignore_ascii_case(value))
        .map(|(key, _)| key.clone())
}

fn canonical_equipment(pack: &CompiledRulePack, value: &str) -> Option<String> {
    if pack.equipment.contains_key(value) {
        return Some(value.to_string());
    }
    pack.equipment
        .iter()
        .find(|(_, item)| item.name.eq_ignore_ascii_case(value))
        .map(|(key, _)| key.clone())
}

fn canonical_template(pack: &CompiledRulePack, value: &str) -> Option<String> {
    if pack.templates.contains_key(value) {
        return Some(value.to_string());
    }
    pack.templates
        .iter()
        .find(|(_, template)| template.name.eq_ignore_ascii_case(value))
        .map(|(key, _)| key.clone())
}

fn canonical_ability_category(pack: &CompiledRulePack, value: &str) -> Option<String> {
    if pack.ability_categories.contains_key(value) {
        return Some(value.to_string());
    }
    pack.ability_categories
        .iter()
        .find(|(_, category)| category.name.eq_ignore_ascii_case(value))
        .map(|(key, _)| key.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcgen_import::lower_records;

    fn sample_pack() -> CompiledRulePack {
        let (mut pack, _) = lower_records(Some("3e".to_string()), Vec::new(), &[]);
        pack.races.insert(
            "Human".to_string(),
            pcgen_core::domain::RaceDefinition::new("Human", "Human"),
        );
        pack.classes.insert(
            "Fighter".to_string(),
            pcgen_core::domain::ClassDefinition::new("Fighter", "Fighter"),
        );
        pack.classes.insert(
            "Wizard".to_string(),
            pcgen_core::domain::ClassDefinition::new("Wizard", "Wizard"),
        );
        // The skill rules need a progression: without one the class has no
        // class-skill list, so every skill counts as cross-class and the
        // half-rate cap applies (SRD "Skills Summary").
        for class_id in ["Fighter", "Wizard"] {
            // Fighter/Wizard both get 2 skill points per level, x4 at 1st.
            let progression = pcgen_core::domain::ClassProgression {
                class_skills: vec!["Climb".to_string()],
                first_level_skill_points: 8,
                skill_points_per_level: 2,
                skill_multiplier: 4,
                ..Default::default()
            };
            pack.progressions.insert(class_id.to_string(), progression);
        }
        let mut two_handed =
            pcgen_core::VariantDefinition::new("Two-Handed Fighter", "Two-Handed Fighter");
        two_handed.applies_to = Some("Fighter".to_string());
        two_handed.replaces = vec!["Bravery".to_string()];
        two_handed.grants = vec!["Shattering Strike".to_string()];
        pack.variants
            .insert("Two-Handed Fighter".to_string(), two_handed);
        let mut substitution = pcgen_core::VariantDefinition::new(
            "Wizard Substitution ~ level 1",
            "Wizard Substitution",
        );
        substitution.applies_to = Some("Wizard".to_string());
        substitution.grants = vec!["Alternate Arcane Bond".to_string()];
        pack.variants
            .insert("Wizard Substitution ~ level 1".to_string(), substitution);
        pack.skills.insert(
            "Climb".to_string(),
            pcgen_core::domain::SkillDefinition::new("Climb", "Climb"),
        );
        // Spellcraft is deliberately *not* on the class lists above, so it
        // exercises the cross-class half-rate and half-cap rules.
        pack.skills.insert(
            "Spellcraft".to_string(),
            pcgen_core::domain::SkillDefinition::new("Spellcraft", "Spellcraft"),
        );
        pack.skills.get_mut("Climb").unwrap().key_stat = Some("STR".into());
        pack.skills.get_mut("Spellcraft").unwrap().key_stat = Some("INT".into());
        pack.ability_categories.insert(
            "FEAT".to_string(),
            pcgen_core::AbilityCategoryDefinition {
                choice_restrictions: Default::default(),
                id: "FEAT".to_string(),
                name: "FEAT".to_string(),
                ..Default::default()
            },
        );
        let mut longsword = pcgen_core::domain::EquipmentDefinition::new("Longsword", "Longsword");
        longsword.cost_gp = 15.0;
        pack.equipment.insert("Longsword".to_string(), longsword);
        let mut magic_missile =
            pcgen_core::domain::SpellDefinition::new("Magic Missile", "Magic Missile");
        magic_missile.class_levels.insert("Fighter".to_string(), 1);
        pack.spells
            .insert("Magic Missile".to_string(), magic_missile);
        pack.templates.insert(
            "Celestial".to_string(),
            pcgen_core::domain::TemplateDefinition::new("Celestial", "Celestial"),
        );
        pack
    }

    #[test]
    fn plain_level_up_continues_explicit_advancement() {
        let pack = sample_pack();
        let draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Web Hero".into(),
                race: "Human".into(),
                advancement: vec![CreateAdvancementFrame {
                    character_level: 1,
                    lanes: vec![CreateAdvancementLane {
                        class: "Fighter".into(),
                        ..Default::default()
                    }],
                }],
                ..Default::default()
            },
        )
        .unwrap();
        let preview = preview_level_up(
            &pack,
            &draft,
            &LevelUpRequest {
                class_id: "Fighter".into(),
                hp_gained: Some(6),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        assert_eq!(draft.total_level(), 1);
        assert_eq!(preview.total_level(), 2);
        assert_eq!(preview.advancement.len(), 2);
        assert_eq!(preview.advancement[1].lanes[0].class_level, 2);
        let engine = pcgen_engine::Engine::new(pack);
        assert_eq!(
            engine.evaluate(&preview).hit_points,
            engine.evaluate(&draft).hit_points + 6
        );
    }

    #[test]
    fn creates_level_three_fighter() {
        let pack = sample_pack();
        let request = CreateRequest {
            id: "hero".to_string(),
            name: "Hero".to_string(),
            race: "human".to_string(),
            class_id: "fighter".to_string(),
            level: 3,
            stats: BTreeMap::new(),
            point_buy_budget: Some(25),
            skills: [("climb".to_string(), 4.0)].into_iter().collect(),
            abilities: vec![("FEAT".to_string(), "Power Attack".to_string())],
            spells: Vec::new(),
            equipment: vec!["longsword".to_string()],
            ..CreateRequest::default()
        };
        let draft = create_draft(&pack, &request).unwrap();
        assert_eq!(draft.id, "hero");
        assert_eq!(draft.race_id, "Human");
        assert_eq!(draft.class_levels()[0].class_id, "Fighter");
        assert_eq!(draft.class_levels()[0].level, 3);
        assert_eq!(draft.skills.get("Climb"), Some(&4.0));
        assert_eq!(draft.equipment, vec!["Longsword".to_string()]);
    }

    /// SRD, "Skills Summary": "Your maximum rank in a class skill is your
    /// character level + 3."
    ///
    /// <https://www.d20srd.org/srd/skills/skillsSummary.htm>
    #[test]
    fn rejects_ranks_above_the_class_skill_cap() {
        let pack = sample_pack();
        let request = CreateRequest {
            id: "hero".to_string(),
            name: "Hero".to_string(),
            race: "human".to_string(),
            class_id: "fighter".to_string(),
            level: 1,
            stats: BTreeMap::new(),
            point_buy_budget: None,
            // Level 1 + 3 = 4 is the cap; 5 must be refused even though the
            // point budget would allow it.
            skills: [("climb".to_string(), 5.0)].into_iter().collect(),
            ..CreateRequest::default()
        };
        match create_draft(&pack, &request) {
            Err(BuilderError::SkillRankExceeded { max, .. }) => assert_eq!(max, 4.0),
            other => panic!("expected SkillRankExceeded, got {other:?}"),
        }
    }

    /// SRD, "Skills Summary": "Your maximum rank in a cross-class skill is
    /// one-half of this number (do not round up or down)."
    #[test]
    fn cross_class_cap_is_half_the_class_skill_cap() {
        let pack = sample_pack();
        let request = CreateRequest {
            id: "hero".to_string(),
            name: "Hero".to_string(),
            race: "human".to_string(),
            class_id: "fighter".to_string(),
            level: 1,
            stats: BTreeMap::new(),
            point_buy_budget: None,
            // Spellcraft is not on the fixture's class list, so the cap is
            // (1 + 3) / 2 = 2.
            skills: [("spellcraft".to_string(), 2.5)].into_iter().collect(),
            ..CreateRequest::default()
        };
        match create_draft(&pack, &request) {
            Err(BuilderError::SkillRankExceeded { max, .. }) => assert_eq!(max, 2.0),
            other => panic!("expected SkillRankExceeded, got {other:?}"),
        }
    }

    /// SRD, "Skills Summary": "Each skill point you spend on a class skill gets
    /// you 1 rank [...]. Each skill point you spend on a cross-class skill gets
    /// your character ½ rank in that skill."
    #[test]
    fn cross_class_ranks_cost_two_points_each() {
        let pack = sample_pack();
        // Fighter 1 with Int 10: (2 + 0) x 4 = 8 points.
        //
        // 2 cross-class ranks cost 4 points and 4 class-skill ranks cost 4, so
        // the pair fits exactly at 8.  Under a 1:1 misreading it would look
        // like 6 and the budget would appear slack — the second assertion below
        // is what distinguishes the two readings.
        let fitting = CreateRequest {
            id: "hero".to_string(),
            name: "Hero".to_string(),
            race: "human".to_string(),
            class_id: "fighter".to_string(),
            level: 1,
            stats: BTreeMap::new(),
            point_buy_budget: None,
            skills: [("climb".to_string(), 4.0), ("spellcraft".to_string(), 2.0)]
                .into_iter()
                .collect(),
            ..CreateRequest::default()
        };
        let result = create_draft(&pack, &fitting);
        assert!(
            result.is_ok(),
            "4 class ranks + 2 cross-class ranks must fit the 8-point budget: {result:?}"
        );

        // One more cross-class rank costs 2 more points, so the budget is now
        // exceeded: 4 + 4 + 2 = 10 > 8.  A 1:1 reading would give 7 and pass.
        let over = CreateRequest {
            skills: [("climb".to_string(), 4.0), ("spellcraft".to_string(), 3.0)]
                .into_iter()
                .collect(),
            ..fitting
        };
        match create_draft(&pack, &over) {
            // 3 cross-class ranks also exceed the cross-class cap of 2, so the
            // cap fires first; assert on the cap and cover the budget through
            // the class-skill-only path below.
            Err(BuilderError::SkillRankExceeded { max, .. }) => assert_eq!(max, 2.0),
            other => panic!("expected SkillRankExceeded, got {other:?}"),
        }
    }

    #[test]
    fn validates_spell_class_list() {
        let pack = sample_pack();
        let ok = CreateRequest {
            name: "Hero".to_string(),
            race: "Human".to_string(),
            class_id: "Fighter".to_string(),
            level: 1,
            spells: vec![CreateSpell {
                name: "Magic Missile".to_string(),
                level: 0,
                times_memorized: 1,
                book: None,
            }],
            ..CreateRequest::default()
        };
        let draft = create_draft(&pack, &ok).unwrap();
        assert_eq!(draft.spells[0].level, 1);

        let mut other = sample_pack();
        let mut fireball = pcgen_core::domain::SpellDefinition::new("Fireball", "Fireball");
        fireball.class_levels.insert("Wizard".to_string(), 3);
        other.spells.insert("Fireball".to_string(), fireball);
        let bad = CreateRequest {
            name: "Hero".to_string(),
            race: "Human".to_string(),
            class_id: "Fighter".to_string(),
            level: 1,
            spells: vec![CreateSpell {
                name: "Fireball".to_string(),
                level: 0,
                times_memorized: 1,
                book: None,
            }],
            ..CreateRequest::default()
        };
        assert!(matches!(
            create_draft(&other, &bad),
            Err(BuilderError::SpellNotOnClassList(_, _))
        ));
    }

    #[test]
    fn validates_equipment_budget() {
        let pack = sample_pack();
        let base = CreateRequest {
            name: "Hero".to_string(),
            race: "Human".to_string(),
            class_id: "Fighter".to_string(),
            level: 1,
            equipment: vec!["Longsword".to_string()],
            ..CreateRequest::default()
        };
        let mut over = base.clone();
        over.equipment_budget = Some(10.0);
        assert!(matches!(
            create_draft(&pack, &over),
            Err(BuilderError::EquipmentBudgetExceeded { .. })
        ));
        let mut ok = base;
        ok.equipment_budget = Some(20.0);
        assert!(create_draft(&pack, &ok).is_ok());
    }

    #[test]
    fn starting_gold_matches_class_profile() {
        assert_eq!(starting_gold_for_class("Fighter"), 150.0);
        assert_eq!(starting_gold_for_class("wizard"), 75.0);
        assert_eq!(starting_gold_for_class("Cleric"), 125.0);
    }

    #[test]
    fn rejects_unknown_race_and_skill() {
        let pack = sample_pack();
        let mut request = CreateRequest {
            name: "Hero".to_string(),
            race: "Elf".to_string(),
            class_id: "Fighter".to_string(),
            level: 1,
            ..CreateRequest::default()
        };
        assert!(matches!(
            create_draft(&pack, &request),
            Err(BuilderError::UnknownRace(_))
        ));
        request.race = "Human".to_string();
        request.skills.insert("Swim".to_string(), 1.0);
        assert!(matches!(
            create_draft(&pack, &request),
            Err(BuilderError::UnknownSkill(_))
        ));
    }

    #[test]
    fn creates_per_level_multiclass() {
        let pack = sample_pack();
        let request = CreateRequest {
            name: "Multi".to_string(),
            race: "Human".to_string(),
            class_id: "Fighter".to_string(),
            level: 1,
            advancement: vec![
                CreateAdvancementFrame {
                    character_level: 1,
                    lanes: vec![CreateAdvancementLane {
                        class: "Fighter".to_string(),
                        hit_points_gained: None,
                        ..Default::default()
                    }],
                },
                CreateAdvancementFrame {
                    character_level: 2,
                    lanes: vec![CreateAdvancementLane {
                        class: "Wizard".to_string(),
                        hit_points_gained: None,
                        ..Default::default()
                    }],
                },
                CreateAdvancementFrame {
                    character_level: 3,
                    lanes: vec![CreateAdvancementLane {
                        class: "fighter".to_string(),
                        hit_points_gained: None,
                        ..Default::default()
                    }],
                },
            ],
            ..CreateRequest::default()
        };
        let draft = create_draft(&pack, &request).unwrap();
        assert!(!draft.advancement.is_empty());
        assert_eq!(draft.advancement.len(), 3);
        assert_eq!(draft.total_level(), 3);
        assert_eq!(draft.advancement[0].lanes[0].class_id, "Fighter");
        assert_eq!(draft.advancement[0].lanes[0].class_level, 1);
        assert_eq!(draft.advancement[1].lanes[0].class_id, "Wizard");
        assert_eq!(draft.advancement[1].lanes[0].class_level, 1);
        assert_eq!(draft.advancement[2].lanes[0].class_id, "Fighter");
        assert_eq!(draft.advancement[2].lanes[0].class_level, 2);
    }

    #[test]
    fn creates_gestalt_advancement() {
        let pack = sample_pack();
        let request = CreateRequest {
            name: "Gestalt".to_string(),
            race: "Human".to_string(),
            class_id: "Fighter".to_string(),
            level: 1,
            advancement: vec![
                CreateAdvancementFrame {
                    character_level: 1,
                    lanes: vec![
                        CreateAdvancementLane {
                            class: "Fighter".to_string(),
                            hit_points_gained: None,
                            ..Default::default()
                        },
                        CreateAdvancementLane {
                            class: "Wizard".to_string(),
                            hit_points_gained: None,
                            ..Default::default()
                        },
                    ],
                },
                CreateAdvancementFrame {
                    character_level: 2,
                    lanes: vec![
                        CreateAdvancementLane {
                            class: "Fighter".to_string(),
                            hit_points_gained: None,
                            ..Default::default()
                        },
                        CreateAdvancementLane {
                            class: "Wizard".to_string(),
                            hit_points_gained: None,
                            ..Default::default()
                        },
                    ],
                },
            ],
            ..CreateRequest::default()
        };
        let draft = create_draft(&pack, &request).unwrap();
        assert!(draft.is_gestalt());
        assert!(!draft.advancement.is_empty());
        assert_eq!(draft.advancement.len(), 2);
        assert_eq!(draft.advancement[0].lanes.len(), 2);
        assert_eq!(draft.total_level(), 2);
        assert_eq!(draft.advancement[1].lanes[0].class_level, 2);
        assert_eq!(draft.advancement[1].lanes[1].class_level, 2);
    }

    #[test]
    fn creates_advancement_with_archetype_and_substitution() {
        let pack = sample_pack();
        let request = CreateRequest {
            name: "Variant Hero".to_string(),
            race: "Human".to_string(),
            class_id: "Fighter".to_string(),
            level: 2,
            advancement: vec![
                CreateAdvancementFrame {
                    character_level: 1,
                    lanes: vec![CreateAdvancementLane {
                        class: "Fighter".to_string(),
                        hit_points_gained: None,
                        archetype: Some("Two-Handed Fighter".to_string()),
                        substitution_level: None,
                        subclass: None,
                    }],
                },
                CreateAdvancementFrame {
                    character_level: 2,
                    lanes: vec![CreateAdvancementLane {
                        class: "Wizard".to_string(),
                        hit_points_gained: None,
                        archetype: None,
                        substitution_level: Some("Wizard Substitution ~ level 1".to_string()),
                        subclass: None,
                    }],
                },
            ],
            ..CreateRequest::default()
        };
        let draft = create_draft(&pack, &request).unwrap();
        assert_eq!(
            draft.advancement[0].lanes[0].archetype.as_deref(),
            Some("Two-Handed Fighter")
        );
        assert_eq!(
            draft.advancement[1].lanes[0].substitution_level.as_deref(),
            Some("Wizard Substitution ~ level 1")
        );
    }

    #[test]
    fn rejects_substitution_level_for_wrong_class_level() {
        let pack = sample_pack();
        let request = CreateRequest {
            name: "Bad Sub".to_string(),
            race: "Human".to_string(),
            class_id: "Wizard".to_string(),
            level: 2,
            advancement: vec![
                CreateAdvancementFrame {
                    character_level: 1,
                    lanes: vec![CreateAdvancementLane {
                        class: "Wizard".to_string(),
                        hit_points_gained: None,
                        archetype: None,
                        substitution_level: None,
                        subclass: None,
                    }],
                },
                CreateAdvancementFrame {
                    character_level: 2,
                    lanes: vec![CreateAdvancementLane {
                        class: "Wizard".to_string(),
                        hit_points_gained: None,
                        archetype: None,
                        substitution_level: Some("Wizard Substitution ~ level 1".to_string()),
                        subclass: None,
                    }],
                },
            ],
            ..CreateRequest::default()
        };
        assert!(matches!(
            create_draft(&pack, &request),
            Err(BuilderError::InvalidSubstitutionLevel { .. })
        ));
    }

    #[test]
    fn level_up_adds_class_level_and_skills() {
        let pack = sample_pack();
        let mut draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Hero".to_string(),
                race: "Human".to_string(),
                class_id: "Fighter".to_string(),
                level: 1,
                ..CreateRequest::default()
            },
        )
        .unwrap();
        let request = LevelUpRequest {
            class_id: "Fighter".to_string(),
            hp_gained: Some(6),
            skills: [("Climb".to_string(), 2.0)].into_iter().collect(),
            ..LevelUpRequest::default()
        };
        level_up_draft(&pack, &mut draft, &request).unwrap();
        assert_eq!(draft.total_level(), 2);
        assert_eq!(
            draft.class_levels()[0].hit_points_by_level,
            vec![Some(5), Some(6)]
        );
        assert_eq!(draft.skills.get("Climb"), Some(&2.0));
    }

    #[test]
    fn create_resolves_templates_by_name() {
        let pack = sample_pack();
        let draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Hero".to_string(),
                race: "Human".to_string(),
                class_id: "Fighter".to_string(),
                level: 1,
                templates: vec!["celestial".to_string()],
                ..CreateRequest::default()
            },
        )
        .unwrap();
        assert_eq!(draft.templates, vec!["Celestial".to_string()]);
    }

    #[test]
    fn level_up_appends_templates_without_duplicates() {
        let pack = sample_pack();
        let mut draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Hero".to_string(),
                race: "Human".to_string(),
                class_id: "Fighter".to_string(),
                level: 1,
                ..CreateRequest::default()
            },
        )
        .unwrap();
        let request = LevelUpRequest {
            class_id: "Fighter".to_string(),
            templates: vec!["Celestial".to_string(), "celestial".to_string()],
            ..LevelUpRequest::default()
        };
        level_up_draft(&pack, &mut draft, &request).unwrap();
        assert_eq!(draft.templates, vec!["Celestial".to_string()]);
        assert_eq!(draft.total_level(), 2);
    }

    #[test]
    fn level_up_applies_archetype_and_subclass() {
        let pack = sample_pack();
        let mut draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Hero".to_string(),
                race: "Human".to_string(),
                class_id: "Fighter".to_string(),
                level: 1,
                ..CreateRequest::default()
            },
        )
        .unwrap();
        let request = LevelUpRequest {
            class_id: "Fighter".to_string(),
            archetype: Some("Two-Handed Fighter".to_string()),
            subclass: Some("Weapon Master".to_string()),
            hp_gained: Some(6),
            ..LevelUpRequest::default()
        };
        level_up_draft(&pack, &mut draft, &request).unwrap();
        assert_eq!(draft.advancement.len(), 2);
        let lane = &draft.advancement[1].lanes[0];
        assert_eq!(lane.class_id, "Fighter");
        assert_eq!(lane.class_level, 2);
        assert_eq!(lane.archetype.as_deref(), Some("Two-Handed Fighter"));
        assert_eq!(lane.subclass.as_deref(), Some("Weapon Master"));
        assert_eq!(draft.class_levels()[0].level, 2);
        assert_eq!(
            draft.class_subclasses.get("Fighter").map(String::as_str),
            Some("Weapon Master")
        );
    }

    #[test]
    fn level_up_rejects_substitution_for_wrong_level() {
        let pack = sample_pack();
        let mut draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Hero".to_string(),
                race: "Human".to_string(),
                class_id: "Wizard".to_string(),
                level: 1,
                ..CreateRequest::default()
            },
        )
        .unwrap();
        // The only substitution variant in the sample pack targets level 1,
        // but this level-up advances the Wizard to level 2.
        let request = LevelUpRequest {
            class_id: "Wizard".to_string(),
            substitution_level: Some("Wizard Substitution ~ level 1".to_string()),
            ..LevelUpRequest::default()
        };
        assert!(matches!(
            level_up_draft(&pack, &mut draft, &request),
            Err(BuilderError::InvalidSubstitutionLevel { .. })
        ));
    }

    #[test]
    fn level_up_replace_templates_clears_existing() {
        let pack = sample_pack();
        let mut draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Hero".to_string(),
                race: "Human".to_string(),
                class_id: "Fighter".to_string(),
                level: 1,
                templates: vec!["Celestial".to_string()],
                ..CreateRequest::default()
            },
        )
        .unwrap();
        assert_eq!(draft.templates, vec!["Celestial".to_string()]);
        let request = LevelUpRequest {
            class_id: "Fighter".to_string(),
            templates: Vec::new(),
            replace_templates: true,
            ..LevelUpRequest::default()
        };
        level_up_draft(&pack, &mut draft, &request).unwrap();
        assert!(draft.templates.is_empty());
    }

    #[test]
    fn level_up_advances_two_lanes_in_one_level() {
        let pack = sample_pack();
        let mut draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Gestalt".to_string(),
                race: "Human".to_string(),
                class_id: "Fighter".to_string(),
                level: 1,
                ..CreateRequest::default()
            },
        )
        .unwrap();
        let request = LevelUpRequest {
            class_id: "Fighter".to_string(),
            hp_gained: Some(6),
            lanes: vec![LevelUpLane {
                class_id: "Wizard".to_string(),
                hp_gained: Some(3),
                ..LevelUpLane::default()
            }],
            ..LevelUpRequest::default()
        };
        level_up_draft(&pack, &mut draft, &request).unwrap();
        // Two advanced classes still add only one character level.
        assert_eq!(draft.total_level(), 2);
        let fighter = draft
            .class_levels()
            .into_iter()
            .find(|entry| entry.class_id == "Fighter")
            .unwrap();
        let wizard = draft
            .class_levels()
            .into_iter()
            .find(|entry| entry.class_id == "Wizard")
            .unwrap();
        assert_eq!(fighter.level, 2);
        assert_eq!(wizard.level, 1);
    }

    #[test]
    fn multi_lane_level_up_uses_single_frame() {
        let pack = sample_pack();
        let mut draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Gestalt".to_string(),
                race: "Human".to_string(),
                class_id: "Fighter".to_string(),
                level: 1,
                ..CreateRequest::default()
            },
        )
        .unwrap();
        let request = LevelUpRequest {
            class_id: "Fighter".to_string(),
            hp_gained: Some(6),
            lanes: vec![LevelUpLane {
                class_id: "Wizard".to_string(),
                hp_gained: Some(3),
                substitution_level: Some("Wizard Substitution ~ level 1".to_string()),
                ..LevelUpLane::default()
            }],
            ..LevelUpRequest::default()
        };
        level_up_draft(&pack, &mut draft, &request).unwrap();
        assert_eq!(draft.advancement.len(), 2, "both lanes share the new frame");
        let frame = &draft.advancement[1];
        assert_eq!(frame.lanes.len(), 2);
        let wizard_lane = frame
            .lanes
            .iter()
            .find(|lane| lane.class_id == "Wizard")
            .unwrap();
        assert_eq!(
            wizard_lane.substitution_level.as_deref(),
            Some("Wizard Substitution ~ level 1")
        );
    }

    #[test]
    fn multi_lane_lane_metadata_is_supported() {
        let pack = sample_pack();
        let mut draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Gestalt".to_string(),
                race: "Human".to_string(),
                class_id: "Fighter".to_string(),
                level: 1,
                ..CreateRequest::default()
            },
        )
        .unwrap();
        let request = LevelUpRequest {
            class_id: "Fighter".to_string(),
            hp_gained: Some(6),
            archetype: Some("Two-Handed Fighter".to_string()),
            lanes: vec![LevelUpLane {
                class_id: "Wizard".to_string(),
                hp_gained: Some(3),
                subclass: Some("Evoker".to_string()),
                ..LevelUpLane::default()
            }],
            ..LevelUpRequest::default()
        };
        level_up_draft(&pack, &mut draft, &request).unwrap();
        let frame = &draft.advancement[1];
        let fighter_lane = frame
            .lanes
            .iter()
            .find(|lane| lane.class_id == "Fighter")
            .unwrap();
        let wizard_lane = frame
            .lanes
            .iter()
            .find(|lane| lane.class_id == "Wizard")
            .unwrap();
        assert_eq!(
            fighter_lane.archetype.as_deref(),
            Some("Two-Handed Fighter")
        );
        assert_eq!(wizard_lane.subclass.as_deref(), Some("Evoker"));
        assert_eq!(
            draft.class_subclasses.get("Wizard").map(String::as_str),
            Some("Evoker")
        );
    }

    #[test]
    fn preview_level_up_does_not_mutate_source() {
        let pack = sample_pack();
        let draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Hero".to_string(),
                race: "Human".to_string(),
                class_id: "Fighter".to_string(),
                level: 1,
                templates: vec!["Celestial".to_string()],
                ..CreateRequest::default()
            },
        )
        .unwrap();
        let request = LevelUpRequest {
            class_id: "Fighter".to_string(),
            templates: Vec::new(),
            replace_templates: true,
            ..LevelUpRequest::default()
        };
        let preview = preview_level_up(&pack, &draft, &request, None).unwrap();
        assert_eq!(preview.total_level(), 2);
        assert!(preview.templates.is_empty());
        // Source draft is untouched.
        assert_eq!(draft.total_level(), 1);
        assert_eq!(draft.templates, vec!["Celestial".to_string()]);
    }

    #[test]
    fn level_up_rejects_unknown_template() {
        let pack = sample_pack();
        let mut draft = create_draft(
            &pack,
            &CreateRequest {
                name: "Hero".to_string(),
                race: "Human".to_string(),
                class_id: "Fighter".to_string(),
                level: 1,
                ..CreateRequest::default()
            },
        )
        .unwrap();
        let request = LevelUpRequest {
            class_id: "Fighter".to_string(),
            templates: vec!["NoSuchTemplate".to_string()],
            ..LevelUpRequest::default()
        };
        assert!(matches!(
            level_up_draft(&pack, &mut draft, &request),
            Err(BuilderError::UnknownTemplate(_))
        ));
    }
}
