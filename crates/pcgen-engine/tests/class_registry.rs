//! One focused cross-class matrix. Numeric substitution does not certify prestige entry legality.
use pcgen_core::{magic::CastRequest, AdvancementFrame, DraftSpell};
use pcgen_engine::{CharacterDraft, Engine};
use std::{path::Path, sync::OnceLock};
fn engine() -> &'static Engine {
    static E: OnceLock<Engine> = OnceLock::new();
    E.get_or_init(|| {
        Engine::new(
            pcgen_import::native::compile_database(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../data/rules/rainbow-warweaver.rules.json")
                    .as_path(),
            )
            .unwrap(),
        )
    })
}
fn character(class: &str, level: u32) -> CharacterDraft {
    let mut d = CharacterDraft {
        race_id: "Human".into(),
        alignment: if ["Monk", "Paladin"].contains(&class) {
            "LG"
        } else {
            "TN"
        }
        .into(),
        stats: ["STR", "DEX", "CON", "INT", "WIS", "CHA"]
            .map(|s| (s.into(), 20))
            .into_iter()
            .collect(),
        ..Default::default()
    };
    for n in 1..=level {
        d.advancement
            .push(AdvancementFrame::single(n, class, n, 3, 0));
    }
    d
}
fn request(class: &str, spell: &str) -> CastRequest {
    CastRequest {
        class_id: class.into(),
        spell: spell.into(),
        ..Default::default()
    }
}
fn prepared(d: &mut CharacterDraft, class: &str, spell: &str, n: u32) {
    d.spells.push(DraftSpell {
        name: spell.into(),
        class_id: Some(class.into()),
        level: 1,
        times_memorized: n,
        book: Some("Prepared".into()),
    });
}
#[test]
fn class_matrix_and_fixed_list_replacements() {
    let e = engine();
    let core = [
        ("Barbarian", 20),
        ("Bard", 15),
        ("Cleric", 15),
        ("Druid", 15),
        ("Fighter", 20),
        ("Monk", 15),
        ("Paladin", 20),
        ("Ranger", 20),
        ("Rogue", 15),
        ("Sorcerer", 10),
        ("Wizard", 10),
    ];
    for (class, bab20) in core {
        for level in [1, 4, 10, 20] {
            let definition = &e.pack.classes[class];
            if definition.spellcasting_ability.is_some() {
                assert_eq!(
                    definition.spell_power,
                    if ["Bard", "Sorcerer", "Wizard"].contains(&class) {
                        pcgen_core::SpellPowerType::Arcane
                    } else {
                        pcgen_core::SpellPowerType::Divine
                    },
                    "{class} magic source"
                );
            }
            assert_eq!(definition.requires_spellbook, class == "Wizard");
            let d = character(class, level);
            let s = e.evaluate(&d);
            assert!(s.is_available(), "{class} {level}: {:?}", s.diagnostics);
            assert_eq!(s.hit_points, 8 * level as i32, "{class} HP");
            if level == 20 {
                assert_eq!(s.base_attack_bonus, bab20, "{class} BAB");
                match class {
                    "Barbarian" => {
                        assert_eq!(s.variables["RageTimes"], 6);
                        assert_eq!(s.variables["RageConBonus"], 8);
                    }
                    "Rogue" => assert_eq!(s.variables["SneakAttackDice"], 10),
                    "Druid" => {
                        assert_eq!(s.variables["WildShapeTimes"], 6);
                        assert_eq!(s.variables["WildShapeElementalTimes"], 3);
                    }
                    "Fighter" => assert_eq!(s.ability_pools["fighter_feat"], 11),
                    _ => {}
                }
            }
            if ["Paladin", "Ranger"].contains(&class) {
                assert_eq!(
                    s.caster_levels[class],
                    if level < 4 { 0 } else { level as i32 / 2 }
                );
            }
        }
    }
    for class in ["Warmage", "Beguiler", "Dread Necromancer"] {
        let d = character(class, 20);
        let catalog = e.spell_catalog(&d, class).unwrap();
        assert!(catalog.iter().all(|s| s.known), "{class}");
        assert!(catalog.iter().any(|s| s.level == 9), "{class}");
        if class == "Dread Necromancer" {
            assert!(!catalog.iter().any(|s| s.level == 0));
            assert_eq!(catalog.len(), 84);
        }
        if class == "Beguiler" {
            assert_eq!(catalog.len(), 111);
        }
    }
    let mut beg = character("Beguiler", 3);
    beg.choices
        .insert("Beguiler.AdvancedLearning.3".into(), "Magic Aura".into());
    assert!(e
        .spell_catalog(&beg, "Beguiler")
        .unwrap()
        .iter()
        .any(|s| s.spell == "Magic Aura" && s.known));
    beg.choices
        .insert("Beguiler.AdvancedLearning.3".into(), "Magic Missile".into());
    assert!(e.spell_catalog(&beg, "Beguiler").is_err());
    let mut dn = character("Dread Necromancer", 4);
    dn.choices.insert(
        "Dread Necromancer.AdvancedLearning.4".into(),
        "Deathwatch".into(),
    );
    assert!(e
        .spell_catalog(&dn, "Dread Necromancer")
        .unwrap()
        .iter()
        .any(|s| s.spell == "Deathwatch" && s.known));
    let dn = character("Dread Necromancer", 17);
    assert_eq!(
        e.evaluate(&dn).turning.unwrap().mode,
        pcgen_engine::snapshot::TurningMode::Rebuke
    );
    let features = e.class_feature_values(&dn).unwrap();
    assert_eq!(
        features
            .iter()
            .find(|f| f.name == "Enervating Touch")
            .unwrap()
            .daily_uses,
        Some(17)
    );
    assert_eq!(
        features
            .iter()
            .find(|f| f.name == "Lich Body")
            .unwrap()
            .values["damage_reduction"],
        8
    );
    let key = "Dread Necromancer: Negative Energy Burst";
    let dn = character("Dread Necromancer", 3);
    assert_eq!(
        e.class_resources(&dn)
            .unwrap()
            .into_iter()
            .find(|r| r.key == key)
            .unwrap()
            .allowance,
        1
    );
    // Swap only the advancing base track; no Warmage name or stat assumptions should survive.
    for track in ["Beguiler", "Dread Necromancer"] {
        let mut d = character(track, 1);
        for (class, count) in [
            ("Rainbow Servant", 10),
            ("Sacred Exorcist", 1),
            ("War Weaver", 5),
            ("Incantatrix", 3),
        ] {
            for n in 1..=count {
                let mut frame = AdvancementFrame::single(d.total_level() + 1, class, n, 3, 0);
                if e.pack.classes[class]
                    .level_spellcaster_advance
                    .contains_key(&n)
                {
                    frame.lanes[0].advances.push(track.into());
                }
                d.advancement.push(frame);
            }
        }
        d.choices
            .insert("Incantatrix.FocusedStudies".into(), "Evocation".into());
        let s = e.evaluate(&d);
        assert!(s.is_available(), "{track}: {:?}", s.diagnostics);
        assert_eq!(s.caster_levels[track], 19);
        assert!(e
            .spell_catalog(&d, track)
            .unwrap()
            .iter()
            .any(|s| s.spell == "Heal"
                && s.known
                && s.power == pcgen_core::SpellPowerType::Divine));
    }
}
#[test]
fn casting_modes_zero_base_slots_and_cloaked_casting() {
    let e = engine();
    for (class, spell) in [("Paladin", "Bless Weapon"), ("Ranger", "Entangle")] {
        let mut d = character(class, 4);
        prepared(&mut d, class, spell, 1);
        let r = e.cast_preview(&d, &request(class, spell)).unwrap();
        assert_eq!(r.caster_level, 2);
        d.class_play = e.cast_and_record(&d, &request(class, spell)).unwrap().state;
        assert!(
            e.cast_and_record(&d, &request(class, spell)).is_err(),
            "prepared copy spent"
        );
    }
    let mut bard = character("Bard", 2);
    prepared(&mut bard, "Bard", "Charm Person", 0);
    assert!(
        e.cast_preview(&bard, &request("Bard", "Charm Person"))
            .is_ok(),
        "bonus-only acquired slot"
    );
    let cleric = character("Cleric", 1);
    assert!(e
        .spell_catalog(&cleric, "Cleric")
        .unwrap()
        .iter()
        .any(|s| s.spell == "Bless" && s.known));
    assert!(
        e.cast_preview(&cleric, &request("Cleric", "Bless"))
            .is_err(),
        "must prepare"
    );
    let sorcerer = character("Sorcerer", 1);
    assert!(e
        .spell_catalog(&sorcerer, "Sorcerer")
        .unwrap()
        .iter()
        .all(|s| !s.known));
    for level in [2, 8, 14, 20] {
        let beg = character("Beguiler", level);
        let mut r = request("Beguiler", "Color Spray");
        let normal = e.cast_preview(&beg, &r).unwrap();
        r.target_denied_dex = true;
        let cloaked = e.cast_preview(&beg, &r).unwrap();
        assert_eq!(
            cloaked.save_dc - normal.save_dc,
            if level >= 14 { 2 } else { 1 }
        );
        assert_eq!(cloaked.penetration_bonus, if level >= 8 { 2 } else { 0 });
        assert_eq!(cloaked.bypass_sr, level == 20);
    }
    let beg = character("Beguiler", 5);
    let mut r = request("Beguiler", "Color Spray");
    r.metamagic = vec![pcgen_core::magic::SpellRule::Silent];
    assert!(
        e.cast_preview(&beg, &r).is_ok(),
        "automatic Silent Spell bonus feat"
    );
}

