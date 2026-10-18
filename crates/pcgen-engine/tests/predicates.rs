//! Consolidated integration tests for all 25 PCGen PRE* predicate forms.
//!
//! Submodules are located in `predicates/*.rs`. Grouping these into a single
//! test binary eliminates 24 separate cargo link and debuginfo phases.

#[path = "predicates/preability_forms.rs"]
mod preability_forms;
#[path = "predicates/prealign_forms.rs"]
mod prealign_forms;
#[path = "predicates/precampaign_forms.rs"]
mod precampaign_forms;
#[path = "predicates/precharactertype_forms.rs"]
mod precharactertype_forms;
#[path = "predicates/precheck_forms.rs"]
mod precheck_forms;
#[path = "predicates/preclass_forms.rs"]
mod preclass_forms;
#[path = "predicates/preclasslevelmax_forms.rs"]
mod preclasslevelmax_forms;
#[path = "predicates/precskill.rs"]
mod precskill;
#[path = "predicates/predr_forms.rs"]
mod predr_forms;
#[path = "predicates/preequip_forms.rs"]
mod preequip_forms;
#[path = "predicates/prefact_forms.rs"]
mod prefact_forms;
#[path = "predicates/preitem_forms.rs"]
mod preitem_forms;
#[path = "predicates/prelegs_forms.rs"]
mod prelegs_forms;
#[path = "predicates/prerace_forms.rs"]
mod prerace_forms;
#[path = "predicates/prereach_forms.rs"]
mod prereach_forms;
#[path = "predicates/presa_forms.rs"]
mod presa_forms;
#[path = "predicates/preskill_forms.rs"]
mod preskill_forms;
#[path = "predicates/preskilltot_forms.rs"]
mod preskilltot_forms;
#[path = "predicates/prespellcast_forms.rs"]
mod prespellcast_forms;
#[path = "predicates/prespelldescriptor_forms.rs"]
mod prespelldescriptor_forms;
#[path = "predicates/prespellschool_forms.rs"]
mod prespellschool_forms;
#[path = "predicates/prespellschoolsub_forms.rs"]
mod prespellschoolsub_forms;
#[path = "predicates/prespelltype_forms.rs"]
mod prespelltype_forms;
#[path = "predicates/preuatt_forms.rs"]
mod preuatt_forms;
#[path = "predicates/prevar_forms.rs"]
mod prevar_forms;
