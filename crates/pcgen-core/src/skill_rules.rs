//! Skills rules that exist only in the SRD's *text*, not in its tables.
//!
//! PCGen stores skills as numbers, so anything the rules state in prose rather
//! than in a table tends to be absent from the data.  These are exactly the
//! rules the engine previously did not implement at all.
//!
//! Source: <https://www.d20srd.org/srd/skills/skillsSummary.htm>
//!
//! > Your maximum rank in a class skill is your character level + 3.
//! > Your maximum rank in a cross-class skill is one-half of this number (do
//! > not round up or down).
//! >
//! > Regardless of whether a skill is purchased as a class skill or a
//! > cross-class skill, if it is a class skill for any of your classes, your
//! > maximum rank equals your total character level + 3.
//!
//! and <https://www.d20srd.org/srd/skills/skillsSummary.htm> again for the
//! purchase rate:
//!
//! > Each skill point you spend on a class skill gets you 1 rank in that skill.
//! > Each skill point you spend on a cross-class skill gets your character ½
//! > rank in that skill. [...] (Half ranks do not improve your skill check,
//! > but two ½ ranks make 1 rank.)

/// Ranks a skill point buys in a class skill.
pub const RANKS_PER_POINT_CLASS: f64 = 1.0;

/// Ranks a skill point buys in a cross-class skill.
pub const RANKS_PER_POINT_CROSS_CLASS: f64 = 0.5;

/// Maximum ranks in a **class** skill: character level + 3.
///
/// Note the use of *character* level, not the level in the class that owns the
/// skill — the SRD is explicit, and it is why a multiclass character's class
/// skills from any class all use the same cap.
pub fn max_rank_class_skill(character_level: u32) -> f64 {
    f64::from(character_level) + 3.0
}

/// Maximum ranks in a **cross-class** skill: half the class-skill cap.
///
/// The SRD says "do not round up or down", so a character level of 1 gives a
/// cap of exactly 2.0 (not 2 or 2.5).
pub fn max_rank_cross_class_skill(character_level: u32) -> f64 {
    max_rank_class_skill(character_level) / 2.0
}

/// Maximum ranks for one skill, given whether it is a class skill.
pub fn max_rank(character_level: u32, is_class_skill: bool) -> f64 {
    if is_class_skill {
        max_rank_class_skill(character_level)
    } else {
        max_rank_cross_class_skill(character_level)
    }
}

/// Ranks bought by `points` skill points in a skill of the given kind.
pub fn ranks_for_points(points: f64, is_class_skill: bool) -> f64 {
    let rate = if is_class_skill {
        RANKS_PER_POINT_CLASS
    } else {
        RANKS_PER_POINT_CROSS_CLASS
    };
    points * rate
}

/// Skill points needed to reach `ranks` in a skill of the given kind.
pub fn points_for_ranks(ranks: f64, is_class_skill: bool) -> f64 {
    let rate = if is_class_skill {
        RANKS_PER_POINT_CLASS
    } else {
        RANKS_PER_POINT_CROSS_CLASS
    };
    ranks / rate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_skill_cap_is_character_level_plus_three() {
        assert_eq!(max_rank_class_skill(1), 4.0);
        assert_eq!(max_rank_class_skill(20), 23.0);
    }

    #[test]
    fn cross_class_cap_is_exactly_half_without_rounding() {
        // "do not round up or down": level 1 -> 2.0, level 2 -> 2.5.
        assert_eq!(max_rank_cross_class_skill(1), 2.0);
        assert_eq!(max_rank_cross_class_skill(2), 2.5);
        assert_eq!(max_rank_cross_class_skill(20), 11.5);
    }

    #[test]
    fn two_cross_class_points_buy_one_rank() {
        assert_eq!(ranks_for_points(1.0, false), 0.5);
        assert_eq!(ranks_for_points(2.0, false), 1.0);
        assert_eq!(ranks_for_points(1.0, true), 1.0);
        // Round trip: the points a given rank count costs.
        assert_eq!(points_for_ranks(1.0, false), 2.0);
        assert_eq!(points_for_ranks(1.0, true), 1.0);
    }
}
