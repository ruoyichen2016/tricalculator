//! Shared bonus type and stacking semantics for variables and derived values.
use std::collections::BTreeMap;

/// How a `TYPE=` expression makes a bonus stack.
///
/// PCGen's bonus documentation gives four kinds:
///
/// ```text
/// Typeless            no TYPE=          always added
/// Standard            TYPE=name         only the highest of the same type counts
/// .REPLACE            TYPE=name.REPLACE counted together, then compared with
///                                       the standards, highest wins
/// .STACK              TYPE=name.STACK   stacks with itself and is added on top
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BonusPolicy {
    /// No type, or a type PCGen treats as untyped: always summed.
    Typeless,
    /// `TYPE=name` — the highest value of that type wins.
    Standard,
    /// `TYPE=name.REPLACE` — summed, then compared with the standards.
    Replace,
    /// `TYPE=name.STACK` — summed and added on top of the rest.
    Stack,
}

impl BonusPolicy {
    pub(crate) fn of(stack_key: Option<&str>, bonus_type: &str) -> Self {
        if stack_key.is_some_and(|key| key.eq_ignore_ascii_case("stack")) {
            return BonusPolicy::Stack;
        }
        if stack_key.is_some_and(|key| key.eq_ignore_ascii_case("nonstack")) {
            return BonusPolicy::Standard;
        }
        if bonus_type.contains("stack") {
            return BonusPolicy::Stack;
        }
        if bonus_type.contains("replace") {
            return BonusPolicy::Replace;
        }
        if is_stacking_bonus_type(bonus_type) {
            BonusPolicy::Typeless
        } else {
            BonusPolicy::Standard
        }
    }

    /// The group a bonus competes within. `.REPLACE` and standard bonuses of the
    /// same base type compete, so they share a group; `.STACK` and typeless
    /// bonuses are additive and get their own.
    pub(crate) fn group(self, bonus_type: &str) -> String {
        let base = bonus_type
            .trim_end_matches(".replace")
            .trim_end_matches(".stack")
            .trim();
        match self {
            BonusPolicy::Typeless => format!("untyped:{base}"),
            BonusPolicy::Standard | BonusPolicy::Replace => format!("typed:{base}"),
            BonusPolicy::Stack => format!("stack:{base}"),
        }
    }
}

/// Accumulates the bonuses that compete within one group.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BonusGroup {
    /// Highest standard value seen.
    standard: Option<i32>,
    /// Sum of the `.REPLACE` values, which are counted together.
    replace: Option<i32>,
    /// Sum of the `.STACK` values.
    stack: i32,
}

impl BonusGroup {
    pub(crate) fn add(&mut self, policy: BonusPolicy, value: i32) {
        match policy {
            BonusPolicy::Standard => {
                self.standard = Some(match self.standard {
                    Some(old) => choose_non_stacking(old, value),
                    None => value,
                });
            }
            BonusPolicy::Replace => {
                self.replace = Some(self.replace.unwrap_or(0) + value);
            }
            BonusPolicy::Stack => self.stack += value,
            BonusPolicy::Typeless => {
                self.standard = Some(self.standard.unwrap_or(0) + value);
            }
        }
    }

    /// The `.REPLACE` total competes with the standards; `.STACK` is added on.
    pub(crate) fn total(self) -> i32 {
        let base = match (self.standard, self.replace) {
            (Some(standard), Some(replace)) => choose_non_stacking(standard, replace),
            (Some(standard), None) => standard,
            (None, Some(replace)) => replace,
            (None, None) => 0,
        };
        base + self.stack
    }
}

pub(crate) fn stacked_components<F>(
    bonuses: &[crate::effects::TypedBonus],
    mut components: F,
) -> BTreeMap<String, i32>
where
    F: FnMut(&crate::effects::TypedBonus) -> Vec<String>,
{
    let mut groups: BTreeMap<(String, String), BonusGroup> = BTreeMap::new();
    for bonus in bonuses {
        let bonus_type = bonus
            .bonus_type
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();
        let policy = BonusPolicy::of(bonus.stack_key.as_deref(), &bonus_type);
        let group_key = policy.group(&bonus_type);
        for component in components(bonus) {
            groups
                .entry((component, group_key.clone()))
                .or_default()
                .add(policy, bonus.value);
        }
    }
    let mut totals: BTreeMap<String, i32> = BTreeMap::new();
    for ((component, _), group) in groups {
        *totals.entry(component).or_insert(0) += group.total();
    }
    totals
}

fn is_stacking_bonus_type(bonus_type: &str) -> bool {
    bonus_type.is_empty()
        || bonus_type == "untyped"
        || bonus_type == "type"
        || bonus_type.contains("stack")
        || bonus_type.contains("dodge")
        || bonus_type.contains("circumstance")
}

fn choose_non_stacking(old: i32, new: i32) -> i32 {
    if old >= 0 && new >= 0 {
        old.max(new)
    } else if old <= 0 && new <= 0 {
        old.min(new)
    } else if new.abs() > old.abs() {
        new
    } else {
        old
    }
}
