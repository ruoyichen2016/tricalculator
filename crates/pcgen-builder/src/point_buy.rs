//! D&D 3.5e DMG point-buy helper.
//!
//! 3.0 compatibility uses the same table for now; the difference is captured
//! in the caller-provided budget.

use crate::BuilderError;
use std::collections::BTreeMap;

/// Standard 3.5e DMG point-buy budget.
pub const STANDARD_BUDGET: u32 = 25;
/// Standard 3.5e DMG array.
pub const STANDARD_ARRAY: [i32; 6] = [15, 14, 13, 12, 10, 8];
/// Scores legal in standard point buy.
pub const MIN_SCORE: i32 = 8;
pub const MAX_SCORE: i32 = 18;

/// Cost of a single ability score in the 3.5e DMG table.
pub fn score_cost(score: i32) -> Option<u32> {
    Some(match score {
        8 => 0,
        9 => 1,
        10 => 2,
        11 => 3,
        12 => 4,
        13 => 5,
        14 => 6,
        15 => 8,
        16 => 10,
        17 => 13,
        18 => 16,
        _ => return None,
    })
}

/// Total point-buy cost of a score map.  Unknown / out-of-range scores return
/// `None`.
pub fn total_cost(scores: &BTreeMap<String, i32>) -> Option<u32> {
    let mut total = 0;
    for score in scores.values() {
        total += score_cost(*score)?;
    }
    Some(total)
}

/// Validate a score map against a point-buy budget.
pub fn validate(scores: &BTreeMap<String, i32>, budget: u32) -> Result<u32, BuilderError> {
    if scores.is_empty() {
        return Err(BuilderError::InvalidPointBuy(
            "no ability scores".to_string(),
        ));
    }
    for (stat, score) in scores {
        if !(MIN_SCORE..=MAX_SCORE).contains(score) {
            return Err(BuilderError::InvalidPointBuy(format!(
                "{stat}={score} outside {MIN_SCORE}..={MAX_SCORE}"
            )));
        }
    }
    let Some(total) = total_cost(scores) else {
        return Err(BuilderError::InvalidPointBuy(
            "score outside point-buy table".to_string(),
        ));
    };
    if total > budget {
        return Err(BuilderError::InvalidPointBuy(format!(
            "spent {total} > budget {budget}"
        )));
    }
    Ok(total)
}

/// Expand the standard array into the canonical six ability keys.
pub fn standard_array_scores() -> BTreeMap<String, i32> {
    let keys = ["STR", "DEX", "CON", "INT", "WIS", "CHA"];
    keys.iter()
        .copied()
        .zip(STANDARD_ARRAY)
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_match_dmg_table() {
        assert_eq!(score_cost(8), Some(0));
        assert_eq!(score_cost(14), Some(6));
        assert_eq!(score_cost(15), Some(8));
        assert_eq!(score_cost(16), Some(10));
        assert_eq!(score_cost(17), Some(13));
        assert_eq!(score_cost(18), Some(16));
        assert_eq!(score_cost(7), None);
    }

    #[test]
    fn accepts_exact_budget() {
        let scores: BTreeMap<String, i32> = [
            ("STR", 16),
            ("DEX", 14),
            ("CON", 12),
            ("INT", 11),
            ("WIS", 10),
            ("CHA", 8),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect();
        assert_eq!(validate(&scores, 25).unwrap(), 25);
    }

    #[test]
    fn rejects_overspend_and_out_of_range() {
        let overspent: BTreeMap<String, i32> = [("STR", 18), ("DEX", 18)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        assert!(validate(&overspent, 25).is_err());

        let out_of_range: BTreeMap<String, i32> = [("STR", 7)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        assert!(validate(&out_of_range, 25).is_err());
    }

    #[test]
    fn standard_array_is_valid() {
        let scores = standard_array_scores();
        assert_eq!(scores.len(), 6);
        assert!(validate(&scores, STANDARD_BUDGET).is_ok());
    }
}
