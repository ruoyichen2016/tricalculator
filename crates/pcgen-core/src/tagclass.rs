//! Classification of known-but-unlowered ("legacy") tags.
//!
//! Not every recognized tag carries runtime semantics.  Many are purely
//! descriptive (`BENEFIT`, `SOURCELONG`, `DISPLAYNAME`) or are consumed by
//! subsystems that are explicitly out of scope.  Treating all of them as
//! "pending migration work" makes the completion metric meaningless.
//!
//! [`TagClass`] splits them into:
//!
//! * [`TagClass::Allowlisted`] — intentionally not lowered now; excluded from
//!   the semantic completion denominator.  Removing an entry here is a
//!   deliberate decision, not an accident.
//! * [`TagClass::Pending`] — has (or may have) runtime semantics and must be
//!   lowered, or explicitly reclassified.

use serde::{Deserialize, Serialize};

/// Whether a recognized-but-unhandled tag is intentionally ignored or still
/// pending work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagClass {
    /// Descriptive / metadata / out-of-scope-for-now.  Not counted as pending.
    Allowlisted,
    /// Has runtime semantics that are not yet lowered.
    Pending,
}

impl TagClass {
    pub fn is_pending(self) -> bool {
        matches!(self, Self::Pending)
    }
}

/// Reporting bucket for every occurrence of a recognized-but-not-lowered tag.
///
/// This is the three-way view used by coverage reports and the completion
/// metric:
///
/// * [`TagBucket::Allowlisted`] — presentation/metadata; excluded from the
///   denominator.
/// * [`TagBucket::Pending`] — recognized, not lowered, has semantics.
/// * [`TagBucket::Unsupported`] — the lowering pass explicitly rejected the
///   tag (e.g. a `BONUS` form we cannot lower yet).  Treated as pending work,
///   because the tag is still recognized and does carry semantics.
///
/// `Allowlisted` is only reachable through [`classify`]; a tag that is
/// allowlisted *and* fails lowering goes to `Allowlisted` too (there is no
/// semantic intent), while everything else that fails lowering is
/// `Unsupported`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum TagBucket {
    Allowlisted,
    #[default]
    Pending,
    Unsupported,
}

impl TagBucket {
    /// Does this bucket count as work still to be done?
    pub fn is_pending_work(self) -> bool {
        matches!(self, Self::Pending | Self::Unsupported)
    }
}

