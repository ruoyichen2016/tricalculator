//! Stable variable names shared by the compiler and runtime.

/// Variable holding the character's total level.
pub const CHARACTER_LEVEL: &str = "CharacterLevel";

/// Total hit dice, including racial and class hit dice.
pub const HIT_DICE: &str = "HitDice";

/// Variable holding a companion's master level.
///
/// PCGen writes `mastervar("SpecialMountLVL")+5`.  The argument names the
/// companion's own progression variable, but every RSRD use resolves to the
/// same thing — the master's level — so the argument carries no information and
/// the term is a single variable.
pub const MASTER_LEVEL: &str = "MasterLevel";

/// Variable holding the character's level in one class.
///
/// The class name is encoded rather than used directly: class names contain
/// spaces, parentheses and slashes (`Ex Paladin`, `Sorcerer/Cleric (Arcane)`),
/// which are not valid in a formula identifier.  Every character outside
/// `[A-Za-z0-9_]` becomes `_<hex>`, which is lossless, so two different class
/// names can never collide on one variable.
pub fn class_level_variable(class: &str) -> String {
    let mut out = String::from("ClassLevel_");
    for character in class.trim().chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            out.push(character.to_ascii_lowercase());
        } else {
            out.push('_');
            out.push_str(&format!("{:x}", character as u32));
        }
    }
    out
}

/// A compiler-generated reference to an active ability count, with lossless selectors.
pub fn ability_count_variable(category: &str, kind: &str) -> String {
    let hex = |s: &str| {
        s.as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    format!("AbilityCount_{}_{}", hex(category), hex(kind))
}
pub fn ability_count_selectors(name: &str) -> Option<(String, String)> {
    let (a, b) = name.strip_prefix("AbilityCount_")?.split_once('_')?;
    let decode = |s: &str| -> Option<String> {
        if !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        String::from_utf8(
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
                .collect::<Option<Vec<_>>>()?,
        )
        .ok()
    };
    Some((decode(a)?, decode(b)?))
}
