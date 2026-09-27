//! `.pcg` draft reader/writer for the new architecture.
//!
//! This migrates the persisted inputs the engine needs.  It does not
//! reproduce every legacy `PlayerCharacter` field; unknown records are
//! ignored and can be added incrementally.

use pcgen_ir::draft::{PcgClassLevel, PcgDraft, PcgSpell};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Errors produced while reading a draft `.pcg`.
#[derive(Debug, thiserror::Error)]
pub enum PcgDraftError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("character has no name")]
    MissingName,
    #[error("malformed GESTALTJSON: {0}")]
    GestaltJson(#[from] serde_json::Error),
}

pub fn parse_pcg_draft_file(path: impl AsRef<Path>) -> Result<PcgDraft, PcgDraftError> {
    let text = fs::read_to_string(path)?;
    parse_pcg_draft_str(&text)
}

pub fn parse_pcg_draft_str(input: &str) -> Result<PcgDraft, PcgDraftError> {
    let mut draft = PcgDraft {
        name: String::new(),
        race_id: "Human".to_string(),
        campaign: "3e".to_string(),
        alignment: "N".to_string(),
        stats: BTreeMap::new(),
        class_levels: Vec::new(),
        skills: BTreeMap::new(),
        spells: Vec::new(),
        equipment: Vec::new(),
        equipment_modifiers: BTreeMap::new(),
        abilities: Vec::new(),
        templates: Vec::new(),
        advancement: Vec::new(),
        choices: BTreeMap::new(),
        class_subclasses: BTreeMap::new(),
    };
    let mut class_hp: BTreeMap<(String, u32), i32> = BTreeMap::new();

    for raw_line in input.lines() {
        let line = raw_line.trim_end_matches('\r').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim().to_ascii_uppercase().as_str() {
            "CHARACTERNAME" => draft.name = value.to_string(),
            "RACE" => draft.race_id = value.to_string(),
            "ALIGN" => draft.alignment = value.to_string(),
            "GAMEMODE" | "CAMPAIGN" => draft.campaign = value.to_string(),
            "STAT" => {
                let parts = split_pipe(value);
                if let (Some(stat), Some(score)) = (
                    parts.first().copied(),
                    parts
                        .iter()
                        .find_map(|part| part.strip_prefix("SCORE:"))
                        .and_then(|score| score.parse::<i32>().ok()),
                ) {
                    draft.stats.insert(stat.to_string(), score);
                }
            }
            "CLASS" => parse_class(&mut draft, value),
            "CLASSABILITIESLEVEL" => parse_class_abilities(&mut class_hp, value),
            "SKILL" => parse_skill(&mut draft, value),
            "SPELLNAME" => parse_spell(&mut draft, value),
            "EQUIPNAME" | "EQUIPMENT" => {
                let name = split_pipe(value).first().copied().unwrap_or_default();
                if !name.is_empty() {
                    draft.equipment.push(name.to_string());
                    let modifiers = extract_equipmods(value);
                    if !modifiers.is_empty() {
                        draft
                            .equipment_modifiers
                            .insert(name.to_ascii_lowercase(), modifiers);
                    }
                }
            }
            "ABILITY" => parse_ability(&mut draft, value),
            "CHOICE" => {
                let parts = split_pipe(value);
                if let (Some(key), Some(value)) = (parts.first(), parts.get(1)) {
                    if !key.is_empty() && !value.is_empty() {
                        draft
                            .choices
                            .insert((*key).to_string(), (*value).to_string());
                    }
                }
            }
            "SUBCLASS" => {
                let parts = split_pipe(value);
                if let (Some(class_id), Some(subclass)) = (parts.first(), parts.get(1)) {
                    if !class_id.is_empty() && !subclass.is_empty() {
                        draft
                            .class_subclasses
                            .insert((*class_id).to_string(), (*subclass).to_string());
                    }
                }
            }
            "TEMPLATESAPPLIED" => {
                for (name, applied) in extract_template_choices(value) {
                    if !draft
                        .templates
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(&name))
                    {
                        draft.templates.push(name.clone());
                    }
                    if let Some(applied) = applied {
                        if !applied.is_empty() {
                            draft.choices.insert(name, applied);
                        }
                    }
                }
            }
            "GESTALTJSON" => draft.advancement = serde_json::from_str(value)?,
            _ => {}
        }
    }

    for class in &mut draft.class_levels {
        let class_key = class.class_id.to_lowercase();
        let mut rolls: Vec<(u32, i32)> = class_hp
            .iter()
            .filter(|((class_id, _), _)| class_id == &class_key)
            .map(|((_, level), hp)| (*level, *hp))
            .collect();
        rolls.sort_by_key(|(level, _)| *level);
        if !rolls.is_empty() {
            class.hit_points_by_level = rolls.into_iter().map(|(_, hp)| hp).collect();
            class.hit_points_gained = class.hit_points_by_level.iter().sum();
        }
    }
    if draft.name.trim().is_empty() {
        return Err(PcgDraftError::MissingName);
    }
    Ok(draft)
}