fn scene(d: &mut CharacterDraft) {
    use pcgen_core::class_play::*;
    for (id, body, x, allied) in [
        ("self", TargetBody::Living, 0, true),
        ("foe", TargetBody::Living, 1, false),
        ("undead", TargetBody::Undead, -1, true),
    ] {
        d.class_play.actors.insert(
            id.into(),
            SceneActor {
                body,
                creature_type: if body == TargetBody::Undead {
                    "undead"
                } else {
                    "humanoid"
                }
                .into(),
                dead: false,
                allied,
                willing: allied,
                unconscious: false,
                x,
                y: 0,
                space: 1,
                touch_reach_feet: 5,
                path: TargetPath::Clear,
            },
        );
        d.class_play.health.insert(
            id.into(),
            HitPoints {
                maximum: 100,
                current: 60,
                temporary: 0,
                stable: true,
            },
        );
        d.class_play.defenses.insert(
            id.into(),
            SpellDefenses {
                fortitude: 0,
                reflex: 0,
                will: 0,
                spell_resistance: 0,
            },
        );
        d.class_play.encounter.combat.insert(
            id.into(),
            CombatFacts {
                fear_immune: false,
                touch_ac: 10,
                hit_dice: 20,
                intelligence: 10,
                sense_motive: 0,
                base_attack_bonus: 0,
            },
        );
    }
}
#[test]
fn executable_single_class_actions_are_atomic() {
    use pcgen_core::class_play::*;
    let e = engine();
    let mut dn = character("Dread Necromancer", 3);
    scene(&mut dn);
    let mut burst = FeatureRequest {
        class_id: "Dread Necromancer".into(),
        feature: "Negative Energy Burst".into(),
        damage_roll: Some(9),
        ..Default::default()
    };
    let before = dn.class_play.clone();
    assert!(e.resolve_class_feature(&dn, &burst).is_err());
    assert_eq!(dn.class_play, before);
    burst.saves.insert("foe".into(), 1);
    burst.saves.insert("self".into(), 20);
    dn.class_play = e.resolve_class_feature(&dn, &burst).unwrap().state;
    assert_eq!(dn.class_play.health["foe"].current, 51);
    assert_eq!(dn.class_play.health["undead"].current, 69);
    assert_eq!(dn.class_play.health["self"].current, 56);
    assert!(e.resolve_class_feature(&dn, &burst).is_err());
    let mut dn = character("Dread Necromancer", 4);
    scene(&mut dn);
    let mut touch = FeatureRequest {
        class_id: "Dread Necromancer".into(),
        feature: "Charnel Touch".into(),
        target: "undead".into(),
        ..Default::default()
    };
    dn.class_play = e.resolve_class_feature(&dn, &touch).unwrap().state;
    assert_eq!(dn.class_play.health["undead"].current, 62);
    assert!(e.resolve_class_feature(&dn, &touch).is_err());
    dn.class_play = e.advance_ability_time(&dn, 6).unwrap().state;
    touch.target = "foe".into();
    touch.roll = Some(20);
    touch.damage_roll = Some(9);
    dn.class_play = e.resolve_class_feature(&dn, &touch).unwrap().state;
    assert_eq!(dn.class_play.health["foe"].current, 51);
    let mut dn = character("Dread Necromancer", 5);
    scene(&mut dn);
    let mut aura = FeatureRequest {
        class_id: "Dread Necromancer".into(),
        feature: "Fear Aura".into(),
        saves: std::collections::BTreeMap::from([("foe".into(), 20)]),
        ..Default::default()
    };
    dn.class_play = e.resolve_class_feature(&dn, &aura).unwrap().state;
    aura.saves.clear();
    assert!(e.resolve_class_feature(&dn, &aura).is_ok());
    dn.class_play = e.advance_ability_time(&dn, 86400).unwrap().state;
    aura.saves.insert("foe".into(), 1);
    dn.class_play = e.resolve_class_feature(&dn, &aura).unwrap().state;
    assert!(dn
        .class_play
        .encounter
        .conditions
        .iter()
        .any(|c| c.target == "foe" && c.name == "shaken"));
    for (class, level, feature, budget) in [
        ("Paladin", 2, "Lay on Hands", 10),
        ("Monk", 7, "Wholeness of Body", 14),
        ("Dragon Shaman", 6, "Touch of Vitality", 60),
    ] {
        let mut d = character(class, level);
        if class == "Dragon Shaman" {
            d.alignment = "NG".into();
            d.choices
                .insert("Dragon Shaman.Totem".into(), "Gold".into());
        }
        scene(&mut d);
        let r = FeatureRequest {
            class_id: class.into(),
            feature: feature.into(),
            target: "self".into(),
            amount: budget,
            ..Default::default()
        };
        d.class_play = e.resolve_class_feature(&d, &r).unwrap().state;
        assert_eq!(
            d.class_play.health["self"].current,
            100.min(60 + i64::from(budget))
        );
        assert!(e.resolve_class_feature(&d, &r).is_err());
    }
    let mut rage = character("Barbarian", 11);
    scene(&mut rage);
    let base = e.evaluate(&rage);
    let r = FeatureRequest {
        class_id: "Barbarian".into(),
        feature: "Rage".into(),
        ..Default::default()
    };
    rage.class_play = e.resolve_class_feature(&rage, &r).unwrap().state;
    let active = e.evaluate(&rage);
    assert!(active.is_available(), "{:?}", active.diagnostics);
    assert_eq!(active.stats["STR"], 26);
    assert_eq!(active.stats["CON"], 26);
    assert_eq!(active.hit_points, base.hit_points + 33);
    assert_eq!(active.armor_class, base.armor_class - 2);
    assert_eq!(active.saves["Will"], base.saves["Will"] + 3);
    assert_eq!(rage.class_play.health["self"].current, 93);
    rage.class_play = e.end_rage(&rage).unwrap().state;
    assert_eq!(rage.class_play.health["self"].current, 60);
    assert_eq!(e.evaluate(&rage).stats["STR"], 18);
    assert!(e.resolve_class_feature(&rage, &r).is_err());
    rage.class_play = e.end_ability_encounter(&rage).unwrap().state;
    assert_eq!(e.evaluate(&rage).stats["STR"], 20);
    let mut beg = character("Beguiler", 2);
    scene(&mut beg);
    let feint = FeatureRequest {
        class_id: "Beguiler".into(),
        feature: "Surprise Casting".into(),
        target: "foe".into(),
        roll: Some(20),
        opposed_roll: Some(1),
        ..Default::default()
    };
    beg.class_play = e.resolve_class_feature(&beg, &feint).unwrap().state;
    let mut cast = request("Beguiler", "Color Spray");
    cast.target = Some("foe".into());
    assert_eq!(e.cast_preview(&beg, &cast).unwrap().save_dc, 17);
    beg.class_play = e.cast_and_record(&beg, &cast).unwrap().state;
    assert!(beg.class_play.encounter.feints.is_empty());
}
#[test]
fn phb2_classes_and_prestige_entry_use_shared_rules() {
    use pcgen_core::magic::{ArmorContext, ArmorWeight, ShieldWeight};
    let e = engine();
    for (class, bab) in [("Dragon Shaman", 15), ("Duskblade", 20), ("Knight", 20)] {
        let mut d = character(class, 20);
        d.alignment = "LG".into();
        if class == "Dragon Shaman" {
            d.choices
                .insert("Dragon Shaman.Totem".into(), "Gold".into());
        }
        if class == "Knight" {
            d.skills.insert("Ride".into(), 1.0);
        }
        let s = e.evaluate(&d);
        assert!(s.is_available(), "{class}: {:?}", s.diagnostics);
        assert_eq!(s.base_attack_bonus, bab);
        let m = e
            .pack
            .entities
            .iter()
            .find(|r| r.name == class && r.metadata.class_magic.is_some())
            .unwrap()
            .metadata
            .class_magic
            .as_ref()
            .unwrap();
        assert_eq!(m.progression.len(), 20);
    }
    let mut knight = character("Knight", 5);
    knight.alignment = "LG".into();
    knight.skills.insert("Ride".into(), 1.0);
    knight
        .abilities
        .push(("knight_bonus_feat".into(), "Iron Will".into()));
    let k = e.evaluate(&knight);
    assert!(k.is_available(), "{:?}", k.diagnostics);
    assert_eq!(k.ability_pool_usage["knight_bonus_feat"].spent, 1);
    knight.abilities[0].1 = "Heighten Spell".into();
    assert!(!e.evaluate(&knight).is_available());
    let mut dusk = character("Duskblade", 7);
    dusk.spells.push(DraftSpell {
        name: "Color Spray".into(),
        class_id: Some("Duskblade".into()),
        level: 1,
        times_memorized: 0,
        book: None,
    });
    assert_eq!(
        e.spell_catalog(&character("Duskblade", 20), "Duskblade")
            .unwrap()
            .len(),
        71
    );
    assert_eq!(
        e.spell_catalog(&dusk, "Duskblade")
            .unwrap()
            .iter()
            .filter(|s| s.known)
            .count(),
        1
    );
    let mut cast = request("Duskblade", "Color Spray");
    cast.quick_cast = true;
    cast.armor = Some(ArmorContext {
        armor: ArmorWeight::Medium,
        shield: ShieldWeight::Heavy,
        armor_failure: 30,
        shield_failure: 15,
    });
    let preview = e.cast_preview(&dusk, &cast).unwrap();
    assert_eq!(preview.arcane_failure_percent, Some(0));
    assert!(preview.casting_time_note.contains("Quick Cast"));
    dusk.class_play = e.cast_and_record(&dusk, &cast).unwrap().state;
    assert!(e.cast_and_record(&dusk, &cast).is_err());
    for (track, spell) in [
        ("Beguiler", "Color Spray"),
        ("Dread Necromancer", "Chill Touch"),
    ] {
        let mut d = character(track, 1);
        d.skills.insert("Knowledge (Arcana)".into(), 4.0);
        d.active_states.push("visited_couatl_temple".into());
        d.abilities = [
            "Heighten Spell",
            "Earth Sense",
            "Earth Spell",
            "Versatile Spellcaster",
        ]
        .map(|n| ("FEAT".into(), n.into()))
        .into();
        d.abilities.extend([
            ("Flaw".into(), "Inattentive".into()),
            ("Flaw".into(), "Poor Reflexes".into()),
        ]);
        let mut proof = request(track, spell);
        proof.heighten_to = Some(2);
        proof.earth = true;
        proof.versatile = true;
        proof.on_stone_or_unworked_earth = true;
        assert!(
            e.class_entry(&d, "Rainbow Servant", Some(&proof)).is_err(),
            "strict RAW cannot use a nonexistent higher slot"
        );
        proof.earth_uses_virtual_slot = true;
        let report = e.class_entry(&d, "Rainbow Servant", Some(&proof)).unwrap();
        assert!(report.requirements_met && report.conditional && !report.qualified);
        d.skills.insert("Knowledge (Arcana)".into(), 100.0);
        assert!(
            !e.class_entry(&d, "Rainbow Servant", Some(&proof))
                .unwrap()
                .requirements_met
        );
        let mut normal = character(track, 6);
        normal.skills.insert("Knowledge (Arcana)".into(), 4.0);
        normal.active_states.push("visited_couatl_temple".into());
        assert!(
            e.class_entry(&normal, "Rainbow Servant", None)
                .unwrap()
                .qualified
        );
        normal.active_states.clear();
        assert!(
            !e.class_entry(&normal, "Rainbow Servant", None)
                .unwrap()
                .qualified
        );
    }
}

