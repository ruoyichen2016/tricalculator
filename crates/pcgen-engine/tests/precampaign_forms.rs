//! `PRECAMPAIGN:x,y,y` — which data sets are loaded.
//!
//! PCGen's reference:
//!
//! ```text
//! PRECAMPAIGN:x,y,y
//!   x: Number (Number of items required to pass)
//!   y: Text (Data set name as set by the CAMPAIGN tag).
//!   y: BOOKTYPE=Text
//!   y: INCLUDES=Text (a data set included in any selected campaign)
//!   y: INCLUDESBOOKTYPE=Text
//! ```
//!
//! "Sets prerequisites for LST objects based upon the data sets loaded into
//! PCGen. If the set is loaded it passes, if not it doesn't."
//!
//! `PreCampaignTester` counts matching *campaigns* and compares with the operand,
//! and the list wrapper sums the selectors — the same shape as the spell tags.
//! Two details matter and are easy to get wrong:
//!
//! * a plain name and `BOOKTYPE=` look at the **selected** campaigns only, while
//!   `INCLUDES=` / `INCLUDESBOOKTYPE=` also look at what those include;
//! * the name is matched against the campaign's `KEY:`, because
//!   `Globals.getCampaignKeyedSilently` compares `getKeyName()`.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/campaign.lst", 1, "campaign.pcc");
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

/// A pack compiled from Complete Divine, which includes the RSRD.
fn campaigns() -> Vec<pcgen_core::CampaignSource> {
    vec![
        pcgen_core::CampaignSource {
            name: "Complete Divine".to_string(),
            key: Some("complete-divine".to_string()),
            book_types: vec!["Supplement".to_string()],
            is_root: true,
        },
        pcgen_core::CampaignSource {
            name: "Revised System Reference Document".to_string(),
            key: Some("rsrd".to_string()),
            book_types: vec!["Core35e".to_string()],
            is_root: false,
        },
    ]
}

/// Is `BONUS:VAR|Gated|1|<gate>` applied?
fn gated(gate: &str) -> bool {
    let records = vec![record(
        "RACE",
        "Test Race",
        &[("BONUS", &format!("VAR|Gated|1|{gate}"))],
    )];
    let (mut pack, report) = lower_records(
        Some("35e".to_string()),
        vec!["campaign".to_string()],
        &records,
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    pack.header.campaigns = campaigns();
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn a_plain_name_matches_the_key_of_a_selected_campaign() {
    assert!(gated("PRECAMPAIGN:1,complete-divine"));
    // `getKeyName()` is the `KEY:` when there is one, so the display name does
    // not match.
    assert!(!gated("PRECAMPAIGN:1,Complete Divine"));
    // The RSRD is loaded, but only as an include.
    assert!(!gated("PRECAMPAIGN:1,rsrd"));
}

#[test]
fn includes_sees_the_included_campaigns() {
    assert!(gated("PRECAMPAIGN:1,INCLUDES=rsrd"));
    assert!(gated("PRECAMPAIGN:1,INCLUDES=complete-divine"));
    assert!(!gated("PRECAMPAIGN:1,INCLUDES=epic-level-handbook"));
    // The Complete Divine class rows gate on exactly this, and its `.pcc` does
    // not include the Epic Level Handbook.
    assert!(!gated("PRECAMPAIGN:1,INCLUDES=Epic Level Handbook"));
}

#[test]
fn book_types_are_split_between_selected_and_included() {
    // The RSRD's book type is visible only through `INCLUDESBOOKTYPE=`.
    assert!(!gated("PRECAMPAIGN:1,BOOKTYPE=Core35e"));
    assert!(gated("PRECAMPAIGN:1,INCLUDESBOOKTYPE=Core35e"));
    // Complete Divine is the selected campaign, so both spellings find it.
    assert!(gated("PRECAMPAIGN:1,BOOKTYPE=Supplement"));
    assert!(gated("PRECAMPAIGN:1,INCLUDESBOOKTYPE=Supplement"));
    // Case-insensitively.
    assert!(gated("PRECAMPAIGN:1,INCLUDESBOOKTYPE=core35e"));
}

#[test]
fn the_count_sums_across_the_selectors() {
    // A plain name counts selected campaigns only, so the RSRD contributes
    // nothing here even though it is loaded as an include.
    assert!(gated("PRECAMPAIGN:1,complete-divine,epic-level-handbook"));
    assert!(!gated("PRECAMPAIGN:2,complete-divine,epic-level-handbook"));
    assert!(!gated("PRECAMPAIGN:1,rsrd,epic-level-handbook"));
    assert!(gated(
        "PRECAMPAIGN:2,INCLUDES=rsrd,INCLUDES=complete-divine"
    ));
    assert!(!gated(
        "PRECAMPAIGN:3,INCLUDES=rsrd,INCLUDES=complete-divine"
    ));
    // A selector that matches two campaigns contributes two to the total.
    assert!(gated(
        "PRECAMPAIGN:2,INCLUDES=rsrd,INCLUDESBOOKTYPE=Supplement"
    ));
}

#[test]
fn a_negated_campaign_gate_holds_when_the_set_is_not_loaded() {
    assert!(gated("!PRECAMPAIGN:1,INCLUDES=epic-level-handbook"));
    assert!(!gated("!PRECAMPAIGN:1,INCLUDES=rsrd"));
}
