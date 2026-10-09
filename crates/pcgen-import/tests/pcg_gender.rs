//! `GENDER:` from a `.pcg`, which the import used to drop.
//!
//! 40 of the 42 vendored `.pcg` files carry a `GENDER:` line, and the character
//! model had no field for it — so `PREGENDER` (60 uses in the data) could never
//! be evaluated.

use pcgen_import::parse_pcg_draft_str;

#[test]
fn gender_is_imported() {
    let draft =
        parse_pcg_draft_str("CHARACTERNAME:Test\nGENDER:Female\nRACE:Human\n").expect("parsed");
    assert_eq!(draft.gender, "Female");
}

#[test]
fn a_missing_gender_stays_empty() {
    let draft = parse_pcg_draft_str("CHARACTERNAME:Test\nRACE:Human\n").expect("parsed");
    assert_eq!(draft.gender, "");
}