#[test]
fn single_class_raw_necromancy_persistent_afflictions_and_atomic_touch() {
    use pcgen_core::{class_play::*, necromancy::*};
    let e = engine();
    let mut d = character("Dread Necromancer", 12);
    scene(&mut d);
    for target in ["self", "foe", "undead"] {
        let actor = AfflictedActor {
            scores: d.stats.clone(),
            ..Default::default()
        };
        d.class_play = e.save_affliction_actor(&d, target, actor).unwrap().state;
    }
    d.class_play = e
        .arm_necromantic_touch(&d, Some("Filth Fever"), 1)
        .unwrap()
        .state;
    d.class_play = e.arm_necromantic_touch(&d, None, 2).unwrap().state;
    assert_eq!(
        d.class_play
            .resources
            .daily
            .get("Dread Necromancer: Enervating Touch")
            .copied()
            .unwrap_or(0),
        0
    );
    let mut r = FeatureRequest {
        class_id: "Dread Necromancer".into(),
        feature: "Charnel Touch".into(),
        target: "foe".into(),
        roll: Some(1),
        damage_roll: Some(4),
        affliction: AfflictionRolls {
            save: Some(1),
            damage: [("DEX".into(), 3), ("CON".into(), 3)].into(),
            ..Default::default()
        },
        ..Default::default()
    };
    d.class_play = e.resolve_class_feature(&d, &r).unwrap().state;
    assert!(d.class_play.necromancy.armed_disease.is_some());
    d.class_play = e.advance_ability_time(&d, 6).unwrap().state;
    r.roll = Some(20);
    let before = d.clone();
    r.affliction.damage.remove("CON");
    assert!(e.resolve_class_feature(&d, &r).is_err());
    assert_eq!(d, before, "failed transition is atomic");
    r.affliction.damage.insert("CON".into(), 3);
    d.class_play = e.resolve_class_feature(&d, &r).unwrap().state;
    assert_eq!(d.class_play.necromancy.diseases.len(), 1);
    assert_eq!(d.class_play.necromancy.diseases[0].rule.dc, 12);
    assert_eq!(d.class_play.necromancy.negative_levels.len(), 2);
    assert_eq!(
        d.class_play.resources.daily["Dread Necromancer: Enervating Touch"],
        2
    );
    assert_eq!(
        d.class_play.health["foe"].maximum, 50,
        "CON modifier loss: 40 HP; negative levels: 10 HP"
    );
    d.class_play = e.end_ability_encounter(&d).unwrap().state;
    assert_eq!(d.class_play.necromancy.diseases.len(), 1);
    assert!(e
        .resolve_disease_day(
            &d,
            0,
            &AfflictionRolls {
                save: Some(20),
                ..Default::default()
            }
        )
        .is_err());
    for _ in 0..2 {
        d.class_play = e.advance_ability_time(&d, 86400).unwrap().state;
        d.class_play = e
            .resolve_disease_day(
                &d,
                0,
                &AfflictionRolls {
                    save: Some(20),
                    ..Default::default()
                },
            )
            .unwrap()
            .state;
    }
    assert!(d.class_play.necromancy.diseases.is_empty());
    assert_eq!(
        d.class_play.necromancy.actors["foe"].damage["CON"], 3,
        "cure does not heal ability damage"
    );
    let (next, _) = e
        .resolve_negative_level(&d, 0, 20, &Default::default(), None, None, None)
        .unwrap();
    d = next;
    assert_eq!(d.class_play.necromancy.negative_levels.len(), 1);
    assert_eq!(d.class_play.health["foe"].maximum, 55);
}
#[test]
fn single_class_raw_necromancy_control_pools_creation_and_lich_rerolls() {
    use pcgen_core::{class_play::*, necromancy::*};
    let e = engine();
    let mut d = character("Dread Necromancer", 20);
    scene(&mut d);
    for target in ["self", "foe", "undead"] {
        d.class_play = e
            .save_affliction_actor(
                &d,
                target,
                AfflictedActor {
                    scores: d.stats.clone(),
                    ..Default::default()
                },
            )
            .unwrap()
            .state;
    }
    d.class_play
        .encounter
        .combat
        .get_mut("undead")
        .unwrap()
        .hit_dice = 2;
    d.class_play = e.rebuke_undead(&d, 20, [6, 6], &[]).unwrap().state;
    assert_eq!(d.class_play.necromancy.commands["undead"], 2);
    assert_eq!(d.class_play.resources.turning_spent, 1);
    let r = UndeadSpellRequest {
        cast: request("Dread Necromancer", "Control Undead"),
        targets: vec!["undead".into()],
        saves: [("undead".into(), 1)].into(),
        ..Default::default()
    };
    d.class_play = e.resolve_undead_spell(&d, &r).unwrap().state;
    assert_eq!(
        d.class_play.necromancy.spell_bonds["undead"].expires,
        Some(1200)
    );
    d.class_play.actors.get_mut("foe").unwrap().dead = true;
    let r = UndeadSpellRequest {
        cast: request("Dread Necromancer", "Animate Dead"),
        targets: vec!["foe".into()],
        intact_remains: true,
        racial_hit_dice: [("foe".into(), 0)].into(),
        hp_totals: [("foe".into(), 7)].into(),
        onyx_gp: 25,
        ..Default::default()
    };
    d.class_play = e.resolve_undead_spell(&d, &r).unwrap().state;
    assert_eq!(d.class_play.health["foe"].maximum, 9);
    assert_eq!(d.class_play.necromancy.actors["foe"].scores["DEX"], 26);
    assert_eq!(d.class_play.necromancy.animated["foe"].hit_dice, 1);
    d.class_play = e.advance_ability_time(&d, 1200).unwrap().state;
    assert!(d.class_play.necromancy.spell_bonds.is_empty());
    assert!(d.class_play.necromancy.animated.contains_key("foe"));
    assert!(d.class_play.necromancy.commands.contains_key("undead"));
    let (d, _) = e.transform_dread_lich(&d, &vec![7; 20]).unwrap();
    let snap = e.evaluate(&d);
    assert!(snap.is_available(), "{:?}", snap.diagnostics);
    assert!(!snap.stats.contains_key("CON"));
    assert_eq!(snap.hit_points, 140);
    assert_eq!(d.class_play.actors["self"].body, TargetBody::Undead);
}

