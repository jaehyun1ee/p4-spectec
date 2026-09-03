//! Expected-packet comparison.
//!
//! Length is checked first, then corresponding nibbles are compared with `*`
//! as the only wildcard; for example, `a0` matches `a*` but not `*`.

/// Tests an actual packet against an STF expectation, where `*` matches one nibble.
pub fn packet_matches(actual: &str, expected: &str) -> bool {
    actual.chars().count() == expected.chars().count()
        && actual
            .chars()
            .zip(expected.chars())
            .all(|(actual, expected)| expected == '*' || actual == expected)
}
