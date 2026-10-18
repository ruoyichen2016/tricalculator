use pcgen_core::{
    actions::*, build::*, magic::CastRequest, AdvancementFrame, AdvancementLane, DraftSpell,
};
use pcgen_engine::{CharacterDraft, Engine};
use std::{collections::BTreeMap, path::Path};
#[test]
fn chronology_projection_and_atomic_operation_boundaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let e = Engine::new(
        pcgen_import::native::compile_database(
            &root.join("data/rules/rainbow-warweaver.rules.json"),
        )
        .unwrap(),
    );
    let mut d = CharacterDraft {
        stats: BTreeMap::from([("CHA".into(), 30), ("INT".into(), 30)]),
        ..Default::default()
    };
    for n in 1..=4 {
        d.advancement
            .push(AdvancementFrame::single(n, "Fighter", n, 5, 0));
    }
    let initial = BTreeMap::from([
        ("STR".into(), 14),
        ("DEX".into(), 14),
        ("CON".into(), 14),
        ("INT".into(), 12),
        ("WIS".into(), 12),
        ("CHA".into(), 18),
    ]);
    d.skills.insert("Knowledge (Arcana)".into(), 23.0);
    d.abilities.push(("FEAT".into(), "Combat Expertise".into()));
    d.build_history = Some(BuildHistory {
        initial_stats: initial.clone(),
        initial_alignment: "LG".into(),
        recorded_through: 4,
        events: vec![
            BuildEvent {
                level: 3,
                action: Acquisition::Feat {
                    name: "Combat Expertise".into(),
                },
            },
            BuildEvent {
                level: 4,
                action: Acquisition::AbilityIncrease {
                    ability: "INT".into(),
                },
            },
        ],
    });
    let before = d.build_at(2, 2).unwrap();
    assert!(before.abilities.is_empty());
    assert!(before.skills.is_empty());
    assert_eq!(before.stats["INT"], 12);
    assert_eq!(e.evaluate(&d).stats["INT"], 13); // Final stale input INT 30 never leaks into computation.
    let audit = e.audit_build(&d);
    assert!(audit.decisions.iter().any(|c| c.level == 3
        && c.subject == "Combat Expertise"
        && c.status == DecisionStatus::Failed));
    d.build_history.as_mut().unwrap().events.swap(0, 1);
    assert!(d.materialized().is_err());
    d.build_history.as_mut().unwrap().events.swap(0, 1);
    // Same-level ordering matters: later choices cannot satisfy an earlier feat.
    d.build_history
        .as_mut()
        .unwrap()
        .initial_stats
        .insert("INT".into(), 14);
    d.build_history.as_mut().unwrap().events = vec![
        BuildEvent {
            level: 3,
            action: Acquisition::Feat {
                name: "Improved Trip".into(),
            },
        },
        BuildEvent {
            level: 3,
            action: Acquisition::Feat {
                name: "Combat Expertise".into(),
            },
        },
    ];
    assert!(e
        .audit_build(&d)
        .decisions
        .iter()
        .any(|c| c.subject == "Improved Trip" && c.status == DecisionStatus::Failed));
    d.build_history.as_mut().unwrap().events.swap(0, 1);
    let reordered = e.audit_build(&d);
    assert!(
        !reordered
            .decisions
            .iter()
            .any(|c| c.subject == "Improved Trip" && c.status == DecisionStatus::Failed),
        "{:?}",
        reordered.decisions
    );
    // History-free input has unknown chronology; numerical evaluation does not certify it.
    assert_eq!(
        e.audit_build(&CharacterDraft::default()).status(),
        DecisionStatus::MissingEvidence
    );
    let mut caster = CharacterDraft {
        stats: initial,
        ..Default::default()
    };
    for n in 1..=4 {
        caster
            .advancement
            .push(AdvancementFrame::single(n, "Sorcerer", n, 2, 0));
    }
    caster.spells.push(DraftSpell {
        name: "Shield".into(),
        level: 1,
        class_id: Some("Sorcerer".into()),
        times_memorized: 0,
        book: None,
    });
    let request = CastRequest {
        class_id: "Sorcerer".into(),
        spell: "Shield".into(),
        ..Default::default()
    };
    let plan = e.plan_cast(&caster, &request).unwrap();
    assert_eq!(plan.resolution.action_cost, Some(ActionCost::Standard));
    assert!(caster.class_play.resources.slots_spent.is_empty());
    assert!(e.execute_cast(&caster, &request, 1).is_err());
    assert!(caster.class_play.resources.slots_spent.is_empty());
    let committed = e.execute_cast(&caster, &request, 0).unwrap();
    assert_eq!(committed.state.revision, 1);
    assert_eq!(committed.state.resources.slots_spent["sorcerer"][&1], 1);
    assert!(caster.class_play.resources.slots_spent.is_empty());
    // Forged client balances cannot fund an exhausted operation.
    caster
        .class_play
        .resources
        .slots_spent
        .insert("sorcerer".into(), BTreeMap::from([(1, 99)]));
    let prior = caster.class_play.clone();
    assert!(e.plan_cast(&caster, &request).is_err());
    assert!(e.execute_cast(&caster, &request, 0).is_err());
    assert_eq!(caster.class_play, prior);
    // Preparations never grant spell knowledge or travel back into acquisition history.
    let mut wizard = CharacterDraft {
        stats: caster.stats.clone(),
        ..Default::default()
    };
    for n in 1..=4 {
        wizard
            .advancement
            .push(AdvancementFrame::single(n, "Wizard", n, 2, 0));
    }
    let shield = DraftSpell {
        name: "Shield".into(),
        level: 1,
        class_id: Some("Wizard".into()),
        times_memorized: 0,
        book: None,
    };
    wizard.build_history = Some(BuildHistory {
        initial_stats: wizard.stats.clone(),
        initial_alignment: "LG".into(),
        recorded_through: 4,
        events: vec![BuildEvent {
            level: 1,
            action: Acquisition::Spell { spell: shield },
        }],
    });
    let wizard_cast = CastRequest {
        class_id: "Wizard".into(),
        spell: "Shield".into(),
        ..Default::default()
    };
    assert!(e.plan_cast(&wizard, &wizard_cast).is_err());
    assert!(e.prepare_spell(&wizard, "Wizard", "False Life", 1).is_err());
    wizard = e.prepare_spell(&wizard, "Wizard", "Shield", 1).unwrap();
    assert!(e.plan_cast(&wizard, &wizard_cast).is_ok());
    assert_eq!(
        wizard
            .build_at(1, 1)
            .unwrap()
            .prepared_copies("Wizard", "Shield")
            .unwrap(),
        0
    );
    wizard.preparations.push(DraftSpell {
        name: "False Life".into(),
        level: 2,
        class_id: Some("Wizard".into()),
        times_memorized: 1,
        book: None,
    });
    assert!(
        !e.spell_catalog(&wizard, "Wizard")
            .unwrap()
            .iter()
            .find(|s| s.spell == "False Life")
            .unwrap()
            .known
    );
    // A list entry is not permission to learn a ninth-level spell at level one.
    let mut early = caster.clone();
    early.advancement.truncate(1);
    early.build_history = Some(BuildHistory {
        initial_stats: early.stats.clone(),
        initial_alignment: "LG".into(),
        recorded_through: 1,
        events: vec![BuildEvent {
            level: 1,
            action: Acquisition::Spell {
                spell: DraftSpell {
                    name: "Wish".into(),
                    level: 9,
                    class_id: Some("Sorcerer".into()),
                    times_memorized: 0,
                    book: None,
                },
            },
        }],
    });
    assert!(e
        .audit_build(&early)
        .decisions
        .iter()
        .any(|d| d.subject == "Wish" && d.status == DecisionStatus::Failed));
    let mut budget = TurnBudget::default();
    budget.spend(ActionCost::Swift).unwrap();
    assert!(budget.spend(ActionCost::Swift).is_err());
    budget.grant(ActionCost::Swift).unwrap();
    budget.spend(ActionCost::Swift).unwrap();
    budget.spend(ActionCost::FullRound).unwrap();
    let prior = budget.clone();
    assert!(budget.spend(ActionCost::Standard).is_err());
    assert_eq!(budget, prior);
    let mut budget = TurnBudget::default();
    budget.spend(ActionCost::Move).unwrap();
    budget.spend(ActionCost::Move).unwrap();
    assert!(budget.spend(ActionCost::FullRound).is_err());

    // Identity-selected entity requirements gate in audit_build:
    // If a race or template has requirements that are not met, audit reports it.
    let mut illegal_race_hero = early.clone();
    illegal_race_hero.race_id = "Dragonborn".into();
    let race_audit = e.audit_build(&illegal_race_hero);
    assert!(race_audit.decisions.iter().any(|d| d.level == 1));
}

