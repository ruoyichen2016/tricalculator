//! Ordinary forms for tactical facts and channeling; no browser-side rule arithmetic.
use super::{
    forms::{begin, checked, input, number, value},
    ui::escape,
};
use pcgen_core::{
    class_play::{DefenseRequest, TargetPath},
    magic::*,
    tactics::*,
};
use pcgen_engine::{class_play::PlayResult, CharacterDraft, Engine};
use std::{collections::BTreeMap, fmt::Write};
fn target_select(out: &mut String, d: &CharacterDraft, name: &str, blank: bool) {
    let _ = write!(out, "<label>Target<select name={name}>");
    if blank {
        out.push_str("<option value=''>No attack</option>");
    }
    for id in d
        .class_play
        .actors
        .keys()
        .filter(|id| id.as_str() != "self")
    {
        let _ = write!(out, "<option>{}</option>", escape(id));
    }
    out.push_str("</select></label>");
}
pub(super) fn render(out: &mut String, id: &str, d: &CharacterDraft, e: &Engine) {
    if d.class_play.tactics.channel.is_none()
        && !d
            .class_levels()
            .iter()
            .any(|c| ["Knight", "Duskblade"].contains(&c.class_id.as_str()))
    {
        return;
    }
    out.push_str("<section id=tactical-actions><h2>Tactical actions</h2>");
    for target in d.class_play.actors.keys() {
        let _ = write!(
            out,
            "<details><summary>{}: tactical facts</summary>",
            escape(target)
        );
        begin(out, id, d.class_play.revision, "tactical_facts");
        let _ = write!(
            out,
            "<input type=hidden name=target value=\"{}\">",
            escape(target)
        );
        let f = d.class_play.tactics.actors.get(target);
        input(
            out,
            "Normal Armor Class",
            "ac",
            &f.map(|f| f.armor_class.to_string()).unwrap_or_default(),
            "number",
        );
        input(
            out,
            "Challenge rating",
            "cr",
            &f.map(|f| f.challenge_rating.to_string())
                .unwrap_or_default(),
            "number",
        );
        checked(
            out,
            "Has a language",
            "language",
            f.is_some_and(|f| f.language),
        );
        checked(
            out,
            "Can hear the knight",
            "hears",
            f.is_some_and(|f| f.hears),
        );
        checked(
            out,
            "Has fled the encounter",
            "fled",
            f.is_some_and(|f| f.fled),
        );
        out.push_str("<button>Save tactical facts</button></form></details>");
    }
    if d.class_levels().iter().any(|c| c.class_id == "Knight") {
        if let Some(c) = &d.class_play.tactics.fighting {
            let _=write!(out,"<p>Fighting Challenge against {}: +{} morale to attacks and damage; +{} morale to Will; expires at {} seconds.</p>",escape(&c.target),c.bonus,c.bonus,c.expires);
        }
        for (target, until) in &d.class_play.tactics.mettle {
            let _ = write!(
                out,
                "<p>{}: Test of Mettle until {until} seconds.</p>",
                escape(target)
            );
        }
        begin(out, id, d.class_play.revision, "tactical_attack_event");
        out.push_str("<label>Attacker<select name=attacker>");
        for id in d.class_play.actors.keys() {
            let _ = write!(out, "<option>{}</option>", escape(id));
        }
        out.push_str("</select></label><label>Attacked actor<select name=target>");
        for id in d.class_play.actors.keys() {
            let _ = write!(out, "<option>{}</option>", escape(id));
        }
        out.push_str("</select></label><p>Record even a missed attack. When the knight has fallen to 0 HP or less, record the attack after updating HP to apply the challenge defeat rules.</p><button>Record challenge attack event</button></form>");
    }
    render_held_touch(out, id, d);
    if e.class_feature_values(d).is_ok_and(|fs| {
        fs.iter()
            .any(|f| f.class_id == "Duskblade" && f.name == "Arcane Channeling")
    }) {
        begin(out, id, d.class_play.revision, "tactical_channel");
        out.push_str("<label>Known touch spell<select name=spell>");
        if let Ok(spells) = e.spell_catalog(d, "Duskblade") {
            for s in spells.into_iter().filter(|s| s.known) {
                if e.pack
                    .spells
                    .values()
                    .any(|p| p.name == s.spell && p.profile.range == SpellRange::Touch)
                {
                    let _ = write!(out, "<option>{}</option>", escape(&s.spell));
                }
            }
        }
        out.push_str("</select></label>");
        out.push_str("<label>Channel action<select name=channel_action>");
        if d.class_play
            .tactics
            .channel
            .as_ref()
            .is_some_and(|c| c.mode != ChargeMode::Touch)
        {
            out.push_str("<option value=resume selected>Continue saved weapon delivery</option><option value=opportunity>Resolve a provoked opportunity attack</option>");
        }
        out.push_str("<option value=standard>Cast new standard channel</option>");
        if e.class_feature_values(d).is_ok_and(|fs| {
            fs.iter()
                .any(|f| f.name == "Arcane Channeling" && f.values.get("full_attack") == Some(&1))
        }) {
            out.push_str("<option value=full>Cast new full-attack channel</option>");
        }
        out.push_str("</select></label>");
        if let Some(c) = &d.class_play.tactics.channel {
            let _=write!(out,"<p>Charge {} · {:?}; iterative attacks {}/{}; sequence {}; touched targets {}. Continuing uses the saved spell and next attack index; selecting a new cast replaces it.</p>",escape(&c.casting),c.mode,c.attacks_used,c.attack_limit,if c.attacks_closed{"closed"}else{"open"},escape(&c.targets.iter().cloned().collect::<Vec<_>>().join(", ")));
        }
        checked(
            out,
            "Finish the remaining iterative attacks after this submission",
            "finish_attacks",
            false,
        );
        out.push_str("<p>Standard channeling delivers one touch, including Chill Touch (official FAQ p.17). Full channeling affects each distinct hit target once and expires at round end. The saved RAW weapon supplies dice, type, material, enhancement and critical profile. Enter dice totals only; Strength, weapon bonuses and DR are calculated. Spell riders resolve separately even when DR absorbs all weapon damage.</p>");
        let attack_count = e.evaluate(d).base_attack_bonus.saturating_sub(1) / 5 + 1;
        for i in 0..attack_count.clamp(1, 4) {
            let _ = write!(out, "<details><summary>Melee attack {}</summary>", i + 1);
            target_select(out, d, &format!("target_{i}"), true);
            input(out, "Attack d20", &format!("roll_{i}"), "", "number");
            input(
                out,
                "Weapon dice total (before modifiers and DR)",
                &format!("weapon_damage_{i}"),
                "",
                "number",
            );
            input(
                out,
                "Critical confirmation d20 (on a threat)",
                &format!("confirmation_{i}"),
                "",
                "number",
            );
            input(
                out,
                "Full critical weapon dice total (if confirmed)",
                &format!("critical_damage_{i}"),
                "",
                "number",
            );
            input(
                out,
                "Spell effect die (idiocy/paralysis/fear as applicable)",
                &format!("effect_{i}"),
                "",
                "number",
            );
            input(
                out,
                "Dimension Hop destination X (visible square)",
                &format!("destination_x_{i}"),
                "",
                "number",
            );
            input(
                out,
                "Dimension Hop destination Y (visible square)",
                &format!("destination_y_{i}"),
                "",
                "number",
            );
            out.push_str(&format!("<label>Destination visibility<select name=destination_path_{i}><option value=unknown>Unrecorded</option><option value=clear>Visible clear path</option><option value=transparent>Visible through transparent barrier</option><option value=blocked>Not visible</option></select></label>"));
            for effect in d.class_play.combat_state.effects.values() {
                input(
                    out,
                    &format!(
                        "Dispelling Touch: {} on {} (CL {}) d20",
                        effect.name, effect.target, effect.caster_level
                    ),
                    &format!("dispel_{i}_{}", effect.casting),
                    "",
                    "number",
                );
            }
            input(
                out,
                "Spell damage roll",
                &format!("spell_damage_{i}"),
                "",
                "number",
            );
            input(
                out,
                "Spell save d20 (if required)",
                &format!("save_{i}"),
                "",
                "number",
            );
            input(
                out,
                "Caster-level d20 against SR (if required)",
                &format!("sr_{i}"),
                "",
                "number",
            );
            input(
                out,
                "Massive-damage Fortitude d20 (if required)",
                &format!("massive_{i}"),
                "",
                "number",
            );
            out.push_str("</details>");
        }
        out.push_str("<button>Channel spell &amp; resolve melee</button></form>");
    }
    out.push_str("</section>");
}
fn optional<T: std::str::FromStr>(
    f: &BTreeMap<String, String>,
    k: &str,
) -> Result<Option<T>, String> {
    f.get(k)
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().map_err(|_| format!("Invalid {k}")))
        .transpose()
}
pub(super) fn execute(
    e: &Engine,
    d: &CharacterDraft,
    f: &BTreeMap<String, String>,
) -> Result<PlayResult, String> {
    match value(f, "op")? {
        "tactical_facts" => e.save_tactical_facts(
            d,
            value(f, "target")?,
            TacticalFacts {
                armor_class: value(f, "ac")?.parse().map_err(|_| "Invalid AC")?,
                challenge_rating: value(f, "cr")?.parse().map_err(|_| "Invalid CR")?,
                language: f.contains_key("language"),
                hears: f.contains_key("hears"),
                fled: f.contains_key("fled"),
            },
        ),
        "tactical_attack_event" => {
            e.record_challenge_attack(d, value(f, "attacker")?, value(f, "target")?)
        }
        "tactical_touch" => e.deliver_held_touch(
            d,
            &TouchRequest {
                roll: optional(f, "touch_attack")?,
                confirmation: optional(f, "touch_confirmation")?,
                defense: DefenseRequest {
                    target: value(f, "target")?.into(),
                    damage_roll: optional(f, "touch_damage")?,
                    effect_roll: optional(f, "touch_effect")?,
                    save_roll: optional(f, "touch_save")?,
                    sr_roll: optional(f, "touch_sr")?,
                    massive_roll: optional(f, "touch_massive")?,
                    waive_save: f.contains_key("waive"),
                    sr_lowered: f.contains_key("lowered"),
                    destination: match (optional(f, "touch_x")?, optional(f, "touch_y")?) {
                        (Some(x), Some(y)) => Some((x, y)),
                        (None, None) => None,
                        _ => return Err("Both destination coordinates are required".into()),
                    },
                    destination_path: match f.get("touch_path").map(String::as_str) {
                        Some("clear") => Some(TargetPath::Clear),
                        Some("transparent") => Some(TargetPath::TransparentSolid),
                        _ => None,
                    },
                    dispel_rolls: f
                        .iter()
                        .filter_map(|(k, v)| {
                            k.strip_prefix("touch_dispel_")
                                .filter(|_| !v.is_empty())
                                .map(|id| (id.to_owned(), v))
                        })
                        .map(|(id, v)| {
                            v.parse()
                                .map(|n| (id, n))
                                .map_err(|_| "Invalid dispel roll")
                        })
                        .collect::<Result<_, _>>()?,
                },
            },
        ),
        "tactical_object_contact" => e.record_charge_object_contact(d),
        "tactical_channel" => {
            let action = f.get("channel_action").ok_or("Choose a channel action")?;
            let mut r = ChannelRequest {
                finish_attacks: f.contains_key("finish_attacks"),
                opportunity: action == "opportunity",
                cast: CastRequest {
                    class_id: "Duskblade".into(),
                    spell: value(f, "spell")?.into(),
                    ..Default::default()
                },
                full_attack: action == "full",
                resume: action == "resume" || action == "opportunity",
                ..Default::default()
            };
            for i in 0..4 {
                let Some(target) = f.get(&format!("target_{i}")).filter(|v| !v.is_empty()) else {
                    continue;
                };
                r.strikes.push(MeleeStrike {
                    target: target.clone(),
                    roll: number(f, &format!("roll_{i}"))?,
                    weapon_dice_roll: value(f, &format!("weapon_damage_{i}"))?
                        .parse()
                        .map_err(|_| "Invalid weapon dice")?,
                    confirmation_roll: optional(f, &format!("confirmation_{i}"))?,
                    critical_dice_roll: optional(f, &format!("critical_damage_{i}"))?,
                    spell: DefenseRequest {
                        destination_path: match f
                            .get(&format!("destination_path_{i}"))
                            .map(String::as_str)
                        {
                            Some("clear") => Some(TargetPath::Clear),
                            Some("transparent") => Some(TargetPath::TransparentSolid),
                            _ => None,
                        },
                        effect_roll: optional(f, &format!("effect_{i}"))?,
                        dispel_rolls: f
                            .iter()
                            .filter_map(|(k, v)| {
                                k.strip_prefix(&format!("dispel_{i}_"))
                                    .filter(|_| !v.is_empty())
                                    .map(|id| (id.to_owned(), v))
                            })
                            .map(|(id, v)| {
                                v.parse()
                                    .map(|n| (id, n))
                                    .map_err(|_| "Invalid dispel roll")
                            })
                            .collect::<Result<_, _>>()?,
                        destination: match (
                            optional(f, &format!("destination_x_{i}"))?,
                            optional(f, &format!("destination_y_{i}"))?,
                        ) {
                            (Some(x), Some(y)) => Some((x, y)),
                            (None, None) => None,
                            _ => return Err("Both destination coordinates are required".into()),
                        },
                        target: target.clone(),
                        massive_roll: optional(f, &format!("massive_{i}"))?,
                        sr_roll: optional(f, &format!("sr_{i}"))?,
                        save_roll: optional(f, &format!("save_{i}"))?,
                        damage_roll: optional(f, &format!("spell_damage_{i}"))?,
                        waive_save: false,
                        sr_lowered: false,
                    },
                });
            }
            e.arcane_channel(d, &r)
        }
        _ => Err("Unknown tactical action".into()),
    }
}

