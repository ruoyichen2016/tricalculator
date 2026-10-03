//! Rule-pack-aware character creation and level-up helpers.

use crate::point_buy;
use crate::BuilderError;
use pcgen_core::{AdvancementFrame, AdvancementLane, LaneId};
use pcgen_engine::{CharacterDraft, DraftClassLevel, DraftSpell};
use pcgen_ir::CompiledRulePack;
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
    let mut class_levels = Vec::new();

    if !request.advancement.is_empty() {
        let mut lane_levels: BTreeMap<(String, String), u32> = BTreeMap::new();
        for (index, frame) in request.advancement.iter().enumerate() {
            if frame.lanes.is_empty() {
                return Err(BuilderError::InvalidLevel(index as u32 + 1));
            }
            let character_level = if frame.character_level == 0 {
                index as u32 + 1
            } else {
                frame.character_level
            };
            if character_level == 0 || character_level > 20 {
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
                let key = (lane_id.to_string(), class_id.clone());
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
        class_levels.push(DraftClassLevel {
            class_id: class_id.clone(),
            level: request.level,
            hit_points_gained: 0,
            hit_points_by_level: average_hit_points(pack, &class_id, request.level),
            experience_gained: 0,
        });
    }

    let intelligence_modifier = intelligence_modifier(&stats);
    let skill_budget = skill_point_budget(pack, &advancement, &class_levels, intelligence_modifier);
    let spent_skill_points: f64 = skills.values().sum();
    if spent_skill_points > f64::from(skill_budget) + 1e-9 {
        return Err(BuilderError::SkillPointsExceeded {
            spent: spent_skill_points,
            budget: skill_budget,
        });
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
        class_levels,
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

/// Update class-level totals so they stay consistent after editing a lane.
fn sync_draft_class_levels(draft: &mut CharacterDraft) {
    let mut totals: BTreeMap<String, u32> = BTreeMap::new();
    for frame in &draft.advancement {
        for lane in &frame.lanes {
            let entry = totals.entry(lane.class_id.clone()).or_insert(0);
            if *entry < lane.class_level {
                *entry = lane.class_level;
            }
        }
    }
    draft
        .class_levels
        .retain(|entry| totals.get(&entry.class_id).is_some_and(|level| *level > 0));
    for (class_id, level) in &totals {
        if let Some(entry) = draft
            .class_levels
            .iter_mut()
            .find(|entry| entry.class_id.eq_ignore_ascii_case(class_id))
        {
            entry.level = *level;
        } else {
            draft.class_levels.push(DraftClassLevel {
                class_id: class_id.clone(),
                level: *level,
                hit_points_gained: 0,
                hit_points_by_level: Vec::new(),
                experience_gained: 0,
            });
        }
    }
}

/// Resolve the ordered list of lanes to advance for a level-up request.
///
/// `request.class_id` is always the primary lane; any extra `lanes` are
/// appended for Gestalt/multiclass progression.  A single lane keeps the
/// legacy behaviour of tracking per-level HP inside `class_levels`.
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

    let next_level = if let Some(entry) = draft
        .class_levels
        .iter_mut()
        .find(|entry| entry.class_id.eq_ignore_ascii_case(&class_id))
    {
        entry.level += 1;
        if !entry.hit_points_by_level.is_empty() || lane.hp_gained.is_some() {
            entry.hit_points_by_level.push(gained);
        } else {
            entry.hit_points_gained += gained;
        }
        entry.level
    } else {
        draft.class_levels.push(DraftClassLevel {
            class_id: class_id.clone(),
            level: 1,
            hit_points_gained: 0,
            hit_points_by_level: vec![gained],
            experience_gained: 0,
        });
        1
    };

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

/// True when this lane asked for an explicit variant or subclass.
fn lane_has_choices(lane: &LevelUpLane) -> bool {
    lane.archetype.is_some() || lane.substitution_level.is_some() || lane.subclass.is_some()
}

/// Apply one character level to `draft`.
///
/// `advancement` overrides the per-level lane choices when non-empty (used for
/// application and preview); otherwise the request's single-class fields and
/// `lanes` are used.  Errors leave `draft` unspecified — callers should work on
/// a clone when the original must be preserved (see [`preview_level_up`]).
pub fn apply_level_up(
    pack: &CompiledRulePack,
    draft: &mut CharacterDraft,
    request: &LevelUpRequest,
    advancement: Option<AdvancementFrame>,
) -> Result<(), BuilderError> {
    if draft.total_level() >= 20 {
        return Err(BuilderError::InvalidLevel(draft.total_level() + 1));
    }
    let frame_level = draft
        .advancement
        .iter()
        .map(|frame| frame.character_level)
        .max()
        .unwrap_or(0)
        + 1;

    if let Some(frame) = advancement {
        for lane in &frame.lanes {
            let class_id = canonical_class(pack, &lane.class_id)
                .ok_or_else(|| BuilderError::UnknownClass(lane.class_id.clone()))?;
            // An explicit frame carries the absolute class level per lane.
            if let Some(entry) = draft
                .class_levels
                .iter_mut()
                .find(|entry| entry.class_id.eq_ignore_ascii_case(&class_id))
            {
                entry.level = lane.class_level.max(entry.level);
                if entry.hit_points_by_level.len() as u32 >= entry.level {
                    entry.hit_points_by_level =
                        entry.hit_points_by_level[..entry.level as usize].to_vec();
                } else {
                    while (entry.hit_points_by_level.len() as u32) < entry.level {
                        entry
                            .hit_points_by_level
                            .push(average_hit_die(pack, &class_id));
                    }
                }
            } else {
                draft.class_levels.push(DraftClassLevel {
                    class_id: class_id.clone(),
                    level: lane.class_level.max(1),
                    hit_points_gained: 0,
                    hit_points_by_level: Vec::new(),
                    experience_gained: 0,
                });
            }
            if let Some(subclass) = &lane.subclass {
                draft
                    .class_subclasses
                    .insert(class_id.clone(), subclass.clone());
            }
        }
        // Rebuild class-level totals from the authoritative advancement chain.
        draft.advancement.push(frame);
        sync_draft_class_levels(draft);
    } else {
        let lanes = collect_level_up_lanes(request);
        let mut frame_lanes = Vec::new();
        let mut any_choices = false;
        let multi = lanes.len() > 1;
        for (index, lane) in lanes.iter().enumerate() {
            let (class_id, next_level, gained) = bump_class_level(pack, draft, lane)?;
            if let Some(subclass) = &lane.subclass {
                draft
                    .class_subclasses
                    .insert(class_id.clone(), subclass.clone());
            }
            if lane_has_choices(lane) {
                any_choices = true;
            }
            // Always record a frame for multi-lane levels so the class-level
            // chain stays authoritative for Gestalt characters.
            if lane_has_choices(lane) || multi {
                let lane_id = if multi {
                    if index == 0 {
                        LaneId::new("primary")
                    } else {
                        LaneId::new(format!("lane-{}", index + 1))
                    }
                } else {
                    LaneId::primary()
                };
                frame_lanes.push(build_advancement_lane(
                    pack, lane, lane_id, &class_id, next_level, gained,
                )?);
            }
        }
        if any_choices || multi {
            draft
                .advancement
                .push(AdvancementFrame::new(frame_level, frame_lanes));
            sync_draft_class_levels(draft);
        }
    }

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

fn intelligence_modifier(stats: &BTreeMap<String, i32>) -> i32 {
    stats
        .get("INT")
        .copied()
        .map(|score| (score - 10) / 2)
        .unwrap_or(0)
}

fn progression_for_class<'a>(
    pack: &'a CompiledRulePack,
    class_id: &str,
) -> Option<&'a pcgen_core::domain::ClassProgression> {
    if let Some(progression) = pack.progressions.get(class_id) {
        return Some(progression);
    }
    pack.progressions
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(class_id))
        .map(|(_, progression)| progression)
}

fn skill_point_budget(
    pack: &CompiledRulePack,
    advancement: &[AdvancementFrame],
    class_levels: &[DraftClassLevel],
    intelligence_modifier: i32,
) -> u32 {
    let mut budget: u32 = 0;
    if !advancement.is_empty() {
        for frame in advancement {
            for lane in &frame.lanes {
                let first = lane.class_level == 1;
                let mut points = 0i32;
                if let Some(progression) = progression_for_class(pack, &lane.class_id) {
                    points = if first {
                        progression.first_level_skill_points as i32
                    } else {
                        progression.skill_points_per_level as i32
                    };
                } else if let Some(class) = pack.classes.get(&lane.class_id) {
                    points = if first {
                        (class.skill_points_per_level as i32)
                            .saturating_mul(class.skill_multiplier.unwrap_or(4) as i32)
                    } else {
                        class.skill_points_per_level as i32
                    };
                }
                budget = budget.saturating_add((points + intelligence_modifier).max(1) as u32);
            }
        }
    } else {
        for entry in class_levels {
            let mut points = 0i32;
            if let Some(progression) = progression_for_class(pack, &entry.class_id) {
                let first_total = (progression.first_level_skill_points as i32
                    + intelligence_modifier)
                    .max(1) as u32;
                let per_level = (progression.skill_points_per_level as i32 + intelligence_modifier)
                    .max(1) as u32;
                if entry.level > 0 {
                    budget = budget
                        .saturating_add(first_total + (entry.level - 1).saturating_mul(per_level));
                }
                continue;
            } else if let Some(class) = pack.classes.get(&entry.class_id) {
                points = class.skill_points_per_level as i32;
            }
            let first_total = (points + intelligence_modifier).max(1) as u32;
            let per_level = (points + intelligence_modifier).max(1) as u32;
            if entry.level > 0 {
                budget = budget
                    .saturating_add(first_total + (entry.level - 1).saturating_mul(per_level));
            } else {
                budget = budget.saturating_add(first_total);
            }
        }
    }
    budget
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
        pack.ability_categories.insert(
            "FEAT".to_string(),
            pcgen_core::AbilityCategoryDefinition {
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
        assert_eq!(draft.class_levels[0].class_id, "Fighter");
        assert_eq!(draft.class_levels[0].level, 3);
        assert_eq!(draft.skills.get("Climb"), Some(&4.0));
        assert_eq!(draft.equipment, vec!["Longsword".to_string()]);
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
        assert!(draft.class_levels.is_empty());
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
        assert!(draft.class_levels.is_empty());
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
        assert_eq!(draft.class_levels[0].hit_points_by_level, vec![5, 6]);
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
        assert_eq!(draft.advancement.len(), 1);
        let lane = &draft.advancement[0].lanes[0];
        assert_eq!(lane.class_id, "Fighter");
        assert_eq!(lane.class_level, 2);
        assert_eq!(lane.archetype.as_deref(), Some("Two-Handed Fighter"));
        assert_eq!(lane.subclass.as_deref(), Some("Weapon Master"));
        assert_eq!(draft.class_levels[0].level, 2);
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
        // A gestalt level adds one level to each lane; the raw sum of class
        // levels is 3 for a single character level.
        assert_eq!(draft.total_level(), 3);
        let fighter = draft
            .class_levels
            .iter()
            .find(|entry| entry.class_id == "Fighter")
            .unwrap();
        let wizard = draft
            .class_levels
            .iter()
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
        assert_eq!(draft.advancement.len(), 1, "both lanes share one frame");
        let frame = &draft.advancement[0];
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
        let frame = &draft.advancement[0];
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
