//! Conversion between PCG records and the canonical character model.
use crate::{PcgClassLevel, PcgDraft};
use pcgen_core::{CharacterDraft, DraftSpell};

/// Convert flat PCG records at the import boundary. PCG records without a
/// chronological chain use declared class order; aggregate HP is distributed
/// evenly when individual rolls are absent. Native saves never use this path.
pub fn advancement_from_class_levels(
    classes: &[PcgClassLevel],
) -> Vec<pcgen_core::AdvancementFrame> {
    let mut frames = Vec::new();
    for class in classes {
        for level in 1..=class.level {
            let hp = class
                .hit_points_by_level
                .get(level as usize - 1)
                .copied()
                .unwrap_or_else(|| {
                    let count = class.level as i32;
                    class.hit_points_gained / count
                        + i32::from((level as i32) <= class.hit_points_gained % count)
                });
            let mut frame = pcgen_core::AdvancementFrame::single(
                frames.len() as u32 + 1,
                class.class_id.clone(),
                level,
                hp,
                if level == 1 {
                    class.experience_gained
                } else {
                    0
                },
            );
            if class.hit_points_by_level.is_empty() && class.hit_points_gained == 0 {
                frame.lanes[0].hit_points_gained = None;
            }
            frames.push(frame);
        }
    }
    frames
}

pub fn draft_from_pcg(draft: PcgDraft) -> CharacterDraft {
    let advancement = if draft.advancement.is_empty() {
        advancement_from_class_levels(&draft.class_levels)
    } else {
        draft.advancement.clone()
    };
    CharacterDraft {
        build_history: None,
        preparations: Vec::new(),
        martial_training: Default::default(),
        feat_acquired_at: Default::default(),
        domain_level_rulings: Default::default(),
        class_play: Default::default(),
        id: format!("pcg:{}", draft.name),
        name: draft.name,
        race_id: draft.race_id,
        campaign: draft.campaign,
        alignment: draft.alignment,
        gender: draft.gender,
        character_type: if draft.character_type.trim().is_empty() {
            pcgen_core::default_character_type()
        } else {
            draft.character_type
        },
        stats: draft.stats,
        advancement,
        skills: draft.skills,
        spells: draft
            .spells
            .into_iter()
            .map(|spell| DraftSpell {
                name: spell.name,
                level: spell.level,
                class_id: spell.class_id,
                times_memorized: spell.times_memorized,
                book: spell.book,
            })
            .collect(),
        equipment: draft.equipment,
        equipment_modifiers: draft.equipment_modifiers,
        equipment_base_items: draft.equipment_base_items,
        equipment_display_names: draft.equipment_display_names,
        equip_slots: draft.equip_slots,
        abilities: draft.abilities,
        templates: draft.templates,
        active_states: Vec::new(),
        choices: draft.choices,
        companions: Vec::new(),
        class_subclasses: draft.class_subclasses,
        deity: draft.deity,
        domains: draft.domains,
        deity_domains: draft.deity_domains,
    }
}

pub fn pcg_from_draft(draft: &CharacterDraft) -> PcgDraft {
    PcgDraft {
        name: draft.name.clone(),
        race_id: draft.race_id.clone(),
        campaign: draft.campaign.clone(),
        alignment: draft.alignment.clone(),
        gender: draft.gender.clone(),
        character_type: if draft.character_type.trim().is_empty() {
            pcgen_core::default_character_type()
        } else {
            draft.character_type.clone()
        },
        stats: draft.stats.clone(),
        class_levels: draft
            .class_levels()
            .iter()
            .map(|entry| crate::PcgClassLevel {
                class_id: entry.class_id.clone(),
                level: entry.level,
                hit_points_gained: entry.hit_points_by_level.iter().flatten().sum(),
                hit_points_by_level: entry
                    .hit_points_by_level
                    .iter()
                    .map(|roll| roll.unwrap_or(0))
                    .collect(),
                experience_gained: entry.experience_gained,
            })
            .collect(),
        skills: draft.skills.clone(),
        spells: draft
            .spells
            .iter()
            .map(|spell| crate::PcgSpell {
                name: spell.name.clone(),
                level: spell.level,
                class_id: spell.class_id.clone(),
                times_memorized: spell.times_memorized,
                book: spell.book.clone(),
            })
            .collect(),
        equipment: draft.equipment.clone(),
        equipment_modifiers: draft.equipment_modifiers.clone(),
        equipment_base_items: draft.equipment_base_items.clone(),
        equipment_display_names: draft.equipment_display_names.clone(),
        equip_slots: draft.equip_slots.clone(),
        abilities: draft.abilities.clone(),
        templates: draft.templates.clone(),
        advancement: draft.advancement.clone(),
        choices: draft.choices.clone(),
        class_subclasses: draft.class_subclasses.clone(),
        deity: draft.deity.clone(),
        domains: draft.domains.clone(),
        deity_domains: draft.deity_domains.clone(),
    }
}
