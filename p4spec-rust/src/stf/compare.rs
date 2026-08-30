//! Expected-packet comparison.

/// Tests an actual packet against an STF expectation, where `*` matches one nibble.
pub fn packet_matches(actual: &str, expected: &str) -> bool {
    actual.chars().count() == expected.chars().count()
        && actual
            .chars()
            .zip(expected.chars())
            .all(|(actual, expected)| expected == '*' || actual == expected)
}
