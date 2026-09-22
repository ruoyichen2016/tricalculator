//! Gestalt advancement merge policy.
//!
//! Gestalt characters advance in parallel lanes.  This module turns the
//! frame/lane representation into a merged per-level plan using an explicit
//! policy, so the rules are testable and do not leak into class definitions.

use crate::advancement::{AdvancementFrame, AdvancementLane, LaneId};
use crate::progression::SaveKind;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Merge mode for one numeric contribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMode {
    Best,
    Sum,
    /// First non-empty value in lane order.
    First,
}

/// Merge mode for collections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionMode {
    Union,
    All,
    First,
}

/// How a class consumes Gestalt lane slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrestigeSlotCost {
    /// Base class or single-lane prestige class.
    SingleLane,
    /// Dual-progression prestige class: advances two progressions in one lane.
    DualLane,
    /// Class must be mirrored on both lanes of a frame.
    BothLanes,
}

impl PrestigeSlotCost {
    pub fn slots(self) -> u8 {
        match self {
            Self::SingleLane => 1,
            Self::DualLane | Self::BothLanes => 2,
        }
    }
}

/// Faith conflict policy for multiple divine lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaithPolicy {
    Enforce,
    #[default]
    Warn,
    Ignore,
}

/// Gestalt merge policy used by the planner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GestaltPolicy {
    pub hit_points: MergeMode,
    pub base_attack_bonus: MergeMode,
    pub saves: MergeMode,
    pub skill_points: MergeMode,
    pub class_skills: CollectionMode,
    pub features: CollectionMode,
    pub default_prestige_slot_cost: PrestigeSlotCost,
    pub max_lane_slots: u8,
    pub faith_policy: FaithPolicy,
    pub duplicate_features_are_warnings: bool,
}

impl Default for GestaltPolicy {
    fn default() -> Self {
        Self {
            hit_points: MergeMode::Best,
            base_attack_bonus: MergeMode::Best,
            saves: MergeMode::Best,
            skill_points: MergeMode::Best,
            class_skills: CollectionMode::Union,
            features: CollectionMode::All,
            default_prestige_slot_cost: PrestigeSlotCost::DualLane,
            max_lane_slots: 2,
            faith_policy: FaithPolicy::Warn,
            duplicate_features_are_warnings: true,
        }
    }
}

/// Read-only class progression lookup used by the Gestalt planner.
pub trait ClassProgressSource {
    fn base_attack_bonus(&self, class_id: &str, class_level: u32) -> Option<i32>;
    fn base_save(&self, class_id: &str, class_level: u32, kind: SaveKind) -> Option<i32>;
    fn skill_points(
        &self,
        class_id: &str,
        intelligence_modifier: i32,
        first_level: bool,
    ) -> Option<u32>;
    fn class_skills(&self, class_id: &str) -> Option<Vec<String>>;
    fn features_up_to_level(&self, class_id: &str, level: u32) -> Option<Vec<String>>;

    /// Slot cost metadata for a class; `None` means base-class single lane.
    fn gestalt_slot_cost(&self, _class_id: &str) -> Option<PrestigeSlotCost> {
        None
    }

    /// Resolve features for one advanced class within a lane.
    ///
    /// The default ignores archetype/substitution metadata.  Data-backed
    /// implementations can override this to apply variants.
    fn features_for_class(&self, lane: &AdvancementLane, class_id: &str) -> Option<Vec<String>> {
        self.features_up_to_level(class_id, lane.class_level)
    }

    /// Resolve class skills for one advanced class within a lane.
    fn class_skills_for_lane(
        &self,
        _lane: &AdvancementLane,
        class_id: &str,
    ) -> Option<Vec<String>> {
        self.class_skills(class_id)
    }
}

/// One divine spell track produced by a lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DivineTrack {
    pub lane_id: LaneId,
    pub class_id: String,
    pub class_level: u32,
    pub deity: Option<String>,
    pub domains: Vec<String>,
}

