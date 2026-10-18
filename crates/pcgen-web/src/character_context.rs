//! Resolve a saved character and its current rules through one shared load path.
use super::*;
#[derive(Default, Deserialize)]
pub(super) struct CharacterQuery {
    #[serde(default)]
    pub id: String,
}
pub(super) fn load(
    state: &AppState,
    id: &str,
) -> Result<(NativeSaveEnvelope, Arc<Engine>), String> {
    let mut envelope = state.repository.load(id).map_err(|e| e.to_string())?;
    envelope.character.refresh_build_projection()?;
    let pcc = envelope
        .pcc
        .as_deref()
        .ok_or("The character has no rule database")?;
    let engine = cached_engine(state, &resolve_path(&state.config.data_root, pcc))?;
    envelope.rules_fingerprint=engine.pack.header.rules_fingerprint.clone();
    envelope.data_ref=engine.pack.header.data_ref.clone();
    Ok((envelope, engine))
}

pub(super) fn classes(engine: &Engine) -> Vec<&pcgen_core::ClassDefinition> {
    engine
        .pack
        .classes
        .values()
        .filter(|c| {
            engine
                .pack
                .entities
                .iter()
                .any(|e| e.name.eq_ignore_ascii_case(&c.id) && e.metadata.class_magic.is_some())
        })
        .collect()
}

pub(super) fn first_casting_track(engine: &Engine, d: &pcgen_core::CharacterDraft) -> String {
    d.class_levels()
        .into_iter()
        .find(|l| {
            engine
                .pack
                .classes
                .get(&l.class_id)
                .is_some_and(|c| c.spellcasting_ability.is_some() && !c.is_prestige)
        })
        .map(|l| l.class_id)
        .unwrap_or_default()
}
