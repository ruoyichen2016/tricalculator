//! Deterministic fingerprints.
//!
//! The fingerprint is a stable, dependency-free FNV-1a 64-bit hash over an
//! explicitly ordered sequence of byte strings.  It is intended for cache
//! invalidation and provenance, not for security.
//!
//! [`rules_fingerprint`] hashes the *semantic* content of a compiled rule
//! pack (entities, typed definitions, progressions, variants) in a
//! deterministic order.  Import coverage statistics are deliberately
//! excluded: they are reporting metadata, and changing them must not
//! invalidate a character cache.

use crate::domain::ClassProgression;
use crate::rules::CompiledEntity;
use crate::variants::VariantDefinition;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Streaming FNV-1a 64-bit hasher with explicit separators.
#[derive(Debug, Clone)]
pub struct FingerprintHasher {
    state: u64,
}

impl Default for FingerprintHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl FingerprintHasher {
    pub fn new() -> Self {
        Self { state: FNV_OFFSET }
    }

    pub fn write(&mut self, part: &str) {
        // Length-prefix each part so that ("ab", "c") and ("a", "bc") differ.
        let prefix = format!("{}:", part.len());
        self.write_raw(prefix.as_bytes());
        self.write_raw(part.as_bytes());
    }

    fn write_raw(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u64::from(*byte);
            self.state = self.state.wrapping_mul(FNV_PRIME);
        }
    }

    pub fn finish(self) -> String {
        format!("fnv1a64:{:016x}", self.state)
    }
}

/// Hash an ordered list of string parts into a stable fingerprint string.
pub fn hash_parts(parts: &[String]) -> String {
    let mut hasher = FingerprintHasher::new();
    for part in parts {
        hasher.write(part);
    }
    hasher.finish()
}

/// Hash an ordered list of string slices into a stable fingerprint string.
pub fn stable_fingerprint<I, S>(parts: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut hasher = FingerprintHasher::new();
    for part in parts {
        hasher.write(part.as_ref());
    }
    hasher.finish()
}

fn write_entity(hasher: &mut FingerprintHasher, entity: &CompiledEntity) {
    hasher.write("entity");
    hasher.write(&entity.id.to_string());
    hasher.write(entity.category.as_str());
    hasher.write(&entity.name);
    // Creature traits are runtime semantics now that the engine consumes
    // them for snapshot fields such as vision / DR / SR / natural attacks.
    hasher.write("traits");
    hasher.write(&format!("{:?}", entity.traits));
    hasher.write("temporary_value");
    hasher.write(&format!("{:?}", entity.temporary_value));
    hasher.write("metadata");
    hasher.write(&format!("{:?}", entity.metadata));
    for effect in &entity.effects {
        hasher.write("effect");
        hasher.write(&format!("{:?}", effect.phase));
        hasher.write(&format!("{:?}", effect.action));
        hasher.write(&format!("{:?}", effect.condition));
        hasher.write(&effect.source.source);
    }
    for requirement in &entity.requirements {
        hasher.write("requirement");
        hasher.write(&format!("{requirement:?}"));
    }
    // `legacy_tags` / `unsupported_tags` are intentionally excluded: they are
    // reporting metadata, not runtime semantics.
}

fn write_progression(hasher: &mut FingerprintHasher, name: &str, progression: &ClassProgression) {
    hasher.write("progression");
    hasher.write(name);
    for value in &progression.base_attack_bonus {
        hasher.write(&value.to_string());
    }
    for value in &progression.fortitude {
        hasher.write(&value.to_string());
    }
    for value in &progression.reflex {
        hasher.write(&value.to_string());
    }
    for value in &progression.will {
        hasher.write(&value.to_string());
    }
    hasher.write(&progression.skill_points_per_level.to_string());
    hasher.write(&progression.first_level_skill_points.to_string());
    hasher.write(&progression.hit_die);
    hasher.write(&progression.skill_multiplier.to_string());
    hasher.write(&progression.levels_per_feat.to_string());
    for (class_id, levels) in &progression.add_levels {
        hasher.write(class_id);
        hasher.write(&levels.to_string());
    }
    if let Some(excluded) = &progression.excluded_class {
        hasher.write(excluded);
    }
    if let Some(exchange) = &progression.exchange_level {
        hasher.write(&exchange.donating_class);
        hasher.write(&exchange.minimum_level.to_string());
        hasher.write(&exchange.maximum_levels.to_string());
        hasher.write(&exchange.floor_level.to_string());
    }
    for skill in &progression.class_skills {
        hasher.write(skill);
    }
}