/// A merged result for one character level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GestaltFramePlan {
    pub character_level: u32,
    pub hit_points: i32,
    pub base_attack_bonus_increment: i32,
    pub fortitude_increment: i32,
    pub reflex_increment: i32,
    pub will_increment: i32,
    pub skill_points: u32,
    pub class_skills: Vec<String>,
    pub features: Vec<String>,
    /// Classes whose progression this frame advanced.
    #[serde(default)]
    pub advanced_classes: Vec<String>,
    /// Lane slot cost consumed by this frame.
    #[serde(default)]
    pub slot_cost: u8,
    /// Archetype/substitution labels active in this frame.
    #[serde(default)]
    pub variants: Vec<String>,
    pub lane_ids: Vec<String>,
}

/// A diagnostic produced while planning Gestalt advancement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GestaltDiagnostic {
    pub code: String,
    pub message: String,
    pub character_level: Option<u32>,
    pub lanes: Vec<String>,
}

/// Complete Gestalt plan for a character.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GestaltPlan {
    pub policy: GestaltPolicy,
    pub frames: Vec<GestaltFramePlan>,
    /// Independent spell/power tracks keyed by lane.
    pub lane_spell_tracks: BTreeMap<LaneId, Vec<String>>,
    /// Divine tracks keyed by lane.
    #[serde(default)]
    pub divine_tracks: BTreeMap<LaneId, DivineTrack>,
    pub diagnostics: Vec<GestaltDiagnostic>,
}

/// Result of an incremental Gestalt update.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncrementalGestaltUpdate {
    pub plan: GestaltPlan,
    /// Character levels whose frame plans were recomputed.
    pub recomputed_levels: Vec<u32>,
}

struct FramePlanning {
    plan: GestaltFramePlan,
    diagnostics: Vec<GestaltDiagnostic>,
    divine_tracks: BTreeMap<LaneId, DivineTrack>,
}

/// Build a Gestalt plan from advancement frames.
pub fn plan_gestalt(
    frames: &[AdvancementFrame],
    source: &dyn ClassProgressSource,
    intelligence_modifier: i32,
    policy: &GestaltPolicy,
) -> GestaltPlan {
    let mut frame_plans = Vec::new();
    let mut diagnostics = Vec::new();
    let mut divine_tracks = BTreeMap::new();

    for frame in frames {
        let result = plan_frame(frame, source, intelligence_modifier, policy);
        frame_plans.push(result.plan);
        diagnostics.extend(result.diagnostics);
        divine_tracks.extend(result.divine_tracks);
    }

    GestaltPlan {
        policy: policy.clone(),
        frames: frame_plans,
        lane_spell_tracks: lane_spell_tracks(frames),
        divine_tracks,
        diagnostics,
    }
}

/// Recompute only frames that changed relative to a previous plan.
pub fn plan_gestalt_incremental(
    previous_frames: &[AdvancementFrame],
    previous: &GestaltPlan,
    frames: &[AdvancementFrame],
    source: &dyn ClassProgressSource,
    intelligence_modifier: i32,
    policy: &GestaltPolicy,
) -> IncrementalGestaltUpdate {
    let previous_by_level: BTreeMap<u32, (&AdvancementFrame, &GestaltFramePlan)> = previous_frames
        .iter()
        .zip(previous.frames.iter())
        .map(|(frame, plan)| (frame.character_level, (frame, plan)))
        .collect();
    let previous_diagnostics_by_level: BTreeMap<u32, Vec<GestaltDiagnostic>> = previous
        .diagnostics
        .iter()
        .filter_map(|diagnostic| {
            diagnostic
                .character_level
                .map(|level| (level, diagnostic.clone()))
        })
        .fold(BTreeMap::new(), |mut acc, (level, diagnostic)| {
            acc.entry(level).or_default().push(diagnostic);
            acc
        });

    let mut frame_plans = Vec::new();
    let mut diagnostics = Vec::new();
    let mut divine_tracks = BTreeMap::new();
    let mut recomputed_levels = Vec::new();

    for frame in frames {
        if let Some((previous_frame, previous_plan)) = previous_by_level.get(&frame.character_level)
        {
            if *previous_frame == frame {
                frame_plans.push((*previous_plan).clone());
                if let Some(previous_diags) =
                    previous_diagnostics_by_level.get(&frame.character_level)
                {
                    diagnostics.extend(previous_diags.iter().cloned());
                }
                continue;
            }
        }
        let result = plan_frame(frame, source, intelligence_modifier, policy);
        frame_plans.push(result.plan);
        diagnostics.extend(result.diagnostics);
        divine_tracks.extend(result.divine_tracks);
        recomputed_levels.push(frame.character_level);
    }

    IncrementalGestaltUpdate {
        plan: GestaltPlan {
            policy: policy.clone(),
            frames: frame_plans,
            lane_spell_tracks: lane_spell_tracks(frames),
            divine_tracks,
            diagnostics,
        },
        recomputed_levels,
    }
}

