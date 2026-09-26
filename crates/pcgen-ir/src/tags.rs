//! Tag-name classification used during lowering.
//!
//! This is deliberately *not* a runtime concept: the engine never sees tag
//! names.  It lives in `pcgen-ir` because the lowering pass (which turns raw
//! records into the rule pack) needs it, and the lowering pass also lives
//! here so the engine can depend on `pcgen-ir` alone.

pub fn is_name_prefix_tag(tag: &str) -> bool {
    matches!(
        tag,
        "CLASS"
            | "RACE"
            | "SKILL"
            | "SPELL"
            | "EQUIPMENT"
            | "ABILITY"
            | "TEMPLATE"
            | "DEITY"
            | "DOMAIN"
            | "LANGUAGE"
            | "KIT"
            | "SIZE"
            | "STAT"
            | "SAVE"
            | "ALIGNMENT"
            | "DATACONTROL"
            | "BIOSET"
            | "ABILITYCATEGORY"
            | "EQUIPMOD"
            | "WEAPONPROF"
            | "ARMORPROF"
            | "SHIELDPROF"
            | "COMPANIONMOD"
            | "GLOBALMODIFIER"
            | "DYNAMIC"
            | "DATATABLE"
    )
}

/// A `PRE*` predicate keyword, with an optional leading `!` negation
/// (`!PRETYPE`, `!PRERACE`, ...).  PCGen treats the negated form as the
/// logical negation of the positive predicate.
pub fn is_predicate_tag(tag: &str) -> bool {
    tag.strip_prefix('!').unwrap_or(tag).starts_with("PRE")
}

/// Tags the tokenizer/store understands well enough to parse into
/// first-class Rust structures.
pub fn is_known_tag(tag: &str) -> bool {
    if is_predicate_tag(tag) {
        return true;
    }
    if is_name_prefix_tag(tag) {
        return true;
    }
    matches!(
        tag,
        "KEY"
            | "NAME"
            | "TYPE"
            | "BONUS"
            | "TEMPBONUS"
            | "TEMPVALUE"
            | "DEFINE"
            | "VAR"
            | "CSKILL"
            | "CCSKILL"
            | "AUTO"
            | "CHOOSE"
            | "ADD"
            | "HD"
            | "BAB"
            | "SAVE"
            | "MAXLEVEL"
            | "KEYSTAT"
            | "ACHECK"
            | "USEUNTRAINED"
            | "SCHOOL"
            | "SCHOOLS"
            | "CLASSES"
            | "COST"
            | "WT"
            | "DAMAGE"
            | "CRIT"
            | "SIZE"
            | "MOVE"
            | "RACETYPE"
            | "FACE"
            | "LANGBONUS"
            | "FAVOREDCLASS"
            | "SOURCEPAGE"
            | "SOURCELONG"
            | "SOURCESHORT"
            | "DESC"
            | "DESCRIPTION"
            | "PRESTAT"
            | "PRECLASS"
            | "PREABILITY"
            | "PREFEAT"
            | "PREVAR"
            | "PREALIGN"
            | "PRERACE"
            | "PRESKILL"
            | "PRELEVEL"
            | "PREMULT"
            | "VISIBLE"
            | "SORTKEY"
            | "OUTPUTNAME"
            | "LEVEL"
            | "RANK"
            | "FACT"
            | "SUBCLASS"
            | "TEMPLATE"
            | "SPELLS"
            | "CAST"
            | "KNOWN"
            | "KNOWNSPELLS"
            | "SPELLLIST"
            | "MEMORIZE"
            | "CASTTIME"
            | "TARGETAREA"
            | "SAVEINFO"
            | "SPELLRES"
            | "SUBSCHOOL"
            | "DESCRIPTOR"
            | "COMPS"
            | "RANGE"
            | "DURATION"
            | "ITEM"
            | "EQMOD"
            | "STARTSKILLPTS"
            | "SPELLSTAT"
            | "PROFICIENCY"
            | "FEAT"
            | "MAXDEX"
            | "SPELLFAILURE"
            | "ACCHECK"
            | "NAMEOPT"
            | "MULT"
            | "QUALIFY"
            | "SELECT"
            | "COUNT"
            | "ALLOWBASECLASS"
            | "LEVELADJUSTMENT"
            | "STARTFEATS"
            | "FAVCLASS"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicate_tag_detection_handles_negation() {
        assert!(is_predicate_tag("PRERACE"));
        assert!(is_predicate_tag("!PRERACE"));
        assert!(is_predicate_tag("PREMULT"));
        assert!(!is_predicate_tag("RACE"));
        assert!(!is_predicate_tag("!RACE"));
    }

    #[test]
    fn negated_predicate_is_a_known_tag() {
        assert!(is_known_tag("!PRERACE"));
        assert!(is_known_tag("!PRETYPE"));
        // Non-predicate negations are unaffected.
        assert!(!is_known_tag("!RACE"));
    }
}
