//! PCGen compatibility and import layer.
//!
//! This crate owns PCGen *file format* handling: `.pcc` graph resolution,
//! `.lst` tokenization, and `.pcg` draft read/write.  Its output is the
//! runtime [`CompiledRulePack`] from `pcgen-ir`; the engine must never parse
//! raw tags.
//!
//! Dependency direction:
//!
//! ```text
//! pcgen-import  ->  pcgen-ir  <-  pcgen-engine
//! ```
//!
//! For convenience the neutral IR types are re-exported here, so callers that
//! already depend on `pcgen-import` (such as the CLI) can keep using a single
//! import path.

pub mod loader;
pub mod pcc;
pub mod pcg;
pub mod tokenizer;

// Re-export the neutral runtime IR so `pcgen_import::CompiledRulePack` etc.
// keep working for existing call sites.
pub use pcgen_ir::draft::{PcgClassLevel, PcgDraft, PcgSpell};
pub use pcgen_ir::patched::apply_record_patches;
pub use pcgen_ir::patches::PatchOperation;
pub use pcgen_ir::rulepack::{
    categories, lower_records, patch_count, CompiledPackHeader, CompiledRulePack, ImportReport,
};
pub use pcgen_ir::source::{RawRecord, RawTag, SourceRef};
pub use pcgen_ir::tags::{is_known_tag, is_name_prefix_tag, is_predicate_tag};
pub use pcgen_ir::typed::build_typed_definitions;
pub use pcgen_ir::typed::{is_class_header_row, is_class_specialty_row};

pub use loader::{compile_pcc, load_pcc, ImportError, LoadedPcc};
pub use pcc::{PccDataFile, PccError, PccFile, PccGraph, PccInclude};
pub use pcg::{
    parse_pcg_draft_file, parse_pcg_draft_str, save_pcg_draft_file, write_pcg_draft_string,
    PcgDraftError,
};
pub use tokenizer::{tokenize_lst, RawLstLine, TagToken};
