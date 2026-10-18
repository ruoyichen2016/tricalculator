//! Translate source formula dialects at the compiler boundary.
use pcgen_core::formula_vocab::{class_level_variable, CHARACTER_LEVEL, HIT_DICE};

/// Rewrite a PCGen formula into the runtime vocabulary.
///
/// `owner` is the class that owns the formula, used for the bare `CL` and for
/// `classlevel()` with no argument.  When it is `None`, those become `0` rather
/// than being left as PCGen tokens — a formula that cannot name its class is
/// not one the engine should be asked to interpret.
pub fn normalize_pcgen_formula(expression: &str, owner: Option<&str>) -> String {
    let mut out = String::with_capacity(expression.len());
    let bytes = expression.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        let rest = &expression[index..];

        if let Some(after) = strip_prefix_ci(rest, "count") {
            if let Some(after) = after.trim_start().strip_prefix('(') {
                if let Some((args, remainder)) = after.split_once(')') {
                    let parts: Vec<_> = args
                        .split(',')
                        .map(|s| s.trim().trim_matches(['\"', '\'']))
                        .collect();
                    if parts.len() == 3 && parts[0].eq_ignore_ascii_case("ABILITIES") {
                        if let (Some(category), Some(kind)) = (
                            parts[1].strip_prefix("CATEGORY="),
                            parts[2].strip_prefix("TYPE="),
                        ) {
                            out.push_str(&pcgen_core::formula_vocab::ability_count_variable(
                                category, kind,
                            ));
                            index = expression.len() - remainder.len();
                            continue;
                        }
                    }
                }
            }
        }
        // `classlevel(...)` — an explicit class, or the owning class.
        if let Some(after) = strip_prefix_ci(rest, "classlevel") {
            let after = after.trim_start();
            if let Some(after) = after.strip_prefix('(') {
                if let Some((argument, remainder)) = after.split_once(')') {
                    let argument = argument.trim().trim_matches(['"', '\'']).trim();
                    let class = if argument.is_empty()
                        || argument.eq_ignore_ascii_case("APPLIEDAS=NONEPIC")
                    {
                        owner
                    } else {
                        Some(argument)
                    };
                    out.push_str(
                        &class
                            .map(class_level_variable)
                            .unwrap_or_else(|| "0".to_string()),
                    );
                    index = expression.len() - remainder.len();
                    continue;
                }
            }
        }

        // raw mastervar returns the named variable from the master's scope,
        // not an interchangeable effective level for every possible name.
        if let Some(after) = strip_prefix_ci(rest, "mastervar") {
            let after = after.trim_start();
            if let Some(after) = after.strip_prefix('(') {
                if let Some((argument, remainder)) = after.split_once(')') {
                    let argument = argument.trim().trim_matches(['"', '\'']);
                    out.push_str(&normalize_pcgen_formula(argument, None));
                    index = expression.len() - remainder.len();
                    continue;
                }
            }
        }

        // `var("X")` — a variable reference.  `var("CL=X")` and `var("BL=X")`
        // are the same idea as `classlevel("X")`: the character's level in that
        // class.  They used to reach the engine unresolved (the name contains
        // `=`, so it matched no variable and evaluated to 0), which silently
        // zeroed the RSRD's Blackguard smite damage.
        if let Some(after) = strip_prefix_ci(rest, "var") {
            let after = after.trim_start();
            if let Some(after) = after.strip_prefix('(') {
                if let Some((argument, remainder)) = after.split_once(')') {
                    let argument = argument.trim().trim_matches(['"', '\'']).trim();
                    let class = argument
                        .strip_prefix("CL=")
                        .or_else(|| argument.strip_prefix("BL="));
                    match class {
                        Some(class) => out.push_str(&class_level_variable(class)),
                        // Any other `=` form names something that is not a
                        // variable; zero is what it evaluated to before.
                        None if argument.contains('=') => out.push('0'),
                        None => out.push_str(argument),
                    }
                    index = expression.len() - remainder.len();
                    continue;
                }
            }
        }

        // Bare identifiers: `CL` and `TL` are the two the dialect reserves.
        if let Some(after) = strip_prefix_ci(rest, "CL") {
            if starts_word(rest, 2) && !continues_word(after) {
                out.push_str(
                    &owner
                        .map(class_level_variable)
                        .unwrap_or_else(|| "0".to_string()),
                );
                index += 2;
                continue;
            }
        }
        if let Some(after) = strip_prefix_ci(rest, "HD") {
            if starts_word(rest, 2) && !continues_word(after) {
                out.push_str(HIT_DICE);
                index += 2;
                continue;
            }
        }
        if let Some(after) = strip_prefix_ci(rest, "TL") {
            if starts_word(rest, 2) && !continues_word(after) {
                out.push_str(CHARACTER_LEVEL);
                index += 2;
                continue;
            }
        }

        // Anything else passes through one character at a time.
        let character = rest.chars().next().expect("non-empty");
        out.push(character);
        index += character.len_utf8();
    }

    out
}