/// Serialize a draft to `.pcg` text.
pub fn write_pcg_draft_string(draft: &PcgDraft) -> Result<String, PcgDraftError> {
    let mut out = String::new();
    use std::fmt::Write as _;
    writeln!(out, "CHARACTERNAME:{}", draft.name).ok();
    writeln!(out, "RACE:{}", draft.race_id).ok();
    writeln!(out, "ALIGN:{}", draft.alignment).ok();
    writeln!(out, "GAMEMODE:{}", draft.campaign).ok();
    for (stat, score) in &draft.stats {
        writeln!(out, "STAT:{stat}|SCORE:{score}").ok();
    }
    for class in &draft.class_levels {
        match draft.class_subclasses.get(&class.class_id) {
            Some(subclass) => {
                writeln!(
                    out,
                    "CLASS:{}|LEVEL:{}|SUBCLASS:{}",
                    class.class_id, class.level, subclass
                )
                .ok();
            }
            None => {
                writeln!(out, "CLASS:{}|LEVEL:{}", class.class_id, class.level).ok();
            }
        }
        writeln!(
            out,
            "CLASSABILITIESLEVEL:{}|HITPOINTS:{}",
            class.class_id, class.hit_points_gained
        )
        .ok();
    }
    for (skill, ranks) in &draft.skills {
        writeln!(out, "SKILL:{skill}|RANKS:{ranks}").ok();
    }
    for spell in &draft.spells {
        writeln!(
            out,
            "SPELLNAME:{}|SPELLLEVEL:{}|TIMES:{}",
            spell.name, spell.level, spell.times_memorized
        )
        .ok();
        if let Some(class_id) = &spell.class_id {
            writeln!(out, "SPELLNAME:{}|CLASS:{class_id}", spell.name).ok();
        }
    }
    for item in &draft.equipment {
        writeln!(out, "EQUIPNAME:{item}").ok();
    }
    for (category, key) in &draft.abilities {
        match draft.choices.get(key) {
            Some(applied) => {
                writeln!(
                    out,
                    "ABILITY:CATEGORY={category}|KEY:{key}|APPLIEDTO:{applied}"
                )
                .ok();
            }
            None => {
                writeln!(out, "ABILITY:CATEGORY={category}|KEY:{key}").ok();
            }
        }
    }
    for (key, value) in &draft.choices {
        writeln!(out, "CHOICE:{key}|{value}").ok();
    }
    for (class_id, subclass) in &draft.class_subclasses {
        writeln!(out, "SUBCLASS:{class_id}|{subclass}").ok();
    }
    if !draft.advancement.is_empty() {
        writeln!(
            out,
            "GESTALTJSON:{}",
            serde_json::to_string(&draft.advancement)?
        )
        .ok();
    }
    Ok(out)
}

pub fn save_pcg_draft_file(path: impl AsRef<Path>, draft: &PcgDraft) -> Result<(), PcgDraftError> {
    fs::write(path, write_pcg_draft_string(draft)?)?;
    Ok(())
}

fn extract_template_choices(value: &str) -> Vec<(String, Option<String>)> {
    let mut result = Vec::new();
    let mut rest = value;
    while let Some(name_start) = rest.find("NAME:") {
        let after = &rest[name_start + "NAME:".len()..];
        let name_end = after.find([']', '|', '[']).unwrap_or(after.len());
        let name = after[..name_end].trim();
        if name.is_empty() {
            rest = &after[name_end..];
            continue;
        }
        let segment_end = after.find('[').unwrap_or(after.len());
        let segment = &after[..segment_end];
        let applied = segment
            .find("APPLIEDTO:")
            .map(|position| {
                let after = &segment[position + "APPLIEDTO:".len()..];
                let end = after.find([']', '|', '[']).unwrap_or(after.len());
                after[..end].trim().to_string()
            })
            .filter(|value| !value.is_empty());
        result.push((name.to_string(), applied));
        rest = &after[name_end..];
    }
    result
}