#[test]
fn single_class_raw_breath_mitigation_and_negative_level_loss() {
    use pcgen_core::{class_play::*, magic::Energy, necromancy::*};
    let e = engine();
    let mut d = character("Dragon Shaman", 6);
    d.alignment = "NG".into();
    d.choices
        .insert("Dragon Shaman.Totem".into(), "Gold".into());
    scene(&mut d);
    for target in ["self", "foe", "undead"] {
        d.class_play.protections.insert(
            target.into(),
            Protections {
                resistance: [(Energy::Fire, 3)].into(),
                immunity: vec![],
                spell_immunity: false,
                evasion: Evasion::None,
                armor_blocks_evasion: false,
                helpless: false,
                bound_or_squeezing: false,
            },
        );
    }
    let r = FeatureRequest {
        class_id: "Dragon Shaman".into(),
        feature: "Breath Weapon".into(),
        target: "east".into(),
        damage_roll: Some(12),
        opposed_roll: Some(2),
        saves: [("foe".into(), 20)].into(),
        ..Default::default()
    };
    d.class_play = e.resolve_class_feature(&d, &r).unwrap().state;
    assert_eq!(
        d.class_play.health["foe"].current, 57,
        "Reflex half then fire resistance"
    );
    assert_eq!(d.class_play.health["undead"].current, 60, "behind origin");
    assert!(e.resolve_class_feature(&d, &r).is_err());
    d.class_play = e.advance_ability_time(&d, 12).unwrap().state;
    assert!(e.resolve_class_feature(&d, &r).is_ok());
    let mut d = character("Dread Necromancer", 12);
    scene(&mut d);
    d.class_play.necromancy.actors.insert(
        "self".into(),
        AfflictedActor {
            scores: d.stats.clone(),
            ..Default::default()
        },
    );
    let original = e.evaluate(&d);
    let key = "Dread Necromancer|6".to_string();
    d.class_play.necromancy.negative_levels.push(NegativeLevel {
        target: "self".into(),
        dc: 21,
        due: 86400,
        lost_slot: Some(key.clone()),
    });
    d.class_play.health.get_mut("self").unwrap().maximum -= 5;
    d.class_play.health.get_mut("self").unwrap().current -= 5;
    let drained = e.evaluate(&d);
    assert_eq!(drained.caster_levels["Dread Necromancer"], 11);
    assert_eq!(drained.spell_slots[&key] + 1, original.spell_slots[&key]);
    assert_eq!(drained.saves["Will"], original.saves["Will"] - 1);
    d.class_play = e.advance_ability_time(&d, 86400).unwrap().state;
    let (d, _) = e
        .resolve_negative_level(&d, 0, 1, &Default::default(), None, None, None)
        .unwrap();
    assert_eq!(d.total_level(), 11);
    assert!(d.class_play.necromancy.negative_levels.is_empty());
    assert_eq!(e.evaluate(&d).caster_levels["Dread Necromancer"], 11);
    assert_eq!(
        d.advancement
            .iter()
            .flat_map(|f| &f.lanes)
            .map(|l| l.experience_gained)
            .sum::<i32>(),
        60500
    );
}
#[test]
fn single_class_raw_channeling_and_knight_challenges() {
    use pcgen_core::{class_play::*, tactics::*};
    let e = engine();
    let mut knight = character("Knight", 6);
    knight.alignment = "LG".into();
    knight.skills.insert("Ride".into(), 1.0);
    scene(&mut knight);
    for target in ["self", "foe", "undead"] {
        knight.class_play = e
            .save_tactical_facts(
                &knight,
                target,
                TacticalFacts {
                    armor_class: 10,
                    challenge_rating: 6.0,
                    language: true,
                    hears: true,
                    fled: false,
                },
            )
            .unwrap()
            .state;
    }
    let before = e.evaluate(&knight).saves["Will"];
    let r = FeatureRequest {
        class_id: "Knight".into(),
        feature: "Fighting Challenge".into(),
        target: "foe".into(),
        ..Default::default()
    };
    knight.class_play = e.resolve_class_feature(&knight, &r).unwrap().state;
    assert_eq!(e.evaluate(&knight).saves["Will"], before + 1);
    assert_eq!(
        e.evaluate(&knight).attack_bonuses["TARGET.foe.TOHIT.MORALE"],
        1
    );
    assert!(e
        .resolve_class_feature(
            &knight,
            &FeatureRequest {
                target: "undead".into(),
                ..r.clone()
            }
        )
        .is_err());
    let mettle = FeatureRequest {
        class_id: "Knight".into(),
        feature: "Test of Mettle".into(),
        saves: [("foe".into(), 1)].into(),
        ..Default::default()
    };
    knight.class_play = e.resolve_class_feature(&knight, &mettle).unwrap().state;
    assert!(knight.class_play.tactics.mettle.contains_key("foe"));
    knight.class_play = e
        .record_challenge_attack(&knight, "undead", "foe")
        .unwrap()
        .state;
    assert!(!knight.class_play.tactics.mettle.contains_key("foe"));
    let mut d = character("Duskblade", 13);
    scene(&mut d);
    prepared(&mut d, "Duskblade", "Shocking Grasp", 0);
    for target in ["self", "foe", "undead"] {
        d.class_play = e
            .save_tactical_facts(
                &d,
                target,
                TacticalFacts {
                    armor_class: 10,
                    challenge_rating: 6.0,
                    language: true,
                    hears: true,
                    fled: false,
                },
            )
            .unwrap()
            .state;
        d.class_play.protections.insert(
            target.into(),
            Protections {
                resistance: Default::default(),
                immunity: vec![],
                spell_immunity: false,
                evasion: Evasion::None,
                armor_blocks_evasion: false,
                helpless: false,
                bound_or_squeezing: false,
            },
        );
    }
    d.class_play = e
        .save_weapon_setup(
            &d,
            pcgen_core::combat::WeaponSetup {
                equipment: "Longsword".into(),
                enhancement: 0,
                material: pcgen_core::combat::WeaponMaterial::Ordinary,
                masterwork: false,
                two_handed: false,
                alignments: vec![],
            },
        )
        .unwrap()
        .state;
    d.class_play = e.save_physical_reduction(&d, "foe", vec![]).unwrap().state;
    let strike = MeleeStrike {
        target: "foe".into(),
        roll: 15,
        weapon_dice_roll: 3,
        confirmation_roll: None,
        critical_dice_roll: None,
        spell: DefenseRequest {
            effect_roll: None,
            dispel_rolls: Default::default(),
            destination: None,
            destination_path: None,
            target: "foe".into(),
            sr_roll: None,
            save_roll: None,
            waive_save: false,
            sr_lowered: false,
            damage_roll: Some(7),
            massive_roll: None,
        },
    };
    let mut r = ChannelRequest {
        cast: request("Duskblade", "Shocking Grasp"),
        full_attack: true,
        strikes: vec![strike.clone(), strike.clone(), strike.clone()],
        ..Default::default()
    };
    d.class_play = e.arcane_channel(&d, &r).unwrap().state;
    assert_eq!(
        d.class_play.health["foe"].current, 29,
        "weapon damage three times, spell once per distinct target"
    );
    assert_eq!(d.class_play.resources.slots_spent["duskblade"][&1], 1);
    d.class_play = e.advance_ability_time(&d, 6).unwrap().state;
    assert!(d.class_play.tactics.channel.is_none());
    r.full_attack = false;
    r.strikes = vec![MeleeStrike {
        roll: 1,
        ..strike.clone()
    }];
    d.class_play = e.arcane_channel(&d, &r).unwrap().state;
    assert!(d.class_play.tactics.channel.is_some());
    let spent = d.class_play.resources.slots_spent.clone();
    r.resume = true;
    r.strikes = vec![strike];
    d.class_play = e.arcane_channel(&d, &r).unwrap().state;
    assert!(d.class_play.tactics.channel.is_none());
    assert_eq!(
        d.class_play.resources.slots_spent, spent,
        "held charge delivery does not cast or pay again"
    );
}

