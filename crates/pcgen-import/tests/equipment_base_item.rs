//! `.pcg` `BASEITEM` / `EQMOD` parsing for customised equipment.

use pcgen_import::parse_pcg_draft_str;

#[test]
fn parses_base_item_and_eqmods_from_customization() {
    let text = "CHARACTERNAME:Custom\nEQUIPNAME:+2 scale mail|OUTPUTORDER:1|COST:4200.0|CUSTOMIZATION:[BASEITEM:Scale Mail|DATA:NAME=+2 scale mail$EQMOD=MWORKA.PLUS2A.STEEL$SPROP=(Scale Mail)]\n";
    let draft = parse_pcg_draft_str(text).unwrap();
    assert_eq!(
        draft
            .equipment_base_items
            .get("+2 scale mail")
            .map(String::as_str),
        Some("Scale Mail")
    );
    assert_eq!(
        draft.equipment_modifiers.get("+2 scale mail").cloned(),
        Some(vec![
            "MWORKA".to_string(),
            "PLUS2A".to_string(),
            "STEEL".to_string()
        ])
    );
}
