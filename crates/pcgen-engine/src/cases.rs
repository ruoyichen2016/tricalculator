//! Case adapters contain inputs and claims; qualification and execution use shared engines.
use crate::Engine;
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct CaseSummary {
    pub id: String,
    pub title: String,
    pub acquisition: crate::build::BuildAudit,
    pub claims: Vec<(String, String)>,
    pub assumptions: Vec<String>,
    pub probe_path: String,
}
impl Engine {
    pub fn case_summary(&self, id: &str) -> Result<CaseSummary, String> {
        let (title, acquisition, claims, assumptions, probe_path) = match id {
            "joker" => {
                let c: crate::joker::JokerCase = serde_json::from_str(include_str!(
                    "../../../tests/fixtures/forum-builds/joker.json"
                ))
                .map_err(|e| e.to_string())?;
                let r = self.audit_joker_case(&c);
                (c.name,r.acquisition,vec![("18th-level Favored Soul spellcasting".into(),format!("Rejected. {} arcane-only advancements rejected; remaining contributions are diagnostic, not a legal build.",r.milestones.last().map(|m|m.rejected_advancements.len()).unwrap_or(0))),("Share all buffs".into(),"Rejected. Spellguard requires defensive personal arcane spells; Deity's Favor does not change spell type.".into())],r.limits,"/joker")
            }
            "shadow-pounce" => {
                let c: crate::martial_case::MartialCase = serde_json::from_str(include_str!(
                    "../../../tests/fixtures/forum-builds/shadow-pounce.json"
                ))
                .map_err(|e| e.to_string())?;
                let r = self.audit_martial_case(&c);
                (c.name,r.acquisition,vec![("Unlimited free Divine Impetus".into(),"Requires an explicit counterfactual action ruling; strict replay rejects the free-action loop.".into()),("Counterfactual full attacks".into(),format!("{} full attacks, {} main-hand and {} off-hand attacks, only under the separate probe assumptions.",r.resource_probe.full_attacks,r.resource_probe.main_attacks,r.resource_probe.off_attacks))],r.unresolved,"/martial")
            }
            "warforged-umbral" => {
                let c: crate::incarnum_case::IncarnumCase = serde_json::from_str(include_str!(
                    "../../../tests/fixtures/forum-builds/warforged-umbral.json"
                ))
                .map_err(|e| e.to_string())?;
                let r = self.audit_incarnum_case(&c);
                let last = r.milestones.last().ok_or("Missing milestones")?;
                (c.name,r.acquisition,vec![("BAB 16".into(),format!("Actual standard multiclass BAB: {}",last.bab)),("16 essentia".into(),format!("Separate repair probe: {} essentia. Repeated Bonus Essentia does not grant a second benefit.",last.essentia))],r.assumptions,"/incarnum")
            }
            _ => return Err("Unknown case".into()),
        };
        Ok(CaseSummary {
            id: id.into(),
            title,
            acquisition,
            claims,
            assumptions,
            probe_path: probe_path.into(),
        })
    }
}
