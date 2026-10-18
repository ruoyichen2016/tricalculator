//! One focused numeric probe, not an endorsement of early-entry legality.
use pcgen_core::AdvancementFrame;
use pcgen_engine::{CharacterDraft, Engine};
use std::{collections::BTreeMap, path::Path};

#[test]
fn rainbow_native_progression_uses_one_casting_track() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/rules/rainbow-warweaver.rules.json");
    let pack = pcgen_import::native::compile_database(&path).unwrap();
    assert_eq!(pack.classes["Warmage"].hit_die, "d6");
    let engine = Engine::new(pack);
    let mut draft = CharacterDraft {
        race_id: "Human".into(),
        stats: BTreeMap::from([
            ("CON".into(), 14),
            ("CHA".into(), 18),
            ("INT".into(), 14),
            ("WIS".into(), 14),
        ]),
        ..Default::default()
    };
    for (class, levels) in [
        ("Warmage", 1),
        ("Rainbow Servant", 10),
        ("Sacred Exorcist", 1),
        ("War Weaver", 5),
        ("Incantatrix", 3),
    ] {
        for level in 1..=levels {
            let mut frame = AdvancementFrame::single(draft.total_level() + 1, class, level, 3, 0);
            if class != "Warmage" && !(class == "War Weaver" && level == 1) {
                frame.lanes[0].advances.push("Warmage".into());
            }
            draft.advancement.push(frame);
        }
    }
    draft
        .choices
        .insert("Incantatrix.FocusedStudies".into(), "Necromancy".into());
    let result = engine.evaluate(&draft);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.total_level, 20);
    assert_eq!(result.hit_points, 100);
    assert_eq!(result.base_attack_bonus, 8);
    assert_eq!(result.caster_levels["Warmage"], 19);
    assert_eq!(result.spell_slots["Warmage|9"], 4);
    assert!(result.rule_gaps.is_empty(), "{:?}", result.rule_gaps);
    assert_eq!(result.ability_pools["incantatrix_metamagic"], 1);
    assert_eq!(result.ability_pool_usage["incantatrix_metamagic"].spent, 0);
    use pcgen_core::magic::{CastRequest, SpellRule};
    draft.alignment = "NG".into();
    draft.abilities = [
        "Heighten Spell",
        "Earth Sense",
        "Earth Spell",
        "Versatile Spellcaster",
        "Extend Spell",
        "Persistent Spell",
        "Divine Metamagic",
        "Sanctum Spell",
        "Enlarge Spell",
    ]
    .iter()
    .map(|n| ("FEAT".into(), (*n).into()))
    .collect();
    draft.abilities.extend([
        ("Flaw".into(), "Inattentive".into()),
        ("Flaw".into(), "Poor Reflexes".into()),
    ]);
    draft
        .choices
        .insert("Divine Metamagic".into(), "Persistent Spell".into());
    let catalog = engine.spell_catalog(&draft, "Warmage").unwrap();
    assert!(catalog
        .iter()
        .any(|s| s.spell == "Heal" && s.known && s.power == pcgen_core::SpellPowerType::Divine));
    assert!(catalog.iter().any(|s| s.spell == "Dismissal"
        && s.known
        && s.power == pcgen_core::SpellPowerType::Arcane));
    let persistent = CastRequest {
        class_id: "Warmage".into(),
        spell: "Divine Power".into(),
        metamagic: vec![SpellRule::Persistent],
        divine_metamagic: Some(SpellRule::Persistent),
        ..Default::default()
    };
    let cast = engine.cast_preview(&draft, &persistent).unwrap();
    assert_eq!(
        (
            cast.turning_cost,
            cast.required_slot_level,
            cast.effective_level
        ),
        (7, 4, 4)
    );
    assert_eq!(cast.duration.as_deref(), Some("24 hours"));
    let mut exhausted = persistent.clone();
    exhausted.turning_spent = 1;
    assert!(engine.cast_preview(&draft, &exhausted).is_err());
    seed_scene(&mut draft, 4);
    draft.class_play.resources.tapestry = Some(pcgen_core::class_play::TapestryState {
        casting_track: "Warmage".into(),
        members: (1..=4).map(|n| format!("ally{n}")).collect(),
        remaining_seconds: 86400,
        stored: vec![],
    });
    let heal = CastRequest {
        class_id: "Warmage".into(),
        spell: "Heal".into(),
        metamagic: vec![SpellRule::Sanctum],
        tapestry_targets: Some(pcgen_core::magic::TapestryTargets {
            recipients: vec![
                "self".into(),
                "ally1".into(),
                "ally2".into(),
                "ally3".into(),
                "ally4".into(),
            ],
            choice: None,
        }),
        tapestry: true,
        quiescent: true,
        ..Default::default()
    };
    let cast = engine.cast_preview(&draft, &heal).unwrap();
    assert_eq!((cast.effective_level, cast.required_slot_level), (5, 6));
    assert_eq!(cast.range, pcgen_core::magic::SpellRange::Close);
    let mut inside = heal.clone();
    inside.inside_sanctum = true;
    assert!(engine.cast_preview(&draft, &inside).is_err());
    let mut early = draft.clone();
    early.advancement.truncate(1);
    early.abilities.truncate(4);
    early.abilities.extend([
        ("Flaw".into(), "Inattentive".into()),
        ("Flaw".into(), "Poor Reflexes".into()),
    ]);
    early.choices.clear();
    early.skills.insert("Knowledge (Arcana)".into(), 4.0);
    early.active_states.push("visited_couatl_temple".into());
    let proof = CastRequest {
        class_id: "Warmage".into(),
        spell: "Magic Missile".into(),
        heighten_to: Some(2),
        earth: true,
        versatile: true,
        on_stone_or_unworked_earth: true,
        earth_uses_virtual_slot: true,
        ..Default::default()
    };
    let entry = engine
        .class_entry(&early, "Rainbow Servant", Some(&proof))
        .unwrap();
    assert!(
        !entry.qualified && entry.requirements_met && entry.conditional,
        "{entry:?}"
    );
    let cast = entry.casting_proof.unwrap();
    assert_eq!(
        (
            cast.effective_level,
            cast.caster_level,
            cast.paid_slot_level,
            cast.paid_slots
        ),
        (3, 2, 1, 2)
    );
    let mut strict = proof.clone();
    strict.earth_uses_virtual_slot = false;
    assert!(engine.cast_preview(&early, &strict).is_err());
    let mut no_ground = proof.clone();
    no_ground.on_stone_or_unworked_earth = false;
    assert!(engine.cast_preview(&early, &no_ground).is_err());
    early.active_states.clear();
    assert!(
        !engine
            .class_entry(&early, "Rainbow Servant", Some(&proof))
            .unwrap()
            .qualified
    );
    // Rulebook examples: casting-only adjustments do not inflate Metamagic Effect DC.
    use pcgen_core::magic::{MetamagicMode, MetamagicRequest};
    draft.skills.insert("Spellcraft".into(), 23.0);
    let mut request = MetamagicRequest {
        mode: MetamagicMode::Effect,
        spell: "Wall of Fire".into(),
        spell_list: "Wizard".into(),
        existing: vec![SpellRule::Silent],
        apply: SpellRule::Extend,
        inside_sanctum: false,
        roll: None,
        threatened: false,
        distracted: false,
        spent: 0,
        adjacent: true,
        within_effect: false,
        willing_ally: false,
        readied_action: false,
        ongoing_effect: true,
    };
    let effect = engine.metamagic_preview(&draft, &request).unwrap();
    assert_eq!(effect.spellcraft_dc, 33);
    assert!(effect.success);
    assert_eq!(
        (
            effect.uses_per_day,
            effect.uses_remaining_after_attempt,
            effect.additional_spell_slots
        ),
        (5, 4, 0)
    );
    assert_eq!(
        effect.resulting_duration.as_deref(),
        Some("Concentration + 2 * (1 round/level)")
    );
    request.existing.push(SpellRule::Empower);
    assert_eq!(
        engine
            .metamagic_preview(&draft, &request)
            .unwrap()
            .spellcraft_dc,
        39
    );
    request.mode = MetamagicMode::Cooperative;
    assert!(engine.metamagic_preview(&draft, &request).is_err());
    request.willing_ally = true;
    request.readied_action = true;
    assert_eq!(
        engine
            .metamagic_preview(&draft, &request)
            .unwrap()
            .spellcraft_dc,
        42
    );
    request.mode = MetamagicMode::Effect;
    request.existing.clear();
    request.spell = "Divine Power".into();
    request.spell_list = "Cleric".into();
    request.apply = SpellRule::Persistent;
    request.roll = Some(20);
    let attempt = engine.metamagic_preview(&draft, &request).unwrap();
    assert_eq!(attempt.spellcraft_dc, 48);
    assert!(
        !attempt.success,
        "A natural 20 must not bypass Spellcraft DC"
    );
    assert_eq!(attempt.resulting_duration, attempt.original_duration);
    assert_eq!(attempt.uses_remaining_after_attempt, 4);
    request.spent = 5;
    assert!(engine.metamagic_preview(&draft, &request).is_err());
    request.spent = 0;
    request.roll = None;
    request.threatened = true;
    assert!(engine.metamagic_preview(&draft, &request).is_err());
    let mut missing_school = draft.clone();
    missing_school.choices.remove("Incantatrix.FocusedStudies");
    assert!(!engine.evaluate(&missing_school).is_available());
    let banned = pcgen_core::magic::CastRequest {
        class_id: "Warmage".into(),
        spell: "Chill Touch".into(),
        ..Default::default()
    };
    assert!(engine.cast_preview(&draft, &banned).is_err());
    use pcgen_core::magic::{ArmorContext, ArmorWeight, ClassActionRequest, ShieldWeight};
    let mut missile = CastRequest {
        class_id: "Warmage".into(),
        spell: "Magic Missile".into(),
        armor: Some(ArmorContext {
            armor: ArmorWeight::Light,
            shield: ShieldWeight::Light,
            armor_failure: 20,
            shield_failure: 5,
        }),
        ..Default::default()
    };
    let result = engine.cast_preview(&draft, &missile).unwrap();
    assert_eq!(result.arcane_failure_percent, Some(0));
    assert_eq!(result.warmage_edge, Some(2));
    let damage = result.damage.unwrap();
    assert_eq!(damage.packets.len(), 5);
    assert_eq!(
        (damage.packets[0].minimum, damage.packets[0].maximum),
        (4, 7)
    );
    assert_eq!(damage.packets[1].maximum, 5);
    missile.armor.as_mut().unwrap().armor = ArmorWeight::Medium;
    assert_eq!(
        engine
            .cast_preview(&draft, &missile)
            .unwrap()
            .arcane_failure_percent,
        Some(20),
        "Warmage 1 must not gain Warmage 8 armor through caster-level advancement"
    );
    let mut damage_draft = draft.clone();
    damage_draft
        .abilities
        .retain(|(_, n)| n != "Enlarge Spell" && n != "Sanctum Spell");
    damage_draft.abilities.extend([
        ("FEAT".into(), "Empower Spell".into()),
        ("FEAT".into(), "Maximize Spell".into()),
    ]);
    missile.metamagic = vec![SpellRule::Empower, SpellRule::Maximize];
    let damage = engine
        .cast_preview(&damage_draft, &missile)
        .unwrap()
        .damage
        .unwrap();
    assert_eq!(
        (damage.packets[0].minimum, damage.packets[0].maximum),
        (9, 10)
    );
    assert_eq!(
        (damage.packets[1].minimum, damage.packets[1].maximum),
        (6, 7)
    );
    let good = CastRequest {
        class_id: "Warmage".into(),
        spell: "Protection from Evil".into(),
        ..Default::default()
    };
    assert_eq!(engine.cast_preview(&draft, &good).unwrap().caster_level, 20);
    let wings = engine
        .class_action(
            &draft,
            &ClassActionRequest::GrowWings {
                spent_seconds: 540,
                use_seconds: 60,
                armor_or_restrictive_clothing: false,
                grappling: false,
            },
        )
        .unwrap();
    assert!(wings.facts.iter().any(|s| s.contains("0 seconds remain")));
    assert!(engine
        .class_action(
            &draft,
            &ClassActionRequest::GrowWings {
                spent_seconds: 600,
                use_seconds: 1,
                armor_or_restrictive_clothing: false,
                grappling: false
            }
        )
        .is_err());
    let exorcism = engine
        .class_action(
            &draft,
            &ClassActionRequest::Exorcism {
                roll: 10,
                possessor_hit_dice: 5,
                possessor_charisma: 10,
            },
        )
        .unwrap();
    assert!(exorcism.success); // 10 + class 1 + CHA 4 = DC 15
    assert!(
        !engine
            .class_action(
                &draft,
                &ClassActionRequest::Exorcism {
                    roll: 20,
                    possessor_hit_dice: 30,
                    possessor_charisma: 10
                }
            )
            .unwrap()
            .success
    );
    let detect = engine
        .class_action(
            &draft,
            &ClassActionRequest::Detect {
                spell: "Detect Thoughts".into(),
            },
        )
        .unwrap();
    assert!(detect.facts.iter().any(|s| s.contains("DC 16")));
    let mut buffs = vec![heal.clone(), heal.clone()];
    buffs[0].spell = "Bull's Strength".into();
    buffs[0].metamagic.clear();
    let weave = engine.quiescent_preview(&draft, &buffs).unwrap();
    assert_eq!(weave.release_order[0].spell, "Bull's Strength");
    assert_eq!(weave.release_order[1].spell, "Heal");
    assert_eq!(weave.slots_used["warmage"][&6], 1);
    assert!(engine
        .quiescent_preview(&draft, &vec![heal.clone(); 5])
        .is_err());
    let mut invalid = heal.clone();
    invalid
        .tapestry_targets
        .as_mut()
        .unwrap()
        .recipients
        .push("unknown".into());
    assert!(engine.cast_preview(&draft, &invalid).is_err());
    draft.skills.insert("Craft (Weaving)".into(), 6.0);
    assert_eq!(engine.evaluate(&draft).skills["Craft (Weaving)"], 8);
    let mut unknown_skill = draft.clone();
    unknown_skill.skills.insert("Unknown specialty".into(), 6.0);
    assert!(!engine.evaluate(&unknown_skill).is_available());
    // Adding a second arcane class must not duplicate all nineteen advances.
    draft
        .advancement
        .push(AdvancementFrame::single(21, "Wizard", 1, 3, 0));
    let result = engine.evaluate(&draft);
    assert_eq!(result.caster_levels["Warmage"], 19);
    assert_eq!(result.caster_levels["Wizard"], 1);
}

