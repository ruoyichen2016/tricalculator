use pcgen_core::{
    magic::{CastRequest, SpellProfile, SpellRange},
    AdvancementFrame, DraftSpell, SpellPowerType,
};
use pcgen_engine::{joker::JokerCase, spell_features::*, CharacterDraft, Engine};
use std::{collections::BTreeMap, path::Path};
fn character(class: &str, levels: u32) -> CharacterDraft {
    let mut d = CharacterDraft {
        race_id: "Human".into(),
        alignment: "LG".into(),
        stats: BTreeMap::from([
            ("CHA".into(), 24),
            ("WIS".into(), 12),
            ("INT".into(), 16),
            ("CON".into(), 14),
        ]),
        ..Default::default()
    };
    for n in 1..=levels {
        d.advancement
            .push(AdvancementFrame::single(n, class, n, 0, 0));
    }
    d
}
#[test]
fn all_class_levels_before_joker_extremes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let e = Engine::new(
        pcgen_import::native::compile_database(
            &root.join("data/rules/rainbow-warweaver.rules.json"),
        )
        .unwrap(),
    );
    for (class, max, advanced) in [
        ("Favored Soul", 20, 0),
        ("Church Inquisitor", 10, 10),
        ("Exalted Arcanist", 5, 4),
        ("War Weaver", 5, 4),
        ("Spellguard of Silverymoon", 5, 5),
        ("Sacred Exorcist", 10, 10),
        ("Incantatrix", 10, 10),
    ] {
        let c = &e.pack.classes[class];
        assert_eq!(c.max_level, max);
        assert_eq!(c.level_spellcaster_advance.len(), advanced);
        if class == "Favored Soul" {
            for n in 1..=max {
                assert!(c.spell_slots.contains_key(&n));
                assert!(c.spells_known.contains_key(&n));
                assert_eq!(c.spell_slots[&n].len(), c.spells_known[&n].len());
            }
        }
        for n in [1, max] {
            let d = character(class, n);
            assert_eq!(
                e.evaluate(&d).base_attack_bonus,
                e.pack.progressions[class].bab(n),
                "{class} {n}"
            );
        }
    }
    let mut fs = character("Favored Soul", 18);
    assert_eq!(
        e.pack.classes["Favored Soul"].spell_power,
        SpellPowerType::Divine
    );
    let list = e.spell_catalog(&fs, "Favored Soul").unwrap();
    assert!(list
        .iter()
        .any(|s| s.spell == "Miracle" && s.level == 9 && !s.known));
    fs.spells.push(DraftSpell {
        name: "Zone of Truth".into(),
        level: 2,
        class_id: Some("Favored Soul".into()),
        times_memorized: 0,
        book: None,
    });
    let cast = e
        .cast_preview(
            &fs,
            &CastRequest {
                class_id: "Favored Soul".into(),
                spell: "Zone of Truth".into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(e.evaluate(&fs).spell_dcs["Zone of Truth|Favored Soul"], 13);
    assert_eq!(cast.save_dc, 13); // WIS +1, not CHA +7.
    let case: JokerCase = serde_json::from_str(include_str!(
        "../../../tests/fixtures/forum-builds/joker.json"
    ))
    .unwrap();
    let report = e.audit_joker_case(&case);
    assert!(report.errors.iter().any(|e| e.contains("cap is 8")));
    assert!(report.errors.iter().any(|e| e.contains("Level 15")));
    let last = report.milestones.last().unwrap();
    assert_eq!(last.favored_soul_casting_level, 6);
    assert_eq!(last.rejected_advancements.len(), 12);
    // A low-level FS on a multiclass character must not lend the Cleric list to a sorcerer.
    let mut mixed = character("Sorcerer", 8);
    mixed
        .advancement
        .push(AdvancementFrame::single(9, "Favored Soul", 1, 0, 0));
    assert!(!e
        .spell_catalog(&mixed, "Sorcerer")
        .unwrap()
        .iter()
        .any(|s| s.spell == "Cure Light Wounds"));
    for (name, ok) in [
        ("Shield", true),
        ("False Life", true),
        ("True Strike", false),
    ] {
        let p = &e
            .pack
            .spells
            .values()
            .find(|s| s.name == name)
            .unwrap()
            .profile;
        assert_eq!(spellguard_range(4, SpellPowerType::Arcane, p).is_ok(), ok);
        assert!(spellguard_range(3, SpellPowerType::Arcane, p).is_err());
        assert!(spellguard_range(4, SpellPowerType::Divine, p).is_err());
    }
    // Full cast path, not just the conversion helper.
    let mut arcane = character("Sorcerer", 8);
    for n in 1..=4 {
        let mut f = AdvancementFrame::single(8 + n, "Spellguard of Silverymoon", n, 0, 0);
        f.lanes[0].advances.push("Sorcerer".into());
        arcane.advancement.push(f);
    }
    arcane.spells.push(DraftSpell {
        name: "Shield".into(),
        level: 1,
        class_id: Some("Sorcerer".into()),
        times_memorized: 0,
        book: None,
    });
    let cast = e
        .cast_preview(
            &arcane,
            &CastRequest {
                class_id: "Sorcerer".into(),
                spell: "Shield".into(),
                spellguard_touch: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(cast.range, SpellRange::Touch);
    use pcgen_core::class_play::*;
    for n in 1..=5 {
        let mut f = AdvancementFrame::single(12 + n, "War Weaver", n, 0, 0);
        if n > 1 {
            f.lanes[0].advances.push("Sorcerer".into());
        }
        arcane.advancement.push(f);
    }
    for (id, x) in [("self", 0), ("ally", 1)] {
        arcane.class_play.actors.insert(
            id.into(),
            SceneActor {
                body: TargetBody::Living,
                creature_type: "humanoid".into(),
                dead: false,
                allied: true,
                willing: true,
                unconscious: false,
                x,
                y: 0,
                space: 1,
                touch_reach_feet: 5,
                path: TargetPath::Clear,
            },
        );
    }
    arcane.class_play.resources.tapestry = Some(TapestryState {
        casting_track: "Sorcerer".into(),
        members: vec!["ally".into()],
        remaining_seconds: 86400,
        stored: vec![],
    });
    let request = CastRequest {
        class_id: "Sorcerer".into(),
        spell: "Shield".into(),
        spellguard_touch: true,
        tapestry: true,
        tapestry_targets: Some(pcgen_core::magic::TapestryTargets {
            recipients: vec!["ally".into()],
            choice: None,
        }),
        ..Default::default()
    };
    assert_eq!(
        e.cast_preview(&arcane, &request).unwrap().range,
        SpellRange::Close
    );
    let mut personal = request.clone();
    personal.spellguard_touch = false;
    assert!(e.cast_preview(&arcane, &personal).is_err());
    let benefit = deity_favor(4, true, "Favored Soul", 6, 4, 1, 0, 0).unwrap();
    assert_eq!(benefit.temporary_hp, 18);
    assert_eq!(benefit.duration_rounds, 60);
    assert!(deity_favor(4, true, "Favored Soul", 6, 4, 4, 0, 0).is_err());
    assert!(deity_favor(12, true, "Favored Soul", 6, 4, 4, 0, 0).is_ok());
    assert!(deity_favor(4, true, "Sorcerer", 6, 1, 1, 0, 0).is_err());
    let benefit = deity_favor(4, true, "Favored Soul", 2, 1, 1, 20, 7).unwrap();
    assert_eq!((benefit.temporary_hp, benefit.duration_rounds), (20, 7));
    assert!(spellguard_range(
        4,
        SpellPowerType::Arcane,
        &SpellProfile {
            range: SpellRange::Personal,
            ..Default::default()
        }
    )
    .is_err());
}