/// Hash what each class level grants.
///
/// This used to hash `ClassProgression::features_by_level`, a copy of the same
/// rows.  Hashing the class directly keeps the coverage now that the copy is
/// gone.
fn write_class_grants(hasher: &mut FingerprintHasher, name: &str, class: &crate::ClassDefinition) {
    hasher.write("class-grants");
    hasher.write(name);
    for (level, grants) in &class.level_abilities {
        hasher.write(&level.to_string());
        for grant in grants {
            hasher.write(&grant.render());
        }
    }
}

fn write_variant(hasher: &mut FingerprintHasher, name: &str, variant: &VariantDefinition) {
    hasher.write("variant");
    hasher.write(name);
    hasher.write(&variant.id);
    for value in &variant.replaces {
        hasher.write(value);
    }
    for value in &variant.grants {
        hasher.write(value);
    }
    for value in &variant.class_skills {
        hasher.write(value);
    }
    for value in &variant.bonus_tags {
        hasher.write(value);
    }
    for effect in &variant.effects {
        hasher.write("variant_effect");
        hasher.write(&format!("{:?}", effect.action));
    }
    hasher.write(&variant.spellcaster_advance.to_string());
}

/// Inputs needed to fingerprint a compiled rule pack.
///
/// `pcgen-import::CompiledRulePack` deserializes into this shape; keeping the
/// fingerprint function generic over borrowed maps avoids a dependency cycle
/// between `pcgen-core` and `pcgen-import`.
pub struct RuleFingerprintInput<'a> {
    pub game_mode: &'a str,
    pub sources: &'a [String],
    pub entities: &'a [CompiledEntity],
    pub progressions: &'a BTreeMap<String, ClassProgression>,
    /// Class definitions, hashed for their per-level grants.
    pub classes: &'a BTreeMap<String, crate::ClassDefinition>,
    pub variants: &'a BTreeMap<String, VariantDefinition>,
    /// Optional digest of typed definitions that are not (yet) expressible as
    /// entities/progressions, so that adding definitions still invalidates.
    pub typed_digest: &'a str,
}

/// Deterministic fingerprint of a compiled rule pack's runtime semantics.
pub fn rules_fingerprint(input: &RuleFingerprintInput<'_>) -> String {
    let mut hasher = FingerprintHasher::new();
    hasher.write("pcgen-rust/rules/v1");
    hasher.write(input.game_mode);

    let mut sources: Vec<&String> = input.sources.iter().collect();
    sources.sort();
    for source in sources {
        hasher.write("source");
        hasher.write(source);
    }

    // Entities are keyed by stable IDs and sorted, so graph load order does
    // not change the fingerprint.
    let mut entities: Vec<&CompiledEntity> = input.entities.iter().collect();
    entities.sort_by_key(|a| a.id.to_string());
    for entity in entities {
        write_entity(&mut hasher, entity);
    }

    for (name, progression) in input.progressions {
        write_progression(&mut hasher, name, progression);
    }

    for (name, class) in input.classes {
        write_class_grants(&mut hasher, name, class);
    }

    for (name, variant) in input.variants {
        write_variant(&mut hasher, name, variant);
    }

    hasher.write("typed");
    hasher.write(input.typed_digest);
    hasher.finish()
}

/// Human-readable build provenance attached to snapshots and reports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildProvenance {
    /// Fingerprint of the compiled rule pack semantics.
    #[serde(default)]
    pub rules_fingerprint: String,
    /// Upstream PCGen data revision (e.g. a commit SHA), when known.
    #[serde(default)]
    pub data_ref: Option<String>,
    /// Game mode of the compiled pack.
    #[serde(default)]
    pub game_mode: Option<String>,
}

impl BuildProvenance {
    pub fn new(
        rules_fingerprint: impl Into<String>,
        data_ref: Option<String>,
        game_mode: Option<String>,
    ) -> Self {
        Self {
            rules_fingerprint: rules_fingerprint.into(),
            data_ref,
            game_mode,
        }
    }

