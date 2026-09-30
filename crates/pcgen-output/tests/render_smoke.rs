use pcgen_engine::CharacterSnapshot;
use pcgen_output::{
    render_csv, render_html, render_json, render_markdown, render_template_file_snapshot,
    render_template_snapshot, render_text,
};
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn text_snapshot_renders() {
    let snapshot = CharacterSnapshot {
        draft_id: "d1".to_string(),
        name: "Gestalt".to_string(),
        race_id: "Human".to_string(),
        total_level: 1,
        stats: BTreeMap::new(),
        hit_points: 10,
        base_attack_bonus: 1,
        saves: BTreeMap::new(),
        gestalt: None,
        explanations: BTreeMap::new(),
        diagnostics: Vec::new(),
        ..CharacterSnapshot::default()
    };
    let text = render_text(&snapshot);
    assert!(text.contains("Gestalt"));
    assert!(text.contains("HP:         10"));
    assert!(render_markdown(&snapshot).contains("# Gestalt"));
    assert!(render_csv(&snapshot).contains("combat,hp,10"));
    assert!(render_html(&snapshot).contains("<!doctype html"));
    assert!(render_json(&snapshot)
        .unwrap()
        .contains("\"hit_points\": 10"));
    let template = "Name: ${name?upper_case}\nHP: ${hp}\n";
    assert_eq!(
        render_template_snapshot(template, &snapshot),
        "Name: GESTALT\nHP: 10\n"
    );
}

#[test]
fn macro_and_include_render() {
    let snapshot = CharacterSnapshot {
        name: "Aria".to_string(),
        ..CharacterSnapshot::default()
    };
    let mut saves = BTreeMap::new();
    saves.insert("Fortitude".to_string(), 4);
    let snapshot = CharacterSnapshot { saves, ..snapshot };
    let list_macro = "<#macro ab short value><x s=\"${short}\" v=\"${value}\"/></#macro><#list saves as save><@ab short=\"${save.name}\" value=\"${save.value}\"/></#list>";
    assert_eq!(
        render_template_snapshot(list_macro, &snapshot),
        "<x s=\"Fortitude\" v=\"4\"/>"
    );
    let macro_template = "<#macro greet who>Hello ${who}!</#macro><@greet who=\"World\"/>";
    assert_eq!(
        render_template_snapshot(macro_template, &snapshot),
        "Hello World!"
    );
    let default_template = "<#macro greet who=\"Aria\">Hello ${who}!</#macro><@greet/>";
    assert_eq!(
        render_template_snapshot(default_template, &snapshot),
        "Hello Aria!"
    );

    let dir = std::env::temp_dir().join("pcgen-rust-template-test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("part.ftl"), "<#macro label>Name: ${name}</#macro>").unwrap();
    std::fs::write(
        dir.join("main.ftl"),
        "#import \"part.ftl\" as part\n<@part.label/>",
    )
    .unwrap();
    let rendered = render_template_file_snapshot(dir.join("main.ftl"), &snapshot).unwrap();
    assert_eq!(rendered.trim(), "Name: Aria");
}

#[test]
fn official_subset_golden() {
    let mut saves = BTreeMap::new();
    saves.insert("Fortitude".to_string(), 4);
    saves.insert("Reflex".to_string(), 5);
    saves.insert("Will".to_string(), 2);
    let snapshot = CharacterSnapshot {
        name: "Aria".to_string(),
        saves,
        ..CharacterSnapshot::default()
    };
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/output-sheets/pcgen_official_subset.ftl");
    let expected = include_str!("../../../tests/fixtures/output-sheets/pcgen_official_subset.xml");
    let rendered = render_template_file_snapshot(fixture, &snapshot).unwrap();
    assert_eq!(rendered.trim(), expected.trim());
}

#[test]
fn real_official_sheet_smoke() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../raw/pcgen-master/outputsheets/d20/5e/htmlxml/csheet_5e_xml_generic.html.ftl",
    );
    if !path.exists() {
        eprintln!("skipping real official sheet: {} missing", path.display());
        return;
    }
    let snapshot = CharacterSnapshot {
        name: "Aria".to_string(),
        ..CharacterSnapshot::default()
    };
    let rendered = render_template_file_snapshot(&path, &snapshot).expect("sheet should render");
    assert!(rendered.contains("<character>"));
    assert!(!rendered.contains("<#"));
    assert!(!rendered.contains("</@"));
    assert!(!rendered.contains("${"));
}

#[test]
fn html_sheet_includes_deep_sections() {
    use pcgen_core::domain::EquipmentDefinition;

    let mut stats = BTreeMap::new();
    stats.insert("STR".to_string(), 16);
    stats.insert("DEX".to_string(), 12);
    let mut saves = BTreeMap::new();
    saves.insert("Fortitude".to_string(), 5);
    let mut skills = BTreeMap::new();
    skills.insert("Climb".to_string(), 6);
    let mut caster_levels = BTreeMap::new();
    caster_levels.insert("Wizard".to_string(), 3);
    let mut spell_dcs = BTreeMap::new();
    spell_dcs.insert("Magic Missile".to_string(), 14);
    let mut prepared_spells = BTreeMap::new();
    prepared_spells.insert("Wizard|Magic Missile".to_string(), 2);
    let mut movement = BTreeMap::new();
    movement.insert("walk".to_string(), 30);
    let snapshot = CharacterSnapshot {
        name: "Aria".to_string(),
        stats,
        saves,
        skills,
        caster_levels,
        spell_dcs,
        prepared_spells,
        movement,
        active_abilities: vec!["Power Attack".to_string()],
        equipment: vec![EquipmentDefinition::new("Longsword", "Longsword")],
        ..CharacterSnapshot::default()
    };
    let html = render_html(&snapshot);
    for section in [
        "<h2>Ability Scores</h2>",
        "<h2>Combat</h2>",
        "<h2>Saving Throws</h2>",
        "<h2>Skills</h2>",
        "<h2>Feats &amp; Abilities</h2>",
        "<h2>Spellcasting</h2>",
        "<h2>Equipment</h2>",
        "<h2>Traits</h2>",
        "@media print",
    ] {
        assert!(html.contains(section), "missing section: {section}");
    }
}
