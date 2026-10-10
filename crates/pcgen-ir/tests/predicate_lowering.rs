use pcgen_core::{EffectAction, Predicate, SpellPowerType};
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
fn preatt_and_prehandsgt_lower_to_existing_predicates() {
    let record = record(
        "FEAT",
        "Big Brawler",
        &[("PREATT", "6"), ("PREHANDSGT", "2")],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::TotalBab { min: 6 }
    );
    assert_eq!(
        pack.entities[0].requirements[1],
        Predicate::HandsAtLeast { count: 3 }
    );
}

#[test]
fn premove_with_multiple_modes_lowers_to_a_count() {
    let record = record(
        "RACE",
        "Amphibious Runner",
        &[("PREMOVE", "1,Walk=30,Swim=20|TYPE=Racial")],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    let condition = pack.entities[0].requirements[0].clone();
    let Predicate::MovementCount { min_count, specs } = condition else {
        panic!("expected MovementCount, got {condition:?}");
    };
    assert_eq!(min_count, 1);
    assert_eq!(specs.len(), 2);
    assert!(specs
        .iter()
        .any(|spec| spec.mode == "walk" && spec.feet == 30));
    assert!(specs
        .iter()
        .any(|spec| spec.mode == "swim" && spec.feet == 20));
}

#[test]
fn prelevel_bounds_and_prelevelmax_lower() {
    let record = record(
        "FEAT",
        "Level Bands",
        &[("PRELEVEL", "MIN=5,MAX=10"), ("PRELEVELMAX", "7")],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::CharacterLevel {
            min: Some(5),
            max: Some(10),
        }
    );
    assert_eq!(
        pack.entities[0].requirements[1],
        Predicate::CharacterClassLevel {
            min: None,
            max: Some(7),
        }
    );
}

#[test]
fn prelevelmax_uses_character_class_levels_not_monster_hd() {
    // `PRELEVELMAX` is documented as a maximum on character (non-monster)
    // levels, which is exactly `CharacterClassLevel`.
    let record = record("FEAT", "Class Cap", &[("PRELEVELMAX", "3")]);
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert!(matches!(
        pack.entities[0].requirements[0],
        Predicate::CharacterClassLevel {
            min: None,
            max: Some(3)
        }
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

#[test]
fn preregion_lowers_to_a_case_insensitive_prefix() {
    let plain = record("FEAT", "Big Game Hunter", &[("PREREGION", "Varisia")]);
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[plain]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::Region {
            prefix: "Varisia".to_string()
        }
    );

    // The whole body is one prefix, commas included.  PCGen's
    // `AbstractPrerequisiteSimpleParser` sets the key to the entire formula, and
    // the Arcanis data really does name a region `Elorii Nations,Dwarven
    // Enclaves`; splitting on the comma would invent a list the reference does
    // not have.
    let commas = record(
        "FEAT",
        "Illiteracy",
        &[("PREREGION", "Elorii Nations,Dwarven Enclaves")],
    );
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[commas]);
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::Region {
            prefix: "Elorii Nations,Dwarven Enclaves".to_string()
        }
    );

    // `Q:` is PCGen's override-qualify marker, stripped before the parser sees
    // the formula, so it is not part of the region name.
    let qualified = record("FEAT", "Centurion", &[("PREREGION", "Q:Coryan")]);
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["pred".to_string()],
        &[qualified],
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::Region {
            prefix: "Coryan".to_string()
        }
    );

    let negated = record("FEAT", "Not From Milandir", &[("!PREREGION", "Milandir")]);
    let (pack, report) =
        lower_records(Some("3e".to_string()), vec!["pred".to_string()], &[negated]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::Not(inner) if matches!(**inner, Predicate::Region { .. })
    ));
}

