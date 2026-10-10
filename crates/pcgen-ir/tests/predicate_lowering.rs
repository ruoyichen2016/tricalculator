use pcgen_core::{Predicate, SpellPowerType};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/pred.lst", 1, "pred.pcc");
    RawRecord {
        category: category.to_string(),
        key: name.to_string(),
        name: name.to_string(),
        tags: tags
            .iter()
            .map(|(n, v)| RawTag {
                name: (*n).to_string(),
                value: (*v).to_string(),
                source: source.clone(),
            })
            .collect(),
        source,
        raw: String::new(),
    }
}

#[test]
fn bonus_with_size_predicate_lowers_to_size_bounds() {
    // A size-gated STAT bonus must now lower (not be unsupported) and carry
    // a size predicate.
    let record = record(
        "RACE",
        "Tiny Fey",
        &[("BONUS", "STAT|STR|-2|PREBASESIZELT:Tiny")],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert_eq!(pack.entities[0].effects.len(), 1);
    let condition = pack.entities[0].effects[0]
        .condition
        .clone()
        .expect("size condition");
    // Strictly-smaller than Tiny => <= Tiny AND NOT >= Tiny.
    match condition {
        Predicate::All(items) => {
            assert!(items
                .iter()
                .any(|p| matches!(p, Predicate::SizeAtMost { .. })));
            assert!(items.iter().any(|p| matches!(p, Predicate::Not(_))));
        }
        other => panic!("expected All, got {other:?}"),
    }
}

#[test]
fn prerule_bonus_lowers_to_rule_enabled() {
    let record = record(
        "RACE",
        "Dwarf",
        &[(
            "BONUS",
            "SKILL|Climb|8|TYPE=Racial|PRERULE:1,SYS_DWS_RACIAL",
        )],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    let condition = pack.entities[0].effects[0]
        .condition
        .clone()
        .expect("rule condition");
    assert!(matches!(condition, Predicate::RuleEnabled { .. }));
}

#[test]
fn pretext_is_a_vacuous_gate_and_does_not_discard_the_bonus() {
    // PCGen's reference: "This tag in itself does not disqualify anything but
    // alerts the player to the additional pre-requisites." Reading it as an
    // unknown tag made `parse_predicate` return `None`, which discarded the
    // whole effect — 7,358 occurrences across the third-party packs.
    let record = record(
        "RACE",
        "Dwarf",
        &[(
            "BONUS",
            "SKILL|Diplomacy|4|PRETEXT:Mind-affecting Spells Only",
        )],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert_eq!(pack.entities[0].effects.len(), 1, "the bonus must survive");
    let condition = pack.entities[0].effects[0]
        .condition
        .clone()
        .expect("condition");
    // Vacuously true: an empty conjunction holds for every character.
    assert!(
        matches!(condition, Predicate::All(ref items) if items.is_empty()),
        "expected a vacuous gate, got {condition:?}"
    );
}

#[test]
fn unreadable_bonus_gate_keeps_the_effect_and_is_reported() {
    // An unknown gate must not make `lower_bonus_effect` return `None`: the
    // bonus is retained with an explicit `Unsupported` condition, and the raw
    // text is still counted as unsupported work rather than disappearing.
    let record = record(
        "RACE",
        "Alchemy Dabbler",
        &[(
            "BONUS",
            "VAR|AlchemyFeat|1|PRESKILLTOT:Alchemy=7|TYPE=NoStack",
        )],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 1);
    assert_eq!(pack.entities[0].effects.len(), 1, "the bonus must survive");
    let condition = pack.entities[0].effects[0]
        .condition
        .clone()
        .expect("the unreadable gate must be preserved");
    assert!(
        matches!(condition, Predicate::Unsupported { ref raw } if raw == "PRESKILLTOT:Alchemy=7"),
        "expected an Unsupported condition, got {condition:?}"
    );
    assert!(
        pack.entities[0]
            .unsupported_tags
            .iter()
            .any(|tag| tag.contains("PRESKILLTOT:Alchemy=7")),
        "raw gate must remain reportable: {:?}",
        pack.entities[0].unsupported_tags
    );
}

#[test]
fn legs_predicate_lowers() {
    let record = record(
        "RACE",
        "Quadruped",
        &[("BONUS", "LOADMULT|TYPE=SIZE|0.25|PRELEGSGTEQ:4")],
    );
    let (_pack, report) =
        lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
}

#[test]
fn size_ordering_is_canonical() {
    assert!(pcgen_core::size_at_least("Large", "Medium"));
    assert!(!pcgen_core::size_at_least("Small", "Medium"));
    assert!(pcgen_core::size_at_most("Small", "Medium"));
    assert!(!pcgen_core::size_at_most("Huge", "Medium"));
    // Unknown sizes rank as Medium.
    assert!(pcgen_core::size_at_least("Bogus", "Small"));
    assert!(!pcgen_core::size_at_least("Bogus", "Large"));
}

#[test]
fn premult_lowers_to_at_least() {
    // PREMULT:1,[A],[B] => AtLeast{1,[A,B]}
    let record = record(
        "EQUIPMOD",
        "Enhancement",
        &[("PREMULT", "1,[PRETYPE:1,Enhancement],[PRETYPE:1,Armor]")],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::AtLeast { n: 1, items } if items.len() == 2
    ));
}

#[test]
fn premult_with_all_groups_lowers_to_all() {
    // PREMULT:2,[A],[B] with exactly two groups => All([A,B])
    let record = record(
        "EQUIPMOD",
        "Both",
        &[("PREMULT", "2,[PRETYPE:1,Enhancement],[PRETYPE:1,Weapon]")],
    );
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::All(items) if items.len() == 2
    ));
}