#[test]
fn full_class_level_boundaries() {
    use pcgen_core::magic::{CastRequest, ClassActionRequest, SpellRule};
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/rules/rainbow-warweaver.rules.json");
    let engine = Engine::new(pcgen_import::native::compile_database(&path).unwrap());
    let mut warmage = CharacterDraft::default();
    warmage.stats = BTreeMap::from([
        ("STR".into(), 10),
        ("DEX".into(), 10),
        ("CON".into(), 14),
        ("INT".into(), 14),
        ("WIS".into(), 14),
        ("CHA".into(), 20),
    ]);
    for level in 1..=20 {
        warmage
            .advancement
            .push(AdvancementFrame::single(level, "Warmage", level, 3, 0));
    }
    let request = CastRequest {
        class_id: "Warmage".into(),
        spell: "Fireball".into(),
        metamagic: vec![SpellRule::Maximize],
        sudden: vec![SpellRule::Maximize],
        ..Default::default()
    };
    let cast = engine.cast_preview(&warmage, &request).unwrap();
    assert_eq!(cast.paid_slot_level, 3);
    let mut lower = warmage.clone();
    lower.advancement.truncate(19);
    assert!(engine.cast_preview(&lower, &request).is_err());
    let mut exhausted = request.clone();
    exhausted.sudden_spent.insert("maximize".into(), 1);
    assert!(engine.cast_preview(&warmage, &exhausted).is_err());
    for (class, max) in [
        ("Rainbow Servant", 10),
        ("Sacred Exorcist", 10),
        ("War Weaver", 5),
        ("Incantatrix", 10),
    ] {
        let mut d = warmage.clone();
        d.advancement.truncate(6);
        d.choices
            .insert("Incantatrix.FocusedStudies".into(), "Necromancy".into());
        for level in 1..=max {
            let mut frame = AdvancementFrame::single(d.total_level() + 1, class, level, 3, 0);
            if class != "War Weaver" || level != 1 {
                frame.lanes[0].advances.push("Warmage".into());
            }
            d.advancement.push(frame);
        }
        let snapshot = engine.evaluate(&d);
        assert!(
            snapshot.is_available(),
            "{class}: {:?}",
            snapshot.diagnostics
        );
        assert_eq!(
            engine
                .class_features(&d)
                .iter()
                .find(|f| f.class_id == class)
                .unwrap()
                .level,
            max
        );
        if class == "Sacred Exorcist" {
            assert!(engine
                .class_action(
                    &d,
                    &ClassActionRequest::SacredSpell {
                        spell: "Dispel Evil".into(),
                        spent: 2
                    }
                )
                .is_ok());
            assert!(engine
                .class_action(
                    &d,
                    &ClassActionRequest::SacredSpell {
                        spell: "Dispel Evil".into(),
                        spent: 3
                    }
                )
                .is_err());
        }
        if class == "Incantatrix" {
            d.abilities.push(("FEAT".into(), "Maximize Spell".into()));
            let r = CastRequest {
                class_id: "Warmage".into(),
                spell: "Fireball".into(),
                metamagic: vec![SpellRule::Maximize],
                ..Default::default()
            };
            assert_eq!(engine.cast_preview(&d, &r).unwrap().paid_slot_level, 5);
            let instant = CastRequest {
                instant_metamagic: Some(SpellRule::Maximize),
                ..r
            };
            let result = engine.cast_preview(&d, &instant).unwrap();
            assert_eq!(result.paid_slot_level, 3);
            assert_eq!(result.instant_uses_remaining, 1);
        }
    }
}