#[test]
fn template_region_is_captured_and_not_reported_as_legacy() {
    let template = record(
        "TEMPLATE",
        "Regional Affinity (Artume)",
        &[
            ("REGION", "The River Kingdoms"),
            ("SUBREGION", "Artume"),
            ("VISIBLE", "NO"),
        ],
    );
    let (pack, report) = lower_records(
        Some("35e".to_string()),
        vec!["region".to_string()],
        &[template],
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(
        report.coverage.legacy_by_tag.get("REGION"),
        None,
        "{:?}",
        report.coverage.legacy_by_tag
    );
    assert_eq!(report.coverage.legacy_tags, 0);
    let definition = pack
        .templates
        .get("Regional Affinity (Artume)")
        .expect("template");
    assert_eq!(
        definition.traits.region.as_deref(),
        Some("The River Kingdoms")
    );
    assert_eq!(definition.traits.subregion.as_deref(), Some("Artume"));
}

#[test]
fn template_region_none_means_no_region_set() {
    // `REGION:NONE` is the reference's "nothing set" spelling; storing it as a
    // literal region would make `PREREGION:None` match a template that declares
    // no region at all.
    let template = record("TEMPLATE", "Unplaced", &[("REGION", "NONE")]);
    let (pack, _) = lower_records(
        Some("35e".to_string()),
        vec!["region".to_string()],
        &[template],
    );
    assert_eq!(pack.templates["Unplaced"].traits.region, None);
}

#[test]
fn region_on_a_race_row_is_not_consumed() {
    // `REGION` is a `PCTemplate` token in PCGen; a `REGION:` on a RACE row is
    // an unrecognized token there, and the third-party data really does write
    // one.  Reading it would invent a region the reference does not have, so it
    // stays legacy and the engine never sees it.
    let race = record("RACE", "Aglarond Human", &[("REGION", "Aglarond")]);
    let (pack, report) =
        lower_records(Some("35e".to_string()), vec!["region".to_string()], &[race]);
    assert_eq!(pack.races["Aglarond Human"].traits.region, None);
    assert_eq!(
        report.coverage.legacy_by_tag.get("REGION").copied(),
        Some(1)
    );
    assert_eq!(report.coverage.unsupported_tags, 0);
}

#[test]
fn precskill_lowers_to_class_skill_selectors() {
    let feat = record(
        "FEAT",
        "Skilled City Dweller",
        &[("PRECSKILL", "2,Spot,TYPE.Spy")],
    );
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[feat]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let Predicate::ClassSkill { specs, min_count } = &pack.entities[0].requirements[0] else {
        panic!(
            "expected ClassSkill, got {:?}",
            pack.entities[0].requirements[0]
        );
    };
    assert_eq!(*min_count, 2);
    assert_eq!(
        specs,
        &vec![
            pcgen_core::SkillSpec {
                name: "Spot".to_string(),
                is_type: false,
            },
            pcgen_core::SkillSpec {
                name: "Spy".to_string(),
                is_type: true,
            },
        ]
    );

    // The 5e data spells the negative form, which is a plain negation.
    let negated = record("ABILITY", "Athletics", &[("!PRECSKILL", "1,Athletics")]);
    let (pack, report) =
        lower_records(Some("5e".to_string()), vec!["pred".to_string()], &[negated]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::Not(inner) if matches!(**inner, Predicate::ClassSkill { .. })
    ));
}

#[test]
fn a_bonus_gated_on_precskill_is_lowered_not_reported() {
    // 319 `BONUS` occurrences across the third-party packs were reported as
    // unsupported only because their gate was `PRECSKILL`, which the tag-name
    // histogram attributes to `BONUS`. Lowering the gate has to clear the report
    // *and* keep the effect with a real condition.
    let record = record(
        "ABILITY",
        "Skilled City Dweller",
        &[("BONUS", "SKILL|Diplomacy|2|PRECSKILL:1,Handle Animal")],
    );
    let (pack, report) =
        lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[record]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(pack.entities[0].effects.len(), 1, "the bonus must survive");
    assert!(matches!(
        pack.entities[0].effects[0].condition,
        Some(Predicate::ClassSkill { .. })
    ));
}

