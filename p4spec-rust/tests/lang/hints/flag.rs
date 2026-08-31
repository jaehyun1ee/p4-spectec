use super::*;

#[test]
fn test_flag_hints_match_only_the_requested_identifier() {
    let hints = vec![(id("enabled", "enabled-hint"), exp(ExpKind::Eps))];

    assert!(flag_impl::init(&hints, "enabled"));
    assert!(!flag_impl::init(&hints, "disabled"));
}