#[test]
fn class_awards_air_resources_and_spell_control_transitions() {
    use pcgen_core::class_play::*;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/rules/rainbow-warweaver.rules.json");
    let engine = Engine::new(pcgen_import::native::compile_database(&path).unwrap());
    let base = |warmage: u32, class: &str, levels: u32| {
        let mut d = CharacterDraft::default();
        d.stats = BTreeMap::from([
            ("STR".into(), 10),
            ("DEX".into(), 10),
            ("CON".into(), 14),
            ("INT".into(), 14),
            ("WIS".into(), 14),
            ("CHA".into(), 20),
        ]);
        for l in 1..=warmage {
            d.advancement
                .push(AdvancementFrame::single(l, "Warmage", l, 3, 0));
        }
        for l in 1..=levels {
            let mut f = AdvancementFrame::single(d.total_level() + 1, class, l, 3, 0);
            f.lanes[0].advances.push("Warmage".into());
            d.advancement.push(f);
        }
        if class == "Incantatrix" {
            d.choices
                .insert("Incantatrix.FocusedStudies".into(), "Necromancy".into());
        }
        d
    };
    let mut warmage = base(7, "", 0);
    warmage.abilities = vec![
        ("FEAT".into(), "Extend Spell".into()),
        ("FEAT".into(), "Sudden Empower".into()),
        ("warmage_sudden_7".into(), "Persistent Spell".into()),
    ];
    warmage.feat_acquired_at =
        BTreeMap::from([("Extend Spell".into(), 1), ("Sudden Empower".into(), 3)]);
    let snapshot = engine.evaluate(&warmage);
    assert!(snapshot.is_available(), "{:?}", snapshot.diagnostics);
    assert_eq!(snapshot.ability_pool_usage["feat"].spent, 2);
    assert_eq!(snapshot.ability_pool_usage["warmage_sudden_7"].spent, 1);
    assert_eq!(
        engine.sudden_replacements(&warmage).unwrap()[0]
            .selected
            .as_deref(),
        Some("Persistent Spell")
    );
    warmage.feat_acquired_at.remove("Sudden Empower");
    assert!(!engine.evaluate(&warmage).is_available());
    warmage.feat_acquired_at.insert("Sudden Empower".into(), 7);
    assert!(!engine.evaluate(&warmage).is_available());
    warmage.feat_acquired_at.insert("Sudden Empower".into(), 3);
    warmage.abilities[2].1 = "Iron Will".into();
    assert!(!engine.evaluate(&warmage).is_available());

    let mut air = base(6, "Rainbow Servant", 4);
    let pool = engine.air_pool(&air).unwrap();
    assert_eq!(pool.total, 8);
    assert_eq!(pool.effective_level, None);
    let target = |id: &str, hd: u32| AirTarget {
        id: id.into(),
        hit_dice: hd,
        turning_resistance: 0,
        distance_feet: 20,
        total_cover: false,
        air: true,
        earth: false,
    };
    let req = AirRequest {
        mode: AirMode::RebukeAir,
        roll: 20,
        damage_die_1: 6,
        damage_die_2: 6,
        targets: vec![target("a", 2), target("b", 2)],
        original_turning_check: None,
    };
    assert!(engine.air_attempt(&air, &req).is_err());
    assert_eq!(air.class_play.air_spent, 0);
    air.domain_level_rulings.insert(
        "Air".into(),
        DomainLevelRuling {
            level: 4,
            reason: "Explicit test table ruling".into(),
        },
    );
    air.class_play = engine.air_attempt(&air, &req).unwrap().state;
    assert_eq!(air.class_play.air_spent, 1);
    assert_eq!(air.class_play.air_commands.len(), 2);
    let third = AirRequest {
        targets: vec![target("c", 2)],
        ..req.clone()
    };
    air.class_play = engine.air_attempt(&air, &third).unwrap().state;
    assert!(!air.class_play.air_commands.contains_key("c"));
    assert_eq!(air.class_play.air_spent, 2);
    air.class_play = engine.air_manage(&air, "release", "a", 0).unwrap().state;
    air.class_play = engine.air_attempt(&air, &third).unwrap().state;
    assert!(air.class_play.air_commands.contains_key("c"));
    air.class_play = engine.air_manage(&air, "new_day", "", 0).unwrap().state;
    assert_eq!(air.class_play.air_spent, 0);
    assert_eq!(air.class_play.air_commands.len(), 2);
    air.class_play.air_spent = 8;
    assert!(engine.air_attempt(&air, &req).is_err());

    let mut inc = base(6, "Incantatrix", 8);
    let spell = ControlledSpell {
        spell: "Major Image".into(),
        original_caster: "Original".into(),
        original_caster_level: 10,
        spell_level: 3,
        original_divine: false,
        concentration: true,
        controllable: true,
        dismissible: true,
        remaining_rounds: Some(10),
        concentration_tail_rounds: 3,
        tail_remaining: None,
        variables: BTreeMap::from([("save_dc".into(), "19".into())]),
        controller: SpellController::Original,
        control_mode: None,
        awaiting_reassertion: false,
        ended: None,
    };
    inc.class_play = engine
        .register_spell_effect(&inc, "image", spell.clone())
        .unwrap()
        .state;
    let seize = ControlEvent::Acquire {
        mode: ControlMode::Seize,
        track: "Warmage".into(),
        distance_feet: 30,
        willing: true,
        roll: 1,
        original_roll: 20,
    };
    inc.class_play = engine.spell_control(&inc, "image", &seize).unwrap().state;
    assert_eq!(
        inc.class_play.spells["image"].controller,
        SpellController::Incantatrix
    );
    inc.class_play = engine
        .spell_control(&inc, "image", &ControlEvent::AdvanceTime { rounds: 2 })
        .unwrap()
        .state;
    inc.class_play = engine
        .spell_control(&inc, "image", &ControlEvent::Lapse)
        .unwrap()
        .state;
    assert!(engine
        .spell_control(&inc, "image", &ControlEvent::AdvanceTime { rounds: 1 })
        .is_err());
    inc.class_play = engine
        .spell_control(&inc, "image", &ControlEvent::Reassert { roll: 8 })
        .unwrap()
        .state;
    assert_eq!(
        inc.class_play.spells["image"].controller,
        SpellController::Original
    );
    assert_eq!(inc.class_play.spells["image"].remaining_rounds, Some(8));
    inc.class_play = engine.spell_control(&inc, "image", &seize).unwrap().state;
    inc.class_play = engine
        .spell_control(&inc, "image", &ControlEvent::Lapse)
        .unwrap()
        .state;
    inc.class_play = engine
        .spell_control(&inc, "image", &ControlEvent::Reassert { roll: 1 })
        .unwrap()
        .state;
    assert_eq!(
        inc.class_play.spells["image"].controller,
        SpellController::None
    );
    inc.class_play = engine
        .spell_control(&inc, "image", &ControlEvent::AdvanceTime { rounds: 3 })
        .unwrap()
        .state;
    assert!(inc.class_play.spells["image"].ended.is_some());
    assert!(engine.spell_control(&inc, "image", &seize).is_err());
    let summon = ControlledSpell {
        spell: "Summon Monster IV".into(),
        concentration: false,
        original_divine: true,
        ..spell
    };
    inc.class_play = engine
        .register_spell_effect(&inc, "summon", summon)
        .unwrap()
        .state;
    let snatch = ControlEvent::Acquire {
        mode: ControlMode::Snatch,
        track: "Warmage".into(),
        distance_feet: 30,
        willing: false,
        roll: 20,
        original_roll: 1,
    };
    inc.class_play = engine.spell_control(&inc, "summon", &snatch).unwrap().state;
    let failed = engine
        .spell_control(
            &inc,
            "summon",
            &ControlEvent::Dismiss {
                track: "Warmage".into(),
                roll: 1,
                original_roll: 20,
            },
        )
        .unwrap();
    assert!(!failed.success);
    inc.class_play = failed.state;
    assert_eq!(
        inc.class_play.spells["summon"].controller,
        SpellController::Original
    );
    assert!(inc.class_play.spells["summon"].ended.is_none());
    inc.class_play = engine.spell_control(&inc, "summon", &snatch).unwrap().state;
    inc.class_play = engine
        .spell_control(
            &inc,
            "summon",
            &ControlEvent::Dismiss {
                track: "Warmage".into(),
                roll: 20,
                original_roll: 1,
            },
        )
        .unwrap()
        .state;
    assert_eq!(
        inc.class_play.spells["summon"].ended.as_deref(),
        Some("Dismissed")
    );
    assert_eq!(inc.class_play.spells["summon"].variables["save_dc"], "19");
}