#[test]
fn prealign_keeps_numeric_keys_instead_of_dropping_the_requirement() {
    // The Book of Erotic Fantasy writes `PREALIGN:0,1,3,4,6,7`. PCGen's
    // `PreAlignParser` does no validation — the key is the raw text — and no
    // vendored alignment file defines a numeric key, so the requirement can
    // never match. That is the right outcome; filtering the numbers out, as the
    // first cut did, left those prestige classes with no alignment restriction
    // at all.
    let class = record("CLASS", "Imagist", &[("PREALIGN", "0,1,3,4,6,7")]);
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[class]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::Alignment {
            allowed: vec![
                "0".to_string(),
                "1".to_string(),
                "3".to_string(),
                "4".to_string(),
                "6".to_string(),
                "7".to_string(),
            ],
        }
    );

    let named = record("CLASS", "Paladin", &[("PREALIGN", "LG,LN,LE")]);
    let (pack, _) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[named]);
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::Alignment {
            allowed: vec!["LG".to_string(), "LN".to_string(), "LE".to_string()],
        }
    );

    // No key at all is still malformed, and stays visible as unsupported.
    let empty = record("CLASS", "Broken", &[("PREALIGN", "")]);
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[empty]);
    assert_eq!(report.coverage.unsupported_tags, 1);
    assert!(pack.entities[0].requirements.is_empty());
}

#[test]
fn precheck_and_precheckbase_lower_with_the_count() {
    let total = record("FEAT", "Iron Will", &[("PRECHECK", "1,Will=5")]);
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[total]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::Check {
            base: false,
            specs: vec![pcgen_core::SaveSpec {
                save: "Will".to_string(),
                min: 5,
            }],
            min_count: 1,
        }
    );

    // The reference's "any two of the three" example. Reading only the first
    // `=` part turned this into "Fortitude >= 5".
    let any_two = record(
        "FEAT",
        "Steadfast",
        &[("PRECHECKBASE", "2,Fortitude=5,Reflex=3,Willpower=4")],
    );
    let (pack, _) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[any_two],
    );
    let Predicate::Check {
        base,
        specs,
        min_count,
    } = &pack.entities[0].requirements[0]
    else {
        panic!("expected Check, got {:?}", pack.entities[0].requirements[0]);
    };
    assert!(*base);
    assert_eq!(*min_count, 2);
    assert_eq!(specs.len(), 3);
    assert_eq!(specs[0].save, "Fortitude");
    assert_eq!(specs[0].min, 5);
    assert_eq!(specs[2].save, "Willpower");

    let negated = record("FEAT", "Frail", &[("!PRECHECK", "1,Will=5")]);
    let (pack, _) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[negated],
    );
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::Not(inner) if matches!(**inner, Predicate::Check { .. })
    ));
}

#[test]
fn the_old_base_save_predicate_still_deserializes() {
    // `PRECHECKBASE` used to lower to this shape. Packs written then must keep
    // loading, so the variant stays even though nothing emits it now.
    let predicate: Predicate =
        serde_json::from_str(r#"{"base_save":{"save":"Will","min":2}}"#).expect("old pack");
    assert_eq!(
        predicate,
        Predicate::BaseSave {
            save: "Will".to_string(),
            min: 2,
        }
    );
}

#[test]
fn prespelldescriptor_lowers_to_descriptor_pairs() {
    let feat = record(
        "FEAT",
        "Mindbender",
        &[("PRESPELLDESCRIPTOR", "4,Mind-Affecting=3")],
    );
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[feat]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::SpellDescriptor {
            specs: vec![pcgen_core::SpellDescriptorSpec {
                descriptor: "Mind-Affecting".to_string(),
                min_level: 3,
            }],
            count: 4,
        }
    );

    // The pairs add up, so every one of them is kept.
    let energy = record(
        "FEAT",
        "Energy Substitution",
        &[("PRESPELLDESCRIPTOR", "1,Acid=3,Cold=3,Electricity=3,Fire=3")],
    );
    let (pack, _) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[energy]);
    let Predicate::SpellDescriptor { specs, count } = &pack.entities[0].requirements[0] else {
        panic!("expected SpellDescriptor");
    };
    assert_eq!(*count, 1);
    assert_eq!(specs.len(), 4);
    assert_eq!(specs[3].descriptor, "Fire");
    assert_eq!(specs[3].min_level, 3);

    let negated = record("FEAT", "Pure Heart", &[("!PRESPELLDESCRIPTOR", "1,Evil=3")]);
    let (pack, _) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[negated],
    );
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::Not(inner) if matches!(**inner, Predicate::SpellDescriptor { .. })
    ));
}

