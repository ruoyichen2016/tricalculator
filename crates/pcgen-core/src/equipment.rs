//! Equipment slot order and conflict helpers.
//!
//! This is the small runtime-facing slice of PCGen's equipment model used by
//! the engine and output layers.  It deliberately keeps the slot vocabulary
//! string-friendly so it can be inferred from PCGen `TYPE:` tags and refined
//! as more data is imported.

use serde::{Deserialize, Serialize};

/// Standard equipment slot vocabulary.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot {
    Head,
    Body,
    Arms,
    Hands,
    Feet,
    Waist,
    Neck,
    Wrist,
    Weapon,
    OffHand,
    Shield,
    Special,
    #[default]
    None,
}

impl EquipSlot {
    /// PCGen output-sheet ordering inherited from the archive implementation.
    pub fn order(self) -> u8 {
        match self {
            Self::Head => 0,
            Self::Neck => 1,
            Self::Body => 2,
            Self::Arms => 3,
            Self::Wrist => 4,
            Self::Hands => 5,
            Self::Waist => 6,
            Self::Feet => 7,
            Self::Weapon => 8,
            Self::OffHand => 9,
            Self::Shield => 10,
            Self::Special => 11,
            Self::None => 12,
        }
    }

    pub fn from_type_tags(type_tags: &[String]) -> Self {
        let tags: Vec<String> = type_tags
            .iter()
            .map(|tag| tag.to_ascii_lowercase())
            .collect();
        let has = |needle: &str| tags.iter().any(|tag| tag.contains(needle));
        if has("shield") {
            Self::Shield
        } else if has("weapon") || has("melee") || has("ranged") {
            Self::Weapon
        } else if has("armor") {
            Self::Body
        } else if has("head") || has("helm") {
            Self::Head
        } else if has("neck") || has("amulet") {
            Self::Neck
        } else if has("wrist") || has("bracer") {
            Self::Wrist
        } else if has("hands") || has("glove") {
            Self::Hands
        } else if has("waist") || has("belt") {
            Self::Waist
        } else if has("feet") || has("boot") {
            Self::Feet
        } else if has("ring") {
            Self::Special
        } else {
            Self::None
        }
    }
}

/// Sort equipment by standard slot order.  Stable for equal slots.
pub fn sort_equipment_by_slot<T, F>(items: &mut [T], mut slot_of: F)
where
    F: FnMut(&T) -> EquipSlot,
{
    items.sort_by_key(|item| slot_of(item).order());
}

/// Report duplicate occupied slots as `(slot, first_id, second_id)`.
pub fn equipment_conflicts<'a, I>(items: I) -> Vec<(EquipSlot, String, String)>
where
    I: IntoIterator<Item = (EquipSlot, &'a str)>,
{
    let mut seen: Vec<(EquipSlot, String)> = Vec::new();
    let mut conflicts = Vec::new();
    for (slot, id) in items {
        if slot == EquipSlot::None {
            continue;
        }
        if let Some((_, existing)) = seen.iter().find(|(existing, _)| *existing == slot) {
            conflicts.push((slot, existing.clone(), id.to_string()));
        } else {
            seen.push((slot, id.to_string()));
        }
    }
    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equipment_slot_sort_uses_standard_order() {
        let mut items = vec![
            ("sword", EquipSlot::Weapon),
            ("helm", EquipSlot::Head),
            ("shield", EquipSlot::Shield),
        ];
        sort_equipment_by_slot(&mut items, |(_, slot)| *slot);
        assert_eq!(
            items.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            vec!["helm", "sword", "shield"]
        );
    }

    #[test]
    fn equipment_conflicts_report_duplicate_slots() {
        let conflicts = equipment_conflicts([
            (EquipSlot::Weapon, "sword"),
            (EquipSlot::Head, "helm"),
            (EquipSlot::Weapon, "axe"),
        ]);
        assert_eq!(
            conflicts,
            vec![(EquipSlot::Weapon, "sword".to_string(), "axe".to_string())]
        );
    }
}