#[test]
fn premult_handles_nesting_and_negation() {
    let record = record(
        "RACE",
        "Nested",
        &[(
            "PREMULT",
            "1,[PREMULT:2,[PRETYPE:1,Enhancement],[PRETYPE:1,Armor]],[!PRETYPE:1,Mundane]",
        )],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    match &pack.entities[0].requirements[0] {
        Predicate::AtLeast { n: 1, items } => {
            assert_eq!(items.len(), 2);
            assert!(matches!(items[0], Predicate::All(_)));
            assert!(matches!(items[1], Predicate::Not(_)));
        }
        other => panic!("expected AtLeast, got {other:?}"),
    }
}

#[test]
fn racetype_and_age_predicates_lower() {
    let race_record = record("RACE", "Elf", &[("PRERACETYPE", "Humanoid")]);
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["pred".to_string()],
        &[race_record],
    );
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::RaceType { .. }
    ));

    let age_record = record("RACE", "Old Elf", &[("PREAGESET", "1,Middle Age")]);
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["pred".to_string()],
        &[age_record],
    );
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::AgeCategory { .. }
    ));
}

#[test]
fn prevarneq_lowers_to_negated_variable() {
    let record = record(
        "ABILITY",
        "Turn Undead",
        &[("PREVARNEQ", "TurnProhibitedByDeity,1")],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    match &pack.entities[0].requirements[0] {
        Predicate::Not(inner) => {
            assert!(matches!(
                **inner,
                Predicate::Variable {
                    min: Some(1),
                    max: Some(1),
                    ..
                }
            ));
        }
        other => panic!("expected Not(Variable), got {other:?}"),
    }
}

#[test]
fn premult_with_prepclevel_lowers() {
    let record = record(
        "KIT",
        "Wizard Spell Book",
        &[("PREMULT", "2,[PREPCLEVEL:MAX=1],[PRECLASS:1,Wizard=1]")],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    match &pack.entities[0].requirements[0] {
        Predicate::All(items) => {
            assert_eq!(items.len(), 2);
            assert!(matches!(
                items[0],
                Predicate::CharacterClassLevel {
                    min: None,
                    max: Some(1)
                }
            ));
            // `PRECLASS` lowers to a count plus a list of class requirements.
            assert!(matches!(
                items[1],
                Predicate::ClassRequirement { ref specs, min_count: 1 } if specs.len() == 1
            ));
        }
        other => panic!("expected All([...]), got {other:?}"),
    }
}

#[test]
fn pretotalab_lowers_to_total_bab() {
    let record = record("ABILITY", "Weapon Focus", &[("PRETOTALAB", "1")]);
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::TotalBab { min: 1 }
    ));
}

