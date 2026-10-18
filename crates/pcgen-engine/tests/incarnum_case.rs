use pcgen_core::AdvancementFrame;
use pcgen_engine::{incarnum::*, incarnum_case::IncarnumCase, CharacterDraft, Engine};
use std::{collections::BTreeMap, path::Path};
#[test]
fn all_class_levels_before_warforged_extremes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let e = Engine::new(
        pcgen_import::native::compile_database(
            &root.join("data/rules/rainbow-warweaver.rules.json"),
        )
        .unwrap(),
    );
    for (c, max, bab_end, essentia_end) in [
        ("Ranger", 20, 20, 0),
        ("Sneak Attack Fighter", 20, 20, 0),
        ("Totemist", 20, 15, 20),
        ("Umbral Disciple", 10, 7, 7),
        ("Warblade", 20, 20, 0),
        ("Incandescent Champion", 10, 7, 8),
    ] {
        assert_eq!(e.pack.classes[c].max_level, max);
        let m = e
            .pack
            .entities
            .iter()
            .find(|x| x.name == c && x.metadata.class_magic.is_some())
            .unwrap()
            .metadata
            .class_magic
            .as_ref()
            .unwrap();
        let mut d = CharacterDraft {
            race_id: "Human".into(),
            alignment: "NG".into(),
            stats: BTreeMap::from([("CON".into(), 20)]),
            ..Default::default()
        };
        for n in 1..=max {
            d.advancement.push(AdvancementFrame::single(n, c, n, 1, 0));
            if let Some(i) = &m.incarnum {
                assert!(i.at_level(n).is_some(), "{c} {n}");
            }
            if let Some(i) = &m.martial {
                assert!(i.at_level(n).is_some(), "{c} {n}");
            }
        }
        assert_eq!(
            e.evaluate(&d).base_attack_bonus,
            e.pack.progressions[c].bab(max),
            "{c} {max}"
        );
        assert_eq!(e.pack.progressions[c].bab(max), bab_end);
        let s = e.incarnum_snapshot(&d, &IncarnumLoadout::default());
        assert!(s.errors.is_empty(), "{:?}", s.errors);
        assert_eq!(s.pool, essentia_end);
        if c == "Totemist" {
            assert_eq!((s.soulmeld_limit, s.bind_limit), (9, 5));
        }
        if c == "Sneak Attack Fighter" {
            assert!(m.bonus_feats.is_empty());
            assert_eq!(m.progression.last().unwrap().level, 19);
        }
    }
    let c: IncarnumCase = serde_json::from_str(
        &std::fs::read_to_string(root.join("tests/fixtures/forum-builds/warforged-umbral.json"))
            .unwrap(),
    )
    .unwrap();
    let mut pre_entry = c.probe_character(5);
    pre_entry.race_id = "Human".into();
    let entry = e.class_entry(&pre_entry, "Umbral Disciple", None).unwrap();
    assert!(entry
        .checks
        .iter()
        .any(|r| r.requirement.contains("Essentia pool") && r.satisfied == Some(true)));
    let r = e.audit_incarnum_case(&c);
    assert_eq!(
        (
            r.milestones[19].bab,
            r.milestones[19].essentia,
            r.milestones[19].warblade_il
        ),
        (15, 14, Some(11))
    );
    assert_eq!(r.milestones[0].ordinary_attacks, 2);
    assert_eq!(r.milestones[3].ordinary_attacks, 4);
    assert_eq!(r.milestones[14].ordinary_attacks, 8);
    assert_eq!(r.milestones[16].warblade_il, Some(9));
    assert!(r
        .original_training
        .errors
        .iter()
        .any(|s| s.contains("Sudden Leap")));
    assert!(r
        .original_allocation
        .errors
        .iter()
        .any(|s| s.contains("not repeatable")));
    assert!(r.repaired_training.errors.is_empty());
    assert!(r.turn_probe.errors.is_empty(), "{:?}", r.turn_probe.errors);
    assert_eq!(r.turn_probe.attacks.len(), 8);
    assert_eq!(r.turn_probe.attacks[0].sneak_dice, 4);
    assert_eq!(r.turn_probe.attacks[0].craven_damage, 20);
    let d = c.probe_character(20);
    let l = c.loadout.clone();
    let p = c.turn.clone();
    assert_eq!(
        (
            r.allocation_probe.pool,
            r.allocation_probe.meldshaper_level,
            r.allocation_probe.bind_limit
        ),
        (14, 2, 1)
    );
    assert_eq!(
        (
            r.allocation_probe.effects.own_turn_reach_feet,
            r.allocation_probe.effects.threatened_reach_feet
        ),
        (20, 5)
    );
    let mut bad = l.clone();
    bad.investments.insert("Kiss of the Shadows".into(), 5);
    assert!(e
        .incarnum_snapshot(&d, &bad)
        .errors
        .iter()
        .any(|s| s.contains("capacity 4")));
    bad = l.clone();
    bad.investments.insert("Step of the Bodiless".into(), 1);
    assert!(e
        .incarnum_snapshot(&d, &bad)
        .errors
        .iter()
        .any(|s| s.contains("shared pool")));
    bad.investments.insert("Fast Healing".into(), u32::MAX);
    assert!(e.incarnum_snapshot(&d, &bad).allocated > u64::from(u32::MAX));
    bad = l.clone();
    bad.binds.insert("Totem Avatar".into(), "Totem".into());
    assert!(e
        .incarnum_snapshot(&d, &bad)
        .errors
        .iter()
        .any(|s| s.contains("more than one")));
    bad = l.clone();
    bad.shaped.push("Girallon Arms".into());
    assert!(e
        .incarnum_snapshot(&d, &bad)
        .errors
        .iter()
        .any(|s| s.contains("Shaped soulmelds")));
    let mut low = d.clone();
    low.stats.insert("CON".into(), 12);
    assert!(e
        .incarnum_snapshot(&low, &l)
        .errors
        .iter()
        .any(|s| s.contains("CON 13")));
    let mut q = p.clone();
    q.actions = vec![
        IncarnumAction::Blink { feet: 30 },
        IncarnumAction::FullAttack,
    ];
    let x = e.replay_incarnum_turn(&d, &l, &q);
    assert!(x.attacks.is_empty());
    assert!(x.errors[0].contains("ends further actions"));
    q.actions = vec![IncarnumAction::Blink { feet: 31 }];
    assert!(!e.replay_incarnum_turn(&d, &l, &q).errors.is_empty());
    q.actions = vec![
        IncarnumAction::Reallocate {
            investments: l.investments.clone(),
        },
        IncarnumAction::SuddenLeap {
            feet: 5,
            jump_check: 10,
            running_start: false,
        },
    ];
    assert!(e.replay_incarnum_turn(&d, &l, &q).errors[0].contains("swift"));
    q.actions = vec![IncarnumAction::SuddenLeap {
        feet: 10,
        jump_check: 10,
        running_start: false,
    }];
    assert!(e.replay_incarnum_turn(&d, &l, &q).errors[0].contains("Jump"));
    q.actions = vec![
        IncarnumAction::SuddenLeap {
            feet: 5,
            jump_check: 10,
            running_start: false,
        },
        IncarnumAction::FullAttack,
    ];
    assert_eq!(e.replay_incarnum_turn(&d, &l, &q).attacks.len(), 8);
    q = p.clone();
    q.target_feet = 25;
    assert!(e.replay_incarnum_turn(&d, &l, &q).errors[0].contains("reach"));
    q = p.clone();
    q.precision_immune = true;
    assert!(e
        .replay_incarnum_turn(&d, &l, &q)
        .attacks
        .iter()
        .all(|a| a.sneak_dice == 0 && a.craven_damage == 0));
    q = p.clone();
    q.target_concealment = true;
    assert!(e
        .replay_incarnum_turn(&d, &l, &q)
        .attacks
        .iter()
        .all(|a| a.sneak_dice == 0));
    q = p.clone();
    q.slam_available = false;
    assert_eq!(e.replay_incarnum_turn(&d, &l, &q).attacks.len(), 6);
    q = p.clone();
    q.gloves = false;
    assert_eq!(e.replay_incarnum_turn(&d, &l, &q).attacks.len(), 7);
    bad = l.clone();
    bad.binds = BTreeMap::from([("Wormtail Belt".into(), "Totem".into())]);
    assert!(e.replay_incarnum_turn(&d, &bad, &p).errors[0].contains("only attack"));
    bad = l.clone();
    bad.shaped[2] = "Girallon Arms".into();
    bad.binds = BTreeMap::from([("Girallon Arms".into(), "Totem".into())]);
    assert_eq!(e.replay_incarnum_turn(&d, &bad, &p).attacks.len(), 10);
    q = p.clone();
    q.actions = vec![IncarnumAction::DancingMongoose, IncarnumAction::FullAttack];
    assert!(e.replay_incarnum_turn(&d, &l, &q).errors[0].contains("not readied"));
    let mut mongoose = d.clone();
    mongoose
        .martial_training
        .acquisitions
        .retain(|a| a.character_level == 17);
    for a in &mut mongoose.martial_training.acquisitions {
        a.maneuver = match a.maneuver.as_str() {
            "Leading The Charge" => "Blood In The Water",
            "Sudden Leap" => "Dancing Mongoose",
            n => n,
        }
        .into();
    }
    mongoose
        .martial_training
        .acquisitions
        .sort_by_key(|a| match a.maneuver.as_str() {
            "Wolf Fang Strike" => 0,
            "Blood In The Water" => 1,
            "Dancing Mongoose" => 2,
            _ => 3,
        });
    q.readied = ["Wolf Fang Strike", "Dancing Mongoose", "Pouncing Charge"]
        .into_iter()
        .map(String::from)
        .collect();
    assert!(e.martial_training(&mongoose).errors.is_empty());
    let x = e.replay_incarnum_turn(&mongoose, &l, &q);
    assert!(x.errors.is_empty(), "{:?}", x.errors);
    assert_eq!(x.attacks.len(), 10);
    q.actions = vec![
        IncarnumAction::DancingMongoose,
        IncarnumAction::PouncingCharge {
            feet: 20,
            path_clear: true,
        },
    ];
    assert_eq!(
        e.replay_incarnum_turn(&mongoose, &bad, &q).attacks.len(),
        12
    );
    q = p.clone();
    q.actions = vec![IncarnumAction::FastHealingTick];
    q.healing_rounds_spent = 2;
    let x = e.replay_incarnum_turn(&d, &l, &q);
    assert_eq!((x.healing_rounds_remaining, x.healing_this_turn), (0, 0));
    let mut healing = l.clone();
    healing.investments.insert("Fast Healing".into(), 2);
    healing.investments.insert("Blink Shirt".into(), 1);
    assert_eq!(
        e.replay_incarnum_turn(&d, &healing, &q).healing_this_turn,
        1
    );
    q.healing_rounds_spent = 3;
    assert!(!e.replay_incarnum_turn(&d, &l, &q).errors.is_empty());
    q = p.clone();
    q.actions = vec![IncarnumAction::WhiteRavenTactics {
        target_is_self: true,
        feet: 0,
        actor_initiative: 10,
        ally_initiative: 10,
    }];
    assert!(e.replay_incarnum_turn(&d, &l, &q).errors[0].contains("Self-target"));
    q.actions = vec![IncarnumAction::WhiteRavenTactics {
        target_is_self: false,
        feet: 10,
        actor_initiative: 10,
        ally_initiative: 5,
    }];
    assert_eq!(e.replay_incarnum_turn(&d, &l, &q).ally_initiative, Some(9));
    q = p.clone();
    q.soulchilling_used_this_round = true;
    assert_eq!(
        e.replay_incarnum_turn(&d, &l, &q).soulchilling_triggers_max,
        0
    );
    let hits = vec![
        IncarnumHit {
            hit: false,
            actual_damage: 0,
            sneak_applies: false,
            soulchilling_natural_roll: 10,
            soulchilling_fortitude_total: 0,
            staggering_natural_roll: 10,
            staggering_fortitude_total: 0,
        },
        IncarnumHit {
            hit: true,
            actual_damage: 30,
            sneak_applies: true,
            soulchilling_natural_roll: 10,
            soulchilling_fortitude_total: 20,
            staggering_natural_roll: 10,
            staggering_fortitude_total: 30,
        },
        IncarnumHit {
            hit: true,
            actual_damage: 10,
            sneak_applies: true,
            soulchilling_natural_roll: 10,
            soulchilling_fortitude_total: 0,
            staggering_natural_roll: 10,
            staggering_fortitude_total: 9,
        },
    ];
    let debuffs = resolve_incarnum_debuffs(&r.allocation_probe, &hits, true, false, false).unwrap();
    assert_eq!(
        (
            debuffs.soulchilling_saves,
            debuffs.strength_damage,
            debuffs.staggering_saves
        ),
        (1, 0, 2)
    );
    assert!(debuffs.staggered);
    assert_eq!(
        resolve_incarnum_debuffs(&r.allocation_probe, &hits, true, false, true)
            .unwrap()
            .soulchilling_saves,
        0
    );
    let mut critical_save = hits[1].clone();
    critical_save.actual_damage = u32::MAX;
    critical_save.staggering_natural_roll = 20;
    critical_save.soulchilling_natural_roll = 1;
    let x = resolve_incarnum_debuffs(&r.allocation_probe, &[critical_save], true, false, false)
        .unwrap();
    assert!(!x.staggered);
    assert_eq!(x.strength_damage, 2);
    bad.investments.remove("Sight of the Eyeless");
    bad.investments.remove("Embrace of Shadow");
    bad.investments.insert("Girallon Arms".into(), 4);
    let x = e.replay_incarnum_turn(&d, &bad, &p);
    assert!(x.errors.is_empty(), "{:?}", x.errors);
    assert!(x
        .attacks
        .iter()
        .filter(|a| a.weapon == "Girallon claw")
        .all(|a| a.incarnum_damage == 6));
}