fn parse_class(draft: &mut PcgDraft, value: &str) {
    let parts = split_pipe(value);
    let Some(class_id) = parts.first().copied() else {
        return;
    };
    if let Some(subclass) = parts.iter().find_map(|part| part.strip_prefix("SUBCLASS:")) {
        if !subclass.is_empty() {
            draft
                .class_subclasses
                .insert(class_id.to_string(), subclass.to_string());
        }
    }
    let level = parts
        .iter()
        .find_map(|part| part.strip_prefix("LEVEL:"))
        .and_then(|level| level.parse::<u32>().ok())
        .unwrap_or(1);
    if let Some(entry) = draft
        .class_levels
        .iter_mut()
        .find(|entry| entry.class_id.eq_ignore_ascii_case(class_id))
    {
        entry.level = entry.level.max(level);
    } else {
        draft.class_levels.push(PcgClassLevel {
            class_id: class_id.to_string(),
            level,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        });
    }
}

fn parse_class_abilities(class_hp: &mut BTreeMap<(String, u32), i32>, value: &str) {
    let parts = split_pipe(value);
    let Some(first) = parts.first().copied() else {
        return;
    };
    let (class_id, level) = first
        .split_once('=')
        .map(|(class_id, level)| {
            (
                class_id.to_lowercase(),
                level.trim().parse::<u32>().unwrap_or(1),
            )
        })
        .unwrap_or_else(|| (first.to_lowercase(), 1));
    if let Some(hit_points) = parts
        .iter()
        .find_map(|part| part.strip_prefix("HITPOINTS:"))
        .and_then(|hp| hp.parse::<i32>().ok())
    {
        *class_hp.entry((class_id, level)).or_insert(0) += hit_points;
    }
}

fn parse_skill(draft: &mut PcgDraft, value: &str) {
    let parts = split_pipe(value);
    let Some(name) = parts.first().copied() else {
        return;
    };
    if name.is_empty() {
        return;
    }
    let mut ranks = 0.0;
    for part in parts.iter().skip(1) {
        if let Some(class_bought) = part.strip_prefix("CLASSBOUGHT:") {
            for sub in class_bought.split('|') {
                if let Some(rank) = sub.strip_prefix("RANKS:") {
                    ranks += rank.parse::<f64>().unwrap_or(0.0);
                }
            }
        }
        if let Some(rank) = part.strip_prefix("RANKS:") {
            ranks += rank.parse::<f64>().unwrap_or(0.0);
        }
    }
    draft.skills.insert(name.to_string(), ranks);
}

fn parse_spell(draft: &mut PcgDraft, value: &str) {
    let parts = split_pipe(value);
    let Some(name) = parts.first().copied() else {
        return;
    };
    let level = parts
        .iter()
        .find_map(|part| part.strip_prefix("SPELLLEVEL:"))
        .and_then(|level| level.parse::<u32>().ok())
        .unwrap_or(0);
    let times = parts
        .iter()
        .find_map(|part| part.strip_prefix("TIMES:"))
        .and_then(|times| times.parse::<u32>().ok())
        .unwrap_or(0);
    let class_id = parts
        .iter()
        .find_map(|part| part.strip_prefix("CLASS:"))
        .map(str::to_string);
    if let Some(existing) = draft
        .spells
        .iter_mut()
        .find(|spell| spell.name.eq_ignore_ascii_case(name))
    {
        if existing.class_id.is_none() {
            existing.class_id = class_id;
        }
        existing.level = existing.level.max(level);
        existing.times_memorized = existing.times_memorized.max(times);
    } else {
        draft.spells.push(PcgSpell {
            name: name.to_string(),
            level,
            class_id,
            times_memorized: times,
        });
    }
}

fn parse_ability(draft: &mut PcgDraft, value: &str) {
    let parts = split_pipe(value);
    let mut category = None;
    let mut key = None;
    let mut applied = None;
    for part in parts {
        if let Some(value) = part.strip_prefix("CATEGORY:") {
            category = Some(value.to_string());
        } else if let Some(value) = part.strip_prefix("CATEGORY=") {
            category = Some(value.to_string());
        } else if let Some(value) = part.strip_prefix("KEY:") {
            key = Some(value.to_string());
        } else if let Some(value) = part.strip_prefix("APPLIEDTO:") {
            applied = Some(value.to_string());
        }
    }
    if let Some(key) = key {
        if let Some(applied) = applied {
            draft.choices.insert(key.clone(), applied);
        }
        draft
            .abilities
            .push((category.unwrap_or_else(|| "Ability".to_string()), key));
    } else if let Some(first) = split_pipe(value).first() {
        if !first.contains(':') {
            draft
                .abilities
                .push(("Ability".to_string(), (*first).to_string()));
        }
    }
}