#[test]
fn prespellschoolsub_lowers_to_subschool_pairs() {
    let feat = record(
        "FEAT",
        "Augment Summoning",
        &[("PRESPELLSCHOOLSUB", "1,Summoning=3")],
    );
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[feat]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::SpellSchoolSub {
            specs: vec![pcgen_core::SpellSchoolSubSpec {
                subschool: "Summoning".to_string(),
                min_level: 3,
            }],
            count: 1,
        }
    );

    // The reference's example, and the count it carries.
    let creation = record(
        "FEAT",
        "Creation Adept",
        &[("PRESPELLSCHOOLSUB", "3,Creation=2")],
    );
    let (pack, _) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[creation],
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::SpellSchoolSub {
            specs: vec![pcgen_core::SpellSchoolSubSpec {
                subschool: "Creation".to_string(),
                min_level: 2,
            }],
            count: 3,
        }
    );

    let negated = record(
        "FEAT",
        "No Illusions",
        &[("!PRESPELLSCHOOLSUB", "1,Illusion=0")],
    );
    let (pack, _) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[negated],
    );
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::Not(inner) if matches!(**inner, Predicate::SpellSchoolSub { .. })
    ));
}

#[test]
fn prespellschool_lowers_with_the_count_and_the_pairs() {
    let archmage = record(
        "CLASS",
        "Archmage",
        &[(
            "PRESPELLSCHOOL",
            "5,Abjuration=5,Conjuration=5,Divination=5,Enchantment=5,Evocation=5,Illusion=5,Necromancy=5,Transmutation=5",
        )],
    );
    let (pack, report) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[archmage],
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let Predicate::SpellSchool { specs, count } = &pack.entities[0].requirements[0] else {
        panic!(
            "expected SpellSchool, got {:?}",
            pack.entities[0].requirements[0]
        );
    };
    // The count used to be skipped entirely, so "at least five" became "all
    // eight".
    assert_eq!(*count, 5);
    assert_eq!(specs.len(), 8);
    assert_eq!(specs[0].school, "Abjuration");
    assert_eq!(specs[0].min_level, 5);
    assert_eq!(specs[7].school, "Transmutation");

    let loremaster = record(
        "CLASS",
        "Loremaster",
        &[("PRESPELLSCHOOL", "7,Divination=0")],
    );
    let (pack, _) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[loremaster],
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::SpellSchool {
            specs: vec![pcgen_core::SpellSchoolSpec {
                school: "Divination".to_string(),
                min_level: 0,
            }],
            count: 7,
        }
    );
}

#[test]
fn the_old_spell_school_level_predicate_still_deserializes() {
    // `PRESPELLSCHOOL` used to lower to this shape.  Packs written then must keep
    // loading, so the variant stays even though nothing emits it now.
    let predicate: Predicate =
        serde_json::from_str(r#"{"spell_school_level":{"entries":[["Divination",3]]}}"#)
            .expect("old pack");
    assert_eq!(
        predicate,
        Predicate::SpellSchoolLevel {
            entries: vec![("Divination".to_string(), 3)],
        }
    );
}

#[test]
fn precampaign_lowers_to_the_four_selectors() {
    let row = record(
        "CLASS",
        "Sacred Exorcist",
        &[("PRECAMPAIGN", "1,INCLUDES=Epic Level Handbook")],
    );
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[row]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::Campaign {
            specs: vec![pcgen_core::CampaignSpec::Includes(
                "Epic Level Handbook".to_string()
            )],
            min_count: 1,
        }
    );

    // `BOOKTYPE=` and `INCLUDESBOOKTYPE=` keep their whole argument, spaces and
    // all — the reference parser cannot read `Core Rules` as a number, so the
    // selector stays in the key.
    let types = record(
        "CLASS",
        "Typed",
        &[(
            "PRECAMPAIGN",
            "2,BOOKTYPE=Core Rules,INCLUDESBOOKTYPE=Supplement",
        )],
    );
    let (pack, _) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[types]);
    let Predicate::Campaign { specs, min_count } = &pack.entities[0].requirements[0] else {
        panic!("expected Campaign");
    };
    assert_eq!(*min_count, 2);
    assert_eq!(
        specs,
        &vec![
            pcgen_core::CampaignSpec::BookType("Core Rules".to_string()),
            pcgen_core::CampaignSpec::IncludesBookType("Supplement".to_string()),
        ]
    );

    // A bare name is a `Key` selector, not a book type.
    let named = record("CLASS", "Named", &[("PRECAMPAIGN", "1,Complete Divine")]);
    let (pack, _) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[named]);
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::Campaign {
            specs: vec![pcgen_core::CampaignSpec::Key("Complete Divine".to_string())],
            min_count: 1,
        }
    );

    let negated = record("CLASS", "NotEpic", &[("!PRECAMPAIGN", "1,INCLUDES=Epic")]);
    let (pack, _) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[negated],
    );
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::Not(inner) if matches!(**inner, Predicate::Campaign { .. })
    ));
}

