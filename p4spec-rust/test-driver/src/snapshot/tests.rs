use super::{compare, normalize_rendered};

#[test]
fn test_rendered_normalization_preserves_internal_spaces_and_line_order() {
    assert_eq!(normalize_rendered("  a  b \t\n\n c \n"), "  a  b\n\n c\n");
    assert_eq!(normalize_rendered("\"text \"\n"), "\"text \"\n");
}
#[test]
fn test_snapshot_detects_order_missing_and_extra_output() {
    compare("a\nb\n", "a\nb\n", "fixture").unwrap();
    for actual in ["b\na\n", "a\n", "a\nb\nc\n", "a\nb"] {
        assert!(compare("a\nb\n", actual, "fixture").is_err());
    }
}