fn split_pipe(value: &str) -> Vec<&str> {
    value.split('|').map(str::trim).collect()
}

/// Extract `EQMOD=` references from a `.pcg` `CUSTOMIZATION` payload.
fn extract_equipmods(value: &str) -> Vec<String> {
    let Some(start) = value.find("DATA:") else {
        return Vec::new();
    };
    value[start + "DATA:".len()..]
        .split('|')
        .flat_map(|part| part.split(';'))
        .filter_map(|part| part.trim().strip_prefix("EQMOD="))
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcgen_core::{AdvancementFrame, AdvancementLane};

    #[test]
    fn parses_core_pcg_inputs() {
        let pcg = "\
CHARACTERNAME:Arden
RACE:Human
ALIGN:NG
GAMEMODE:3e
STAT:STR|SCORE:14
CLASS:Fighter|LEVEL:2
CLASSABILITIESLEVEL:Fighter|HITPOINTS:16
SKILL:Climb|RANKS:4
SPELLNAME:Magic Missile|SPELLLEVEL:1|CLASS:Wizard|TIMES:2
EQUIPNAME:Chainmail
ABILITY:CATEGORY=Feat|KEY:Power Attack
GESTALTJSON:[{\"character_level\":1,\"lanes\":[{\"lane_id\":\"a\",\"class_id\":\"Fighter\",\"class_level\":1,\"hit_points_gained\":10,\"experience_gained\":0,\"advances\":[],\"archetype\":null,\"substitution_level\":null,\"deity\":null,\"domains\":[]}]}]
";
        let draft = parse_pcg_draft_str(pcg).unwrap();
        assert_eq!(draft.stats["STR"], 14);
        assert_eq!(draft.class_levels[0].level, 2);
        assert_eq!(draft.class_levels[0].hit_points_gained, 16);
        assert_eq!(draft.skills["Climb"], 4.0);
        assert_eq!(draft.spells[0].class_id.as_deref(), Some("Wizard"));
        assert_eq!(draft.equipment, vec!["Chainmail"]);
        assert_eq!(draft.abilities[0].1, "Power Attack");
        assert_eq!(draft.advancement.len(), 1);
        let text = write_pcg_draft_string(&draft).unwrap();
        assert_eq!(parse_pcg_draft_str(&text).unwrap(), draft);
    }

    #[test]
    fn gestalt_slot_history_round_trips() {
        let draft = PcgDraft {
            name: "Arden".to_string(),
            race_id: "Human".to_string(),
            campaign: "3e".to_string(),
            alignment: "N".to_string(),
            stats: BTreeMap::from([("INT".to_string(), 14)]),
            class_levels: Vec::new(),
            skills: BTreeMap::new(),
            spells: Vec::new(),
            equipment: Vec::new(),
            equipment_modifiers: BTreeMap::new(),
            abilities: Vec::new(),
            templates: Vec::new(),
            advancement: vec![AdvancementFrame::new(
                1,
                vec![
                    AdvancementLane::new("a", "Fighter", 1, 10, 0),
                    AdvancementLane::new("b", "Wizard", 1, 4, 0),
                ],
            )],
            ..PcgDraft::default()
        };
        let text = write_pcg_draft_string(&draft).unwrap();
        assert!(text.contains("GESTALTJSON:"));
        assert_eq!(parse_pcg_draft_str(&text).unwrap(), draft);
    }
    #[test]
    fn parses_applied_to_choices_and_class_subclass() {
        let text = "CHARACTERNAME:Choice Test\nCLASS:Psion|LEVEL:5|SUBCLASS:Telepath\nABILITY:FEAT|CATEGORY:FEAT|KEY:Expanded Knowledge|APPLIEDTO:Animal Affinity\n";
        let draft = parse_pcg_draft_str(text).unwrap();
        assert_eq!(
            draft.class_subclasses.get("Psion").map(String::as_str),
            Some("Telepath")
        );
        assert_eq!(
            draft.choices.get("Expanded Knowledge").map(String::as_str),
            Some("Animal Affinity")
        );
    }
}