fn plan_frame(
    frame: &AdvancementFrame,
    source: &dyn ClassProgressSource,
    intelligence_modifier: i32,
    policy: &GestaltPolicy,
) -> FramePlanning {
    let mut diagnostics = Vec::new();
    let mut divine_tracks = BTreeMap::new();
    let mut hit_points = Vec::new();
    let mut bab = Vec::new();
    let mut fort = Vec::new();
    let mut reflex = Vec::new();
    let mut will = Vec::new();
    let mut skill_points = Vec::new();
    let mut class_skills = BTreeSet::new();
    let mut features = Vec::new();
    let mut advanced_seen = BTreeSet::new();
    let mut variants = BTreeSet::new();
    let mut deities = BTreeSet::new();
    let mut feature_seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut slot_cost = 0u8;

    for lane in &frame.lanes {
        let advanced_classes = lane.advanced_classes();
        let lane_variant = lane.variant_label();
        if let Some(variant) = &lane_variant {
            variants.insert(variant.clone());
            diagnostics.push(GestaltDiagnostic {
                code: "gestalt.variant_lane".to_string(),
                message: format!("lane {} uses variant {}", lane.lane_id, variant),
                character_level: Some(frame.character_level),
                lanes: vec![lane.lane_id.to_string()],
            });
        }
        if lane.is_divine() {
            deities.insert(lane.deity.clone().unwrap_or_else(|| "(none)".to_string()));
            divine_tracks.insert(
                lane.lane_id.clone(),
                DivineTrack {
                    lane_id: lane.lane_id.clone(),
                    class_id: lane.class_id.clone(),
                    class_level: lane.class_level,
                    deity: lane.deity.clone(),
                    domains: lane.domains.clone(),
                },
            );
        }
        if advanced_classes.len() > 1 {
            diagnostics.push(GestaltDiagnostic {
                code: "gestalt.dual_progression".to_string(),
                message: format!(
                    "lane {} advances multiple classes: {}",
                    lane.lane_id,
                    advanced_classes.join(", ")
                ),
                character_level: Some(frame.character_level),
                lanes: vec![lane.lane_id.to_string()],
            });
        }
        hit_points.push(lane.hit_points_gained);

        let lane_slot = if advanced_classes.len() > 1 {
            PrestigeSlotCost::DualLane
        } else {
            source
                .gestalt_slot_cost(&lane.class_id)
                .unwrap_or(PrestigeSlotCost::SingleLane)
        };
        slot_cost = slot_cost.saturating_add(lane_slot.slots());
        if lane_slot == PrestigeSlotCost::BothLanes {
            diagnostics.push(GestaltDiagnostic {
                code: "gestalt.prestige_both_lanes".to_string(),
                message: format!(
                    "class `{}` requires mirrored lanes in frame {}",
                    lane.class_id, frame.character_level
                ),
                character_level: Some(frame.character_level),
                lanes: vec![lane.lane_id.to_string()],
            });
        }

        for class_id in &advanced_classes {
            advanced_seen.insert(class_id.clone());

            if let Some(current_bab) = source.base_attack_bonus(class_id, lane.class_level) {
                let previous = if lane.class_level > 1 {
                    source
                        .base_attack_bonus(class_id, lane.class_level - 1)
                        .unwrap_or(current_bab)
                } else {
                    0
                };
                bab.push(current_bab - previous);
            } else {
                diagnostics.push(missing_class_diagnostic(frame, class_id.as_str(), "BAB"));
            }

            for (kind, target) in [
                (SaveKind::Fortitude, &mut fort),
                (SaveKind::Reflex, &mut reflex),
                (SaveKind::Will, &mut will),
            ] {
                if let Some(current) = source.base_save(class_id, lane.class_level, kind) {
                    let previous = if lane.class_level > 1 {
                        source
                            .base_save(class_id, lane.class_level - 1, kind)
                            .unwrap_or(current)
                    } else {
                        0
                    };
                    target.push(current - previous);
                } else {
                    diagnostics.push(missing_class_diagnostic(frame, class_id.as_str(), "save"));
                }
            }

            if let Some(points) =
                source.skill_points(class_id, intelligence_modifier, lane.class_level == 1)
            {
                skill_points.push(points);
            } else {
                diagnostics.push(missing_class_diagnostic(
                    frame,
                    class_id.as_str(),
                    "skill points",
                ));
            }

            if let Some(skills) = source.class_skills_for_lane(lane, class_id) {
                class_skills.extend(skills);
            }
            if let Some(level_features) = source.features_for_class(lane, class_id) {
                for feature in level_features {
                    let key = feature.to_ascii_lowercase();
                    feature_seen
                        .entry(key)
                        .or_default()
                        .push(lane.lane_id.to_string());
                    features.push(feature);
                }
            }
        }
    }

    for (feature, lanes) in &feature_seen {
        if lanes.len() > 1 {
            diagnostics.push(GestaltDiagnostic {
                code: if policy.duplicate_features_are_warnings {
                    "gestalt.duplicate_feature"
                } else {
                    "gestalt.duplicate_feature_error"
                }
                .to_string(),
                message: format!(
                    "feature `{feature}` is granted by multiple Gestalt lanes: {}",
                    lanes.join(", ")
                ),
                character_level: Some(frame.character_level),
                lanes: lanes.clone(),
            });
        }
    }

    if deities.len() > 1 && policy.faith_policy != FaithPolicy::Ignore {
        diagnostics.push(GestaltDiagnostic {
            code: if policy.faith_policy == FaithPolicy::Enforce {
                "gestalt.multiple_deities_error"
            } else {
                "gestalt.multiple_deities"
            }
            .to_string(),
            message: format!(
                "multiple divine lanes reference different deities: {}",
                deities.into_iter().collect::<Vec<_>>().join(", ")
            ),
            character_level: Some(frame.character_level),
            lanes: frame
                .lanes
                .iter()
                .filter(|lane| lane.is_divine())
                .map(|lane| lane.lane_id.to_string())
                .collect(),
        });
    }

    if slot_cost > policy.max_lane_slots {
        diagnostics.push(GestaltDiagnostic {
            code: "gestalt.prestige_slot_overflow".to_string(),
            message: format!(
                "frame {} consumes {} lane slots but the policy allows {}",
                frame.character_level, slot_cost, policy.max_lane_slots
            ),
            character_level: Some(frame.character_level),
            lanes: frame
                .lanes
                .iter()
                .map(|lane| lane.lane_id.to_string())
                .collect(),
        });
    }

    FramePlanning {
        plan: GestaltFramePlan {
            character_level: frame.character_level,
            hit_points: merge_numeric(&hit_points, policy.hit_points),
            base_attack_bonus_increment: merge_numeric(&bab, policy.base_attack_bonus),
            fortitude_increment: merge_numeric(&fort, policy.saves),
            reflex_increment: merge_numeric(&reflex, policy.saves),
            will_increment: merge_numeric(&will, policy.saves),
            skill_points: merge_numeric_u32(&skill_points, policy.skill_points),
            class_skills: class_skills.into_iter().collect(),
            features,
            advanced_classes: advanced_seen.into_iter().collect(),
            slot_cost,
            variants: variants.into_iter().collect(),
            lane_ids: frame
                .lanes
                .iter()
                .map(|lane| lane.lane_id.to_string())
                .collect(),
        },
        diagnostics,
        divine_tracks,
    }
}

