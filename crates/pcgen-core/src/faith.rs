//! Faith contracts and decisions for divine lanes.

use crate::gestalt::{DivineTrack, FaithPolicy};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaithDecision {
    pub lane_id: String,
    pub deity: Option<String>,
    pub domains: Vec<String>,
    pub allowed: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Evaluate divine tracks under a campaign faith policy.
pub fn evaluate_faith(tracks: &[DivineTrack], policy: FaithPolicy) -> Vec<FaithDecision> {
    let deities: Vec<String> = {
        let mut deities: Vec<String> = tracks
            .iter()
            .filter_map(|track| track.deity.clone())
            .collect();
        deities.sort();
        deities.dedup();
        deities
    };
    let multiple_deities = deities.len() > 1;
    tracks
        .iter()
        .map(|track| {
            let mut warnings = Vec::new();
            let mut errors = Vec::new();
            if track.deity.is_none() {
                warnings.push("divine lane has no deity".to_string());
            }
            if track.domains.is_empty() {
                warnings.push("divine lane has no domains".to_string());
            }
            if multiple_deities {
                match policy {
                    FaithPolicy::Enforce => errors.push(format!(
                        "multiple deities in one gestalt character: {}",
                        deities.join(", ")
                    )),
                    FaithPolicy::Warn => warnings.push(format!(
                        "multiple deities in one gestalt character: {}",
                        deities.join(", ")
                    )),
                    FaithPolicy::Ignore => {}
                }
            }
            FaithDecision {
                lane_id: track.lane_id.to_string(),
                deity: track.deity.clone(),
                domains: track.domains.clone(),
                allowed: errors.is_empty(),
                warnings,
                errors,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advancement::LaneId;

    fn track(deity: &str) -> DivineTrack {
        DivineTrack {
            lane_id: LaneId::new(deity.to_lowercase()),
            class_id: "Cleric".to_string(),
            class_level: 1,
            deity: Some(deity.to_string()),
            domains: vec!["War".to_string()],
        }
    }

    #[test]
    fn multiple_deities_follow_policy() {
        let tracks = [track("Pelor"), track("Obad-Hai")];
        let warn = evaluate_faith(&tracks, FaithPolicy::Warn);
        assert!(warn.iter().all(|decision| decision.allowed));
        assert!(warn.iter().any(|decision| !decision.warnings.is_empty()));
        let enforce = evaluate_faith(&tracks, FaithPolicy::Enforce);
        assert!(enforce.iter().any(|decision| !decision.allowed));
    }
}
