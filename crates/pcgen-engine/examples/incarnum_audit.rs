use pcgen_engine::{incarnum_case::IncarnumCase, Engine};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let case: IncarnumCase = serde_json::from_str(&std::fs::read_to_string(
        root.join("tests/fixtures/forum-builds/warforged-umbral.json"),
    )?)?;
    let pack = pcgen_import::native::compile_database(
        &root.join("data/rules/rainbow-warweaver.rules.json"),
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&Engine::new(pack).audit_incarnum_case(&case))?
    );
    Ok(())
}
