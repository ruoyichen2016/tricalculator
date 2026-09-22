//! Per-character-level advancement frames.
//!
//! A frame represents one character level.  Normal characters have exactly
//! one lane per frame; Gestalt characters have one lane per parallel
//! progression.  Keeping this representation in the domain model lets the
//! engine grow Gestalt support without turning classes into combination
//! classes or bolting Gestalt checks onto the flat class-level list.

use serde::{Deserialize, Serialize};

/// Stable identifier for one parallel progression lane.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LaneId(pub String);

impl LaneId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn primary() -> Self {
        Self("primary".to_string())
    }
}

impl Default for LaneId {
    fn default() -> Self {
        Self::primary()
    }
}

impl From<&str> for LaneId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for LaneId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for LaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One class-progression contribution inside an advancement frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvancementLane {
    pub lane_id: LaneId,
    pub class_id: String,
    /// Level within the class at this character level (1-based).
    pub class_level: u32,
    pub hit_points_gained: i32,
    #[serde(default)]
    pub experience_gained: i32,
    /// Classes whose progression this lane advances.
    ///
    /// Empty means this lane advances only `class_id`.  A dual-progression
    /// prestige class lists both target classes here.
    #[serde(default)]
    pub advances: Vec<String>,
    /// Optional archetype name applied to this lane.
    #[serde(default)]
    pub archetype: Option<String>,
    /// Optional substitution-level identifier applied to this lane.
    #[serde(default)]
    pub substitution_level: Option<String>,
    /// Deity bound to this divine lane, if any.
    #[serde(default)]
    pub deity: Option<String>,
    /// Domains granted by this divine lane.
    #[serde(default)]
    pub domains: Vec<String>,
}

impl AdvancementLane {
    pub fn new(
        lane_id: impl Into<LaneId>,
        class_id: impl Into<String>,
        class_level: u32,
        hit_points_gained: i32,
        experience_gained: i32,
    ) -> Self {
        Self {
            lane_id: lane_id.into(),
            class_id: class_id.into(),
            class_level,
            hit_points_gained,
            experience_gained,
            advances: Vec::new(),
            archetype: None,
            substitution_level: None,
            deity: None,
            domains: Vec::new(),
        }
    }

    pub fn with_advances(mut self, advances: impl IntoIterator<Item = String>) -> Self {
        self.advances = advances.into_iter().collect();
        self
    }

    pub fn with_archetype(mut self, archetype: impl Into<String>) -> Self {
        self.archetype = Some(archetype.into());
        self
    }

    pub fn with_substitution_level(mut self, substitution_level: impl Into<String>) -> Self {
        self.substitution_level = Some(substitution_level.into());
        self
    }

    pub fn with_faith(
        mut self,
        deity: impl Into<String>,
        domains: impl IntoIterator<Item = String>,
    ) -> Self {
        self.deity = Some(deity.into());
        self.domains = domains.into_iter().collect();
        self
    }

    pub fn is_divine(&self) -> bool {
        self.deity.is_some() || !self.domains.is_empty()
    }

    pub fn variant_label(&self) -> Option<String> {
        match (&self.archetype, &self.substitution_level) {
            (Some(archetype), Some(substitution)) => Some(format!("{archetype}+{substitution}")),
            (Some(archetype), None) => Some(archetype.clone()),
            (None, Some(substitution)) => Some(format!("substitution:{substitution}")),
            (None, None) => None,
        }
    }

    pub fn advanced_classes(&self) -> Vec<String> {
        if self.advances.is_empty() {
            vec![self.class_id.clone()]
        } else {
            self.advances.clone()
        }
    }

    pub fn primary(
        class_id: impl Into<String>,
        class_level: u32,
        hit_points_gained: i32,
        experience_gained: i32,
    ) -> Self {
        Self::new(
            LaneId::primary(),
            class_id,
            class_level,
            hit_points_gained,
            experience_gained,
        )
    }
}

/// One character level, containing one or more parallel lanes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvancementFrame {
    pub character_level: u32,
    pub lanes: Vec<AdvancementLane>,
}

impl AdvancementFrame {
    pub fn new(character_level: u32, lanes: Vec<AdvancementLane>) -> Self {
        Self {
            character_level,
            lanes,
        }
    }

    pub fn single(
        character_level: u32,
        class_id: impl Into<String>,
        class_level: u32,
        hit_points_gained: i32,
        experience_gained: i32,
    ) -> Self {
        Self::new(
            character_level,
            vec![AdvancementLane::primary(
                class_id,
                class_level,
                hit_points_gained,
                experience_gained,
            )],
        )
    }

    pub fn is_gestalt(&self) -> bool {
        self.lanes.len() > 1
    }

    pub fn lane_for_class(&self, class_id: &str) -> Option<&AdvancementLane> {
        self.lanes
            .iter()
            .find(|lane| lane.class_id.eq_ignore_ascii_case(class_id))
    }

    pub fn hit_points(&self) -> i32 {
        self.lanes.iter().map(|lane| lane.hit_points_gained).sum()
    }

    pub fn experience(&self) -> i32 {
        self.lanes.iter().map(|lane| lane.experience_gained).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_lane_frame_is_not_gestalt() {
        let frame = AdvancementFrame::single(1, "Fighter", 1, 10, 0);
        assert!(!frame.is_gestalt());
        assert_eq!(frame.lanes.len(), 1);
        assert_eq!(frame.hit_points(), 10);
        assert_eq!(frame.lane_for_class("fighter").unwrap().class_level, 1);
    }

    #[test]
    fn multi_lane_frame_represents_gestalt() {
        let frame = AdvancementFrame::new(
            5,
            vec![
                AdvancementLane::new("a", "Fighter", 5, 32, 10_000),
                AdvancementLane::new("b", "Wizard", 5, 18, 10_000),
            ],
        );
        assert!(frame.is_gestalt());
        assert_eq!(frame.hit_points(), 50);
        assert_eq!(frame.lanes.len(), 2);
    }
}