#[test]
fn single_class_resource_lifecycles_and_high_level_spell_list() {
    use pcgen_core::magic::*;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/rules/rainbow-warweaver.rules.json");
    let engine = Engine::new(pcgen_import::native::compile_database(&path).unwrap());
    let base = |class: &str, levels: u32| {
        let mut d = CharacterDraft::default();
        d.stats = BTreeMap::from([
            ("STR".into(), 10),
            ("DEX".into(), 10),
            ("CON".into(), 14),
            ("INT".into(), 18),
            ("WIS".into(), 14),
            ("CHA".into(), 20),
        ]);
        for l in 1..=levels {
            d.advancement
                .push(AdvancementFrame::single(l, class, l, 3, 0));
        }
        d
    };
    let prestige = |class: &str, levels: u32| {
        let mut d = base("Warmage", 6);
        for l in 1..=levels {
            let mut f = AdvancementFrame::single(d.total_level() + 1, class, l, 3, 0);
            if class != "War Weaver" || l > 1 {
                f.lanes[0].advances.push("Warmage".into());
            }
            d.advancement.push(f);
        }
        d.choices
            .insert("Incantatrix.FocusedStudies".into(), "Necromancy".into());
        d
    };
    let warmage = base("Warmage", 20);
    let catalog = engine.spell_catalog(&warmage, "Warmage").unwrap();
    for name in [
        "Implosion",
        "Meteor Swarm",
        "Prismatic Sphere",
        "Wail of the Banshee",
        "Weird",
    ] {
        let s = catalog.iter().find(|s| s.spell == name).unwrap();
        assert_eq!(s.level, 9);
        assert!(s.known);
    }
    assert!(engine
        .class_proficiencies(&warmage)
        .iter()
        .any(|g| g.armor.contains(&ArmorWeight::Medium)));
    let mut casting = warmage.clone();
    let sudden = CastRequest {
        class_id: "Warmage".into(),
        spell: "Fireball".into(),
        metamagic: vec![SpellRule::Empower],
        sudden: vec![SpellRule::Empower],
        ..Default::default()
    };
    casting.class_play = engine.cast_and_record(&casting, &sudden).unwrap().state;
    assert_eq!(casting.class_play.resources.slots_spent["warmage"][&3], 1);
    assert!(engine.cast_and_record(&casting, &sudden).is_err());
    casting.class_play = engine
        .refresh_class_resources(&casting, "spells")
        .unwrap()
        .state;
    assert!(engine.cast_and_record(&casting, &sudden).is_err());
    casting.class_play = engine
        .refresh_class_resources(&casting, "day")
        .unwrap()
        .state;
    assert!(engine.cast_and_record(&casting, &sudden).is_ok());
    let mut trigger = prestige("Incantatrix", 10);
    trigger.abilities.extend([
        ("FEAT".into(), "Craft Wand".into()),
        ("FEAT".into(), "Maximize Spell".into()),
    ]);
    trigger.class_play = engine
        .register_trigger_item(
            &trigger,
            "wand",
            pcgen_core::class_play::TriggerItem {
                powers: BTreeMap::from([(
                    "fire".into(),
                    pcgen_core::class_play::TriggerPower {
                        spell: "Fireball".into(),
                        spell_list: "Wizard".into(),
                        base_charges: 1,
                        heightened_level: None,
                    },
                )]),
                kind: pcgen_core::class_play::TriggerKind::Wand,
                caster_level: 5,
                casting_track: "Warmage".into(),
                charges: 50,
            },
        )
        .unwrap()
        .state;
    trigger.class_play = engine
        .use_trigger_item(&trigger, "wand", "fire", vec![SpellRule::Maximize])
        .unwrap()
        .state;
    assert_eq!(
        trigger.class_play.resources.trigger_items["wand"].charges,
        47
    );
    trigger.class_play = engine
        .refresh_class_resources(&trigger, "day")
        .unwrap()
        .state;
    assert_eq!(
        trigger.class_play.resources.trigger_items["wand"].charges,
        47
    );
    let mut sacred = prestige("Sacred Exorcist", 10);
    sacred
        .choices
        .insert("Sacred Exorcist.ChosenFoe".into(), "undead".into());
    let defense = engine
        .class_action(
            &sacred,
            &ClassActionRequest::SacredDefense {
                target_kind: "undead".into(),
                desecrated: false,
            },
        )
        .unwrap();
    assert_eq!(defense.modifiers["spell_resistance_check"], 3);
    let spell = ClassActionRequest::SacredSpell {
        spell: "Dispel Evil".into(),
        spent: 0,
    };
    for _ in 0..3 {
        sacred.class_play = engine
            .use_class_ability(&sacred, &spell, "", "")
            .unwrap()
            .state;
    }
    assert!(engine.use_class_ability(&sacred, &spell, "", "").is_err());
    sacred.class_play = engine
        .refresh_class_resources(&sacred, "day")
        .unwrap()
        .state;
    assert!(engine.use_class_ability(&sacred, &spell, "", "").is_err());
    sacred.class_play = engine
        .refresh_class_resources(&sacred, "week")
        .unwrap()
        .state;
    assert!(engine.use_class_ability(&sacred, &spell, "", "").is_ok());
    let exorcism = ClassActionRequest::Exorcism {
        roll: 10,
        possessor_hit_dice: 1,
        possessor_charisma: 10,
    };
    sacred.class_play = engine
        .use_class_ability(&sacred, &exorcism, "victim", "ghost")
        .unwrap()
        .state;
    assert_eq!(
        sacred.class_play.resources.exorcised["victim"]["ghost"],
        86400
    );
    sacred.class_play = engine.advance_class_time(&sacred, 86400).unwrap().state;
    assert!(sacred.class_play.resources.exorcised.is_empty());
    let mut rainbow = prestige("Rainbow Servant", 4);
    let wings = ClassActionRequest::GrowWings {
        spent_seconds: 0,
        use_seconds: 240,
        armor_or_restrictive_clothing: false,
        grappling: false,
    };
    rainbow.class_play = engine
        .use_class_ability(&rainbow, &wings, "", "")
        .unwrap()
        .state;
    assert!(engine.use_class_ability(&rainbow, &wings, "", "").is_err());
    let mut inc = prestige("Incantatrix", 3);
    inc.skills.insert("Spellcraft".into(), 1.0);
    inc.abilities.push(("FEAT".into(), "Extend Spell".into()));
    let meta = MetamagicRequest {
        mode: MetamagicMode::Effect,
        spell: "Bull's Strength".into(),
        spell_list: "Wizard".into(),
        existing: vec![],
        apply: SpellRule::Extend,
        inside_sanctum: false,
        roll: Some(1),
        threatened: false,
        distracted: false,
        spent: 0,
        adjacent: true,
        within_effect: false,
        willing_ally: false,
        readied_action: false,
        ongoing_effect: true,
    };
    let r = engine.use_metamagic_feature(&inc, &meta).unwrap();
    assert!(!r.success);
    inc.class_play = r.state;
    assert_eq!(inc.class_play.resources.daily["Metamagic Effect"], 1);
    assert!(!inc
        .class_play
        .resources
        .daily
        .contains_key("Cooperative Metamagic"));
    let mut weaver = base("Warmage", 6);
    for l in 1..=5 {
        let mut f = AdvancementFrame::single(6 + l, "War Weaver", l, 3, 0);
        if l > 1 {
            f.lanes[0].advances.push("Warmage".into());
        }
        weaver.advancement.push(f);
    }
    weaver.skills.insert("Craft (Weaving)".into(), 6.0);
    seed_scene(&mut weaver, 2);
    let weave = ClassActionRequest::WeaveTapestry {
        class_id: "Warmage".into(),
        roll: None,
        threatened: false,
        distracted: false,
        members: vec!["ally1".into(), "ally2".into()],
    };
    weaver.class_play = engine
        .use_class_ability(&weaver, &weave, "", "")
        .unwrap()
        .state;
    let targets = TapestryTargets {
        recipients: vec!["self".into(), "ally1".into(), "ally2".into()],
        choice: Some("warm".into()),
    };
    let cast = CastRequest {
        class_id: "Warmage".into(),
        spell: "Fire Shield, Mass".into(),
        tapestry_targets: Some(targets.clone()),
        ..Default::default()
    };
    weaver.class_play = engine.store_weaving(&weaver, &cast).unwrap().state;
    assert_eq!(weaver.class_play.resources.slots_spent["warmage"][&5], 1);
    assert_eq!(
        weaver
            .class_play
            .resources
            .tapestry
            .as_ref()
            .unwrap()
            .stored[0]
            .range,
        SpellRange::Medium
    );
    let mut changed = targets.clone();
    changed.choice = Some("chill".into());
    assert!(engine.release_weaving(&weaver, &[changed]).is_err());
    let release = targets;
    weaver.class_play = engine.release_weaving(&weaver, &[release]).unwrap().state;
    assert_eq!(weaver.class_play.resources.slots_spent["warmage"][&5], 1);
    assert!(weaver
        .class_play
        .resources
        .tapestry
        .as_ref()
        .unwrap()
        .stored
        .is_empty());
    assert!(engine.release_weaving(&weaver, &[]).is_err());
    weaver.class_play = engine.advance_class_time(&weaver, 86400).unwrap().state;
    assert!(weaver.class_play.resources.tapestry.is_none());
}