#[test]
fn precharactertype_lowers_to_a_type_list() {
    let npc = record("ABILITY", "NPC Only", &[("PRECHARACTERTYPE", "1,NPC")]);
    let (pack, report) = lower_records(Some("5e".to_string()), vec!["pred".to_string()], &[npc]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::CharacterType {
            types: vec!["NPC".to_string()],
            min_count: 1,
        }
    );

    // The reference's own second example.
    let either = record("ABILITY", "Not A PC", &[("PRECHARACTERTYPE", "1,PC,NPC")]);
    let (pack, _) = lower_records(Some("5e".to_string()), vec!["pred".to_string()], &[either]);
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::CharacterType {
            types: vec!["PC".to_string(), "NPC".to_string()],
            min_count: 1,
        }
    );

    let negated = record(
        "ABILITY",
        "Not A Monster",
        &[("!PRECHARACTERTYPE", "1,Monster")],
    );
    let (pack, _) = lower_records(Some("5e".to_string()), vec!["pred".to_string()], &[negated]);
    assert!(matches!(
        &pack.entities[0].requirements[0],
        Predicate::Not(inner) if matches!(**inner, Predicate::CharacterType { .. })
    ));
}

#[test]
fn sa_tags_lower_to_granted_special_abilities() {
    let rogue = record(
        "CLASS",
        "Rogue",
        &[
            ("SAB", "Sneak Attack +%d%|Sneak Attack|Sneak Attack Die"),
            ("SA", "Evasion"),
        ],
    );
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[rogue]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let names: Vec<&str> = pack.entities[0]
        .effects
        .iter()
        .filter_map(|effect| match &effect.action {
            EffectAction::GrantSpecialAbility { name } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    // The key name is the first `|`-separated token; the rest is display text and
    // the variables its `%` placeholders consume.
    assert_eq!(names, vec!["Sneak Attack +%d%", "Evasion"]);
    assert!(pack.entities[0]
        .effects
        .iter()
        .all(|effect| effect.condition.is_none()));
}

#[test]
fn a_qualified_sa_carries_its_prerequisites() {
    let template = record(
        "TEMPLATE",
        "Bone Creature",
        &[("SAB", "Create Skeletal Warrior (Su)|PRELEVEL:MIN=15")],
    );
    let (pack, _) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[template],
    );
    assert_eq!(
        pack.entities[0].effects[0].condition,
        Some(Predicate::CharacterLevel {
            min: Some(15),
            max: None,
        })
    );

    // Several prerequisites are all required, and display text is skipped.
    let multi = record(
        "TEMPLATE",
        "Fascinating",
        &[(
            "SAB",
            "Facinate(Ex)|PRELEVEL:MIN=10|PRESKILL:1,TYPE.Perform=3",
        )],
    );
    let (pack, _) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[multi]);
    let Some(Predicate::All(parts)) = &pack.entities[0].effects[0].condition else {
        panic!(
            "expected All, got {:?}",
            pack.entities[0].effects[0].condition
        );
    };
    assert_eq!(parts.len(), 2);
}