fn lane_spell_tracks(frames: &[AdvancementFrame]) -> BTreeMap<LaneId, Vec<String>> {
    let mut tracks: BTreeMap<LaneId, Vec<String>> = BTreeMap::new();
    for frame in frames {
        for lane in &frame.lanes {
            tracks
                .entry(lane.lane_id.clone())
                .or_default()
                .extend(lane.advanced_classes());
        }
    }
    tracks
}

fn missing_class_diagnostic(
    frame: &AdvancementFrame,
    class_id: &str,
    value: &str,
) -> GestaltDiagnostic {
    GestaltDiagnostic {
        code: "gestalt.unknown_class_progress".to_string(),
        message: format!("no {value} progression for class `{class_id}`"),
        character_level: Some(frame.character_level),
        lanes: frame
            .lanes
            .iter()
            .filter(|lane| lane.class_id.eq_ignore_ascii_case(class_id))
            .map(|lane| lane.lane_id.to_string())
            .collect(),
    }
}

fn merge_numeric(values: &[i32], mode: MergeMode) -> i32 {
    match mode {
        MergeMode::Best => values.iter().copied().max().unwrap_or(0),
        MergeMode::Sum => values.iter().sum(),
        MergeMode::First => values.first().copied().unwrap_or(0),
    }
}

