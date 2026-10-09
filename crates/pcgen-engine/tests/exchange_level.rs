//! `EXCHANGELEVEL` — a raw PCGen row that reached the snapshot as a string.
//!
//! PCGen's tag reference defines `EXCHANGELEVEL:w|x|y|z`:
//!
//! ```text
//! w: Class Name
//! x: Class level, minimum level required in donating class
//! y: Class level, maximum levels donated from class
//! z: Class level, lowest that donation can lower donating class level to
//!
//! EXCHANGELEVEL:Ex Paladin|11|10|1
//!   "Up to 10 levels of Ex Paladin can be exchanged as long as there were at
//!    least 11 levels of Ex Paladin available."
//! ```

use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/exchange.lst", 1, "exchange.pcc");
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
fn the_four_fields_are_named_from_the_spec() {
    let records = vec![record(
        "CLASS",
        "Blackguard",
        &[
            ("HD", "10"),
            ("TYPE", "PC.Prestige"),
            ("MAXLEVEL", "10"),
            ("EXCHANGELEVEL", "Ex Paladin|11|10|1"),
        ],
    )];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let exchange = pack
        .classes
        .get("Blackguard")
        .expect("Blackguard")
        .exchange_level
        .clone()
        .expect("EXCHANGELEVEL was parsed");

    assert_eq!(exchange.donating_class, "Ex Paladin", "w is the class name");
    assert_eq!(exchange.minimum_level, 11, "x is the minimum to qualify");
    assert_eq!(
        exchange.maximum_levels, 10,
        "y is the most that may be donated"
    );
    assert_eq!(
        exchange.floor_level, 1,
        "z is the floor it may be reduced to"
    );
}

#[test]
fn a_missing_specification_is_not_invented() {
    let records = vec![record("CLASS", "Fighter", &[("HD", "10")])];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    assert!(pack
        .classes
        .get("Fighter")
        .expect("Fighter")
        .exchange_level
        .is_none());
}

#[test]
fn an_empty_class_name_is_refused() {
    // Nothing sensible can be done with `|11|10|1`; better absent than wrong.
    let records = vec![record(
        "CLASS",
        "Odd",
        &[("HD", "8"), ("EXCHANGELEVEL", "|11|10|1")],
    )];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    assert!(pack
        .classes
        .get("Odd")
        .expect("Odd")
        .exchange_level
        .is_none());
}