#[test]
fn duskblade_nonenergy_and_physical_dr() {
    use pcgen_core::{class_play::*, combat::*, tactics::*};
    let e = engine();
    let setup = || WeaponSetup {
        equipment: "Longsword".into(),
        enhancement: 0,
        material: WeaponMaterial::Ordinary,
        masterwork: false,
        two_handed: false,
        alignments: vec![],
    };
    let fresh = || {
        let mut d = character("Duskblade", 13);
        scene(&mut d);
        for name in [
            "Vampiric Touch",
            "Chill Touch",
            "Touch of Fatigue",
            "Touch of Idiocy",
            "Ghoul Touch",
            "Dispelling Touch",
        ] {
            prepared(&mut d, "Duskblade", name, 0);
        }
        for target in ["self", "foe", "undead"] {
            d.class_play
                .protections
                .insert(target.into(), Protections::default());
            d.class_play.necromancy.actors.insert(
                target.into(),
                pcgen_core::necromancy::AfflictedActor {
                    scores: d.stats.clone(),
                    ..Default::default()
                },
            );
            d.class_play.tactics.actors.insert(
                target.into(),
                TacticalFacts {
                    armor_class: 10,
                    challenge_rating: 6.0,
                    language: true,
                    hears: true,
                    fled: false,
                },
            );
        }
        d.class_play = e.save_weapon_setup(&d, setup()).unwrap().state;
        d.class_play = e
            .save_physical_reduction(
                &d,
                "foe",
                vec![PhysicalReduction {
                    amount: 20,
                    bypass: vec![],
                }],
            )
            .unwrap()
            .state;
        d
    };
    let strike = |target: &str, damage, save, effect| MeleeStrike {
        target: target.into(),
        roll: 15,
        weapon_dice_roll: 3,
        confirmation_roll: None,
        critical_dice_roll: None,
        spell: DefenseRequest {
            target: target.into(),
            damage_roll: damage,
            save_roll: save,
            effect_roll: effect,
            ..Default::default()
        },
    };
    let channel = |spell: &str, s| ChannelRequest {
        cast: request("Duskblade", spell),
        strikes: vec![s],
        ..Default::default()
    };
    let mut d = fresh();
    d.class_play = e
        .arcane_channel(
            &d,
            &channel("Vampiric Touch", strike("foe", Some(20), None, None)),
        )
        .unwrap()
        .state;
    assert_eq!(
        d.class_play.health["foe"].current, 40,
        "DR prevents weapon damage, not the spell"
    );
    assert_eq!(d.class_play.health["self"].temporary, 20);
    d.class_play = e.advance_ability_time(&d, 3600).unwrap().state;
    assert_eq!(d.class_play.health["self"].temporary, 0);
    let mut d = fresh();
    d.class_play
        .protections
        .get_mut("foe")
        .unwrap()
        .resistance
        .insert(pcgen_core::magic::Energy::Cold, 99);
    d.class_play = e
        .arcane_channel(
            &d,
            &channel("Chill Touch", strike("foe", Some(4), Some(20), None)),
        )
        .unwrap()
        .state;
    assert_eq!(
        d.class_play.health["foe"].current, 56,
        "Chill Touch is negative energy, and Fort only prevents STR damage"
    );
    assert!(d.class_play.necromancy.actors["foe"].damage.is_empty());
    let mut d = fresh();
    d.class_play = e
        .arcane_channel(
            &d,
            &channel("Chill Touch", strike("foe", Some(4), Some(1), None)),
        )
        .unwrap()
        .state;
    assert_eq!(d.class_play.necromancy.actors["foe"].damage["STR"], 1);
    let mut d = fresh();
    d.class_play = e
        .save_physical_reduction(
            &d,
            "undead",
            vec![PhysicalReduction {
                amount: 20,
                bypass: vec![],
            }],
        )
        .unwrap()
        .state;
    d.class_play = e
        .arcane_channel(
            &d,
            &channel("Chill Touch", strike("undead", None, Some(1), Some(2))),
        )
        .unwrap()
        .state;
    assert_eq!(d.class_play.health["undead"].current, 60);
    assert!(d
        .class_play
        .combat_state
        .effects
        .values()
        .any(|e| matches!(e.condition, SpellCondition::Panicked) && e.expires == 90));
    let mut d = fresh();
    d.class_play = e
        .save_weapon_setup(
            &d,
            WeaponSetup {
                enhancement: 1,
                material: WeaponMaterial::Silver,
                ..setup()
            },
        )
        .unwrap()
        .state;
    d.class_play = e
        .save_physical_reduction(
            &d,
            "foe",
            vec![
                PhysicalReduction {
                    amount: 10,
                    bypass: vec![vec![WeaponTrait::Silver, WeaponTrait::Magic]],
                },
                PhysicalReduction {
                    amount: 5,
                    bypass: vec![vec![WeaponTrait::Bludgeoning]],
                },
            ],
        )
        .unwrap()
        .state;
    d.class_play = e
        .arcane_channel(
            &d,
            &channel("Chill Touch", strike("foe", Some(4), Some(20), None)),
        )
        .unwrap()
        .state;
    assert_eq!(
        d.class_play.health["foe"].current, 53,
        "silver+magic bypasses DR10, remaining DR5 reduces weapon8 to3; spell4"
    );
    let mut d = fresh();
    d.class_play = e
        .arcane_channel(
            &d,
            &channel("Touch of Idiocy", strike("foe", None, None, Some(6))),
        )
        .unwrap()
        .state;
    assert_eq!(d.class_play.health["foe"].current, 60);
    assert!(d.class_play.combat_state.effects.values().any(|e| matches!(
        e.condition,
        SpellCondition::AbilityPenalty { amount: 6, .. }
    ) && e.expires == 7800));
    let mut dispel = channel("Dispelling Touch", strike("foe", None, None, None));
    for effect in d.class_play.combat_state.effects.values() {
        dispel.strikes[0]
            .spell
            .dispel_rolls
            .insert(effect.casting.clone(), 20);
    }
    d.class_play = e.arcane_channel(&d, &dispel).unwrap().state;
    assert!(d.class_play.combat_state.effects.is_empty());
    let mut d = fresh();
    d.class_play = e
        .arcane_channel(
            &d,
            &channel("Touch of Fatigue", strike("foe", None, Some(1), None)),
        )
        .unwrap()
        .state;
    assert!(d
        .class_play
        .combat_state
        .effects
        .values()
        .any(|e| matches!(e.condition, SpellCondition::Fatigued) && e.expires == 78));
    d.class_play = e
        .arcane_channel(
            &d,
            &channel("Touch of Fatigue", strike("foe", None, None, None)),
        )
        .unwrap()
        .state;
    assert_eq!(
        d.class_play.combat_state.effects.len(),
        1,
        "Already fatigued: no escalation, duration refresh or extra save"
    );
    d.class_play = e.advance_ability_time(&d, 78).unwrap().state;
    assert!(d.class_play.combat_state.effects.is_empty());
    let mut d = fresh();
    d.class_play = e
        .arcane_channel(
            &d,
            &channel("Ghoul Touch", strike("foe", None, Some(1), Some(2))),
        )
        .unwrap()
        .state;
    assert!(d
        .class_play
        .combat_state
        .effects
        .values()
        .any(|e| matches!(e.condition, SpellCondition::Paralyzed) && e.expires == 24));
    let exposure = d
        .class_play
        .combat_state
        .exposures
        .keys()
        .next()
        .unwrap()
        .clone();
    d.class_play = e.resolve_stench_exposure(&d, &exposure, 1).unwrap().state;
    assert!(d
        .class_play
        .combat_state
        .effects
        .values()
        .any(|e| matches!(e.condition, SpellCondition::Sickened)));
    d.class_play = e.advance_ability_time(&d, 24).unwrap().state;
    assert!(d.class_play.combat_state.effects.is_empty());
    assert!(d.class_play.combat_state.exposures.is_empty());
}