#[test]
fn negated_top_level_predicates_lower() {
    // PCGen writes boolean prerequisites in both `PREX` and `!PREX` form.
    // The negated form used to be an unknown tag, so it was silently dropped.
    let record = record(
        "RACE",
        "Anti-Elf",
        &[
            ("!PRERACE", "1,Elf"),
            ("!PRETYPE", "1,Mundane"),
            ("!PREABILITY", "1,CATEGORY=Feat,Weapon Focus"),
            ("!PRECLASS", "1,Wizard=1"),
            ("!PREAGESET", "1,Middle Age"),
        ],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert_eq!(report.coverage.legacy_tags, 0);
    assert_eq!(pack.entities[0].requirements.len(), 5);
    assert!(pack.entities[0]
        .requirements
        .iter()
        .all(|predicate| matches!(predicate, Predicate::Not(_))));
}

#[test]
fn negated_bonus_condition_lowers() {
    // The same `!PREX` form can gate a BONUS condition.
    let record = record(
        "RACE",
        "No Elf Bonus",
        &[("BONUS", "SKILL|Climb|2|!PRERACE:1,Elf")],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert_eq!(report.coverage.legacy_tags, 0);
    let condition = pack.entities[0].effects[0]
        .condition
        .clone()
        .expect("negated condition");
    assert!(matches!(condition, Predicate::Not(_)));
}

#[test]
fn negated_unknown_predicate_is_reported_as_unsupported() {
    // A genuinely unknown `!PRE*` form must surface as unsupported work
    // rather than silently disappearing into the legacy histogram.
    let record = record("RACE", "No Unknown", &[("!PREUNKNOWN", "1,Foo")]);
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert!(pack.entities[0].requirements.is_empty());
    assert_eq!(report.coverage.unsupported_tags, 1);
    assert_eq!(report.coverage.legacy_tags, 0);
}

#[test]
fn prespell_predicates_lower() {
    let record = record(
        "CLASS",
        "Archmage",
        &[
            ("PRESPELLTYPE", "1,Arcane=9"),
            ("PRESPELLBOOK", "YES"),
            ("PRESPELLCAST", "MEMORIZE=N"),
            ("PRESPELLCAST", "TYPE=Divine"),
        ],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    let requirements = &pack.entities[0].requirements;
    assert!(matches!(
        requirements[0],
        Predicate::SpellType {
            ref specs,
            count: 1
        } if specs
            == &vec![pcgen_core::SpellTypeSpec {
                power: Some(SpellPowerType::Arcane),
                min_level: 9,
            }]
    ));
    assert!(matches!(requirements[1], Predicate::HasSpellbook));
    assert!(matches!(
        requirements[2],
        Predicate::CastingTypeIs { prepared: false }
    ));
    assert!(matches!(
        requirements[3],
        Predicate::HasSpellPower {
            power: SpellPowerType::Divine
        }
    ));
}

#[test]
fn prespell_book_can_be_negated() {
    let record = record("CLASS", "Fighter", &[("PRESPELLBOOK", "NO")]);
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::Not(inner) if matches!(**inner, Predicate::HasSpellbook)
    ));
}

#[test]
fn current_size_predicates_lower() {
    let equal = record("RACE", "Large", &[("PRESIZEEQ", "L")]);
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[equal]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::All(items)
            if matches!(items[0], Predicate::SizeAtLeast { .. })
                && matches!(items[1], Predicate::SizeAtMost { .. })
    ));

    let at_least = record("RACE", "Huge", &[("PRESIZEGTEQ", "L")]);
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["pred".to_string()],
        &[at_least],
    );
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::SizeAtLeast { .. }
    ));
}

#[test]
fn template_hands_and_movement_predicates_lower() {
    let record = record(
        "ABILITY",
        "Monster Trait",
        &[
            ("PRETEMPLATE", "1,Dragon,Half-Dragon"),
            ("PREHANDSGTEQ", "3"),
            ("PREHANDSEQ", "2"),
            ("PREMOVE", "1,Fly=1|TYPE=Racial"),
        ],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    let requirements = &pack.entities[0].requirements;
    assert!(matches!(
        &requirements[0],
        Predicate::TemplateCount { names, min_count: 1 }
            if names.contains("dragon") && names.contains("half-dragon")
    ));
    assert!(matches!(
        requirements[1],
        Predicate::HandsAtLeast { count: 3 }
    ));
    assert!(matches!(requirements[2], Predicate::All(_)));
    assert!(matches!(
        &requirements[3],
        Predicate::MovementAtLeast { mode, feet: 1 } if mode == "fly"
    ));
}

#[test]
fn pretemplate_tolerates_trailing_bracket_in_data() {
    // The RSRD data contains `PRETEMPLATE:1,Vampire]`.
    let record = record("CLASS", "Vampire", &[("PRETEMPLATE", "1,Vampire]")]);
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::TemplateCount { names, min_count: 1 } if names.contains("vampire")
    ));
}

#[test]
fn precheckbase_lowers_to_base_save() {
    let records = vec![record(
        "FEAT",
        "Mental Resistance",
        &[("PRECHECKBASE", "1,Will=2")],
    )];
    let (_, report) = lower_records(
        Some("35e".to_string()),
        vec!["precheckbase".to_string()],
        &records,
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
}

#[test]
fn choose_is_lowered_once_and_not_reported_as_legacy() {
    // `CHOOSE` has always been lowered to `EffectAction::Choose`, but it was
    // also counted as a legacy pending tag.  Ensure it only contributes to
    // the lowered/choose metrics.
    let record = record("ABILITY", "Skill Focus", &[("CHOOSE", "SKILL|Climb")]);
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["choose".to_string()],
        &[record],
    );
    assert!(pack.entities[0]
        .effects
        .iter()
        .any(|effect| matches!(effect.action, pcgen_core::EffectAction::Choose { .. })));
    assert_eq!(report.coverage.choose_effects, 1);
    assert_eq!(report.coverage.legacy_by_tag.get("CHOOSE"), None);
    assert_eq!(report.coverage.legacy_pending_by_tag.get("CHOOSE"), None);
}
