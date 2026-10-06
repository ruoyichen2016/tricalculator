//! Build-time transpiler from PCGen rule packs to the normalised on-disk format.
//!
//! See `docs/DATA_FORMAT.md` for the design and `format` for the record shapes.
//!
//! The point of the format is that the runtime should not have to recover
//! meaning from PCGen's conventions.  This crate does that recovery once, at
//! build time, and writes the result as records that state their own kind and
//! owner — so the whole class of defect that came from a wandering class cursor
//! becomes unrepresentable.

pub mod format;
pub mod pack;
pub mod reconstruct;
pub mod transpile;

pub use format::{
    ClassLevelRecord, ClassRecord, GrantMode, GrantRecord, LoadError, Origin, PackManifest, Record,
    ReplaceRecord, SourceInfo, VariantGrantRecord, VariantRecord, FORMAT_VERSION,
};
pub use pack::{read_pack, write_pack, LoadedPack};
pub use reconstruct::{definitions, levels_for};
pub use transpile::{transpile, transpile_pack, TranspileError, TranspiledPack, TRANSPILER};