fn render_held_touch(out: &mut String, id: &str, d: &CharacterDraft) {
    let Some(c) = d
        .class_play
        .tactics
        .channel
        .as_ref()
        .filter(|c| c.mode != ChargeMode::FullChannel)
    else {
        return;
    };
    let Some(cast) = d.class_play.cast_effects.get(&c.casting) else {
        return;
    };
    let _=write!(out,"<h3>Held touch: {}</h3><p>{} touches remaining · CL {} · no additional spell slot. A miss keeps the charge; contact spends a touch even if saved or resisted. Casting another spell dissipates it.</p>",escape(&cast.spell),c.remaining,cast.caster_level);
    begin(out, id, d.class_play.revision, "tactical_touch");
    target_select(out, d, "target", false);
    for (label, key) in [
        ("Melee touch attack d20 (unwilling target)", "touch_attack"),
        (
            "Touch critical confirmation d20 (natural 20, damage spell)",
            "touch_confirmation",
        ),
        (
            "Spell damage dice total (include confirmed critical dice)",
            "touch_damage",
        ),
        ("Saving throw d20", "touch_save"),
        ("Caster SR check d20", "touch_sr"),
        ("Effect d4 / d6", "touch_effect"),
        ("Massive damage Fortitude d20", "touch_massive"),
    ] {
        input(out, label, key, "", "number");
    }
    checked(out, "Recipient voluntarily waives save", "waive", false);
    checked(out, "Recipient already lowered SR", "lowered", false);
    out.push_str("<details><summary>Teleport or dispel inputs</summary>");
    input(out, "Destination X", "touch_x", "", "number");
    input(out, "Destination Y", "touch_y", "", "number");
    out.push_str("<label>Destination visibility<select name=touch_path><option value=unknown>Unrecorded</option><option value=clear>Visible clear path</option><option value=transparent>Visible through transparent barrier</option></select></label>");
    for effect in d.class_play.combat_state.effects.values() {
        input(
            out,
            &format!("Dispel {} on {} d20", effect.name, effect.target),
            &format!("touch_dispel_{}", effect.casting),
            "",
            "number",
        );
    }
    out.push_str("</details><button>Deliver held touch</button></form>");
    begin(out, id, d.class_play.revision, "tactical_object_contact");
    out.push_str("<p>Record an actual incidental touch of an object, including an ineligible object. This is not a free spell dismissal.</p><button>Record incidental object contact</button></form>");
}
