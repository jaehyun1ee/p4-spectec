use p4spec_rust::backend::adoc::pl::doc::{
    self as doc, Block, Code, FallthroughLabel, ItemKind, Link, Prose, Subject,
};

#[test]
fn code_links_merge_adjacent_tokens_and_drop_nested_targets() {
    let code = Code::Link(
        Link::Direct("outer".into()),
        Box::new(Code::Seq(vec![
            Code::Token("a ".into()),
            Code::Link(Link::Direct("inner".into()), Box::new(Code::Token("b".into()))),
        ])),
    );
    assert_eq!(doc::ser_prose(&Prose::Code(code), &doc::subject_name), "xref:outer[``a`` ``b``]");
}

#[test]
fn unresolved_subject_keeps_body_without_cross_reference() {
    let prose = Prose::Link(
        Link::Subject(Subject::Function("f".into())),
        Box::new(Prose::Text("call".into())),
    );
    assert_eq!(doc::ser_prose(&prose, &|_| None), "call");
}

#[test]
fn fallthrough_labels_follow_nested_ordered_list_markers() {
    let block = Block::Seq(vec![Block::Item {
        level: 0,
        kind: ItemKind::Ordered(None),
        prose_head: Prose::Text("Choose".into()),
        block_body: Box::new(Block::Seq(vec![
            Block::Item {
                level: 1,
                kind: ItemKind::Ordered(Some("one".into())),
                prose_head: Prose::Fallthrough("two".into(), FallthroughLabel::Derived),
                block_body: Box::new(Block::Empty),
            },
            Block::Item {
                level: 1,
                kind: ItemKind::Ordered(Some("two".into())),
                prose_head: Prose::Text("Done".into()),
                block_body: Box::new(Block::Empty),
            },
        ])),
    }]);
    let text = doc::ser_block(&block, &doc::subject_name);
    assert!(text.contains("[<a href=\"#two\">→ b</a>]"), "{text}");
    assert!(
        text.contains(" .. +++<span class=\"bk-arm-anchor\" id=\"two\"></span>+++Done"),
        "{text}"
    );
}

#[test]
fn capitalization_stops_at_code_and_reaches_text_after_empty_nodes() {
    let prose = Prose::Seq(vec![Prose::Empty, Prose::Text("hello".into())]);
    assert_eq!(doc::ser_prose(&prose.capitalize_first(), &doc::subject_name), "Hello");
    let prose =
        Prose::Seq(vec![Prose::Code(Code::Token("x".into())), Prose::Text(" stays".into())]);
    assert_eq!(doc::ser_prose(&prose.capitalize_first(), &doc::subject_name), "``x`` stays");
}

#[test]
fn link_delimiters_and_quoted_code_preserve_literal_content() {
    let prose = Prose::Link(Link::Direct("target".into()), Box::new(Prose::Text("a[b]".into())));
    assert_eq!(doc::ser_prose(&prose, &doc::subject_name), "<<target,a[b]>>");
    let prose = Prose::Code(Code::Token("\"a\" \"b\"".into()));
    assert_eq!(doc::ser_prose(&prose, &doc::subject_name), "``{quot}a{quot}`` ``{quot}b{quot}``");
}

#[test]
fn table_serialization_keeps_header_and_cell_boundaries() {
    let block = Block::Table {
        header: vec![Prose::Text("Input".into()), Prose::Text("Output".into())],
        rows: vec![vec![Code::Token("a".into()), Code::Token("b".into())]],
    };
    assert_eq!(
        doc::ser_block(&block, &doc::subject_name),
        "[cols=\"2\", options=\"header\"]\n|===\n| Input | Output \n\n| a | b\n\n|==="
    );
}
