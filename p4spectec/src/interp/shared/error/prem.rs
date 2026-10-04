//! Prem diagnostics for interpreter operations
//!
//! Builds diagnostics; callers choose whether to stop or try another candidate.

use std::fmt::Display;

use crate::diagnostic::Diagnostic;

use super::diagnostic;

const CONDITION_UNMET: &str = "runtime/condition-unmet";

/// Reports condition not met.
pub fn condition_unmet(exp: String) -> Diagnostic {
    condition_unmet_display(exp)
}

/// Formats owned or borrowed condition text into the ordinary diagnostic.
pub(crate) fn condition_unmet_display(exp: impl Display) -> Diagnostic {
    diagnostic(CONDITION_UNMET, format!("condition {exp} was not met"), Vec::new())
}

const HOLD_CONDITION_UNMET: &str = "runtime/hold-condition-unmet";

/// Reports hold condition not met.
pub fn hold_condition_unmet(relation: String) -> Diagnostic {
    diagnostic(HOLD_CONDITION_UNMET, format!("condition hold {relation} was not met"), Vec::new())
}

const NOT_HOLD_CONDITION_UNMET: &str = "runtime/not-hold-condition-unmet";

/// Reports not hold condition not met.
pub fn not_hold_condition_unmet(relation: String) -> Diagnostic {
    diagnostic(
        NOT_HOLD_CONDITION_UNMET,
        format!("condition not-hold {relation} was not met"),
        Vec::new(),
    )
}