#[test]
fn duskblade_charge_lifecycle() {
    use pcgen_core::{class_play::*, combat::*, tactics::*};
    let e = engine();
    let fresh = |level| {
        let mut d = character("Duskblade", level);
        scene(&mut d);
        for name in ["Chill Touch", "Touch of Fatigue"] {
            prepared(&mut d, "Duskblade", name, 0);
        }
        for target in ["self", "foe", "undead"] {
            d.class_play
                .protections
                .insert(target.into(), Protections::default());
            d.class_play.necromancy.actors.insert(
                target.into(),
                pcgen_core::necromancy::AfflictedActor {
                    scores: d.stats.clone(),
                    ..Default::default()
                },
            );
            d.class_play.tactics.actors.insert(
                target.into(),
                TacticalFacts {
                    armor_class: 10,
                    challenge_rating: 6.0,
                    language: true,
                    hears: true,
                    fled: false,
                },
            );
            d.class_play.combat_state.reductions.insert(
                target.into(),
                vec![PhysicalReduction {
                    amount: 20,
                    bypass: vec![],
                }],
            );
        }
        d.class_play = e
            .save_weapon_setup(
                &d,
                WeaponSetup {
                    equipment: "Longsword".into(),
                    enhancement: 0,
                    material: WeaponMaterial::Ordinary,
                    masterwork: false,
                    two_handed: false,
                    alignments: vec![],
                },
            )
            .unwrap()
            .state;
        d
    };
    let strike = |target: &str, roll, damage, save, effect| MeleeStrike {
        target: target.into(),
        roll,
        weapon_dice_roll: 3,
        confirmation_roll: None,
        critical_dice_roll: None,
        spell: DefenseRequest {
            target: target.into(),
            damage_roll: damage,
            save_roll: save,
            effect_roll: effect,
            ..Default::default()
        },
    };
    let mut d = fresh(3);
    let mut r = ChannelRequest {
        cast: request("Duskblade", "Chill Touch"),
        strikes: vec![strike("foe", 1, None, None, None)],
        ..Default::default()
    };
    d.class_play = e.arcane_channel(&d, &r).unwrap().state;
    let key = d
        .class_play
        .tactics
        .channel
        .as_ref()
        .unwrap()
        .casting
        .clone();
    assert_eq!(
        d.class_play.tactics.channel.as_ref().unwrap().remaining,
        1,
        "Channeling does not inherit Chill Touch's CL uses"
    );
    let spent = d.class_play.resources.slots_spent.clone();
    d.class_play = e.advance_ability_time(&d, 12).unwrap().state;
    assert!(d.class_play.tactics.channel.is_some());
    r.resume = true;
    r.strikes = vec![strike("foe", 15, Some(3), Some(20), None)];
    d.class_play = e.arcane_channel(&d, &r).unwrap().state;
    assert!(d.class_play.tactics.channel.is_none());
    assert!(d.class_play.cast_effects[&key].ended);
    assert_eq!(d.class_play.resources.slots_spent, spent);
    assert!(
        e.resolve_spell_defenses(&d, &key, &r.strikes[0].spell)
            .is_err(),
        "History cannot replay a delivered charge"
    );

    let mut d = fresh(3);
    d.class_play = e
        .cast_and_record(&d, &request("Duskblade", "Chill Touch"))
        .unwrap()
        .state;
    assert_eq!(d.class_play.tactics.channel.as_ref().unwrap().remaining, 3);
    let mut touch = TouchRequest {
        roll: Some(1),
        defense: DefenseRequest {
            target: "foe".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    d.class_play = e.deliver_held_touch(&d, &touch).unwrap().state;
    assert_eq!(d.class_play.tactics.channel.as_ref().unwrap().remaining, 3);
    touch.roll = Some(15);
    touch.defense.damage_roll = Some(3);
    touch.defense.save_roll = Some(20);
    for remaining in [2, 1] {
        d.class_play = e.deliver_held_touch(&d, &touch).unwrap().state;
        assert_eq!(
            d.class_play.tactics.channel.as_ref().unwrap().remaining,
            remaining
        );
    }
    assert_eq!(
        d.class_play.health["foe"].current, 54,
        "Ordinary multi-touch may hit the same target again"
    );
    d.class_play = e.record_charge_object_contact(&d).unwrap().state;
    assert!(d.class_play.tactics.channel.is_none());
    d.class_play = e
        .cast_and_record(&d, &request("Duskblade", "Chill Touch"))
        .unwrap()
        .state;
    let replaced = d
        .class_play
        .tactics
        .channel
        .as_ref()
        .unwrap()
        .casting
        .clone();
    d.class_play = e
        .cast_and_record(&d, &request("Duskblade", "Touch of Fatigue"))
        .unwrap()
        .state;
    assert!(d.class_play.cast_effects[&replaced].ended);
    assert_ne!(
        d.class_play.tactics.channel.as_ref().unwrap().casting,
        replaced
    );

    let mut d = fresh(13);
    r = ChannelRequest {
        cast: request("Duskblade", "Chill Touch"),
        full_attack: true,
        strikes: vec![strike("foe", 1, None, None, None)],
        ..Default::default()
    };
    d.class_play = e.arcane_channel(&d, &r).unwrap().state;
    let key = d
        .class_play
        .tactics
        .channel
        .as_ref()
        .unwrap()
        .casting
        .clone();
    r.resume = true;
    r.full_attack = false;
    r.strikes = vec![strike("foe", 15, Some(3), Some(20), None)];
    let result = e.arcane_channel(&d, &r).unwrap();
    assert!(
        result.facts.iter().any(|s| s.contains("+ 13 vs normal AC")),
        "Split submission must use second iterative attack"
    );
    d.class_play = result.state;
    r.strikes = vec![strike("foe", 15, None, None, None)];
    d.class_play = e.arcane_channel(&d, &r).unwrap().state;
    assert_eq!(
        d.class_play.health["foe"].current, 57,
        "Same full-channel target receives only one spell effect"
    );
    assert!(
        d.class_play
            .tactics
            .channel
            .as_ref()
            .unwrap()
            .attacks_closed
    );
    assert!(
        e.arcane_channel(&d, &r).is_err(),
        "Cannot reset exhausted full-attack sequence"
    );
    r.opportunity = true;
    r.strikes = vec![strike("undead", 15, None, Some(1), Some(1))];
    d.class_play = e.arcane_channel(&d, &r).unwrap().state;
    assert_eq!(
        d.class_play.tactics.channel.as_ref().unwrap().attacks_used,
        3
    );
    assert_eq!(
        d.class_play.tactics.channel.as_ref().unwrap().targets.len(),
        2
    );
    assert!(
        e.arcane_channel(&d, &r).is_err(),
        "One opportunity per round without Combat Reflexes"
    );
    d.class_play = e.advance_ability_time(&d, 6).unwrap().state;
    assert!(d.class_play.tactics.channel.is_none());
    assert!(d.class_play.cast_effects[&key].ended);
    assert!(e.arcane_channel(&d, &r).is_err());

    let mut d = fresh(13);
    r = ChannelRequest {
        cast: request("Duskblade", "Chill Touch"),
        full_attack: true,
        strikes: vec![strike("foe", 1, None, None, None)],
        ..Default::default()
    };
    d.class_play = e.arcane_channel(&d, &r).unwrap().state;
    r.resume = true;
    r.finish_attacks = true;
    r.strikes.clear();
    d.class_play = e.arcane_channel(&d, &r).unwrap().state;
    assert!(
        d.class_play
            .tactics
            .channel
            .as_ref()
            .unwrap()
            .attacks_closed
    );
    assert!(
        !d.class_play.cast_effects[&d.class_play.tactics.channel.as_ref().unwrap().casting].ended,
        "Ending the attack sequence does not prematurely end the round rider"
    );
    d.class_play = e.advance_ability_time(&d, 6).unwrap().state;
    assert!(d.class_play.tactics.channel.is_none());

    let mut d = fresh(13);
    d.class_play
        .defenses
        .get_mut("foe")
        .unwrap()
        .spell_resistance = 17;
    let mut sr_strike = strike("foe", 15, Some(3), Some(20), None);
    sr_strike.spell.sr_roll = Some(1);
    r = ChannelRequest {
        cast: request("Duskblade", "Chill Touch"),
        strikes: vec![sr_strike.clone()],
        ..Default::default()
    };
    d.class_play = e.arcane_channel(&d, &r).unwrap().state;
    assert_eq!(
        d.class_play.health["foe"].current, 60,
        "DR absorbs the weapon; Spell Power does not activate and SR resists the touch"
    );
    assert!(
        d.class_play.tactics.channel.is_none(),
        "SR still discharges a standard channel hit"
    );
    d.class_play = e.save_physical_reduction(&d, "foe", vec![]).unwrap().state;
    let result = e.arcane_channel(&d, &r).unwrap();
    assert!(result.facts.iter().any(|s| s.contains("Spell Power +3")));
    d.class_play = result.state;
    assert_eq!(
        d.class_play.health["foe"].current, 49,
        "Weapon8 then spell3: injury grants Spell Power before SR"
    );
    d.class_play = e.end_ability_encounter(&d).unwrap().state;
    assert!(d.class_play.tactics.wounded_in_melee.is_empty());
}