fn merge_numeric_u32(values: &[u32], mode: MergeMode) -> u32 {
    match mode {
        MergeMode::Best => values.iter().copied().max().unwrap_or(0),
        MergeMode::Sum => values.iter().sum(),
        MergeMode::First => values.first().copied().unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advancement::{AdvancementFrame, AdvancementLane};

    struct FakeSource;

    impl ClassProgressSource for FakeSource {
        fn base_attack_bonus(&self, class_id: &str, class_level: u32) -> Option<i32> {
            match class_id {
                "Fighter" => Some(class_level as i32),
                "Wizard" => Some((class_level / 2) as i32),
                "Mystic Theurge" => Some((class_level / 2) as i32),
                _ => None,
            }
        }

        fn base_save(&self, class_id: &str, class_level: u32, kind: SaveKind) -> Option<i32> {
            Some(match (class_id, kind) {
                ("Fighter", SaveKind::Fortitude) => 2 + (class_level as i32) / 2,
                ("Wizard", SaveKind::Will) => 2 + (class_level as i32) / 2,
                ("Mystic Theurge", SaveKind::Fortitude) => 1 + (class_level as i32) / 2,
                ("Mystic Theurge", SaveKind::Will) => 1 + (class_level as i32) / 2,
                _ => class_level as i32 / 3,
            })
        }

        fn skill_points(
            &self,
            _class_id: &str,
            intelligence_modifier: i32,
            first_level: bool,
        ) -> Option<u32> {
            let base = if first_level { 8 } else { 2 };
            Some((base + intelligence_modifier).max(1) as u32)
        }

        fn class_skills(&self, class_id: &str) -> Option<Vec<String>> {
            Some(match class_id {
                "Fighter" => vec!["Climb".to_string()],
                "Wizard" => vec!["Spellcraft".to_string()],
                "Mystic Theurge" => vec!["Knowledge (Arcana)".to_string()],
                _ => vec![],
            })
        }

        fn features_up_to_level(&self, class_id: &str, level: u32) -> Option<Vec<String>> {
            Some(
                (1..=level)
                    .map(|level| format!("{class_id} Feature {level}"))
                    .collect(),
            )
        }

        fn gestalt_slot_cost(&self, class_id: &str) -> Option<PrestigeSlotCost> {
            match class_id {
                "Mystic Theurge" => Some(PrestigeSlotCost::DualLane),
                _ => None,
            }
        }

        fn features_for_class(
            &self,
            lane: &AdvancementLane,
            class_id: &str,
        ) -> Option<Vec<String>> {
            if lane.archetype.is_some() {
                Some(vec![format!("{class_id} Archetype Feature")])
            } else {
                self.features_up_to_level(class_id, lane.class_level)
            }
        }
    }

    #[test]
    fn gestalt_policy_merges_best_hp_bab_saves_and_unions_skills() {
        let frames = vec![AdvancementFrame::new(
            1,
            vec![
                AdvancementLane::new("a", "Fighter", 1, 10, 0),
                AdvancementLane::new("b", "Wizard", 1, 4, 0),
            ],
        )];
        let plan = plan_gestalt(&frames, &FakeSource, 2, &GestaltPolicy::default());
        let frame = &plan.frames[0];
        assert_eq!(frame.hit_points, 10);
        assert_eq!(frame.base_attack_bonus_increment, 1);
        assert_eq!(frame.fortitude_increment, 2);
        assert_eq!(frame.will_increment, 2);
        assert_eq!(frame.skill_points, 10);
        assert!(frame.class_skills.contains(&"Climb".to_string()));
        assert!(frame.class_skills.contains(&"Spellcraft".to_string()));
        assert_eq!(plan.lane_spell_tracks.len(), 2);
    }

    #[test]
    fn dual_progression_lane_advances_both_classes() {
        let frame = AdvancementFrame::new(
            1,
            vec![AdvancementLane::new("dual", "Mystic Theurge", 1, 8, 0)
                .with_advances(["Fighter".to_string(), "Wizard".to_string()])],
        );
        let plan = plan_gestalt(&[frame], &FakeSource, 0, &GestaltPolicy::default());
        assert!(plan.frames[0]
            .advanced_classes
            .contains(&"Fighter".to_string()));
        assert!(plan.frames[0]
            .advanced_classes
            .contains(&"Wizard".to_string()));
        assert!(plan.frames[0].class_skills.contains(&"Climb".to_string()));
        assert!(plan.frames[0]
            .class_skills
            .contains(&"Spellcraft".to_string()));
        assert!(plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "gestalt.dual_progression"));
    }

    #[test]
    fn archetype_substitution_and_faith_metadata_reach_the_plan() {
        let frame = AdvancementFrame::new(
            1,
            vec![
                AdvancementLane::new("a", "Fighter", 1, 10, 0)
                    .with_archetype("Two-Handed Fighter")
                    .with_substitution_level("1"),
                AdvancementLane::new("b", "Cleric", 1, 8, 0)
                    .with_faith("Pelor", ["Sun".to_string(), "Healing".to_string()]),
                AdvancementLane::new("c", "Druid", 1, 8, 0)
                    .with_faith("Obad-Hai", ["Nature".to_string()]),
            ],
        );
        let plan = plan_gestalt(&[frame], &FakeSource, 0, &GestaltPolicy::default());
        assert!(plan.frames[0]
            .variants
            .contains(&"Two-Handed Fighter+1".to_string()));
        assert_eq!(plan.divine_tracks.len(), 2);
        assert!(plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "gestalt.multiple_deities"));
        assert!(plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "gestalt.variant_lane"));
    }

    #[test]
    fn prestige_slot_cost_and_overflow_are_reported() {
        let frame = AdvancementFrame::new(
            1,
            vec![
                AdvancementLane::new("a", "Mystic Theurge", 1, 8, 0)
                    .with_advances(["Fighter".to_string(), "Wizard".to_string()]),
                AdvancementLane::new("b", "Fighter", 1, 10, 0),
            ],
        );
        let plan = plan_gestalt(&[frame], &FakeSource, 0, &GestaltPolicy::default());
        assert_eq!(plan.frames[0].slot_cost, 3);
        assert!(plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "gestalt.prestige_slot_overflow"));
    }

    #[test]
    fn incremental_plan_recomputes_only_changed_frames() {
        let frames = vec![
            AdvancementFrame::single(1, "Fighter", 1, 10, 0),
            AdvancementFrame::single(2, "Fighter", 2, 6, 0),
        ];
        let plan = plan_gestalt(&frames, &FakeSource, 0, &GestaltPolicy::default());
        let mut changed = frames.clone();
        changed[1].lanes[0].hit_points_gained = 8;
        let update = plan_gestalt_incremental(
            &frames,
            &plan,
            &changed,
            &FakeSource,
            0,
            &GestaltPolicy::default(),
        );
        assert_eq!(update.recomputed_levels, vec![2]);
        assert_eq!(update.plan.frames[1].hit_points, 8);
        // Unchanged frame remains the same plan object contents.
        assert_eq!(update.plan.frames[0], plan.frames[0]);
    }
}