    /// Compact one-line rendering for CLI/output layers.
    pub fn render(&self) -> String {
        let mut out = self.rules_fingerprint.clone();
        if let Some(data_ref) = &self.data_ref {
            let _ = write!(out, " data={data_ref}");
        }
        if let Some(game_mode) = &self.game_mode {
            let _ = write!(out, " mode={game_mode}");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::EntityId;
    use crate::rules::{
        CompiledEntity, Effect, EffectAction, EffectPhase, Provenance as EffectSource,
    };
    use crate::vocab::EntityCategory;

    fn entity(key: &str, effects: Vec<Effect>) -> CompiledEntity {
        CompiledEntity {
            id: EntityId::from_pcgen("3e", "srd", "CLASS", key),
            category: EntityCategory::Class,
            name: key.to_string(),
            type_tags: Vec::new(),
            effects,
            requirements: Vec::new(),
            legacy_tags: Vec::new(),
            unsupported_tags: Vec::new(),
            source: "test.lst".to_string(),
            ability_category: None,
            temporary_value: None,
            traits: Default::default(),
            metadata: Default::default(),
        }
    }

    fn effect(value: &str) -> Effect {
        Effect {
            phase: EffectPhase::Class,
            condition: None,
            action: EffectAction::GrantTag {
                tag: value.to_string(),
            },
            source: EffectSource::new("test.lst", "test"),
        }
    }

    #[test]
    fn fingerprint_is_stable_and_order_independent() {
        let a = entity("Fighter", vec![effect("A")]);
        let b = entity("Wizard", vec![effect("B")]);
        let progressions = BTreeMap::new();
        let variants = BTreeMap::new();

        let first = rules_fingerprint(&RuleFingerprintInput {
            game_mode: "3e",
            sources: &["a.pcc".to_string(), "b.pcc".to_string()],
            entities: &[a.clone(), b.clone()],
            progressions: &progressions,
            classes: &BTreeMap::new(),
            variants: &variants,
            typed_digest: "typed-v1",
        });
        let second = rules_fingerprint(&RuleFingerprintInput {
            game_mode: "3e",
            sources: &["b.pcc".to_string(), "a.pcc".to_string()],
            entities: &[b, a],
            progressions: &progressions,
            classes: &BTreeMap::new(),
            variants: &variants,
            typed_digest: "typed-v1",
        });
        assert_eq!(first, second);
        assert!(first.starts_with("fnv1a64:"));
    }

    #[test]
    fn fingerprint_changes_with_creature_traits() {
        let progressions = BTreeMap::new();
        let variants = BTreeMap::new();
        let mut with_traits = entity("Fighter", vec![]);
        with_traits.traits.vision.push("Darkvision".to_string());

        let base = rules_fingerprint(&RuleFingerprintInput {
            game_mode: "3e",
            sources: &[],
            entities: &[entity("Fighter", vec![])],
            progressions: &progressions,
            classes: &BTreeMap::new(),
            variants: &variants,
            typed_digest: "typed-v1",
        });
        let changed = rules_fingerprint(&RuleFingerprintInput {
            game_mode: "3e",
            sources: &[],
            entities: &[with_traits],
            progressions: &progressions,
            classes: &BTreeMap::new(),
            variants: &variants,
            typed_digest: "typed-v1",
        });
        assert_ne!(base, changed);
    }

    #[test]
    fn fingerprint_changes_with_semantics() {
        let progressions = BTreeMap::new();
        let variants = BTreeMap::new();
        let base = rules_fingerprint(&RuleFingerprintInput {
            game_mode: "3e",
            sources: &[],
            entities: &[entity("Fighter", vec![effect("A")])],
            progressions: &progressions,
            classes: &BTreeMap::new(),
            variants: &variants,
            typed_digest: "typed-v1",
        });
        let changed = rules_fingerprint(&RuleFingerprintInput {
            game_mode: "3e",
            sources: &[],
            entities: &[entity("Fighter", vec![effect("B")])],
            progressions: &progressions,
            classes: &BTreeMap::new(),
            variants: &variants,
            typed_digest: "typed-v1",
        });
        assert_ne!(base, changed);
    }

    #[test]
    fn hash_parts_is_separator_safe() {
        let ab = hash_parts(&["ab".to_string(), "c".to_string()]);
        let bc = hash_parts(&["a".to_string(), "bc".to_string()]);
        assert_ne!(ab, bc);
    }
}
