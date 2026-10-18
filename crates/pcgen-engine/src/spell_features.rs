//! Personal-spell conversion and temporary-HP riders, independent of forum cases.
use pcgen_core::{
    magic::{SpellProfile, SpellRange},
    SpellPowerType,
};
use serde::Serialize;
pub fn spellguard_range(
    level: u32,
    power: SpellPowerType,
    profile: &SpellProfile,
) -> Result<SpellRange, String> {
    if level < 4 {
        return Err("Spellguard requires actual Spellguard of Silverymoon level 4".into());
    }
    if power != SpellPowerType::Arcane {
        return Err(
            "Spellguard requires an arcane spell; Deity's Favor does not change spell type".into(),
        );
    }
    if profile.range != SpellRange::Personal || !profile.spellguard_defensive {
        return Err(
            "Spellguard requires a verified intrinsically defensive personal-range spell".into(),
        );
    }
    Ok(SpellRange::Touch)
}
#[derive(Debug, PartialEq, Serialize)]
pub struct FavorBenefit {
    pub temporary_hp: u32,
    pub duration_rounds: u32,
    pub recipients: u32,
}
/// Resolve one use, before overlap with previous uses. Never rewrites a spell profile.
pub fn deity_favor(
    actual_class_level: u32,
    selected: bool,
    spell_class: &str,
    spell_level: u32,
    affected_allies: u32,
    chosen_allies: u32,
    spell_temporary_hp: u32,
    spell_duration_rounds: u32,
) -> Result<FavorBenefit, String> {
    if !selected || actual_class_level < 3 || spell_class != "Favored Soul" {
        return Err(
            "Deity's Favor requires the replacement feature and a Favored Soul spell".into(),
        );
    }
    if chosen_allies == 0
        || chosen_allies > affected_allies
        || (actual_class_level < 12 && chosen_allies > 1)
    {
        return Err(
            "Choose affected allies only; multiple recipients require actual Favored Soul level 12"
                .into(),
        );
    }
    if spell_level > 9 {
        return Err("Spell level must be 0 through 9".into());
    }
    let hp = 3 * spell_level;
    if hp == spell_temporary_hp && hp != 0 && spell_duration_rounds != 10 * spell_level {
        return Err(
            "Equal temporary HP with different durations needs an explicit adjudication".into(),
        );
    }
    Ok(FavorBenefit {
        temporary_hp: hp.max(spell_temporary_hp),
        duration_rounds: if spell_temporary_hp > hp {
            spell_duration_rounds
        } else {
            10 * spell_level
        },
        recipients: chosen_allies,
    })
}