/// Tags that are recognized but intentionally not lowered.
///
/// Grouped by *why* they are allowlisted so the list is auditable.  Any tag
/// not present here is [`TagClass::Pending`].
pub const ALLOWLISTED_TAGS: &[&str] = &[
    // NOTE: REACH / REACHMULT are deliberately *not* allowlisted:
    // natural reach affects combat for PCs and large races.
    // --- Catalog / source metadata -------------------------------------
    "SOURCEPAGE",
    "SOURCELONG",
    "SOURCESHORT",
    "SOURCEWEB",
    "SOURCEDATE",
    "SOURCESHORT",
    "DATAFORMAT",
    "COPYRIGHT",
    "LICENSE",
    "PUBNAME",
    "PUBNAMEWEB",
    "URL",
    "BOOKTYPE",
    "TYPE", // record type, not a rules input
    "KEY",  // stable identifier, already used for EntityId
    // --- Display / presentation ---------------------------------------
    "DISPLAYNAME",
    "DISPLAYLOCATION",
    "OUTPUTNAME",
    "ABB",
    "PLURAL",
    "BENEFIT",
    "EXPLANATION",
    "SITUATION",
    "PRETEXT", // prerequisite display text
    "QUALITY",
    "SPROP",    // special property text blob
    "TEMPDESC", // temporary description
    "DESC",
    "DESCRIPTION",
    "ASPECT", // presentation aspect
    "NAMEOPT",
    "SORTKEY",
    "ITEMCREATE",
    "NUMPAGES",
    "PAGEUSAGE",
    "EDITABLE",
    "SELECTABLE",
    "GROUP",
    "FORMATCAT",
    "VISIBLE",
    // --- Editor / pool bookkeeping (not character math) ---------------
    "EDITPOOL",
    "FRACTIONALPOOL",
    "ASSIGNTOALL",
    "REMOVABLE",
    "REQUIRED",
    "EXCLUSIVE",
    "MODS",
    "FREE",
    // Dropped by P4-DEC-2 (XP table / point buy / item XP-cost workflow).
    // The tag is still recognized but intentionally out of scope.
    "XPCOST",
    "GENDERLOCK",
    "SERVESAS",
    "MODTOSKILLS",
    "MONCSKILL",
    "MONSKILL",
    "USEMASTERSKILL",
    "MONNONSKILLHD",
    // --- Monster/companion construction (out of scope for PCs 1-20) --
    "MONSTERCLASS",
    "HITDICEADVANCEMENT",
    "FOLLOWERS",
    "COMPANIONLIST",
    "COPYMASTERBAB",
    "COPYMASTERCHECK",
    "COPYMASTERHP",
    "COPYMASTERBAB",
    "LEGS", // creature anatomy (no PC effect beyond movement)
    "HANDS",
    "FACE",
    "SUBRACE",
    "ISDEFAULTSIZE",
    // --- Biography / random character generation / UI role -------------
    // Evidence (DS-35E `_rsrd_basics.pcc`): these occur only in
    // `rsrd_biosettings.lst` (BIOSET) and `rsrd_classes.lst` (CLASS `ROLE`).
    // They describe a character, they do not feed any derived rule value.
    "AGEDIEROLL", // random starting-age roll
    "BASEAGE",    // random starting-age base
    "MAXAGE",
    "HAIR",
    "EYES",
    "SKINTONE",
    "SEX",
    "ROLE", // class UI role (e.g. Combat), not a rules input
];

/// KIT / STARTPACK tags captured by the typed KIT import.
///
/// These are not "presentation metadata"; they are consumed structurally by
/// `KitDefinition`, so they must not be reported as pending work.  The same
/// tag names can appear on equipment records, where they retain equipment
/// semantics and therefore stay pending until `P4-EQP-1`.
pub const KIT_CONSUMED_TAGS: &[&str] = &[
    "STARTPACK",
    "KIT",
    "TABLE",
    "SELECT",
    "GEAR",
    "RACE",
    "CLASS",
    "STAT",
    "SKILL",
    "ABILITY",
    "TEMPLATE",
    "ALIGN",
    "NAME",
    "QTY",
    "OPTION",
    "LOOKUP",
    "VALUES",
    "EQUIPBUY",
    "APPLY",
    "LOCATION",
];

pub fn is_kit_category(category: &str) -> bool {
    matches!(
        category.trim().to_ascii_uppercase().as_str(),
        "KIT" | "STARTPACK"
    )
}

/// Classify a bare (upper-case) tag name.
///
/// Matching is exact and case-insensitive; leading `!`-negation prefixes (as
/// in `!PRERACE`) are preserved, so `!PRETYPE` is *pending* unless explicitly
/// allowlisted.
pub fn classify(tag_name: &str) -> TagClass {
    let upper = tag_name.trim().to_ascii_uppercase();
    if ALLOWLISTED_TAGS.contains(&upper.as_str()) {
        TagClass::Allowlisted
    } else {
        TagClass::Pending
    }
}

/// Category-aware classification.
///
/// KIT / STARTPACK tags listed in [`KIT_CONSUMED_TAGS`] are consumed by the
/// typed KIT import and are therefore excluded from pending coverage.  Other
/// categories, including equipment, keep the tag-only classification.
pub fn classify_for_category(category: &str, tag_name: &str) -> TagClass {
    let upper = tag_name.trim().to_ascii_uppercase();
    if is_kit_category(category) && KIT_CONSUMED_TAGS.contains(&upper.as_str()) {
        TagClass::Allowlisted
    } else {
        classify(&upper)
    }
}