#[test]
fn sa_clear_edits_only_this_record() {
    let cleared = record(
        "TEMPLATE",
        "Alternate Form",
        &[
            ("SAB", "Alternate Form %/day|AltForm"),
            ("SAB", ".CLEAR.Alternate Form %/day|AltForm"),
            ("SAB", "Scent (Ex)"),
        ],
    );
    let (pack, _) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[cleared],
    );
    let names: Vec<&str> = pack.entities[0]
        .effects
        .iter()
        .filter_map(|effect| match &effect.action {
            EffectAction::GrantSpecialAbility { name } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["Scent (Ex)"]);
}

#[test]
fn presa_lowers_to_prefixes_and_a_count() {
    let feat = record("FEAT", "Watchful Spirit", &[("PRESA", "1,Watchful Spirit")]);
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[feat]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::SpecialAbility {
            prefixes: vec!["Watchful Spirit".to_string()],
            min_count: 1,
        }
    );

    // The reference's own example.
    let three = record(
        "FEAT",
        "Divine Feat",
        &[("PRESA", "1,Turn undead,Rebuke undead,Smite Evil")],
    );
    let (pack, _) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[three]);
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::SpecialAbility {
            prefixes: vec![
                "Turn undead".to_string(),
                "Rebuke undead".to_string(),
                "Smite Evil".to_string(),
            ],
            min_count: 1,
        }
    );
}

#[test]
fn fact_tags_are_captured_on_the_entity() {
    let ability = record(
        "ABILITY",
        "Tiefling Trait",
        &[
            ("FACT", "Tiefling_Trait_AbilityScoreIncrease|true"),
            ("FACT", "Tiefling_Trait_InfernalLegacy|true"),
            ("FACT", "Empty|"),
        ],
    );
    let (pack, report) =
        lower_records(Some("5e".to_string()), vec!["pred".to_string()], &[ability]);
    assert_eq!(report.coverage.unsupported_tags, 0);
    assert_eq!(
        pack.entities[0]
            .facts
            .get("Tiefling_Trait_AbilityScoreIncrease"),
        Some(&"true".to_string())
    );
    assert_eq!(pack.entities[0].facts.len(), 3);
    // `FACT:Name|` is a fact with an empty value, which is what the data means.
    assert_eq!(pack.entities[0].facts.get("Empty"), Some(&String::new()));
}

#[test]
fn prefact_lowers_for_the_locations_the_runtime_models() {
    let ability = record(
        "ABILITY",
        "Baalzebul",
        &[(
            "PREMULT",
            "1,[!PREFACT:1,ABILITIES,Tiefling_Trait_AbilityScoreIncrease=true],[PREABILITY:1,CATEGORY=Internal,Tiefling Subrace ~ Baalzebul]",
        )],
    );
    let (pack, report) =
        lower_records(Some("5e".to_string()), vec!["pred".to_string()], &[ability]);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let Predicate::AtLeast { items, .. } = &pack.entities[0].requirements[0] else {
        panic!(
            "expected AtLeast, got {:?}",
            pack.entities[0].requirements[0]
        );
    };
    let Predicate::Not(inner) = &items[0] else {
        panic!("expected Not, got {:?}", items[0]);
    };
    assert_eq!(
        **inner,
        Predicate::Fact {
            // The location is normalised; `OutputDB` looks it up case-insensitively.
            location: "abilities".to_string(),
            specs: vec![pcgen_core::FactSpec {
                name: "Tiefling_Trait_AbilityScoreIncrease".to_string(),
                value: "true".to_string(),
            }],
            min_count: 1,
        }
    );

    // The list form sums its specs, as every other list prerequisite does.
    let multi = record(
        "ABILITY",
        "Fierna",
        &[(
            "PREFACT",
            "1,ABILITIES,Tiefling_Trait_AbilityScoreIncrease=true,Tiefling_Trait_InfernalLegacy=true",
        )],
    );
    let (pack, _) = lower_records(Some("5e".to_string()), vec!["pred".to_string()], &[multi]);
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::Fact {
            location: "abilities".to_string(),
            specs: vec![
                pcgen_core::FactSpec {
                    name: "Tiefling_Trait_AbilityScoreIncrease".to_string(),
                    value: "true".to_string(),
                },
                pcgen_core::FactSpec {
                    name: "Tiefling_Trait_InfernalLegacy".to_string(),
                    value: "true".to_string(),
                },
            ],
            min_count: 1,
        }
    );
}

