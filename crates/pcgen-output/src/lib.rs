//! Snapshot renderers for the new architecture.

use pcgen_engine::CharacterSnapshot;
use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Markdown,
    Csv,
    Json,
    Html,
}

impl OutputFormat {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "text" | "txt" | "plain" => Some(Self::Text),
            "markdown" | "md" => Some(Self::Markdown),
            "csv" => Some(Self::Csv),
            "json" => Some(Self::Json),
            "html" | "htm" => Some(Self::Html),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Text => "txt",
            Self::Markdown => "md",
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Html => "html",
        }
    }
}

pub fn render(
    snapshot: &CharacterSnapshot,
    format: OutputFormat,
) -> Result<String, serde_json::Error> {
    Ok(match format {
        OutputFormat::Text => render_text(snapshot),
        OutputFormat::Markdown => render_markdown(snapshot),
        OutputFormat::Csv => render_csv(snapshot),
        OutputFormat::Json => render_json(snapshot)?,
        OutputFormat::Html => render_html(snapshot),
    })
}

pub fn render_text(snapshot: &CharacterSnapshot) -> String {
    let mut out = String::new();
    writeln!(out, "{}", snapshot.name).ok();
    writeln!(out, "{}", "=".repeat(snapshot.name.chars().count().max(8))).ok();
    writeln!(out, "Race:       {}", snapshot.race_id).ok();
    writeln!(out, "Level:      {}", snapshot.total_level).ok();
    writeln!(out, "HP:         {}", snapshot.hit_points).ok();
    writeln!(out, "BAB:        {:+}", snapshot.base_attack_bonus).ok();
    for (save, value) in &snapshot.saves {
        writeln!(out, "{save:<11} {value:+}").ok();
    }
    writeln!(out, "Initiative: {:+}", snapshot.initiative).ok();
    writeln!(out, "AC:         {}", snapshot.armor_class).ok();
    writeln!(
        out,
        "  armor {} + shield {} + touch {} + flat {}",
        snapshot.armor_bonus, snapshot.shield_bonus, snapshot.touch_ac, snapshot.flat_footed_ac
    )
    .ok();
    if !snapshot.skills.is_empty() {
        writeln!(out, "\nSkills").ok();
        for (skill, value) in &snapshot.skills {
            writeln!(out, "  {skill:<24} {value:+}").ok();
        }
    }
    if !snapshot.caster_levels.is_empty() || !snapshot.spell_dcs.is_empty() {
        writeln!(out, "\nSpellcasting").ok();
        for (class_id, level) in &snapshot.caster_levels {
            writeln!(out, "  {class_id:<24} CL {level}").ok();
        }
        for (spell, dc) in &snapshot.spell_dcs {
            writeln!(out, "  {spell:<32} DC {dc}").ok();
        }
        for (prepared, count) in &snapshot.prepared_spells {
            writeln!(out, "  {prepared:<32} x{count} prepared").ok();
        }
    }
    if !snapshot.power_points.is_empty() {
        writeln!(out, "\nPsionics").ok();
        for (class_id, points) in &snapshot.power_points {
            writeln!(out, "  {class_id:<24} PP {points}").ok();
        }
    }
    if let Some(plan) = &snapshot.gestalt {
        writeln!(out, "\nGestalt Lanes").ok();
        writeln!(out, "-------------").ok();
        for frame in &plan.frames {
            writeln!(
                out,
                "  Level {:<2} HP {:>3} BAB {:+} slots {} lanes [{}] variants [{}]",
                frame.character_level,
                frame.hit_points,
                frame.base_attack_bonus_increment,
                frame.slot_cost,
                frame.lane_ids.join(", "),
                frame.variants.join(", ")
            )
            .ok();
        }
        for track in plan.divine_tracks.values() {
            writeln!(
                out,
                "  Divine lane {} -> {} deity={} domains=[{}]",
                track.lane_id,
                track.class_id,
                track.deity.as_deref().unwrap_or("(none)"),
                track.domains.join(", ")
            )
            .ok();
        }
        for diagnostic in &plan.diagnostics {
            writeln!(out, "  ! {}", diagnostic.message).ok();
        }
    }
    for diagnostic in &snapshot.diagnostics {
        writeln!(out, "  ! {diagnostic}").ok();
    }
    if !snapshot.provenance.rules_fingerprint.is_empty() {
        writeln!(out, "\nProvenance").ok();
        writeln!(out, "  {}", snapshot.provenance.render()).ok();
    }
    out
}