fn strip_prefix_ci<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let head = value.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &value[prefix.len()..])
}

/// True when a two-character token at the start of `value` is not part of a
/// longer identifier.
fn starts_word(value: &str, len: usize) -> bool {
    value.len() >= len
}

fn continues_word(after: &str) -> bool {
    after
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_cl_becomes_the_owning_class() {
        assert_eq!(
            normalize_pcgen_formula("CL", Some("Cleric")),
            "ClassLevel_cleric"
        );
        assert_eq!(
            normalize_pcgen_formula("CL/2+2", Some("Fighter")),
            "ClassLevel_fighter/2+2"
        );
    }

    #[test]
    fn tl_becomes_character_level() {
        assert_eq!(normalize_pcgen_formula("TL", None), "CharacterLevel");
        assert_eq!(normalize_pcgen_formula("TL-3", None), "CharacterLevel-3");
    }

    #[test]
    fn classlevel_names_a_class_explicitly() {
        assert_eq!(
            normalize_pcgen_formula("classlevel(\"Wizard\")/2", Some("Fighter")),
            "ClassLevel_wizard/2"
        );
        assert_eq!(
            normalize_pcgen_formula("classlevel()", Some("Wizard")),
            "ClassLevel_wizard"
        );
        // `APPLIEDAS=NONEPIC` is a PCGen flag meaning "this class's levels";
        // it resolves to the owning class.
        assert_eq!(
            normalize_pcgen_formula("classlevel(\"APPLIEDAS=NONEPIC\")/3", Some("Cleric")),
            "ClassLevel_cleric/3"
        );
    }

    #[test]
    fn mastervar_preserves_the_named_master_variable() {
        // The RSRD writes `mastervar("SpecialMountLVL")+5` and
        // `mastervar("FamiliarLVL")+5`; these are distinct master variables.
        assert_eq!(
            normalize_pcgen_formula("mastervar(\"SpecialMountLVL\")+5", None),
            "SpecialMountLVL+5"
        );
        assert_eq!(
            normalize_pcgen_formula("mastervar(\"FamiliarLVL\")+5", None),
            "FamiliarLVL+5"
        );
        // Case-insensitively, and with the unquoted spelling.
        assert_eq!(
            normalize_pcgen_formula("MASTERVAR(FamiliarLVL)+5", None),
            "FamiliarLVL+5"
        );
    }

    #[test]
    fn mastervar_is_not_confused_with_var() {
        // `mastervar` must not be read as `var`, which would leave
        // `master` behind and produce `masterMasterLevel`.
        assert_eq!(normalize_pcgen_formula("mastervar(\"X\")", None), "X");
    }

    #[test]
    fn var_calls_become_plain_references() {
        assert_eq!(
            normalize_pcgen_formula("var(\"TurnLevelBase\")+1", None),
            "TurnLevelBase+1"
        );
    }

    #[test]
    fn var_with_a_class_prefix_names_that_class_level() {
        // Both spellings mean "the character's level in that class".
        assert_eq!(
            normalize_pcgen_formula("var(\"CL=Cleric\")", None),
            "ClassLevel_cleric"
        );
        assert_eq!(
            normalize_pcgen_formula("var(\"BL=Cleric\")", None),
            "ClassLevel_cleric"
        );
        assert_eq!(
            normalize_pcgen_formula("var(\"CL=Ex Paladin\")/4+1", None),
            "ClassLevel_ex_20paladin/4+1"
        );
    }

    #[test]
    fn identifiers_containing_cl_are_left_alone() {
        // `CLERIC` starts with `CL` but is not the `CL` token.
        assert_eq!(normalize_pcgen_formula("CLERICLVL", None), "CLERICLVL");
        assert_eq!(normalize_pcgen_formula("TLX", None), "TLX");
    }

    #[test]
    fn class_names_encode_losslessly() {
        assert_eq!(
            class_level_variable("Ex Paladin"),
            "ClassLevel_ex_20paladin"
        );
        assert_ne!(
            class_level_variable("A B"),
            class_level_variable("A/B"),
            "different class names must not share a variable"
        );
    }

    #[test]
    fn a_classless_formula_resolves_to_zero() {
        assert_eq!(normalize_pcgen_formula("CL", None), "0");
    }
}
