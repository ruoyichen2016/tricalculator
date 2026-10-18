//! Editable native rule database. Only the base campaign crosses the LST boundary.
//! Native definitions lower directly to the same runtime model as imported rules.
use pcgen_core::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleDatabase {
    #[serde(default)]
    pub maneuvers: BTreeMap<String, pcgen_core::martial::Maneuver>,
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub class_profiles: BTreeMap<String, ClassProfile>,
    pub format: String,
    pub ruleset: String,
    pub base_campaign: String,
    pub sources: BTreeMap<String, RuleSource>,
    pub rules: Vec<NativeRule>,
    #[serde(default)]
    pub spells: Vec<SpellDefinition>,
    #[serde(default)]
    pub skills: Vec<SkillDefinition>,
    #[serde(default)]
    pub spell_profiles: BTreeMap<String, pcgen_core::magic::SpellProfile>,
    #[serde(default)]
    pub spell_lists: BTreeMap<String, BTreeMap<String, u32>>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassProfile {
    #[serde(default)]
    pub effects: Vec<Effect>,
    pub source: String,
    pub magic: pcgen_core::magic::ClassMagic,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleModule {
    #[serde(default)]
    skills: Vec<SkillDefinition>,
    #[serde(default)]
    maneuvers: BTreeMap<String, pcgen_core::martial::Maneuver>,
    format: String,
    #[serde(default)]
    sources: BTreeMap<String, RuleSource>,
    #[serde(default)]
    rules: Vec<NativeRule>,
    #[serde(default)]
    spells: Vec<SpellDefinition>,
    #[serde(default)]
    spell_profiles: BTreeMap<String, pcgen_core::magic::SpellProfile>,
    #[serde(default)]
    spell_lists: BTreeMap<String, BTreeMap<String, u32>>,
    #[serde(default)]
    class_profiles: BTreeMap<String, ClassProfile>,
}
fn merge_unique<T>(
    target: &mut BTreeMap<String, T>,
    values: BTreeMap<String, T>,
    kind: &str,
) -> Result<(), String> {
    for (key, value) in values {
        if target.insert(key.clone(), value).is_some() {
            return Err(format!("Duplicate {kind}: {key}"));
        }
    }
    Ok(())
}
pub fn database_paths(path: &Path) -> Result<Vec<PathBuf>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let db: RuleDatabase = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(std::iter::once(path.to_path_buf())
        .chain(
            db.includes
                .iter()
                .map(|p| path.parent().unwrap_or(Path::new(".")).join(p)),
        )
        .collect())
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSource {
    pub title: String,
    pub reference: String,
    pub locator: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRule {
    /// Stable source-qualified identity, independent of the display name.
    pub id: String,
    pub source: String,
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub replaces_base: bool,
    #[serde(default)]
    pub requirements: Vec<Predicate>,
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// Unimplemented mechanics remain data, never a silent claim of support.
    #[serde(default)]
    pub pending: Vec<String>,
    pub mechanics: NativeMechanics,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativeMechanics {
    Class {
        hit_die: u32,
        max_level: u32,
        prestige: bool,
        attack: AttackProgression,
        fortitude: SaveProgression,
        reflex: SaveProgression,
        will: SaveProgression,
        skill_points: u32,
        class_skills: Vec<String>,
        #[serde(default)]
        feat_pools: Vec<ClassFeatPool>,
        #[serde(default)]
        casting: Option<NativeCasting>,
        #[serde(default)]
        magic: pcgen_core::magic::ClassMagic,
        #[serde(default)]
        advancement: Vec<CastingAdvance>,
    },
    Feat {
        types: Vec<String>,
        #[serde(default)]
        repeatable: bool,
        #[serde(default)]
        choice: Option<FeatChoice>,
        #[serde(default)]
        spell_rule: Option<pcgen_core::magic::SpellRule>,
        #[serde(default)]
        metamagic_cost: Option<pcgen_core::magic::MetamagicCost>,
        #[serde(default)]
        sudden_rule: Option<pcgen_core::magic::SpellRule>,
    },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatChoice {
    pub category: String,
    pub types: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassFeatPool {
    #[serde(default)]
    pub allowed_feats: Vec<String>,
    #[serde(default)]
    pub choice_restrictions: BTreeMap<String, Vec<String>>,
    pub id: String,
    pub name: String,
    pub levels: Vec<u32>,
    pub allowed_types: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastingAdvance {
    pub level: u32,
    pub scope: AdvancementScope,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpellSlotRow {
    pub level: u32,
    pub slots: Vec<u32>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvancementScope {
    Arcane,
    Divine,
    Any,
}
impl AdvancementScope {
    fn name(&self) -> &'static str {
        match self {
            Self::Arcane => "Arcane",
            Self::Divine => "Divine",
            Self::Any => "SPELLCASTER",
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCasting {
    pub ability: String,
    pub mode: CastingType,
    pub power: SpellPowerType,
    pub slots: Vec<SpellSlotRow>,
    #[serde(default)]
    pub known: Vec<SpellSlotRow>,
}

pub fn read_database(path: &Path) -> Result<RuleDatabase, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut db: RuleDatabase =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if db.format != "gladius.rules/1" {
        return Err(format!("unsupported native rule format: {}", db.format));
    }
    for module in &db.includes {
        let module_path = path.parent().unwrap_or(Path::new(".")).join(module);
        let text = std::fs::read_to_string(&module_path)
            .map_err(|e| format!("{}: {e}", module_path.display()))?;
        let m: RuleModule =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", module_path.display()))?;
        if m.format != "gladius.module/1" {
            return Err(format!("Unsupported module format: {}", m.format));
        }
        merge_unique(&mut db.sources, m.sources, "source")?;
        merge_unique(&mut db.spell_profiles, m.spell_profiles, "spell profile")?;
        merge_unique(&mut db.spell_lists, m.spell_lists, "spell list")?;
        merge_unique(&mut db.class_profiles, m.class_profiles, "class profile")?;
        merge_unique(&mut db.maneuvers, m.maneuvers, "maneuver")?;
        db.rules.extend(m.rules);
        db.spells.extend(m.spells);
        db.skills.extend(m.skills);
    }
    Ok(db)
}
pub fn base_path(path: &Path, db: &RuleDatabase) -> PathBuf {
    path.parent()
        .unwrap_or(Path::new("."))
        .join(&db.base_campaign)
}
pub fn compile_database(path: &Path) -> Result<CompiledRulePack, String> {
    let db = read_database(path)?;
    let (mut pack, _) = crate::compile_pcc(base_path(path, &db)).map_err(|e| e.to_string())?;
    merge_unique(&mut pack.maneuvers, db.maneuvers.clone(), "maneuver")?;
    for skill in &db.skills {
        if pack.skills.values().any(|s| {
            s.id.eq_ignore_ascii_case(&skill.id) || s.name.eq_ignore_ascii_case(&skill.name)
        }) {
            return Err(format!(
                "Native skill collides with base skill: {}",
                skill.name
            ));
        }
        pack.skills.insert(skill.id.clone(), skill.clone());
    }
    for spell in &db.spells {
        pack.spells.insert(spell.id.clone(), spell.clone());
    }
    for (class, entries) in &db.spell_lists {
        for (name, level) in entries {
            let spell = pack
                .spells
                .values_mut()
                .find(|s| s.id.eq_ignore_ascii_case(name) || s.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| format!("{class}: spell list references missing spell {name}"))?;
            spell.class_levels.insert(class.clone(), *level);
        }
    }
    for (name, profile) in &db.spell_profiles {
        let spell = pack
            .spells
            .values_mut()
            .find(|s| s.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("Missing spell for profile: {name}"))?;
        let casting_time = profile.casting_time.or(spell.profile.casting_time);
        let somatic = profile.somatic.or(spell.profile.somatic);
        spell.profile = profile.clone();
        spell.profile.casting_time = casting_time;
        spell.profile.somatic = somatic;
    }
    pack.ability_categories
        .entry("feat".into())
        .or_insert_with(|| AbilityCategoryDefinition {
            choice_restrictions: Default::default(),
            id: "feat".into(),
            name: "FEAT".into(),
            category: Some("FEAT".into()),
            type_tags: Vec::new(),
            ability_list: Vec::new(),
            pool: None,
        });
    let mut ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for rule in &db.rules {
        let source = db
            .sources
            .get(&rule.source)
            .ok_or_else(|| format!("{}: unknown source {}", rule.id, rule.source))?;
        let kind = match &rule.mechanics {
            NativeMechanics::Class { .. } => "class",
            NativeMechanics::Feat { .. } => "feat",
        };
        let id = EntityId::new(&db.ruleset, &rule.source, kind, &rule.key);
        if rule.id != id.as_string() {
            return Err(format!("{}: expected source-qualified id {id}", rule.id));
        }
        if !ids.insert(rule.id.clone()) || !keys.insert((kind, id.key.clone())) {
            return Err(format!("duplicate native rule identity: {}", rule.id));
        }
        let category = if kind == "class" {
            EntityCategory::Class
        } else {
            EntityCategory::Feat
        };
        let same_entity = |e: &CompiledEntity| {
            (e.category == category || (kind == "feat" && e.category == EntityCategory::Ability))
                && (e.id.key.eq_ignore_ascii_case(&id.key)
                    || e.name.eq_ignore_ascii_case(&rule.key))
        };
        let definition_exists = if kind == "class" {
            pack.classes.contains_key(&rule.key)
        } else {
            pack.abilities.contains_key(&id.key)
        };
        let collision = definition_exists || pack.entities.iter().any(&same_entity);
        if collision && !rule.replaces_base {
            return Err(format!(
                "{} collides with base data; explicit replacement required",
                rule.id
            ));
        }
        if rule.replaces_base && !collision {
            return Err(format!(
                "{} declares replacement but base rule is absent",
                rule.id
            ));
        }
        pack.entities.retain(|e| {
            !same_entity(e)
                && !(kind == "class"
                    && e.category == EntityCategory::ClassLevel
                    && e.id.key.split_once('#').is_some_and(|(owner, _)| {
                        owner.eq_ignore_ascii_case(&id.key) || owner.eq_ignore_ascii_case(&rule.key)
                    }))
        });
        let mut entity = CompiledEntity {
            id,
            category,
            name: rule.name.clone(),
            type_tags: Vec::new(),
            effects: rule.effects.clone(),
            requirements: rule.requirements.clone(),
            legacy_tags: Vec::new(),
            unsupported_tags: Vec::new(),
            source: format!(
                "{} — {} ({})",
                source.title, source.locator, source.reference
            ),
            ability_category: None,
            temporary_value: None,
            facts: BTreeMap::new(),
            traits: Default::default(),
            metadata: Default::default(),
        };
        entity.facts.insert("NativeRuleId".into(), rule.id.clone());
        if !rule.pending.is_empty() {
            entity
                .facts
                .insert("NativeRulePending".into(), rule.pending.join("; "));
        }
        match &rule.mechanics {
            NativeMechanics::Class {
                hit_die,
                max_level,
                prestige,
                attack,
                fortitude,
                reflex,
                will,
                skill_points,
                class_skills,
                feat_pools,
                casting,
                magic,
                advancement,
            } => {
                if *hit_die == 0
                    || *max_level == 0
                    || advancement
                        .iter()
                        .any(|row| row.level == 0 || row.level > *max_level)
                {
                    return Err(format!("{}: invalid class level or hit die", rule.id));
                }
                let mut class = ClassDefinition::new(&rule.key, &rule.name);
                class.hit_die = format!("d{hit_die}");
                class.max_level = *max_level;
                class.is_prestige = *prestige;
                class.attack_progression = *attack;
                class.fortitude = *fortitude;
                class.reflex = *reflex;
                class.will = *will;
                class.skill_points_per_level = *skill_points;
                class.class_skills = class_skills.clone();
                class.type_tags = vec![
                    if *prestige { "Prestige" } else { "Base" }.into(),
                    "PC".into(),
                ];
                if let Some(casting) = casting {
                    class.spellcasting_ability = Some(casting.ability.clone());
                    class.casting_type = casting.mode;
                    class.spell_power = casting.power;
                    class.spell_type = power_name(casting.power).into();
                    class.spell_slots = casting
                        .slots
                        .iter()
                        .map(|row| (row.level, row.slots.clone()))
                        .collect();
                    class.spells_known = casting
                        .known
                        .iter()
                        .map(|row| (row.level, row.slots.clone()))
                        .collect();
                }
                class.level_spellcaster_advance = advancement
                    .iter()
                    .map(|row| (row.level, vec![row.scope.name().into()]))
                    .collect();
                entity.type_tags = class.type_tags.clone();
                entity.metadata.class_magic = Some(magic.clone());
                for grant in &magic.sudden_grants {
                    let id = format!(
                        "{}_sudden_{}",
                        rule.key.to_lowercase().replace(' ', "_"),
                        grant.level
                    );
                    pack.ability_categories.insert(
                        id.clone(),
                        AbilityCategoryDefinition {
                            choice_restrictions: Default::default(),
                            id,
                            name: format!("{} {} replacement", rule.key, grant.level),
                            category: Some("FEAT".into()),
                            type_tags: vec!["Metamagic".into()],
                            ability_list: vec![],
                            pool: Some("0".into()),
                        },
                    );
                }
                for pool in feat_pools {
                    if pool.id.is_empty()
                        || pack.ability_categories.contains_key(&pool.id)
                        || pool
                            .levels
                            .iter()
                            .any(|level| *level == 0 || *level > *max_level)
                        || pool.levels.iter().collect::<BTreeSet<_>>().len() != pool.levels.len()
                    {
                        return Err(format!(
                            "{}: invalid or duplicate feat pool {}",
                            rule.id, pool.id
                        ));
                    }
                    let variable = format!("NativePool_{}", pool.id);
                    let class_variable = formula_vocab::class_level_variable(&rule.key);
                    let expression = pool
                        .levels
                        .iter()
                        .map(|level| format!("if({class_variable}>={level},1,0)"))
                        .collect::<Vec<_>>()
                        .join("+");
                    pack.ability_categories.insert(
                        pool.id.clone(),
                        AbilityCategoryDefinition {
                            choice_restrictions: pool.choice_restrictions.clone(),
                            id: pool.id.clone(),
                            name: pool.name.clone(),
                            category: Some("FEAT".into()),
                            type_tags: pool.allowed_types.clone(),
                            ability_list: pool.allowed_feats.clone(),
                            pool: Some(variable.clone()),
                        },
                    );
                    entity.effects.push(Effect {
                        phase: EffectPhase::Class,
                        condition: None,
                        action: EffectAction::DefineVariable {
                            name: variable,
                            expression,
                        },
                        source: Provenance::new(&entity.source, format!("{} feat grant", rule.id)),
                    });
                }
                pack.progressions
                    .insert(rule.key.clone(), ClassProgression::from_class(&class));
                pack.classes.insert(rule.key.clone(), class);
            }
            NativeMechanics::Feat {
                types,
                repeatable,
                choice,
                spell_rule,
                metamagic_cost,
                sudden_rule,
            } => {
                entity.metadata.sudden_rule = *sudden_rule;
                entity.metadata.metamagic_cost = *metamagic_cost;
                entity.metadata.spell_rule = *spell_rule;
                entity.type_tags = types.clone();
                let category = if types.iter().any(|t| t == "Flaw") {
                    "Flaw"
                } else {
                    "FEAT"
                };
                if category == "Flaw" {
                    pack.ability_categories.insert(
                        "flaw".into(),
                        AbilityCategoryDefinition {
                            choice_restrictions: Default::default(),
                            id: "flaw".into(),
                            name: "Flaw".into(),
                            category: Some("Flaw".into()),
                            type_tags: vec!["Flaw".into()],
                            ability_list: Vec::new(),
                            pool: Some("2".into()),
                        },
                    );
                }
                entity.ability_category = Some(category.into());
                pack.abilities.insert(
                    entity.id.key.clone(),
                    AbilityDefinition {
                        id: entity.id.key.clone(),
                        name: rule.name.clone(),
                        category: Some(category.into()),
                        type_tags: types.clone(),
                        stack: Some(*repeatable),
                        multi: Some(*repeatable),
                        choose: choice.as_ref().map(|choice| {
                            format!(
                                "ABILITY|{}|TYPE={}",
                                choice.category,
                                choice.types.join(",")
                            )
                        }),
                    },
                );
            }
        }
        pack.entities.push(entity);
    }
    let semantics = serde_json::to_string(&db).map_err(|e| e.to_string())?;
    pack.header.rules_fingerprint =
        stable_fingerprint([pack.header.rules_fingerprint.as_str(), semantics.as_str()]);
    pack.header.sources.push(path.display().to_string());
    for (id, profile) in &db.class_profiles {
        if !pack.classes.contains_key(id) {
            return Err(format!("Profile references unknown class {id}"));
        }
        let source = db
            .sources
            .get(&profile.source)
            .ok_or("Class profile source unavailable")?;
        let entity = pack
            .entities
            .iter_mut()
            .find(|e| e.category == EntityCategory::Class && e.name.eq_ignore_ascii_case(id))
            .ok_or("Class entity unavailable")?;
        if entity.metadata.class_magic.is_some() {
            return Err(format!("Duplicate mechanics for {id}"));
        }
        entity.metadata.class_magic = Some(profile.magic.clone());
        entity.effects.extend(profile.effects.clone());
        entity.source = format!("{}; {} ({})", entity.source, source.title, source.reference);
    }
    let grants: Vec<_> = pack
        .entities
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            e.metadata
                .class_magic
                .as_ref()
                .map(|m| (i, e.name.clone(), m.bonus_feats.clone()))
        })
        .collect();
    for (index, class, grants) in grants {
        for grant in grants {
            let feat = pack
                .entities
                .iter()
                .find(|e| {
                    e.name.eq_ignore_ascii_case(&grant.feat)
                        && matches!(e.category, EntityCategory::Feat | EntityCategory::Ability)
                })
                .ok_or_else(|| format!("Missing granted feat {}", grant.feat))?
                .id
                .clone();
            let e = &mut pack.entities[index];
            e.effects.push(Effect {
                phase: EffectPhase::Class,
                condition: Some(Predicate::ClassRequirement {
                    specs: vec![ClassSpec::Class {
                        class_id: class.clone(),
                        min_level: grant.level,
                    }],
                    min_count: 1,
                }),
                action: EffectAction::Grant { entity: feat },
                source: Provenance::new(&e.source, "Automatic class bonus feat"),
            });
        }
    }
    Ok(pack)
}
fn power_name(power: SpellPowerType) -> &'static str {
    match power {
        SpellPowerType::Arcane => "Arcane",
        SpellPowerType::Divine => "Divine",
        SpellPowerType::Psionic => "Psionic",
        _ => "SPELLCASTER",
    }
}