pub fn render_markdown(snapshot: &CharacterSnapshot) -> String {
    let mut out = String::new();
    writeln!(out, "# {}\n", snapshot.name).ok();
    writeln!(out, "**Race:** {}  ", snapshot.race_id).ok();
    writeln!(out, "**Level:** {}  ", snapshot.total_level).ok();
    writeln!(out, "**HP:** {}  ", snapshot.hit_points).ok();
    writeln!(out, "**BAB:** {:+}\n", snapshot.base_attack_bonus).ok();
    for (save, value) in &snapshot.saves {
        writeln!(out, "- **{save}:** {value:+}").ok();
    }
    writeln!(out, "- **Initiative:** {:+}", snapshot.initiative).ok();
    writeln!(
        out,
        "- **AC:** {} (touch {}, flat {})",
        snapshot.armor_class, snapshot.touch_ac, snapshot.flat_footed_ac
    )
    .ok();
    if !snapshot.skills.is_empty() {
        writeln!(out, "\n## Skills\n").ok();
        for (skill, value) in &snapshot.skills {
            writeln!(out, "- {skill}: {value:+}").ok();
        }
    }
    if !snapshot.caster_levels.is_empty() {
        writeln!(out, "\n## Spellcasting\n").ok();
        for (class_id, level) in &snapshot.caster_levels {
            writeln!(out, "- **{class_id}** caster level: {level}").ok();
        }
        for (spell, dc) in &snapshot.spell_dcs {
            writeln!(out, "- {spell}: DC {dc}").ok();
        }
    }
    if !snapshot.power_points.is_empty() {
        writeln!(out, "\n## Psionics\n").ok();
        for (class_id, points) in &snapshot.power_points {
            writeln!(out, "- **{class_id}**: {points} PP").ok();
        }
    }
    out
}

pub fn render_csv(snapshot: &CharacterSnapshot) -> String {
    let mut out = String::from("section,key,value\n");
    let mut row = |section: &str, key: &str, value: &str| {
        writeln!(out, "{section},{key},{value}").ok();
    };
    row("identity", "name", &snapshot.name);
    row("identity", "race", &snapshot.race_id);
    row("identity", "level", &snapshot.total_level.to_string());
    row("combat", "hp", &snapshot.hit_points.to_string());
    row("combat", "bab", &snapshot.base_attack_bonus.to_string());
    row("combat", "ac", &snapshot.armor_class.to_string());
    row("combat", "touch_ac", &snapshot.touch_ac.to_string());
    row(
        "combat",
        "flat_footed_ac",
        &snapshot.flat_footed_ac.to_string(),
    );
    for (stat, value) in &snapshot.stats {
        row("ability", stat, &value.to_string());
    }
    for (save, value) in &snapshot.saves {
        row("save", save, &value.to_string());
    }
    for (skill, value) in &snapshot.skills {
        row("skill", skill, &value.to_string());
    }
    for (class_id, level) in &snapshot.caster_levels {
        row("caster_level", class_id, &level.to_string());
    }
    for (spell, dc) in &snapshot.spell_dcs {
        row("spell_dc", spell, &dc.to_string());
    }
    for (class_id, points) in &snapshot.power_points {
        row("power_points", class_id, &points.to_string());
    }
    out
}

pub fn render_json(snapshot: &CharacterSnapshot) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(snapshot)
}

