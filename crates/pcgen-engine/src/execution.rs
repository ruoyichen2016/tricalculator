//! Operation plans are derived from current resources; commits re-evaluate and are atomic.
use crate::{class_play::PlayResult, magic::CastResolution, CharacterDraft, Engine};
use pcgen_core::{
    build::{DecisionStatus, RuleDecision},
    magic::CastRequest,
};
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct CastPlan {
    pub revision: u64,
    pub resolution: CastResolution,
    pub steps: Vec<RuleDecision>,
}
impl Engine {
    pub fn plan_cast(&self, d: &CharacterDraft, request: &CastRequest) -> Result<CastPlan, String> {
        let current = d.materialized()?;
        let mut request = request.clone();
        crate::class_resources::fund_request(&current, &mut request);
        let r = self.cast_preview(&current, &request)?;
        let mut steps = Vec::new();
        for (subject,text) in [
            ("Access",format!("{} via {} ({:?}); knowledge and prohibited schools checked",r.spell,r.class_id,r.power)),
            ("Transformations",format!("Effective level {}; required slot {}; caster level {}; save DC {}; range {:?}",r.effective_level,r.required_slot_level,r.caster_level,r.save_dc,r.range)),
            ("Resources",format!("Pay {} slot(s) of level {}; {} turning attempts. Current saved resource counters used.",r.paid_slots,r.paid_slot_level,r.turning_cost)),
            ("Action",format!("{:?}. {}. Encounter-wide action expenditure is not automatically tracked by this operation.",r.action_cost,r.casting_time_note)),
            ("Targets",if request.tapestry{"Tapestry membership, target restrictions and geometry checked".into()}else{"No tapestry requested; effect-specific recipient resolution remains separate".into()}),
        ] {steps.push(RuleDecision{level:d.total_level(),subject:subject.into(),status:DecisionStatus::Satisfied,tags:Vec::new(),explanation:text,source:None});}
        if r.action_cost.is_none() {
            steps.push(RuleDecision {level:d.total_level(),subject:"Action timing".into(),status:DecisionStatus::NotImplemented,tags:Vec::new(),explanation:"Exact action cost is unavailable or requires a longer/immediate-action timeline".into(),source:None});
        }
        for text in &r.adjudications {
            steps.push(RuleDecision {
                level: d.total_level(),
                subject: "Adjudication".into(),
                status: DecisionStatus::NeedsRuling,
                tags: Vec::new(),
                explanation: text.clone(),
                source: None,
            });
        }
        Ok(CastPlan {
            revision: d.class_play.revision,
            resolution: r,
            steps,
        })
    }
    pub fn execute_cast(
        &self,
        d: &CharacterDraft,
        request: &CastRequest,
        revision: u64,
    ) -> Result<PlayResult, String> {
        if d.class_play.revision != revision {
            return Err("Character changed; preview the action again".into());
        }
        let d = d.materialized()?;
        if request.quiescent {
            self.store_weaving(&d, request)
        } else {
            self.cast_and_record(&d, request)
        }
    }
}

impl Engine {
    /// Record ordinary preparations before expenditure; this never replenishes spent slots.
    pub fn prepare_spell(
        &self,
        d: &CharacterDraft,
        track: &str,
        spell: &str,
        copies: u32,
    ) -> Result<CharacterDraft, String> {
        let mut current = d.materialized()?;
        let class = self
            .pack
            .classes
            .get(track)
            .ok_or("Unknown casting track")?;
        if class.casting_type != pcgen_core::CastingType::Prepared {
            return Err("This track does not prepare spells".into());
        }
        if current
            .class_play
            .resources
            .slots_spent
            .get(&track.to_ascii_lowercase())
            .is_some_and(|rows| rows.values().any(|n| *n > 0))
        {
            return Err(
                "Complete the appropriate rest before changing preparations after expenditure"
                    .into(),
            );
        }
        let list = self.spell_catalog(&current, track)?;
        let selected = list
            .iter()
            .find(|s| s.spell.eq_ignore_ascii_case(spell) && s.known && !s.prohibited)
            .ok_or("Spell is not available for preparation on this track")?;
        let mut errors = Vec::new();
        let tracks = crate::magic::casting_levels(&self.pack, &current, &mut errors);
        if !errors.is_empty() {
            return Err(errors.join("; "));
        }
        let row = class
            .spell_slots
            .get(tracks.get(track).ok_or("Casting progression unavailable")?)
            .ok_or("Slot row unavailable")?;
        let base = *row
            .get(selected.level as usize)
            .ok_or("Spell level unavailable")?;
        let ability = class
            .spellcasting_ability
            .as_ref()
            .ok_or("Casting ability unavailable")?;
        let score = self
            .evaluate(&current)
            .stats
            .get(ability)
            .copied()
            .ok_or("Casting score unavailable")?;
        if score < 10 + selected.level as i32 {
            return Err("Casting ability is too low".into());
        }
        let modifier = (score - 10).div_euclid(2);
        let bonus = if selected.level > 0 && modifier >= selected.level as i32 {
            1 + (modifier - selected.level as i32) as u32 / 4
        } else {
            0
        };
        let others = list
            .iter()
            .filter(|s| s.level == selected.level && !s.spell.eq_ignore_ascii_case(spell))
            .try_fold(0u32, |n, s| {
                let count = current.prepared_copies(track, &s.spell)?;
                n.checked_add(count)
                    .ok_or_else(|| String::from("Preparation count overflow"))
            })?;
        if copies > base + bonus || others > base + bonus - copies {
            return Err("Ordinary preparation slots exceeded; restricted domain/specialist slots are not assigned by this form".into());
        }
        let prep = pcgen_core::DraftSpell {
            name: selected.spell.clone(),
            class_id: Some(track.into()),
            level: selected.level,
            times_memorized: copies,
            book: Some("Daily preparation".into()),
        };
        let mut original = d.clone();
        original.preparations.retain(|s| {
            !(s.class_id.as_deref() == Some(track) && s.name.eq_ignore_ascii_case(spell))
        });
        original.preparations.push(prep);
        original.refresh_build_projection()?;
        original.class_play.revision = original
            .class_play
            .revision
            .checked_add(1)
            .ok_or("Revision overflow")?;
        current = original;
        Ok(current)
    }
}
