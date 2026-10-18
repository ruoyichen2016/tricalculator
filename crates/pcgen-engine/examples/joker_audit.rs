use pcgen_engine::{joker::JokerCase, Engine};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let case: JokerCase = serde_json::from_str(&std::fs::read_to_string(
        root.join("tests/fixtures/forum-builds/joker.json"),
    )?)?;
    let engine = Engine::new(pcgen_import::native::compile_database(
        &root.join("data/rules/rainbow-warweaver.rules.json"),
    )?);
    println!(
        "{}",
        serde_json::to_string_pretty(&engine.audit_joker_case(&case))?
    );
    Ok(())
}