pub fn render_html(snapshot: &CharacterSnapshot) -> String {
    let mut out = String::new();
    writeln!(
        out,
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title>",
        escape_html(&snapshot.name)
    )
    .ok();
    out.push_str(
        "<style>\
         body{font-family:system-ui,sans-serif;margin:2rem;color:#222;line-height:1.35}\
         h1{margin:0 0 .25rem 0}h2{margin:1.25rem 0 .35rem 0;border-bottom:1px solid #ddd}\
         .subtitle{color:#555;margin:0 0 1rem 0}\
         table{border-collapse:collapse;width:100%;margin:.2rem 0 1rem 0}\
         th,td{border:1px solid #ccc;padding:.2rem .5rem;text-align:left;vertical-align:top}\
         th{background:#f5f5f5}\
         ul{margin:.2rem 0 1rem 1.25rem;padding:0}\
         .grid-skills{column-count:2;column-gap:2rem}\
         .diagnostics{color:#a00}\
         footer{margin-top:2rem;color:#777;font-size:.85rem}\
         @media print{body{margin:.5in;font-size:11pt} .diagnostics{display:none} h2{page-break-after:avoid} table,ul{page-break-inside:avoid}}\
         </style></head><body>",
    );
    writeln!(
        out,
        "<h1>{}</h1><p class=\"subtitle\">{} · Level {}</p>",
        escape_html(&snapshot.name),
        escape_html(&snapshot.race_id),
        snapshot.total_level
    )
    .ok();

    if !snapshot.stats.is_empty() {
        out.push_str("<section><h2>Ability Scores</h2><table>");
        out.push_str("<tr><th>Ability</th><th>Score</th></tr>");
        for (stat, value) in &snapshot.stats {
            writeln!(
                out,
                "<tr><td>{}</td><td>{value}</td></tr>",
                escape_html(stat)
            )
            .ok();
        }
        out.push_str("</table></section>");
    }

    out.push_str("<section><h2>Combat</h2><table>");
    writeln!(
        out,
        "<tr><th>HP</th><td>{}</td><th>BAB</th><td>{:+}</td></tr>",
        snapshot.hit_points, snapshot.base_attack_bonus
    )
    .ok();
    writeln!(
        out,
        "<tr><th>Melee</th><td>{:+}</td><th>Ranged</th><td>{:+}</td></tr>",
        snapshot.melee_attack, snapshot.ranged_attack
    )
    .ok();
    writeln!(
        out,
        "<tr><th>Grapple</th><td>{:+}</td><th>Initiative</th><td>{:+}</td></tr>",
        snapshot.grapple_attack, snapshot.initiative
    )
    .ok();
    writeln!(
        out,
        "<tr><th>AC</th><td>{}</td><th>Touch</th><td>{}</td></tr>",
        snapshot.armor_class, snapshot.touch_ac
    )
    .ok();
    writeln!(
        out,
        "<tr><th>Flat-footed</th><td>{}</td><th>Armor / Shield</th><td>{} / {}</td></tr>",
        snapshot.flat_footed_ac, snapshot.armor_bonus, snapshot.shield_bonus
    )
    .ok();
    out.push_str("</table></section>");

    if !snapshot.saves.is_empty() {
        out.push_str("<section><h2>Saving Throws</h2><table>");
        for (save, value) in &snapshot.saves {
            writeln!(
                out,
                "<tr><td>{}</td><td>{value:+}</td></tr>",
                escape_html(save)
            )
            .ok();
        }
        out.push_str("</table></section>");
    }

    if !snapshot.skills.is_empty() {
        out.push_str("<section><h2>Skills</h2><div class=\"grid-skills\"><table>");
        out.push_str("<tr><th>Skill</th><th>Total</th></tr>");
        for (skill, value) in &snapshot.skills {
            writeln!(
                out,
                "<tr><td>{}</td><td>{value:+}</td></tr>",
                escape_html(skill)
            )
            .ok();
        }
        out.push_str("</table></div></section>");
    }

    let has_abilities = !snapshot.active_abilities.is_empty()
        || snapshot
            .ability_lists
            .values()
            .any(|values| !values.is_empty());
    if has_abilities {
        out.push_str("<section><h2>Feats &amp; Abilities</h2><ul>");
        for ability in &snapshot.active_abilities {
            writeln!(out, "<li>{}</li>", escape_html(ability)).ok();
        }
        for values in snapshot.ability_lists.values() {
            for ability in values {
                writeln!(out, "<li>{}</li>", escape_html(ability)).ok();
            }
        }
        out.push_str("</ul></section>");
    }

    let has_spellcasting = !snapshot.caster_levels.is_empty()
        || !snapshot.spell_dcs.is_empty()
        || !snapshot.prepared_spells.is_empty()
        || !snapshot.spell_slots.is_empty()
        || !snapshot.power_points.is_empty()
        || !snapshot.powers_known.is_empty()
        || !snapshot.domain_spells.is_empty();
    if has_spellcasting {
        out.push_str("<section><h2>Spellcasting</h2>");
        if !snapshot.caster_levels.is_empty() {
            out.push_str("<table><tr><th>Class</th><th>Caster level</th></tr>");
            for (class_id, level) in &snapshot.caster_levels {
                writeln!(
                    out,
                    "<tr><td>{}</td><td>{level}</td></tr>",
                    escape_html(class_id)
                )
                .ok();
            }
            out.push_str("</table>");
        }
        if !snapshot.spell_slots.is_empty() {
            out.push_str("<table><tr><th>Class | level</th><th>Slots</th></tr>");
            for (key, count) in &snapshot.spell_slots {
                writeln!(
                    out,
                    "<tr><td>{}</td><td>{count}</td></tr>",
                    escape_html(key)
                )
                .ok();
            }
            out.push_str("</table>");
        }
        if !snapshot.spell_dcs.is_empty() {
            out.push_str("<table><tr><th>Spell</th><th>DC</th></tr>");
            for (spell, dc) in &snapshot.spell_dcs {
                writeln!(out, "<tr><td>{}</td><td>{dc}</td></tr>", escape_html(spell)).ok();
            }
            out.push_str("</table>");
        }
        if !snapshot.prepared_spells.is_empty() {
            out.push_str("<table><tr><th>Prepared spell</th><th>Count</th></tr>");
            for (spell, count) in &snapshot.prepared_spells {
                writeln!(
                    out,
                    "<tr><td>{}</td><td>{count}</td></tr>",
                    escape_html(spell)
                )
                .ok();
            }
            out.push_str("</table>");
        }
        if !snapshot.domain_spells.is_empty() {
            out.push_str("<table><tr><th>Domain</th><th>Spells</th></tr>");
            for (domain, spells) in &snapshot.domain_spells {
                let names = spells
                    .iter()
                    .map(|spell| escape_html(&spell.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    out,
                    "<tr><td>{}</td><td>{names}</td></tr>",
                    escape_html(domain)
                )
                .ok();
            }
            out.push_str("</table>");
        }
        if !snapshot.power_points.is_empty() {
            out.push_str("<table><tr><th>Manifester</th><th>Power points</th></tr>");
            for (class_id, points) in &snapshot.power_points {
                writeln!(
                    out,
                    "<tr><td>{}</td><td>{points}</td></tr>",
                    escape_html(class_id)
                )
                .ok();
            }
            out.push_str("</table>");
        }
        if !snapshot.powers_known.is_empty() {
            out.push_str("<ul>");
            for (class_id, powers) in &snapshot.powers_known {
                writeln!(
                    out,
                    "<li><strong>{}</strong>: {}</li>",
                    escape_html(class_id),
                    powers
                        .iter()
                        .map(|power| escape_html(power))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .ok();
            }
            out.push_str("</ul>");
        }
        out.push_str("</section>");
    }

    if !snapshot.equipment.is_empty() {
        out.push_str("<section><h2>Equipment</h2><table>");
        out.push_str("<tr><th>Item</th><th>Cost</th><th>Armor</th><th>Max Dex</th><th>ACP</th><th>Type</th></tr>");
        for item in &snapshot.equipment {
            writeln!(
                out,
                "<tr><td>{}</td><td>{:.2} gp</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&item.name),
                item.cost_gp,
                item.armor_bonus,
                item.max_dex_bonus
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "—".to_string()),
                item.armor_check_penalty,
                escape_html(&item.type_tags.join(", "))
            )
            .ok();
        }
        out.push_str("</table></section>");
    }

    let has_traits = !snapshot.vision.is_empty()
        || !snapshot.damage_reduction.is_empty()
        || snapshot.spell_resistance.is_some()
        || !snapshot.languages.is_empty()
        || !snapshot.movement.is_empty()
        || !snapshot.natural_attacks.is_empty();
    if has_traits {
        out.push_str("<section><h2>Traits</h2><ul>");
        if !snapshot.vision.is_empty() {
            writeln!(
                out,
                "<li><strong>Vision:</strong> {}</li>",
                escape_html(&snapshot.vision.join(", "))
            )
            .ok();
        }
        if !snapshot.damage_reduction.is_empty() {
            writeln!(
                out,
                "<li><strong>DR:</strong> {}</li>",
                escape_html(&snapshot.damage_reduction.join(", "))
            )
            .ok();
        }
        if let Some(sr) = snapshot.spell_resistance {
            writeln!(out, "<li><strong>SR:</strong> {sr}</li>").ok();
        }
        if !snapshot.languages.is_empty() {
            writeln!(
                out,
                "<li><strong>Languages:</strong> {}</li>",
                escape_html(&snapshot.languages.join(", "))
            )
            .ok();
        }
        if !snapshot.movement.is_empty() {
            let movement = snapshot
                .movement
                .iter()
                .map(|(mode, feet)| format!("{} {} ft", escape_html(mode), feet))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(out, "<li><strong>Movement:</strong> {movement}</li>").ok();
        }
        if !snapshot.natural_attacks.is_empty() {
            writeln!(
                out,
                "<li><strong>Natural attacks:</strong> {}</li>",
                escape_html(&snapshot.natural_attacks.join(", "))
            )
            .ok();
        }
        out.push_str("</ul></section>");
    }

    if !snapshot.diagnostics.is_empty() {
        out.push_str("<section class=\"diagnostics\"><h2>Diagnostics</h2><ul>");
        for diagnostic in &snapshot.diagnostics {
            writeln!(out, "<li>{}</li>", escape_html(diagnostic)).ok();
        }
        out.push_str("</ul></section>");
    }

    if !snapshot.provenance.rules_fingerprint.is_empty() {
        writeln!(
            out,
            "<footer>{}</footer>",
            escape_html(&snapshot.provenance.render())
        )
        .ok();
    }
    out.push_str("</body></html>");
    out
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Build a JSON context for the minimal FreeMarker-style renderer.
pub fn snapshot_context(snapshot: &CharacterSnapshot) -> serde_json::Value {
    serde_json::json!({
        "name": snapshot.name,
        "race": snapshot.race_id,
        "level": snapshot.total_level,
        "hp": snapshot.hit_points,
        "bab": snapshot.base_attack_bonus,
        "initiative": snapshot.initiative,
        "ac": snapshot.armor_class,
        "touch_ac": snapshot.touch_ac,
        "flat_ac": snapshot.flat_footed_ac,
        "stats": snapshot.stats,
        "saves": snapshot.saves.iter().map(|(name, value)| serde_json::json!({"name": name, "value": value})).collect::<Vec<_>>(),
        "skills": snapshot.skills.iter().map(|(name, value)| serde_json::json!({"name": name, "value": value})).collect::<Vec<_>>(),
        "spells": snapshot.spell_dcs.iter().map(|(name, dc)| serde_json::json!({"name": name, "dc": dc})).collect::<Vec<_>>(),
        "power_points": snapshot.power_points.iter().map(|(name, value)| serde_json::json!({"name": name, "value": value})).collect::<Vec<_>>(),
    })
}

/// Render a minimal FreeMarker-style template for a snapshot.
pub fn render_template_snapshot(template: &str, snapshot: &CharacterSnapshot) -> String {
    let context = snapshot_context(snapshot);
    let (template, macros) = extract_macros(template);
    let rendered = render_template_value(&template, &context);
    cleanup_ftl(&expand_macros(&rendered, &macros, &context))
}

#[derive(Debug, Clone)]
struct MacroDefinition {
    params: Vec<(String, Option<String>)>,
    body: String,
}

fn extract_macros(template: &str) -> (String, std::collections::BTreeMap<String, MacroDefinition>) {
    let mut output = template.to_string();
    let mut macros = std::collections::BTreeMap::new();
    while let Some(start) = output.find("<#macro ") {
        let Some(header_end) = output[start..].find('>') else {
            break;
        };
        let header_end = start + header_end + 1;
        let header = &output[start + "<#macro ".len()..header_end - 1];
        let mut parts = header.split_whitespace();
        let Some(name) = parts.next() else {
            break;
        };
        let params = parts
            .map(|part| {
                part.split_once('=')
                    .map(|(name, default)| {
                        (name.trim().to_string(), Some(default.trim().to_string()))
                    })
                    .unwrap_or_else(|| (part.trim().to_string(), None))
            })
            .collect::<Vec<_>>();
        let Some(end) = output[header_end..].find("</#macro>") else {
            break;
        };
        let end = header_end + end;
        let body = output[header_end..end].to_string();
        macros.insert(name.to_string(), MacroDefinition { params, body });
        output.replace_range(start..end + "</#macro>".len(), "");
    }
    (output, macros)
}

fn expand_macros(
    template: &str,
    macros: &std::collections::BTreeMap<String, MacroDefinition>,
    context: &serde_json::Value,
) -> String {
    let mut output = template.to_string();
    for _ in 0..16 {
        let Some(start) = output.find("<@") else {
            break;
        };
        let Some(end) = output[start..].find("/>") else {
            break;
        };
        let end = start + end + 2;
        let call = output[start + 2..end - 2].trim().to_string();
        let mut parts = call.split_whitespace();
        let raw_name = parts.next().unwrap_or("");
        let short_name = raw_name.rsplit('.').next().unwrap_or(raw_name);
        let Some(definition) = macros.get(raw_name).or_else(|| macros.get(short_name)) else {
            // Remove unknown macro calls to avoid leaking FTL syntax.
            output.replace_range(start..end, "");
            continue;
        };
        let mut local = context.clone();
        let args: Vec<String> = parts.map(str::to_string).collect();
        for (index, arg) in args.iter().enumerate() {
            if let Some((key, value)) = arg.split_once('=') {
                local[key.trim()] =
                    serde_json::Value::String(macro_arg_value(context, value.trim()));
            } else if let Some((param, _)) = definition.params.get(index) {
                local[param] = serde_json::Value::String(macro_arg_value(context, arg.trim()));
            }
        }
        for (param, default) in &definition.params {
            if local.get(param).is_none() {
                if let Some(default) = default {
                    local[param] = serde_json::Value::String(macro_arg_value(context, default));
                }
            }
        }
        let body = expand_macros(&definition.body, macros, &local);
        let rendered = render_template_value(&body, &local);
        output.replace_range(start..end, &rendered);
    }
    output
}

fn render_template_value(template: &str, context: &serde_json::Value) -> String {
    let mut context = context.clone();
    let template = strip_ftl_comments(template);
    let mut output = apply_assignments(&template, &mut context);

    // #include "path" is resolved by render_template_file_snapshot before
    // this function is called; leave the directive harmless here.

    // #list path as var ... #endlist
    while let Some(start) = output.find("#list ") {
        let (end, closing_len) = match directive_close(&output[start..], "#endlist", "</#list>") {
            Some(value) => (start + value.0, value.1),
            None => break,
        };
        let header_end = match output[start..].find('>') {
            Some(offset) => start + offset + 1,
            None => break,
        };
        let header = &output[start + "#list ".len()..header_end - 1];
        let body = &output[header_end..end];
        let mut parts = header.split_whitespace();
        let path = parts.next().unwrap_or("");
        let var = parts.nth(1).unwrap_or("item");
        let mut rendered = String::new();
        if let Some(items) = resolve_path(&context, path).and_then(|value| value.as_array()) {
            for item in items {
                let mut local = context.clone();
                local[var] = item.clone();
                rendered.push_str(&render_template_value(body, &local));
            }
        }
        output.replace_range(
            directive_start(&output, start)..end + closing_len,
            &rendered,
        );
    }

    // #if path operator literal ... #endif
    while let Some(start) = output.find("#if ") {
        let (end, closing_len) = match directive_close(&output[start..], "#endif", "</#if>") {
            Some(value) => (start + value.0, value.1),
            None => break,
        };
        let header_end = match output[start..].find('>') {
            Some(offset) => start + offset + 1,
            None => break,
        };
        let condition = output[start + "#if ".len()..header_end - 1]
            .trim()
            .to_string();
        let body = &output[header_end..end];
        let (then_body, else_body) = match body.split_once("#else") {
            Some((then_body, else_body)) => (then_body, else_body),
            None => (body, ""),
        };
        let rendered = if evaluate_condition(&context, &condition) {
            render_template_value(then_body, &context)
        } else {
            render_template_value(else_body, &context)
        };
        output.replace_range(
            directive_start(&output, start)..end + closing_len,
            &rendered,
        );
    }

    // ${path?filter(...)} handling.
    while let Some(start) = output.find("${") {
        let Some(end) = output[start..].find('}') else {
            break;
        };
        let end = start + end;
        let expression = &output[start + 2..end];
        let value = evaluate_expression(&context, expression);
        output.replace_range(start..end + 1, &value);
    }

    output
}

fn directive_close(input: &str, long: &str, short: &str) -> Option<(usize, usize)> {
    let long_index = input.find(long).map(|index| (index, long.len()));
    let short_index = input.find(short).map(|index| (index, short.len()));
    match (long_index, short_index) {
        (Some(long), Some(short)) => Some(if long.0 <= short.0 { long } else { short }),
        (Some(long), None) => Some(long),
        (None, Some(short)) => Some(short),
        (None, None) => None,
    }
}

fn directive_start(input: &str, start: usize) -> usize {
    if start > 0 && input.as_bytes().get(start - 1) == Some(&b'<') {
        start - 1
    } else {
        start
    }
}

fn directive_end(input: &str, start: usize) -> usize {
    let greater = input[start..].find('>').map(|offset| start + offset + 1);
    let newline = input[start..].find('\n').map(|offset| start + offset);
    match (greater, newline) {
        (Some(greater), Some(newline)) => greater.min(newline),
        (Some(greater), None) => greater,
        (None, Some(newline)) => newline,
        (None, None) => input.len(),
    }
}

fn strip_ftl_comments(template: &str) -> String {
    let mut output = template.to_string();
    while let Some(start) = output.find("<#--") {
        let Some(end) = output[start..].find("-->") else {
            break;
        };
        output.replace_range(start..start + end + 3, "");
    }
    output
}

fn cleanup_ftl(template: &str) -> String {
    let mut output = template.to_string();
    while let Some(start) = output.find("<#") {
        let Some(end) = output[start..].find('>') else {
            break;
        };
        output.replace_range(start..start + end + 1, "");
    }
    while let Some(start) = output.find("</@") {
        let Some(end) = output[start..].find('>') else {
            break;
        };
        output.replace_range(start..start + end + 1, "");
    }
    while let Some(start) = output.find("<@") {
        let Some(end) = output[start..].find("/>") else {
            break;
        };
        output.replace_range(start..start + end + 2, "");
    }
    output
}

fn apply_assignments(template: &str, context: &mut serde_json::Value) -> String {
    let mut output = template.to_string();
    while let Some(start) = output.find("#assign ") {
        let end = directive_end(&output, start);
        let directive = output[start + "#assign ".len()..end]
            .trim_end_matches('>')
            .trim();
        if let Some((name, expression)) = directive.split_once('=') {
            let name = name.trim();
            let value = evaluate_expression(context, expression.trim());
            context[name] = serde_json::Value::String(value);
        }
        output.replace_range(directive_start(&output, start)..end, "");
    }
    output
}

fn evaluate_expression(context: &serde_json::Value, expression: &str) -> String {
    let expression = expression.trim();
    if expression.starts_with('\'') || expression.starts_with('"') {
        return first_string_argument(expression).unwrap_or_default();
    }
    if expression.parse::<f64>().is_ok() {
        return expression.to_string();
    }
    let (path, filter) = match expression.split_once('?') {
        Some((path, filter)) => (path.trim(), Some(filter.trim())),
        None => (expression, None),
    };
    let raw = resolve_path(context, path);
    let Some(filter) = filter else {
        return raw.map(render_value).unwrap_or_default();
    };
    let (filter_name, args) = match filter.split_once('(') {
        Some((name, rest)) => (name, rest.trim_end_matches(')')),
        None => (filter, ""),
    };
    match filter_name {
        "upper_case" => raw.map(render_value).unwrap_or_default().to_uppercase(),
        "lower_case" => raw.map(render_value).unwrap_or_default().to_lowercase(),
        "size" => raw
            .map(|value| {
                if let Some(array) = value.as_array() {
                    array.len().to_string()
                } else if let Some(object) = value.as_object() {
                    object.len().to_string()
                } else {
                    "0".to_string()
                }
            })
            .unwrap_or_default(),
        "join" => {
            let delimiter = first_string_argument(args).unwrap_or_else(|| ",".to_string());
            raw.and_then(|value| value.as_array())
                .map(|array| {
                    array
                        .iter()
                        .map(render_value)
                        .collect::<Vec<_>>()
                        .join(&delimiter)
                })
                .unwrap_or_default()
        }
        "replace" => {
            let (from, to) = two_string_arguments(args);
            raw.map(render_value)
                .unwrap_or_default()
                .replace(&from.unwrap_or_default(), &to.unwrap_or_default())
        }
        "string" => {
            let pattern = first_string_argument(args).unwrap_or_default();
            let decimals = pattern
                .split_once('.')
                .map(|(_, decimals)| decimals.len())
                .unwrap_or(0);
            raw.map(|value| {
                if let Some(number) = value.as_f64() {
                    format!("{number:.decimals$}")
                } else {
                    render_value(value)
                }
            })
            .unwrap_or_default()
        }
        _ => raw.map(render_value).unwrap_or_default(),
    }
}

fn macro_arg_value(context: &serde_json::Value, raw: &str) -> String {
    let trimmed = raw.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed.trim_matches('"').trim_matches('\'').to_string()
    } else {
        evaluate_expression(context, trimmed)
    }
}

fn first_string_argument(args: &str) -> Option<String> {
    let trimmed = args.trim();
    let quoted = trimmed
        .strip_prefix('\'')
        .or_else(|| trimmed.strip_prefix('"'))?;
    let quote = trimmed.chars().next()?;
    quoted.split(quote).next().map(str::to_string)
}

fn two_string_arguments(args: &str) -> (Option<String>, Option<String>) {
    let mut parts = args.split(',');
    let from = parts.next().and_then(first_string_argument);
    let to = parts.next().and_then(first_string_argument);
    (from, to)
}

/// Render a template file with simple relative `#include` support.
pub fn render_template_file_snapshot(
    path: impl AsRef<std::path::Path>,
    snapshot: &CharacterSnapshot,
) -> std::io::Result<String> {
    let path = path.as_ref();
    let template = std::fs::read_to_string(path)?;
    let mut expanded = template.clone();
    while let Some(start) = expanded.find("#import ") {
        let end = directive_end(&expanded, start);
        let directive = expanded[start + "#import ".len()..end]
            .trim_end_matches('>')
            .trim();
        let name = first_string_argument(directive).unwrap_or_default();
        let namespace = directive
            .split_once(" as ")
            .map(|(_, namespace)| namespace.trim().to_string())
            .unwrap_or_default();
        let include_path = path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(name);
        let include = std::fs::read_to_string(&include_path).unwrap_or_default();
        let include = if namespace.is_empty() {
            include
        } else {
            include
                .replace("#macro ", &format!("#macro {namespace}."))
                .replace("<@", &format!("<@{namespace}."))
        };
        expanded.replace_range(directive_start(&expanded, start)..end, &include);
    }
    while let Some(start) = expanded.find("#include ") {
        let end = directive_end(&expanded, start);
        let directive = expanded[start + "#include ".len()..end]
            .trim_end_matches('>')
            .trim();
        let name = first_string_argument(directive).unwrap_or_default();
        let include_path = path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(name);
        let include = std::fs::read_to_string(&include_path).unwrap_or_default();
        expanded.replace_range(directive_start(&expanded, start)..end, &include);
    }
    let context = snapshot_context(snapshot);
    let (template, macros) = extract_macros(&expanded);
    let rendered = render_template_value(&template, &context);
    Ok(cleanup_ftl(&expand_macros(&rendered, &macros, &context)))
}

fn resolve_path<'a>(context: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut value = context;
    for part in path.split('.') {
        value = value.get(part)?;
    }
    Some(value)
}

fn render_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn evaluate_condition(context: &serde_json::Value, condition: &str) -> bool {
    let mut parts = condition.split_whitespace();
    let path = parts.next().unwrap_or("");
    let Some(left) = resolve_path(context, path) else {
        return false;
    };
    let Some(operator) = parts.next() else {
        return left.as_bool().unwrap_or(false);
    };
    let right = parts.next().unwrap_or("");
    match operator {
        "==" => render_value(left) == right,
        "!=" => render_value(left) != right,
        ">" => number(left) > right.parse::<f64>().ok(),
        "<" => number(left) < right.parse::<f64>().ok(),
        ">=" => number(left) >= right.parse::<f64>().ok(),
        "<=" => number(left) <= right.parse::<f64>().ok(),
        "exists" => !left.is_null(),
        _ => false,
    }
}

fn number(value: &serde_json::Value) -> Option<f64> {
    value.as_f64()
}
