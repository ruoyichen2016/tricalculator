//! Shared error boundary for numeric derived formulas. A failed contribution
//! is reported as unavailable; callers may retain other valid contributions.
use pcgen_formula::FormulaError;

pub(crate) fn context(error: FormulaError, owner: &str, expression: &str) -> FormulaError {
    FormulaError {
        message: format!("{owner}: formula `{expression}` failed: {}", error.message),
        position: error.position,
    }
}

pub(crate) fn report<T>(
    result: Result<T, FormulaError>,
    diagnostics: &mut Vec<String>,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            let message = format!(
                "{}; derived value unavailable (snapshot is incomplete)",
                error
            );
            if !diagnostics.contains(&message) {
                diagnostics.push(message);
            }
            None
        }
    }
}