fn seed_scene(d: &mut CharacterDraft, allies: u32) {
    use pcgen_core::class_play::*;
    for n in 0..=allies {
        d.class_play.actors.insert(
            if n == 0 {
                "self".into()
            } else {
                format!("ally{n}")
            },
            SceneActor {
                body: TargetBody::Living,
                creature_type: "humanoid".into(),
                dead: false,
                allied: true,
                willing: true,
                unconscious: false,
                x: n as i32,
                y: 0,
                space: 1,
                touch_reach_feet: 5,
                path: TargetPath::Clear,
            },
        );
    }
}

#[test]
fn named_tapestry_geometry_and_spell_trigger_rules() {
    use pcgen_core::{class_play::*, magic::*};
    use pcgen_engine::targeting::distance;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/rules/rainbow-warweaver.rules.json");
    let engine = Engine::new(pcgen_import::native::compile_database(&path).unwrap());
    let mut d = CharacterDraft::default();
    d.stats = BTreeMap::from([
        ("STR".into(), 10),
        ("DEX".into(), 10),
        ("CON".into(), 14),
        ("INT".into(), 18),
        ("WIS".into(), 14),
        ("CHA".into(), 20),
    ]);
    for l in 1..=10 {
        d.advancement
            .push(AdvancementFrame::single(l, "Warmage", l, 3, 0));
    }
    seed_scene(&mut d, 3);
    let a = d.class_play.actors["self"].clone();
    let mut b = a.clone();
    b.x = 1;
    b.y = 1;
    assert_eq!(distance(&a, &b), 5);
    b.x = 2;
    b.y = 2;
    assert_eq!(distance(&a, &b), 15);
    assert!(engine
        .weave_members(&d, &["ally1".into(), "ally1".into()])
        .is_err());
    d.class_play.actors.get_mut("ally1").unwrap().path = TargetPath::TransparentSolid;
    assert!(engine.weave_members(&d, &["ally1".into()]).is_ok());
    d.class_play.actors.get_mut("ally2").unwrap().willing = false;
    assert!(engine.weave_members(&d, &["ally2".into()]).is_err());
    d.class_play.actors.get_mut("ally2").unwrap().unconscious = true;
    assert!(engine.weave_members(&d, &["ally2".into()]).is_ok());
    d.class_play.resources.tapestry = Some(TapestryState {
        casting_track: "Warmage".into(),
        members: vec!["ally1".into(), "ally2".into(), "ally3".into()],
        remaining_seconds: 86400,
        stored: vec![],
    });
    let targets = TapestryTargets {
        recipients: vec!["self".into(), "ally1".into(), "ally2".into()],
        choice: None,
    };
    let r = engine
        .tapestry_targets(
            &d,
            "Bull's Strength",
            &targets,
            SpellRange::Touch,
            None,
            false,
        )
        .unwrap();
    assert_eq!(r.eligible, vec!["self"]);
    assert_eq!(r.rejected.len(), 2);
    assert!(r.rejected.iter().any(|s| s.contains("barrier")));
    assert!(r.rejected.iter().any(|s| s.contains("touch reach")));
    assert_eq!(
        engine
            .tapestry_targets(
                &d,
                "Bull's Strength",
                &targets,
                SpellRange::Touch,
                None,
                true
            )
            .unwrap()
            .eligible
            .len(),
        3
    );
    d.class_play.actors.get_mut("ally1").unwrap().path = TargetPath::Clear;
    d.class_play.actors.get_mut("ally1").unwrap().body = TargetBody::Object;
    let r = engine
        .tapestry_targets(
            &d,
            "Bull's Strength",
            &targets,
            SpellRange::Close,
            Some(50),
            false,
        )
        .unwrap();
    assert!(r.rejected.iter().any(|s| s.contains("objects")));
    assert!(r.eligible.contains(&"ally2".into()));
    d.class_play.actors.get_mut("ally1").unwrap().body = TargetBody::Living;
    d.class_play.actors.get_mut("ally2").unwrap().x = 11;
    let r = engine
        .tapestry_targets(
            &d,
            "Bull's Strength",
            &targets,
            SpellRange::Close,
            Some(50),
            false,
        )
        .unwrap();
    assert!(r.rejected.iter().any(|s| s.contains("outside spell range")));
    d.class_play.actors.get_mut("ally2").unwrap().x = 10;
    assert_eq!(
        engine
            .tapestry_targets(
                &d,
                "Bull's Strength",
                &targets,
                SpellRange::Close,
                Some(50),
                false
            )
            .unwrap()
            .eligible
            .len(),
        3
    );
    let mut mass = targets.clone();
    mass.choice = Some("warm".into());
    assert!(engine
        .tapestry_targets(
            &d,
            "Fire Shield, Mass",
            &mass,
            SpellRange::Medium,
            Some(200),
            false
        )
        .unwrap_err()
        .contains("separation"));
    d.class_play.actors.get_mut("ally2").unwrap().x = 2;
    assert!(engine
        .tapestry_targets(
            &d,
            "Fire Shield, Mass",
            &targets,
            SpellRange::Medium,
            Some(200),
            false
        )
        .is_err());
    // Stored spells consume no second slot; current geometry excludes recipients on release.
    d.class_play
        .resources
        .tapestry
        .as_mut()
        .unwrap()
        .stored
        .push(StoredWeaving {
            spell: "Fire Shield, Mass".into(),
            caster_level: 10,
            save_dc: 20,
            effective_level: 5,
            duration: Some("10 rounds".into()),
            range: SpellRange::Medium,
            range_feet: Some(200),
            metamagic: vec![],
            targets: mass.clone(),
            material_cost_gp: 0,
            xp_cost: 0,
        });
    d.class_play.actors.get_mut("ally1").unwrap().path = TargetPath::Solid;
    let released = engine.release_weaving(&d, &[mass]).unwrap();
    assert!(released.facts.iter().any(|s| s.contains("Excluded ally1")));
    assert!(released.state.resources.tapestry.unwrap().stored.is_empty());
    let wand = TriggerItem {
        powers: BTreeMap::from([(
            "fire".into(),
            TriggerPower {
                spell: "Fireball".into(),
                spell_list: "Wizard".into(),
                base_charges: 1,
                heightened_level: None,
            },
        )]),
        kind: TriggerKind::Wand,
        caster_level: 5,
        casting_track: "Warmage".into(),
        charges: 10,
    };
    let w = engine.item_cast_preview(&d, &wand, "fire", &[]).unwrap();
    assert_eq!(
        (w.caster_level, w.save_dc, w.warmage_edge, w.charges_spent),
        (5, 14, Some(0), 1)
    );
    let mut staff = wand.clone();
    staff.kind = TriggerKind::Staff;
    staff.caster_level = 8;
    let s = engine.item_cast_preview(&d, &staff, "fire", &[]).unwrap();
    assert_eq!(
        (s.caster_level, s.save_dc, s.warmage_edge),
        (10, 18, Some(4))
    );
    d.abilities.push(("FEAT".into(), "Spell Focus".into()));
    d.choices.insert("Spell Focus".into(), "Evocation".into());
    assert_eq!(
        engine
            .item_cast_preview(&d, &staff, "fire", &[])
            .unwrap()
            .save_dc,
        19
    );
    assert_eq!(
        engine
            .item_cast_preview(&d, &wand, "fire", &[])
            .unwrap()
            .save_dc,
        14
    );
    d.abilities
        .push(("FEAT".into(), "Greater Spell Focus".into()));
    d.choices
        .insert("Greater Spell Focus".into(), "Evocation".into());
    assert_eq!(
        engine
            .item_cast_preview(&d, &staff, "fire", &[])
            .unwrap()
            .save_dc,
        20
    );
    assert_eq!(
        engine
            .cast_preview(
                &d,
                &CastRequest {
                    class_id: "Warmage".into(),
                    spell: "Fireball".into(),
                    ..Default::default()
                }
            )
            .unwrap()
            .save_dc,
        20
    );

    assert!(engine
        .item_cast_preview(&d, &wand, "fire", &[SpellRule::Maximize])
        .is_err());
    let mut unavailable = wand.clone();
    unavailable.powers.get_mut("fire").unwrap().spell = "Heal".into();
    unavailable.powers.get_mut("fire").unwrap().spell_list = "Cleric".into();
    unavailable.kind = TriggerKind::Staff;
    unavailable.caster_level = 13;
    assert!(engine
        .item_cast_preview(&d, &unavailable, "fire", &[])
        .unwrap_err()
        .contains("class list"));
    d.class_play = engine
        .register_trigger_item(&d, "wand", wand)
        .unwrap()
        .state;
    d.class_play = engine
        .use_trigger_item(&d, "wand", "fire", vec![])
        .unwrap()
        .state;
    assert_eq!(d.class_play.resources.trigger_items["wand"].charges, 9);
    assert!(d.class_play.resources.slots_spent.is_empty());
    d.class_play
        .resources
        .trigger_items
        .get_mut("wand")
        .unwrap()
        .charges = 0;
    assert!(engine.use_trigger_item(&d, "wand", "fire", vec![]).is_err());
    assert_eq!(d.class_play.resources.trigger_items["wand"].charges, 0);
}

