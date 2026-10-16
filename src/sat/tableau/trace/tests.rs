use crate::{
    formula::parser::parse_formula,
    sat::tableau::{node::Node, trace::TraceBuilder},
};

#[test]
fn deduplicates_literals_without_changing_source_ids_or_polarity() {
    let (remaining, formula) = parse_formula(
        "a && a && !a && !a && x > 0 && x > 0 && !(x > 0) && !(x > 0) && x < 0 && G[0,1] b",
    )
    .unwrap();
    assert!(remaining.is_empty());
    let mut node = Node::from_operands(vec![formula.into()]);
    node.flatten();
    let original = node.operands.clone();
    assert_ne!(original[0].kind, original[1].kind);
    assert!(original[0].kind.eq_structural(&original[1].kind));
    let expected: Vec<_> = [0, 2, 4, 6, 8]
        .into_iter()
        .map(|i| original[i].kind.clone())
        .collect();

    let (_, terminal_formula) = parse_formula("b").unwrap();
    let mut terminal = Node::from_operands(vec![terminal_formula.clone().into()]);
    terminal.current_time = 2;
    let mut builder = TraceBuilder::new();
    builder.add_node(&terminal);
    builder.add_node(&node);
    let trace = builder.freeze();

    assert_eq!(node.operands, original);
    assert_eq!(trace.eval(0), expected);
    assert_eq!(
        trace.full_trace(),
        vec![expected.clone(), expected, vec![terminal_formula]],
    );
}
