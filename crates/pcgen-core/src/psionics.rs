//! RSRD psionic power-point tables.
//!
//! PCGen encodes these as conditional `BONUS:VAR|BasePowerPoints` rows.
//! The tables below are the effective RSRD values after those conditional
//! bonuses are accumulated.

const PSION_WILDER: [i32; 20] = [
    2, 6, 11, 17, 25, 35, 46, 58, 72, 88, 106, 126, 147, 170, 195, 221, 250, 280, 311, 343,
];

const PSYCHIC_WARRIOR: [i32; 20] = [
    0, 1, 3, 5, 7, 11, 15, 19, 23, 27, 35, 43, 51, 59, 67, 79, 91, 103, 115, 127,
];

const PSIONIC_FIST: [i32; 10] = [1, 3, 6, 10, 15, 23, 31, 43, 55, 71];

const WAR_MIND: [i32; 10] = [2, 5, 9, 14, 20, 28, 37, 47, 58, 70];

/// Base power points for a manifester class at a class level.
pub fn base_power_points(class_id: &str, class_level: u32) -> i32 {
    let level = class_level.max(1) as usize;
    let key = class_id.trim().to_ascii_lowercase();
    let table: &[i32] = match key.as_str() {
        "psion" | "wilder" => &PSION_WILDER,
        "psychic warrior" => &PSYCHIC_WARRIOR,
        "psionic fist" => &PSIONIC_FIST,
        "war mind" | "warmind" => &WAR_MIND,
        _ => return 0,
    };
    table.get(level - 1).copied().unwrap_or(0)
}

/// Standard bonus power points: key ability modifier * manifester level / 2.
pub fn ability_bonus_power_points(ability_modifier: i32, manifester_level: u32) -> i32 {
    ((ability_modifier * manifester_level as i32) / 2).max(0)
}

/// Total power points for one class track.
pub fn total_power_points(
    class_id: &str,
    class_level: u32,
    ability_modifier: i32,
    misc_bonus: i32,
) -> i32 {
    base_power_points(class_id, class_level)
        + ability_bonus_power_points(ability_modifier, class_level)
        + misc_bonus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsrd_tables_match_known_values() {
        assert_eq!(base_power_points("Psion", 1), 2);
        assert_eq!(base_power_points("Psion", 5), 25);
        assert_eq!(base_power_points("Psion", 20), 343);
        assert_eq!(base_power_points("Wilder", 10), 88);
        assert_eq!(base_power_points("Psychic Warrior", 1), 0);
        assert_eq!(base_power_points("Psychic Warrior", 6), 11);
        assert_eq!(base_power_points("Psychic Warrior", 20), 127);
        assert_eq!(base_power_points("Psionic Fist", 10), 71);
        assert_eq!(base_power_points("War Mind", 10), 70);
        assert_eq!(base_power_points("Soulknife", 10), 0);
    }

    #[test]
    fn ability_bonus_uses_half_level_rule() {
        assert_eq!(ability_bonus_power_points(4, 5), 10);
        assert_eq!(total_power_points("Psion", 5, 4, 2), 37);
    }
}