#[test]
fn shared_staff_metamagic_and_initial_defenses() {
    use pcgen_core::{class_play::*, magic::*};
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/rules/rainbow-warweaver.rules.json");
    let engine = Engine::new(pcgen_import::native::compile_database(&path).unwrap());
    let mut d = CharacterDraft::default();
    d.stats = BTreeMap::from([
        ("STR".into(), 10),
        ("DEX".into(), 10),
        ("CON".into(), 14),
        ("INT".into(), 18),
        ("WIS".into(), 14),
        ("CHA".into(), 20),
    ]);
    for l in 1..=6 {
        d.advancement
            .push(AdvancementFrame::single(l, "Warmage", l, 3, 0));
    }
    for l in 1..=10 {
        let mut f = AdvancementFrame::single(6 + l, "Incantatrix", l, 3, 0);
        f.lanes[0].advances.push("Warmage".into());
        d.advancement.push(f);
    }
    d.choices
        .insert("Incantatrix.FocusedStudies".into(), "Necromancy".into());
    for name in [
        "Craft Staff",
        "Empower Spell",
        "Maximize Spell",
        "Extend Spell",
        "Quicken Spell",
        "Spell Penetration",
        "Greater Spell Penetration",
    ] {
        d.abilities.push(("FEAT".into(), name.into()));
    }
    seed_scene(&mut d, 3);
    let mut staff = pcgen_engine::item_casting::item_templates()
        .unwrap()
        .remove("Staff of Fire")
        .unwrap();
    // Owning a staff does not require being able to activate every power.
    staff.powers.insert(
        "heal".into(),
        TriggerPower {
            spell: "Heal".into(),
            spell_list: "Cleric".into(),
            base_charges: 1,
            heightened_level: None,
        },
    );
    d.class_play = engine
        .register_trigger_item(&d, "fire", staff.clone())
        .unwrap()
        .state;
    assert!(engine.item_cast_preview(&d, &staff, "heal", &[]).is_err());
    let extended = engine
        .item_cast_preview(&d, &staff, "Wall of Fire", &[SpellRule::Extend])
        .unwrap();
    assert_eq!(extended.charges_spent, 3);
    assert_eq!(extended.range_feet, Some(260));
    assert_eq!(
        extended.duration.as_deref(),
        Some("Concentration + 2 * (1 round/level)")
    );
    let mut inc5 = d.clone();
    inc5.advancement.truncate(11);
    inc5.abilities
        .retain(|(_, n)| ["Craft Staff", "Empower Spell", "Maximize Spell"].contains(&n.as_str()));
    assert_eq!(
        engine
            .item_cast_preview(
                &inc5,
                &staff,
                "Fireball",
                &[SpellRule::Empower, SpellRule::Maximize]
            )
            .unwrap()
            .charges_spent,
        6
    );
    let both = vec![SpellRule::Empower, SpellRule::Maximize];
    let p = engine
        .item_cast_preview(&d, &staff, "Fireball", &both)
        .unwrap();
    assert_eq!((p.charges_spent, p.caster_level, p.save_dc), (4, 16, 18));
    let packet = &p.damage.as_ref().unwrap().packets[0];
    assert_eq!((packet.minimum, packet.maximum), (71, 96));
    d.class_play = engine
        .use_trigger_item(&d, "fire", "Fireball", both)
        .unwrap()
        .state;
    assert_eq!(d.class_play.resources.trigger_items["fire"].charges, 46);
    let cast = d.class_play.cast_effects.keys().next().unwrap().clone();
    d.class_play = engine
        .use_trigger_item(&d, "fire", "Burning Hands", vec![])
        .unwrap()
        .state;
    assert_eq!(d.class_play.resources.trigger_items["fire"].charges, 45);
    d.class_play = engine
        .use_trigger_item(&d, "fire", "Wall of Fire", vec![])
        .unwrap()
        .state;
    assert_eq!(d.class_play.resources.trigger_items["fire"].charges, 43);
    assert!(d.class_play.resources.slots_spent.is_empty());
    let mut heightened = staff.clone();
    heightened
        .powers
        .get_mut("Fireball")
        .unwrap()
        .heightened_level = Some(5);
    assert_eq!(
        engine
            .item_cast_preview(&d, &heightened, "Fireball", &[])
            .unwrap()
            .save_dc,
        20
    );
    assert!(engine
        .item_cast_preview(
            &d,
            &staff,
            "Fireball",
            &[SpellRule::Empower, SpellRule::Empower]
        )
        .is_err());
    d.class_play
        .resources
        .trigger_items
        .get_mut("fire")
        .unwrap()
        .charges = 2;
    let before = d.class_play.clone();
    assert!(engine
        .use_trigger_item(&d, "fire", "Fireball", vec![SpellRule::Maximize])
        .is_err());
    assert_eq!(d.class_play, before);
    for id in ["ally1", "ally2", "ally3"] {
        d.class_play = engine
            .save_spell_defenses(
                &d,
                id,
                SpellDefenses {
                    fortitude: 0,
                    reflex: 100,
                    will: 0,
                    spell_resistance: 21,
                },
            )
            .unwrap()
            .state;
    }
    for id in ["ally1", "ally2", "ally3"] {
        d.class_play = engine
            .save_encounter_health(
                &d,
                id,
                pcgen_core::class_play::HitPoints {
                    maximum: 200,
                    current: 200,
                    temporary: 0,
                    stable: false,
                },
                pcgen_core::class_play::Protections::default(),
            )
            .unwrap()
            .state;
    }
    let r = DefenseRequest {
        massive_roll: Some(20),
        target: "ally1".into(),
        sr_roll: Some(1),
        save_roll: Some(1),
        waive_save: false,
        sr_lowered: false,
        damage_roll: Some(71),
        ..Default::default()
    };
    // SR 1 + CL 16 + penetration 4 succeeds at equality; save natural 1 fails despite +100.
    d.class_play = engine.resolve_spell_defenses(&d, &cast, &r).unwrap().state;
    let o = &d.class_play.cast_effects[&cast].resolved["0:ally1"];
    assert!(!o.resisted);
    assert_eq!((o.saved, o.damage), (Some(false), Some(71)));
    assert!(engine.resolve_spell_defenses(&d, &cast, &r).is_err());
    let different = DefenseRequest {
        target: "ally2".into(),
        damage_roll: Some(72),
        ..r.clone()
    };
    assert!(engine
        .resolve_spell_defenses(&d, &cast, &different)
        .is_err());
    let r = DefenseRequest {
        target: "ally2".into(),
        save_roll: Some(20),
        ..r.clone()
    };
    d.class_play = engine.resolve_spell_defenses(&d, &cast, &r).unwrap().state;
    assert_eq!(
        d.class_play.cast_effects[&cast].resolved["0:ally2"].damage,
        Some(35)
    );
    d.class_play
        .defenses
        .get_mut("ally3")
        .unwrap()
        .spell_resistance = 41;
    let r = DefenseRequest {
        target: "ally3".into(),
        sr_roll: Some(20),
        save_roll: None,
        ..r
    };
    d.class_play = engine.resolve_spell_defenses(&d, &cast, &r).unwrap().state;
    assert!(d.class_play.cast_effects[&cast].resolved["0:ally3"].resisted);
    assert_eq!(
        d.class_play.cast_effects[&cast].resolved["0:ally3"].damage,
        Some(0)
    );
}

