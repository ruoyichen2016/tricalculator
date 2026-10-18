use pcgen_engine::{martial_case::MartialCase, Engine};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let case: MartialCase = serde_json::from_str(&std::fs::read_to_string(
        root.join("tests/fixtures/forum-builds/shadow-pounce.json"),
    )?)?;
    let pack = pcgen_import::native::compile_database(
        &root.join("data/rules/rainbow-warweaver.rules.json"),
    )?;
    let report = Engine::new(pack).audit_martial_case(&case);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