/// Convenience: is this tag intentionally ignored?
pub fn is_allowlisted(tag_name: &str) -> bool {
    classify(tag_name) == TagClass::Allowlisted
}

/// Bucket for a tag that the lowering pass failed to handle.
///
/// If the tag is allowlisted we keep it out of the denominator even when
/// lowering rejects it; otherwise it is [`TagBucket::Unsupported`].
pub fn bucket_for_unsupported(tag_name: &str) -> TagBucket {
    match classify(tag_name) {
        TagClass::Allowlisted => TagBucket::Allowlisted,
        TagClass::Pending => TagBucket::Unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptive_tags_are_allowlisted() {
        for tag in [
            "SPROP",
            "TEMPDESC",
            "BENEFIT",
            "SOURCELONG",
            "DISPLAYNAME",
            "PRETEXT",
        ] {
            assert!(
                is_allowlisted(tag),
                "{tag} should be allowlisted as presentation metadata"
            );
        }
    }

    #[test]
    fn semantic_tags_are_pending() {
        for tag in [
            "NATURALATTACKS",
            "DOMAINS",
            "DR",
            "SR",
            "TEMPBONUS",
            "CHOOSE",
        ] {
            assert!(
                !is_allowlisted(tag),
                "{tag} carries runtime semantics and must be pending"
            );
        }
    }

    #[test]
    fn classification_is_case_insensitive_and_exact() {
        assert_eq!(classify("sprop"), TagClass::Allowlisted);
        assert_eq!(classify(" SPROP "), TagClass::Allowlisted);
        // Prefix must not match.
        assert_eq!(classify("SPROPX"), TagClass::Pending);
        assert_eq!(classify("!PRETYPE"), TagClass::Pending);
    }

    #[test]
    fn category_aware_classification_splits_kit_and_equipment_tags() {
        assert_eq!(
            classify_for_category("KIT", "OPTION"),
            TagClass::Allowlisted
        );
        assert_eq!(
            classify_for_category("STARTPACK", "LOCATION"),
            TagClass::Allowlisted
        );
        assert_eq!(
            classify_for_category("EQUIPMENT", "OPTION"),
            TagClass::Pending
        );
        assert_eq!(
            classify_for_category("EQUIPMENT", "LOCATION"),
            TagClass::Pending
        );
    }

    #[test]
    fn biography_and_ui_role_tags_are_allowlisted() {
        for tag in [
            "AGEDIEROLL",
            "BASEAGE",
            "MAXAGE",
            "HAIR",
            "EYES",
            "SKINTONE",
            "SEX",
            "ROLE",
        ] {
            assert!(
                is_allowlisted(tag),
                "{tag} is character-generation / UI metadata, not rules input"
            );
        }
    }

    #[test]
    fn dropped_xp_cost_is_out_of_scope_allowlist() {
        // P4-DEC-2 dropped the XP table / point-buy workflow.  XPCOST must
        // not keep contributing to the semantic-completion denominator.
        assert_eq!(classify("XPCOST"), TagClass::Allowlisted);
        assert!(is_allowlisted("XPCOST"));
    }

    #[test]
    fn unsupported_bucket_follows_allowlist() {
        // Semantic tag rejected by lowering -> unsupported work.
        assert_eq!(bucket_for_unsupported("BONUS"), TagBucket::Unsupported);
        assert_eq!(bucket_for_unsupported("PREMULT"), TagBucket::Unsupported);
        // Allowlisted tag is kept out of the denominator even if lowering fails.
        assert_eq!(bucket_for_unsupported("SPROP"), TagBucket::Allowlisted);
        assert!(TagBucket::Unsupported.is_pending_work());
        assert!(TagBucket::Pending.is_pending_work());
        assert!(!TagBucket::Allowlisted.is_pending_work());
    }
}
