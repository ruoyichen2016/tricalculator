//! Plain server-rendered rows for native inputs. No JSON editing or background lookup.
use super::ui::escape;
use super::*;
use std::{collections::BTreeMap, fmt::Write};
fn input(out: &mut String, label: &str, name: &str, value: &str, kind: &str) {
    let _ = write!(
        out,
        "<label>{}<input name=\"{}\" value=\"{}\" type=\"{kind}\" step=\"any\"></label>",
        escape(label),
        escape(name),
        escape(value)
    );
}
fn hidden(out: &mut String, name: &str, value: usize) {
    let _ = write!(
        out,
        "<input type=\"hidden\" name=\"{name}\" value=\"{value}\">"
    );
}
fn remove(out: &mut String, name: &str) {
    let _ = write!(
        out,
        "<label class=\"check\"><input type=\"checkbox\" name=\"{name}\">Remove</label>"
    );
}
pub(super) fn render(out: &mut String, d: &CharacterDraft, engine: Option<&Engine>) {
    out.push_str("<details><summary>Level history</summary>");
    for (i, f) in d.advancement.iter().enumerate() {
        for (j, l) in f.lanes.iter().enumerate() {
            let _ = write!(
                out,
                "<fieldset><legend>Character {} · {} {}</legend>",
                f.character_level,
                escape(&l.class_id),
                l.class_level
            );
            input(
                out,
                "Hit die roll",
                &format!("hp_{i}_{j}"),
                &l.hit_points_gained
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                "number",
            );
            input(
                out,
                "Casting class advanced (comma separated if more than one)",
                &format!("advance_{i}_{j}"),
                &l.advances.join(", "),
                "text",
            );
            out.push_str("</fieldset>");
        }
    }
    if !d.advancement.is_empty() {
        out.push_str("<label class=\"check\"><input name=\"remove_last_level\" type=\"checkbox\">Remove the last character level</label>");
    }
    out.push_str("</details><details open><summary>Feats and acquisition levels</summary><p class=\"muted\">Acquisition levels establish when a feat was already owned. Automatic class grants do not spend general feat slots.</p>");
    let feats: Vec<_> = d
        .abilities
        .iter()
        .filter(|(pool, _)| !pool.contains("_sudden_"))
        .collect();
    hidden(out, "feat_count", feats.len());
    for (i, (pool, name)) in feats.iter().enumerate() {
        out.push_str("<fieldset>");
        input(out, "Feat", &format!("feat_{i}"), name, "text");
        input(out, "Pool", &format!("pool_{i}"), pool, "text");
        input(
            out,
            "Character level acquired",
            &format!("acquired_{i}"),
            &d.feat_acquired_at
                .get(name.as_str())
                .map(u32::to_string)
                .unwrap_or_default(),
            "number",
        );
        remove(out, &format!("remove_feat_{i}"));
        out.push_str("</fieldset>");
    }
    out.push_str("<fieldset><legend>Add a feat</legend><label>Feat<input name=\"new_feat\" list=\"feat_catalog\"></label><label>Pool<select name=\"new_pool\"><option value=\"FEAT\">General feat</option>");
    if let Some(e) = engine {
        let pools = e.evaluate(d).ability_pools;
        for (id, c) in &e.pack.ability_categories {
            if id != "feat"
                && !id.contains("_sudden_")
                && (c.category.as_deref() == Some("FEAT") || c.category.as_deref() == Some("Flaw"))
                && pools.get(id).is_some_and(|n| *n > 0)
            {
                let _ = write!(
                    out,
                    "<option value=\"{}\">{}</option>",
                    escape(id),
                    escape(&c.name)
                );
            }
        }
    }
    out.push_str("</select></label>");
    input(
        out,
        "Character level acquired",
        "new_acquired",
        "",
        "number",
    );
    out.push_str("</fieldset><datalist id=\"feat_catalog\">");
    if let Some(e) = engine {
        for a in e
            .pack
            .abilities
            .values()
            .filter(|a| matches!(a.category.as_deref(), Some("FEAT" | "Flaw")))
        {
            let _ = write!(out, "<option value=\"{}\">", escape(&a.name));
        }
    }
    out.push_str("</datalist></details><details><summary>Class bonus feat replacements</summary>");
    if let Some(e) = engine {
        let slots = match e.sudden_replacements(d) {
            Ok(slots) => slots,
            Err(error) => {
                let _ = write!(out, "<p class=\"error\">{}</p>", escape(&error));
                e.pack
                    .entities
                    .iter()
                    .flat_map(|entity| {
                        entity.metadata.class_magic.iter().flat_map(move |m| {
                            m.sudden_grants
                                .iter()
                                .filter(|g| d.level_in_class(&entity.name) >= g.level)
                                .map(move |g| {
                                    let pool = format!(
                                        "{}_sudden_{}",
                                        entity.name.to_lowercase().replace(' ', "_"),
                                        g.level
                                    );
                                    pcgen_engine::class_features::SuddenReplacement {
                                        selected: d
                                            .abilities
                                            .iter()
                                            .find(|(p, _)| p == &pool)
                                            .map(|(_, n)| n.clone()),
                                        pool,
                                        class_id: entity.name.clone(),
                                        class_level: g.level,
                                        character_level: 0,
                                        granted_feat: g.feat.clone(),
                                        available: false,
                                    }
                                })
                        })
                    })
                    .collect()
            }
        };
        for slot in slots {
            let _ = write!(
                out,
                "<label>{} {}: {}<select name=\"replacement_{}\"><option value=\"\">{}</option>",
                escape(&slot.class_id),
                slot.class_level,
                escape(&slot.granted_feat),
                escape(&slot.pool),
                if slot.available {
                    "Choose a replacement"
                } else {
                    "Automatic grant; establish prior ownership for a replacement"
                }
            );
            for a in e.pack.abilities.values().filter(|a| {
                a.type_tags
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case("Metamagic"))
                    && a.name != slot.granted_feat
            }) {
                let _ = write!(
                    out,
                    "<option value=\"{}\" {}>{}</option>",
                    escape(&a.name),
                    if slot.selected.as_ref() == Some(&a.name) {
                        "selected"
                    } else {
                        ""
                    },
                    escape(&a.name)
                );
            }
            out.push_str("</select></label>");
        }
    }
    out.push_str("</details><details><summary>Skill ranks</summary>");
    hidden(out, "skill_count", d.skills.len());
    for (i, (name, rank)) in d.skills.iter().enumerate() {
        let _ = write!(
            out,
            "<input type=\"hidden\" name=\"skill_{i}\" value=\"{}\">",
            escape(name)
        );
        input(out, name, &format!("rank_{i}"), &rank.to_string(), "number");
        remove(out, &format!("remove_skill_{i}"));
    }
    input(out, "New skill", "new_skill", "", "text");
    input(out, "Ranks", "new_rank", "0", "number");
    out.push_str("</details><details><summary>Class and feat choices</summary>");
    if d.level_in_class("Sacred Exorcist") >= 3 {
        out.push_str(
            "<label>Chosen foe<select name=\"chosen_foe\"><option value=\"\">Choose a foe</option>",
        );
        for (value, name) in [("undead", "Undead"), ("evil_outsider", "Evil outsiders")] {
            let selected = d
                .choices
                .get("Sacred Exorcist.ChosenFoe")
                .is_some_and(|v| v == value);
            let _ = write!(
                out,
                "<option value=\"{value}\" {}>{name}</option>",
                if selected { "selected" } else { "" }
            );
        }
        out.push_str("</select></label>");
    }
    let mut choices: BTreeMap<String, String> = d
        .choices
        .iter()
        .filter(|(k, _)| {
            !k.contains(".AdvancedLearning.") && k.as_str() != "Sacred Exorcist.ChosenFoe"
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut rules = BTreeMap::new();
    if let Some(e) = engine {
        for entity in &e.pack.entities {
            if let Some(m) = &entity.metadata.class_magic {
                for rule in m
                    .choices
                    .iter()
                    .filter(|r| d.level_in_class(&entity.name) >= r.at_level)
                {
                    choices.entry(rule.key.clone()).or_default();
                    rules.insert(rule.key.clone(), rule);
                }
            }
        }
    }
    hidden(out, "choice_count", choices.len());
    for (i, (key, value)) in choices.iter().enumerate() {
        let _ = write!(
            out,
            "<input type=hidden name=\"choice_key_{i}\" value=\"{}\">",
            escape(key)
        );
        if let Some(rule) = rules.get(key) {
            let _=write!(out,"<label>{}<select name=\"choice_value_{i}\"><option value=\"\">Choose an option</option>",escape(&rule.label));
            for option in &rule.options {
                let _ = write!(
                    out,
                    "<option value=\"{}\" {}>{} · {}</option>",
                    escape(&option.value),
                    if value == &option.value {
                        "selected"
                    } else {
                        ""
                    },
                    escape(&option.value),
                    escape(&option.alignments.join(", "))
                );
            }
            out.push_str("</select></label>");
        } else {
            input(out, key, &format!("choice_value_{i}"), value, "text");
        }
    }
    input(out, "New choice name", "new_choice_key", "", "text");
    input(out, "Selection", "new_choice_value", "", "text");
    out.push_str("</details><details><summary>Selected spells</summary>");
    hidden(out, "spell_count", d.spells.len());
    for (i, s) in d.spells.iter().enumerate() {
        let _ = write!(
            out,
            "<fieldset><legend>{} · {}</legend>",
            escape(&s.name),
            escape(s.class_id.as_deref().unwrap_or("Unassigned"))
        );
        input(
            out,
            "Prepared copies",
            &format!("spell_copies_{i}"),
            &s.times_memorized.to_string(),
            "number",
        );
        remove(out, &format!("remove_spell_{i}"));
        out.push_str("</fieldset>");
    }
    input(out, "New spell", "new_spell", "", "text");
    let default_track = d
        .class_levels()
        .into_iter()
        .find(|c| {
            engine.is_some_and(|e| {
                e.pack
                    .classes
                    .get(&c.class_id)
                    .is_some_and(|c| c.spellcasting_ability.is_some())
            })
        })
        .map(|c| c.class_id)
        .unwrap_or_default();
    input(
        out,
        "Casting class",
        "new_spell_class",
        &default_track,
        "text",
    );
    out.push_str("</details><details><summary>Equipment and active conditions</summary>");
    for (prefix, label, items) in [
        ("equipment", "Equipment", &d.equipment),
        ("condition", "Active condition", &d.active_states),
    ] {
        hidden(out, &format!("{prefix}_count"), items.len());
        for (i, name) in items.iter().enumerate() {
            input(out, label, &format!("{prefix}_{i}"), name, "text");
            remove(out, &format!("remove_{prefix}_{i}"));
        }
        input(
            out,
            &format!("Add {label}"),
            &format!("new_{prefix}"),
            "",
            "text",
        );
    }
    out.push_str("</details><details><summary>Air-domain effective level</summary><p>Cleric levels apply normally. For a noncleric, RAW supplies no effective-level conversion; record the table ruling explicitly.</p>");
    input(
        out,
        "Effective Cleric level (blank: use RAW Cleric levels)",
        "air_level",
        &d.domain_level_rulings
            .get("Air")
            .map(|r| r.level.to_string())
            .unwrap_or_default(),
        "number",
    );
    input(
        out,
        "Reason for this table ruling",
        "air_reason",
        d.domain_level_rulings
            .get("Air")
            .map(|r| r.reason.as_str())
            .unwrap_or(""),
        "text",
    );
    out.push_str("</details>");
}
fn get<'a>(f: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    f.get(key)
        .map(|s| s.trim())
        .ok_or_else(|| format!("Missing form field {key}"))
}
fn count(f: &BTreeMap<String, String>, key: &str) -> Result<usize, String> {
    let n = get(f, key)?
        .parse::<usize>()
        .map_err(|_| format!("Invalid {key}"))?;
    if n > 5000 {
        Err("Too many form rows".into())
    } else {
        Ok(n)
    }
}
pub(super) fn apply(
    f: &BTreeMap<String, String>,
    d: &mut CharacterDraft,
    e: &Engine,
) -> Result<(), String> {
    for (i, frame) in d.advancement.iter_mut().enumerate() {
        for (j, l) in frame.lanes.iter_mut().enumerate() {
            let hp = get(f, &format!("hp_{i}_{j}"))?;
            let roll = hp
                .parse::<i32>()
                .map_err(|_| "Hit die roll must be an integer")?;
            let max = e
                .pack
                .classes
                .get(&l.class_id)
                .ok_or("Unknown class")?
                .hit_die
                .trim_start_matches('d')
                .parse::<i32>()
                .map_err(|_| "Hit die unavailable")?;
            if !(1..=max).contains(&roll) {
                return Err("Hit die roll is outside the class die range".into());
            }
            l.hit_points_gained = Some(roll);
            l.advances = get(f, &format!("advance_{i}_{j}"))?
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .collect();
        }
    }
    if f.contains_key("remove_last_level") {
        d.advancement.pop();
    }
    d.abilities.clear();
    d.feat_acquired_at.clear();
    for i in 0..count(f, "feat_count")? {
        if f.contains_key(&format!("remove_feat_{i}")) {
            continue;
        }
        let name = get(f, &format!("feat_{i}"))?;
        let pool = get(f, &format!("pool_{i}"))?;
        d.abilities.push((pool.into(), name.into()));
        let at = get(f, &format!("acquired_{i}"))?;
        if !at.is_empty() {
            let n = at
                .parse::<u32>()
                .map_err(|_| "Invalid feat acquisition level")?;
            if n == 0 || n > d.total_level() {
                return Err("Feat acquisition must be an existing character level".into());
            }
            d.feat_acquired_at.insert(name.into(), n);
        }
    }
    let new = get(f, "new_feat")?;
    if !new.is_empty() {
        if !e
            .pack
            .abilities
            .values()
            .any(|a| a.name.eq_ignore_ascii_case(new))
        {
            return Err("Unknown feat".into());
        }
        let at = get(f, "new_acquired")?
            .parse::<u32>()
            .map_err(|_| "Enter the feat's acquisition level")?;
        if at == 0 || at > d.total_level() {
            return Err("Feat acquisition level is outside the current history".into());
        }
        d.abilities.push((get(f, "new_pool")?.into(), new.into()));
        d.feat_acquired_at.insert(new.into(), at);
    }
    for (key, value) in f {
        if let Some(pool) = key.strip_prefix("replacement_") {
            if !value.trim().is_empty() {
                d.abilities.push((pool.into(), value.trim().into()));
            }
        }
    }
    d.skills.clear();
    for i in 0..count(f, "skill_count")? {
        if f.contains_key(&format!("remove_skill_{i}")) {
            continue;
        }
        let rank = get(f, &format!("rank_{i}"))?
            .parse::<f64>()
            .map_err(|_| "Invalid skill rank")?;
        if !rank.is_finite() || rank < 0.0 {
            return Err("Skill ranks must be finite and nonnegative".into());
        }
        d.skills.insert(get(f, &format!("skill_{i}"))?.into(), rank);
    }
    let skill = get(f, "new_skill")?;
    if !skill.is_empty() {
        let rank = get(f, "new_rank")?
            .parse::<f64>()
            .map_err(|_| "Invalid new skill rank")?;
        if !rank.is_finite() || rank < 0.0 {
            return Err("Invalid skill rank".into());
        }
        d.skills.insert(skill.into(), rank);
    }
    for i in 0..count(f, "choice_count")? {
        let key = get(f, &format!("choice_key_{i}"))?;
        let val = get(f, &format!("choice_value_{i}"))?;
        if val.is_empty() {
            d.choices.remove(key);
        } else {
            d.choices.insert(key.into(), val.into());
        }
    }
    let key = get(f, "new_choice_key")?;
    if !key.is_empty() {
        d.choices
            .insert(key.into(), get(f, "new_choice_value")?.into());
    }
    if let Some(foe) = f.get("chosen_foe") {
        if !["undead", "evil_outsider"].contains(&foe.as_str()) {
            return Err("Select a Chosen Foe".into());
        }
        d.choices
            .insert("Sacred Exorcist.ChosenFoe".into(), foe.clone());
    }
    let n = count(f, "spell_count")?;
    if n != d.spells.len() {
        return Err("Spell list changed; reload the editor".into());
    }
    let mut spells = vec![];
    for (i, s) in d.spells.iter().enumerate() {
        if f.contains_key(&format!("remove_spell_{i}")) {
            continue;
        }
        let mut s = s.clone();
        s.times_memorized = get(f, &format!("spell_copies_{i}"))?
            .parse()
            .map_err(|_| "Invalid prepared copies")?;
        spells.push(s);
    }
    d.spells = spells;
    let name = get(f, "new_spell")?;
    if !name.is_empty() {
        let track = get(f, "new_spell_class")?;
        let access = e
            .spell_catalog(d, track)?
            .into_iter()
            .find(|s| s.spell.eq_ignore_ascii_case(name))
            .ok_or("Spell is not on the selected casting list")?;
        d.spells.push(pcgen_core::character::DraftSpell {
            name: access.spell,
            level: access.level,
            class_id: Some(track.into()),
            times_memorized: 0,
            book: Some("Known Spells".into()),
        });
    }
    for prefix in ["equipment", "condition"] {
        let mut items = vec![];
        for i in 0..count(f, &format!("{prefix}_count"))? {
            if !f.contains_key(&format!("remove_{prefix}_{i}")) {
                let item = get(f, &format!("{prefix}_{i}"))?;
                if !item.is_empty() {
                    items.push(item.into());
                }
            }
        }
        let new = get(f, &format!("new_{prefix}"))?;
        if !new.is_empty() {
            items.push(new.into());
        }
        if prefix == "equipment" {
            d.equipment = items
        } else {
            d.active_states = items
        }
    }
    let level = get(f, "air_level")?;
    if level.is_empty() {
        d.domain_level_rulings.remove("Air");
    } else {
        let level = level.parse::<u32>().map_err(|_| "Invalid Air level")?;
        let reason = get(f, "air_reason")?;
        if level > 10000 || reason.is_empty() {
            return Err("Air ruling requires a reason and a level from 0 to 10000".into());
        }
        d.domain_level_rulings.insert(
            "Air".into(),
            pcgen_core::class_play::DomainLevelRuling {
                level,
                reason: reason.into(),
            },
        );
    }
    Ok(())
}
