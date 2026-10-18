//! Chronological qualification. Numeric availability is not a certificate of legality.
use crate::{CharacterDraft, Engine};
use pcgen_core::{build::*, CompiledEntity, Predicate};
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct BuildAudit {
    pub recorded_through: u32,
    pub decisions: Vec<RuleDecision>,
}
impl BuildAudit {
    pub fn errors(&self) -> Vec<String> {
        self.decisions
            .iter()
            .filter(|d| d.status == DecisionStatus::Failed)
            .map(|d| format!("Level {}: {}: {}", d.level, d.subject, d.explanation))
            .collect()
    }
    pub fn status(&self) -> DecisionStatus {
        for status in [
            DecisionStatus::Failed,
            DecisionStatus::MissingEvidence,
            DecisionStatus::NeedsRuling,
            DecisionStatus::NotImplemented,
        ] {
            if self.decisions.iter().any(|d| d.status == status) {
                return status;
            }
        }
        DecisionStatus::Satisfied
    }
}
fn decision(
    level: u32,
    subject: &str,
    status: DecisionStatus,
    explanation: impl Into<String>,
    source: Option<&str>,
) -> RuleDecision {
    RuleDecision {
        level,
        subject: subject.into(),
        status,
        explanation: explanation.into(),
        source: source.map(str::to_owned),
    }
}
fn requirement_label(pre: &Predicate) -> String {
    let bound = |min: Option<i32>, max: Option<i32>| match (min, max) {
        (Some(a), Some(b)) => format!("{a}–{b}"),
        (Some(a), None) => format!("at least {a}"),
        (None, Some(b)) => format!("at most {b}"),
        _ => "recorded".into(),
    };
    match pre {
        Predicate::Stat { stat, min, max } => format!("{stat} {}", bound(*min, *max)),
        Predicate::Alignment { allowed } => format!("Alignment: {}", allowed.join(" or ")),
        Predicate::SkillRanks { specs, min_count } => format!(
            "Meet {min_count} skill requirement(s): {}",
            specs
                .iter()
                .map(|s| format!("{} {} ranks", s.name, s.min_ranks))
                .collect::<Vec<_>>()
                .join("; ")
        ),
        Predicate::AbilityRequirement {
            category,
            wanted,
            excluded,
            min_count,
        } => format!(
            "Have {min_count} {category} selection(s) from {}; excluded: {}",
            wanted
                .iter()
                .map(|a| match a {
                    pcgen_core::AbilityRef::Name(n) => n.clone(),
                    pcgen_core::AbilityRef::Type(t) => format!("{t} type"),
                })
                .collect::<Vec<_>>()
                .join(", "),
            if excluded.is_empty() {
                "none".into()
            } else {
                excluded.join(", ")
            }
        ),
        Predicate::SpellType { specs, count } => format!(
            "Cast {count} qualifying spell(s): {}",
            specs
                .iter()
                .map(|s| format!(
                    "{} level {} or higher",
                    s.spell_type.as_deref().unwrap_or("any type"),
                    s.min_level
                ))
                .collect::<Vec<_>>()
                .join(" or ")
        ),
        Predicate::BaseSave { save, min } => format!("Base {save} save at least +{min}"),
        Predicate::Deity { names, min_count } => format!(
            "Deity requirement ({min_count}): {}",
            names.iter().cloned().collect::<Vec<_>>().join(", ")
        ),
        Predicate::All(items) => items
            .iter()
            .map(requirement_label)
            .collect::<Vec<_>>()
            .join("; and "),
        Predicate::Any(items) => items
            .iter()
            .map(requirement_label)
            .collect::<Vec<_>>()
            .join("; or "),
        Predicate::Not(item) => format!("Must not meet: {}", requirement_label(item)),
        Predicate::Always => "No additional prerequisite".into(),
        Predicate::Variable { name, min, max } => format!("{name}: {}", bound(*min, *max)),
        _ => "Additional source requirement; see the attached RAW reference".into(),
    }
}
impl Engine {
    fn acquisition_entity(&self, name: &str, feat: bool) -> Option<&CompiledEntity> {
        let base = name.split_once(" (").map(|(base, _)| base).unwrap_or(name);
        self.pack.entities.iter().find(|e| {
            let category = if feat {
                e.category == pcgen_core::EntityCategory::Feat
                    || (e.category == pcgen_core::EntityCategory::Ability
                        && e.ability_category
                            .as_deref()
                            .is_some_and(|c| c.eq_ignore_ascii_case("FEAT")))
            } else {
                e.category == pcgen_core::EntityCategory::Class
            };
            category && (e.name.eq_ignore_ascii_case(name) || e.name.eq_ignore_ascii_case(base))
        })
    }
    fn requirements_at(
        &self,
        d: &CharacterDraft,
        entity: &CompiledEntity,
        level: u32,
        complete: bool,
    ) -> Vec<RuleDecision> {
        let state = crate::apply_effects(&self.pack, d);
        let mut out = Vec::new();
        for pre in &entity.requirements {
            if let Predicate::SkillRanks { specs, min_count } = pre {
                let caps: Vec<_> = specs
                    .iter()
                    .map(|spec| {
                        let cap = pcgen_core::skill_rules::max_rank(
                            d.total_level(),
                            state.class_skills.contains(&spec.name.to_ascii_lowercase()),
                        );
                        (spec, cap)
                    })
                    .collect();
                if caps
                    .iter()
                    .filter(|(spec, cap)| spec.min_ranks as f64 <= *cap)
                    .count()
                    < *min_count as usize
                {
                    for (spec, cap) in caps
                        .iter()
                        .filter(|(spec, cap)| spec.min_ranks as f64 > *cap)
                    {
                        out.push(decision(level,&entity.name,DecisionStatus::Failed,format!("{} requires {} ranks before entry; the unmodified cap is {cap}. No class-skill override is recorded.",spec.name,spec.min_ranks),Some(&entity.source)));
                    }
                }
            }
            let result = crate::effects::predicate_result(Some(pre), d, &state);
            let status = match result {
                Some(true) => DecisionStatus::Satisfied,
                None => DecisionStatus::NotImplemented,
                Some(false) => {
                    if !complete
                        && matches!(
                            pre,
                            Predicate::SkillRanks { .. }
                                | Predicate::AbilityRequirement { .. }
                                | Predicate::SpellType { .. }
                        )
                    {
                        DecisionStatus::MissingEvidence
                    } else {
                        DecisionStatus::Failed
                    }
                }
            };
            out.push(decision(
                level,
                &entity.name,
                status,
                match pre {
                    Predicate::TotalBab { min } => format!(
                        "Requires BAB {min}; actual BAB {}",
                        self.evaluate(d).base_attack_bonus
                    ),
                    _ => requirement_label(pre),
                },
                Some(&entity.source),
            ));
        }
        if let Some(pending) = entity.facts.get("NativeRulePending") {
            out.push(decision(
                level,
                &entity.name,
                DecisionStatus::NotImplemented,
                pending,
                Some(&entity.source),
            ));
        }
        if !entity.unsupported_tags.is_empty() {
            out.push(decision(
                level,
                &entity.name,
                DecisionStatus::NotImplemented,
                entity.unsupported_tags.join("; "),
                Some(&entity.source),
            ));
        }
        out
    }
    pub fn feat_acquisition(
        &self,
        d: &CharacterDraft,
        name: &str,
        level: u32,
        complete: bool,
    ) -> Vec<RuleDecision> {
        let Some(entity) = self.acquisition_entity(name, true) else {
            return vec![decision(
                level,
                name,
                DecisionStatus::NotImplemented,
                "No structured feat definition",
                None,
            )];
        };
        let mut out = self.requirements_at(d, entity, level, complete);
        let repeatable = self
            .pack
            .abilities
            .values()
            .find(|a| a.name.eq_ignore_ascii_case(&entity.name))
            .is_some_and(|a| a.multi == Some(true));
        if !repeatable
            && d.abilities
                .iter()
                .any(|(_, n)| n.eq_ignore_ascii_case(name))
        {
            out.push(decision(
                level,
                name,
                DecisionStatus::Failed,
                "This feat cannot be selected more than once",
                Some(&entity.source),
            ));
        }
        if entity.metadata.spell_rule == Some(pcgen_core::magic::SpellRule::DivineMetamagic) {
            let turning = self.evaluate(d).turning.is_some();
            out.push(decision(
                level,
                name,
                if turning {
                    DecisionStatus::Satisfied
                } else {
                    DecisionStatus::Failed
                },
                "Turn/rebuke undead must be acquired before selecting Divine Metamagic",
                Some(&entity.source),
            ));
        }
        if out.is_empty() {
            out.push(decision(level,name,DecisionStatus::Satisfied,"Recorded acquisition prerequisites satisfied; feat-slot allocation is checked separately",Some(&entity.source)));
        }
        out
    }
    pub fn audit_build(&self, d: &CharacterDraft) -> BuildAudit {
        let mut out = BuildAudit {
            recorded_through: 0,
            decisions: Vec::new(),
        };
        let Some(history) = &d.build_history else {
            out.decisions.push(decision(0,"Build history",DecisionStatus::MissingEvidence,"No acquisition history. Final skills, feats and spells cannot prove earlier prerequisites.",None));
            return out;
        };
        out.recorded_through = history.recorded_through;
        if history.recorded_through > d.total_level() {
            out.decisions.push(decision(
                0,
                "Build history",
                DecisionStatus::Failed,
                "Completeness exceeds recorded character levels",
                None,
            ));
        }
        if let Err(e) = d.materialized() {
            out.decisions.push(decision(
                0,
                "Build history",
                DecisionStatus::Failed,
                e,
                None,
            ));
            return out;
        }
        if history.recorded_through < d.total_level() {
            out.decisions.push(decision(
                0,
                "Build history",
                DecisionStatus::MissingEvidence,
                "Some levels have incomplete acquisition records",
                None,
            ));
        }
        if let Some(race_entity) = self.pack.entities.iter().find(|e| {
            e.category == pcgen_core::EntityCategory::Race
                && (e.id.key.eq_ignore_ascii_case(&d.race_id) || e.name.eq_ignore_ascii_case(&d.race_id))
        }) {
            out.decisions.extend(self.requirements_at(
                d,
                race_entity,
                1,
                history.recorded_through >= 1,
            ));
        }
        for template in &d.templates {
            if let Some(template_entity) = self.pack.entities.iter().find(|e| {
                e.category == pcgen_core::EntityCategory::Template
                    && (e.id.key.eq_ignore_ascii_case(template) || e.name.eq_ignore_ascii_case(template))
            }) {
                out.decisions.extend(self.requirements_at(
                    d,
                    template_entity,
                    1,
                    history.recorded_through >= 1,
                ));
            }
        }
        let mut class_counts = std::collections::BTreeMap::new();
        for (i, frame) in d.advancement.iter().enumerate() {
            let n = frame.character_level;
            if n != i as u32 + 1 {
                out.decisions.push(decision(
                    n,
                    "Class level",
                    DecisionStatus::Failed,
                    "Character levels must be consecutive and ordered",
                    None,
                ));
            }
            let before = d
                .build_at(n.saturating_sub(1), history.events.len())
                .unwrap();
            for lane in &frame.lanes {
                let count = class_counts.entry(&lane.class_id).or_insert(0u32);
                *count += 1;
                let Some(c) = self.pack.classes.get(&lane.class_id) else {
                    out.decisions.push(decision(
                        n,
                        &lane.class_id,
                        DecisionStatus::NotImplemented,
                        "Unknown class",
                        None,
                    ));
                    continue;
                };
                if lane.class_level != *count || lane.class_level > c.max_level {
                    out.decisions.push(decision(
                        n,
                        &lane.class_id,
                        DecisionStatus::Failed,
                        "Class level is out of sequence or exceeds the class table",
                        None,
                    ));
                }
                if *count == 1 {
                    if let Some(entity) = self.acquisition_entity(&lane.class_id, false) {
                        out.decisions.extend(self.requirements_at(
                            &before,
                            entity,
                            n,
                            history.recorded_through >= n.saturating_sub(1),
                        ));
                        if let Some(m) = &entity.metadata.class_magic {
                            for evidence in &m.entry_states {
                                out.decisions.push(decision(
                                    n,
                                    &lane.class_id,
                                    if before.active_states.contains(evidence) {
                                        DecisionStatus::Satisfied
                                    } else {
                                        DecisionStatus::MissingEvidence
                                    },
                                    evidence,
                                    Some(&entity.source),
                                ));
                            }
                            if !m.entry_spells_any.is_empty() {
                                match self.class_entry(&before, &lane.class_id, None) {
                                    Ok(r) => {
                                        for check in r.checks.into_iter().filter(|c| {
                                            c.requirement.starts_with("Can cast any of:")
                                        }) {
                                            out.decisions.push(decision(
                                                n,
                                                &lane.class_id,
                                                match check.satisfied {
                                                    Some(true) => DecisionStatus::Satisfied,
                                                    Some(false)
                                                        if history.recorded_through >= n =>
                                                    {
                                                        DecisionStatus::Failed
                                                    }
                                                    _ => DecisionStatus::MissingEvidence,
                                                },
                                                check.requirement,
                                                Some(&entity.source),
                                            ));
                                        }
                                    }
                                    Err(e) => out.decisions.push(decision(
                                        n,
                                        &lane.class_id,
                                        DecisionStatus::MissingEvidence,
                                        e,
                                        Some(&entity.source),
                                    )),
                                }
                            }
                        }
                    } else if c.is_prestige {
                        out.decisions.push(decision(
                            n,
                            &lane.class_id,
                            DecisionStatus::NotImplemented,
                            "Class entry model unavailable",
                            None,
                        ));
                    }
                }
            }
            let after = d.build_at(n, history.events.len()).unwrap();
            let mut errors = Vec::new();
            crate::magic::casting_levels(&self.pack, &after, &mut errors);
            for e in errors {
                if !out.decisions.iter().any(|r| r.explanation == e) {
                    out.decisions.push(decision(
                        n,
                        "Casting advancement",
                        DecisionStatus::Failed,
                        e,
                        None,
                    ));
                }
            }
        }
        for (i, event) in history.events.iter().enumerate() {
            let before = d.build_at(event.level, i).unwrap();
            match &event.action {
                Acquisition::Feat { name } => out.decisions.extend(self.feat_acquisition(
                    &before,
                    name,
                    event.level,
                    history.recorded_through >= event.level,
                )),
                Acquisition::RetrainFeat { old, new } => {
                    out.decisions.extend(self.feat_acquisition(
                        &before,
                        new,
                        event.level,
                        history.recorded_through >= event.level,
                    ));
                    out.decisions.push(decision(event.level,new,DecisionStatus::NeedsRuling,format!("Retraining {old}: verify original acquisition-time prerequisites and dependencies before certifying this replacement"),None));
                }
                Acquisition::Skill { name, ranks } => {
                    let state = crate::apply_effects(&self.pack, &before);
                    let cap = pcgen_core::skill_rules::max_rank(
                        event.level,
                        state.class_skills.contains(&name.to_ascii_lowercase()),
                    );
                    let total = before.skills.get(name).unwrap_or(&0.0) + ranks;
                    out.decisions.push(decision(
                        event.level,
                        name,
                        if total <= cap {
                            DecisionStatus::Satisfied
                        } else {
                            DecisionStatus::Failed
                        },
                        format!("{total} ranks after acquisition; maximum {cap}"),
                        None,
                    ));
                }
                Acquisition::Spell { spell } => {
                    let mut casting_errors = Vec::new();
                    let tracks =
                        crate::magic::casting_levels(&self.pack, &before, &mut casting_errors);
                    let within_level = spell
                        .class_id
                        .as_ref()
                        .and_then(|track| {
                            self.pack
                                .classes
                                .get(track)
                                .and_then(|c| tracks.get(track).and_then(|n| c.spell_slots.get(n)))
                        })
                        .is_some_and(|row| (spell.level as usize) < row.len());
                    let result = spell
                        .class_id
                        .as_ref()
                        .and_then(|track| self.spell_catalog(&before, track).ok())
                        .map(|list| {
                            list.iter().any(|s| {
                                within_level
                                    && s.spell.eq_ignore_ascii_case(&spell.name)
                                    && s.level == spell.level
                                    && !s.prohibited
                            })
                        });
                    out.decisions.push(decision(event.level,&spell.name,match result{Some(true)=>DecisionStatus::Satisfied,Some(false)=>DecisionStatus::Failed,None=>DecisionStatus::MissingEvidence},"Spell must be on the selected casting track at its recorded level; known-spell quota remains a separate check",None));
                }
                Acquisition::AbilityIncrease { .. } => {
                    let count = history.events[..=i]
                        .iter()
                        .filter(|e| matches!(e.action, Acquisition::AbilityIncrease { .. }))
                        .count() as u32;
                    if count > event.level / 4 {
                        out.decisions.push(decision(
                            event.level,
                            "Ability increase",
                            DecisionStatus::Failed,
                            "Standard level-up ability increases are available every four levels",
                            None,
                        ));
                    }
                }
                _ => {}
            }
        }
        out.decisions.push(decision(0,"Coverage",DecisionStatus::NotImplemented,"Chronology, rank caps, class entry and recorded feat prerequisites are audited. Full skill-point budgets, every feat source, known-spell quotas and retraining dependencies are not yet certified.",None));
        out
    }
    /// Adapt an incomplete forum chronology without inventing omitted selections.
    pub fn audit_sequence(&self, d: &CharacterDraft, feats: &[(u32, String)]) -> BuildAudit {
        let mut input = d.clone();
        input.build_history = Some(BuildHistory {
            initial_stats: d.stats.clone(),
            initial_alignment: d.alignment.clone(),
            recorded_through: 0,
            events: feats
                .iter()
                .filter(|(n, _)| *n <= d.total_level())
                .map(|(level, name)| BuildEvent {
                    level: *level,
                    action: Acquisition::Feat { name: name.clone() },
                })
                .collect(),
        });
        self.audit_build(&input)
    }
}