#[test]
fn prefact_stays_unsupported_where_the_reference_rejects_it() {
    // `PreFactParser` throws when the formula contains `|`, so the tag is
    // malformed data rather than something to guess at.
    let malformed = record(
        "CLASS",
        "Sacred Warder",
        &[("PREFACT", "1,RACE,HasFirstLevelFeat=True|PRECLASS:1,ANY=1")],
    );
    let (pack, report) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[malformed],
    );
    assert!(pack.entities[0].requirements.is_empty());
    assert_eq!(report.coverage.unsupported_tags, 1);
    assert_eq!(
        report.coverage.unsupported_pending_by_tag.get("PREFACT"),
        Some(&1)
    );

    // A location the runtime cannot enumerate is left unsupported rather than
    // lowered into a prerequisite that can only ever be false.
    let other = record(
        "ABILITY",
        "Elsewhere",
        &[("PREFACT", "1,CLASSES,SomeFact=true")],
    );
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[other]);
    assert!(pack.entities[0].requirements.is_empty());
    assert_eq!(report.coverage.unsupported_tags, 1);
}

#[test]
fn predr_lowers_to_type_value_pairs() {
    // The third-party form, on a prestige class header.
    let class = record(
        "CLASS",
        "Sacred Warder of Bahamut",
        &[("PREDR", "1,Magic=5")],
    );
    let (pack, report) = lower_records(
        Some("35e".to_string()),
        vec!["pred".to_string()],
        &[class],
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::DamageReduction {
            specs: vec![pcgen_core::DamageReductionSpec {
                kind: "Magic".to_string(),
                min: 5,
            }],
            min_count: 1,
        }
    );

    // The reference's own example: `-` is the untyped reduction, and `.` works as
    // well as `=` because the parser tokenises on both.
    let either = record(
        "FEAT",
        "Damage Reduction",
        &[("PREDR", "1,-=10,+1.10,+5=10,Silver=10")],
    );
    let (pack, _) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[either]);
    let Predicate::DamageReduction { specs, min_count } = &pack.entities[0].requirements[0] else {
        panic!("expected DamageReduction");
    };
    assert_eq!(*min_count, 1);
    assert_eq!(
        specs,
        &vec![
            pcgen_core::DamageReductionSpec {
                kind: "-".to_string(),
                min: 10,
            },
            pcgen_core::DamageReductionSpec {
                kind: "+1".to_string(),
                min: 10,
            },
            pcgen_core::DamageReductionSpec {
                kind: "+5".to_string(),
                min: 10,
            },
            pcgen_core::DamageReductionSpec {
                kind: "Silver".to_string(),
                min: 10,
            },
        ]
    );

    // A pair with no value defaults to 0, as the reference's tokeniser does.
    let bare = record("FEAT", "Any DR", &[("PREDR", "1,Magic")]);
    let (pack, _) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[bare]);
    assert_eq!(
        pack.entities[0].requirements[0],
        Predicate::DamageReduction {
            specs: vec![pcgen_core::DamageReductionSpec {
                kind: "Magic".to_string(),
                min: 0,
            }],
            min_count: 1,
        }
    );

    // A `|PRE` suffix makes the value unparseable, which is what the reference's
    // `Integer.parseInt` also sees, so the tag stays unsupported.
    let gated = record("FEAT", "Gated DR", &[("PREDR", "1,Magic=5|PRELEVEL:MIN=5")]);
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["pred".to_string()], &[gated]);
    assert!(pack.entities[0].requirements.is_empty());
    assert_eq!(report.coverage.unsupported_tags, 1);
}