#[test]
fn gestalt_npc_dual_prestige_generates_npc_tag_and_preserves_evaluation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let e = Engine::new(
        pcgen_import::native::compile_database(
            &root.join("data/rules/rainbow-warweaver.rules.json"),
        )
        .unwrap(),
    );
    let mut d = CharacterDraft {
        alignment: "LG".into(),
        stats: BTreeMap::from([
            ("STR".into(), 10),
            ("DEX".into(), 10),
            ("CON".into(), 14),
            ("INT".into(), 14),
            ("WIS".into(), 14),
            ("CHA".into(), 18),
        ]),
        skills: BTreeMap::from([
            ("Knowledge (Arcana)".into(), 10.0),
            ("Knowledge (The Planes)".into(), 10.0),
            ("Knowledge (Religion)".into(), 10.0),
        ]),
        ..Default::default()
    };
    // 5 levels of Warmage on lane a and lane b
    for lvl in 1..=5 {
        d.advancement.push(AdvancementFrame {
            character_level: lvl,
            lanes: vec![
                AdvancementLane::new("a", "Warmage", lvl, 6, 0),
                AdvancementLane::new("b", "Warmage", lvl, 6, 0),
            ],
        });
    }
    // Level 6: dual prestige classes: Rainbow Servant and Sacred Exorcist!
    // Also simultaneously advancing Warmage casting track on both lanes.
    let mut lane_a = AdvancementLane::new("a", "Rainbow Servant", 1, 6, 0);
    lane_a.advances.push("Warmage".into());
    let mut lane_b = AdvancementLane::new("b", "Sacred Exorcist", 1, 8, 0);
    lane_b.advances.push("Warmage".into());

    d.advancement.push(AdvancementFrame {
        character_level: 6,
        lanes: vec![lane_a, lane_b],
    });
    d.build_history = Some(BuildHistory {
        initial_stats: d.stats.clone(),
        initial_alignment: "LG".into(),
        recorded_through: 6,
        events: vec![],
    });

    // 1. Numerical evaluation must succeed and remain available (not nulled/broken)
    let snapshot = e.evaluate(&d);
    assert!(
        snapshot.is_available(),
        "Numerical derive must succeed despite dual prestige NPC relaxation: {:?}",
        snapshot.diagnostics
    );
    assert_eq!(snapshot.total_level, 6);
    // Gestalt diagnostics records the UA p. 72 violation
    assert!(snapshot
        .gestalt
        .as_ref()
        .expect("Gestalt plan must be present")
        .diagnostics
        .iter()
        .any(|diag| diag.code == "gestalt.dual_prestige_lanes"));

    // 2. Audit check must produce NPC and Gestalt tags
    let audit = e.audit_build(&d);
    assert!(
        audit.tags.contains(&RuleTag::Gestalt),
        "Audit must be tagged as Gestalt"
    );
    assert!(
        audit.tags.contains(&RuleTag::Npc),
        "Audit must be tagged as NPC due to dual prestige rule relaxation"
    );
    assert!(audit.is_npc(), "audit.is_npc() must return true");

    // 3. Find the specific rule decisions for Level 6 dual prestige and dual casting advancement
    let dual_prc_decision = audit
        .decisions
        .iter()
        .find(|dec| dec.level == 6 && dec.subject == "Gestalt prestige")
        .expect("Must have a decision for Gestalt prestige at level 6");
    assert_eq!(dual_prc_decision.status, DecisionStatus::NeedsRuling);
    assert!(dual_prc_decision.tags.contains(&RuleTag::Npc));
    assert!(dual_prc_decision.tags.contains(&RuleTag::Gestalt));
    assert!(dual_prc_decision
        .explanation
        .contains("violates PC Gestalt RAW (UA p.72), relaxed as NPC"));

    let dual_casting_decision = audit
        .decisions
        .iter()
        .find(|dec| dec.level == 6 && dec.subject == "Casting advancement")
        .expect("Must have a decision for Casting advancement at level 6");
    assert_eq!(dual_casting_decision.status, DecisionStatus::NeedsRuling);
    assert!(dual_casting_decision.tags.contains(&RuleTag::Npc));
    assert!(dual_casting_decision.tags.contains(&RuleTag::Gestalt));
    assert!(dual_casting_decision.explanation.contains(
        "advances casting track `Warmage` on multiple lanes simultaneously; relaxed as NPC"
    ));
}