#[test]
fn ongoing_energy_defenses_and_hp() {
    use pcgen_core::{class_play::*, magic::*};
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/rules/rainbow-warweaver.rules.json");
    let engine = Engine::new(pcgen_import::native::compile_database(&path).unwrap());
    let mut d = CharacterDraft::default();
    d.stats = BTreeMap::from([
        ("STR".into(), 10),
        ("DEX".into(), 10),
        ("CON".into(), 14),
        ("INT".into(), 18),
        ("WIS".into(), 10),
        ("CHA".into(), 18),
    ]);
    for l in 1..=6 {
        d.advancement
            .push(AdvancementFrame::single(l, "Warmage", l, 3, 0));
    }
    seed_scene(&mut d, 3);
    for id in ["ally1", "ally2", "ally3"] {
        d.class_play = engine
            .save_spell_defenses(
                &d,
                id,
                SpellDefenses {
                    fortitude: 0,
                    reflex: 0,
                    will: 0,
                    spell_resistance: 30,
                },
            )
            .unwrap()
            .state;
        d.class_play = engine
            .save_encounter_health(
                &d,
                id,
                HitPoints {
                    maximum: 30,
                    current: 4,
                    temporary: 3,
                    stable: false,
                },
                Protections {
                    resistance: BTreeMap::from([(Energy::Acid, 2)]),
                    spell_immunity: true,
                    ..Default::default()
                },
            )
            .unwrap()
            .state;
    }
    let item = TriggerItem {
        kind: TriggerKind::Wand,
        caster_level: 6,
        casting_track: "Warmage".into(),
        charges: 10,
        powers: BTreeMap::from([(
            "acid".into(),
            TriggerPower {
                spell: "Acid Arrow".into(),
                spell_list: "Sorcerer".into(),
                base_charges: 1,
                heightened_level: None,
            },
        )]),
    };
    d.class_play = engine
        .register_trigger_item(&d, "acid", item)
        .unwrap()
        .state;
    d.class_play = engine
        .use_trigger_item(&d, "acid", "acid", vec![])
        .unwrap()
        .state;
    let cast = d.class_play.cast_effects.keys().next().unwrap().clone();
    assert_eq!(d.class_play.cast_effects[&cast].rounds, 3);
    let mut r = DefenseRequest {
        target: "ally1".into(),
        sr_roll: None,
        save_roll: None,
        waive_save: false,
        sr_lowered: false,
        damage_roll: Some(8),
        massive_roll: None,
        ..Default::default()
    };
    d.class_play = engine.resolve_spell_defenses(&d, &cast, &r).unwrap().state;
    assert_eq!(d.class_play.health["ally1"].current, 1); // 8 - resistance 2 - temporary 3
    assert!(engine.resolve_spell_defenses(&d, &cast, &r).is_err());
    d.class_play = engine
        .advance_damage_effect(&d, &cast, false)
        .unwrap()
        .state;
    r.target = "ally2".into();
    assert!(engine.resolve_spell_defenses(&d, &cast, &r).is_err());
    r.target = "ally1".into();
    r.damage_roll = None;
    let before = d.class_play.clone();
    assert!(engine.resolve_spell_defenses(&d, &cast, &r).is_err());
    assert_eq!(before, d.class_play);
    r.damage_roll = Some(8);
    d.class_play = engine.resolve_spell_defenses(&d, &cast, &r).unwrap().state;
    assert_eq!(d.class_play.health["ally1"].current, -5);
    assert!(d.class_play.actors["ally1"].unconscious);
    d.class_play = engine.dying_turn(&d, "ally1", 10).unwrap().state;
    assert!(d.class_play.health["ally1"].stable);
    d.class_play = engine
        .advance_damage_effect(&d, &cast, false)
        .unwrap()
        .state;
    d.class_play = engine.resolve_spell_defenses(&d, &cast, &r).unwrap().state;
    assert!(d.class_play.actors["ally1"].dead);
    d.class_play = engine
        .advance_damage_effect(&d, &cast, false)
        .unwrap()
        .state;
    assert!(engine.resolve_spell_defenses(&d, &cast, &r).is_err());

    // An ordinary single-class Warmage can activate this staff without crafting it.
    let mut fire = pcgen_engine::item_casting::item_templates().unwrap()["Staff of Fire"].clone();
    fire.casting_track = "Warmage".into();
    d.class_play = engine
        .register_trigger_item(&d, "fire", fire)
        .unwrap()
        .state;
    d.class_play = engine
        .use_trigger_item(&d, "fire", "Fireball", vec![])
        .unwrap()
        .state;
    let fire = d
        .class_play
        .cast_effects
        .iter()
        .find(|(_, c)| c.spell == "Fireball")
        .unwrap()
        .0
        .clone();
    let mut r = DefenseRequest {
        target: "ally2".into(),
        sr_roll: Some(20),
        save_roll: Some(20),
        waive_save: false,
        sr_lowered: true,
        damage_roll: None,
        massive_roll: None,
        ..Default::default()
    };
    d.class_play
        .protections
        .get_mut("ally2")
        .unwrap()
        .spell_immunity = false;
    d.class_play.protections.get_mut("ally2").unwrap().evasion = Evasion::Evasion;
    d.class_play = engine.resolve_spell_defenses(&d, &fire, &r).unwrap().state;
    assert_eq!(d.class_play.health["ally2"].current, 4); // evasion needs no damage roll
    let p = d.class_play.protections.get_mut("ally3").unwrap();
    p.spell_immunity = false;
    p.evasion = Evasion::Improved;
    p.resistance.insert(Energy::Fire, 3);
    r.target = "ally3".into();
    r.save_roll = Some(1);
    r.damage_roll = Some(20);
    d.class_play = engine.resolve_spell_defenses(&d, &fire, &r).unwrap().state;
    assert_eq!(d.class_play.health["ally3"].current, 0); // 20 / 2 - resistance 3 - temporary 3
                                                         // The control acquisition itself is covered separately; this isolates damage -> pending check -> lapse.
    let mut focused = d.clone();
    focused.class_play = engine
        .save_encounter_health(
            &focused,
            "self",
            HitPoints {
                maximum: 100,
                current: 100,
                temporary: 0,
                stable: false,
            },
            Protections::default(),
        )
        .unwrap()
        .state;
    focused.class_play = engine
        .save_spell_defenses(
            &focused,
            "self",
            SpellDefenses {
                fortitude: 0,
                reflex: 0,
                will: 0,
                spell_resistance: 0,
            },
        )
        .unwrap()
        .state;
    focused.class_play.spells.insert(
        "held".into(),
        ControlledSpell {
            spell: "Wall of Fire".into(),
            original_caster: "other".into(),
            original_caster_level: 6,
            spell_level: 4,
            original_divine: false,
            concentration: true,
            controllable: true,
            dismissible: true,
            remaining_rounds: None,
            concentration_tail_rounds: 6,
            tail_remaining: None,
            variables: BTreeMap::new(),
            controller: SpellController::Incantatrix,
            control_mode: Some(ControlMode::Seize),
            awaiting_reassertion: false,
            ended: None,
        },
    );
    focused.class_play = engine
        .use_trigger_item(&focused, "acid", "acid", vec![])
        .unwrap()
        .state;
    let next = focused
        .class_play
        .cast_effects
        .iter()
        .find(|(_, c)| c.spell == "Acid Arrow" && !c.ended)
        .unwrap()
        .0
        .clone();
    r.target = "self".into();
    r.damage_roll = Some(8);
    r.save_roll = None;
    focused.class_play = engine
        .resolve_spell_defenses(&focused, &next, &r)
        .unwrap()
        .state;
    let check = focused
        .class_play
        .concentration_checks
        .keys()
        .next()
        .unwrap()
        .clone();
    assert_eq!(focused.class_play.concentration_checks[&check].dc, 18);
    assert!(engine
        .spell_control(&focused, "held", &ControlEvent::Maintain)
        .is_err());
    focused.class_play = engine
        .resolve_damage_concentration(&focused, &check, 1)
        .unwrap()
        .state;
    assert!(focused.class_play.spells["held"].awaiting_reassertion);
    assert_eq!(
        focused.class_play.spells["held"].controller,
        SpellController::None
    );
    assert!(engine
        .resolve_damage_concentration(&focused, &check, 20)
        .is_err());
}
