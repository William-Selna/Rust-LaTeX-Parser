//! Source spans: every node knows which part of the input it came from.

use rust_latex_parser::{parse_equation, EqNode, Node, Span};

/// The source text of a node.
fn src<'a>(input: &'a str, node: &Node) -> &'a str {
    &input[node.span.range()]
}

/// Find the first node (depth-first) matching a predicate.
fn find<'a>(node: &'a Node, pred: &dyn Fn(&Node) -> bool) -> Option<&'a Node> {
    if pred(node) {
        return Some(node);
    }
    node.children().into_iter().find_map(|c| find(c, pred))
}

fn text_node<'a>(tree: &'a Node, text: &str) -> &'a Node {
    find(tree, &|n| matches!(&n.kind, EqNode::Text(t) if t == text)).unwrap_or_else(|| panic!("no text {:?}", text))
}

/// Check span invariants over a whole tree: spans lie within the input on
/// character boundaries, and children lie inside their parent.
fn check_spans(input: &str, node: &Node) {
    let s = node.span;
    assert!(s.start <= s.end && s.end <= input.len(), "span {:?} out of range for {:?}", s, input);
    assert!(input.is_char_boundary(s.start) && input.is_char_boundary(s.end), "span {:?} splits a char in {:?}", s, input);
    for child in node.children() {
        if !child.span.is_empty() && !node.span.is_empty() {
            assert!(
                node.span.start <= child.span.start && child.span.end <= node.span.end,
                "child {:?} {:?} outside parent {:?} {:?} in {:?}",
                child.span, child.kind, node.span, node.kind, input
            );
        }
        check_spans(input, child);
    }
}

#[test]
fn simple_spans() {
    let input = r"a + \sqrt{x}";
    let tree = parse_equation(input);
    assert_eq!(tree.span, Span::new(0, input.len()));
    let EqNode::Seq(parts) = &tree.kind else { panic!() };
    assert_eq!(src(input, &parts[0]), "a");
    assert_eq!(src(input, &parts[1]), "+");
    assert_eq!(src(input, &parts[2]), r"\sqrt{x}");
    assert_eq!(src(input, text_node(&tree, "x")), "x");
}

#[test]
fn spans_are_byte_offsets() {
    let input = "α + β²";
    let tree = parse_equation(input);
    let beta = text_node(&tree, "β");
    assert_eq!(beta.span, Span::new(5, 7)); // α is 2 bytes, β is 2 bytes
    let sup = find(&tree, &|n| matches!(n.kind, EqNode::Sup(..))).unwrap();
    assert_eq!(src(input, sup), "β²");
}

#[test]
fn script_and_fraction_spans() {
    let input = r"x^{n+1} + \frac{a}{b}";
    let tree = parse_equation(input);
    let sup = find(&tree, &|n| matches!(n.kind, EqNode::Sup(..))).unwrap();
    assert_eq!(src(input, sup), "x^{n+1}");
    let frac = find(&tree, &|n| matches!(n.kind, EqNode::Frac(..))).unwrap();
    assert_eq!(src(input, frac), r"\frac{a}{b}");
}

#[test]
fn natural_input_spans() {
    let input = "(a+b)/(c+d) + dy/dx";
    let tree = parse_equation(input);
    let EqNode::Seq(parts) = &tree.kind else { panic!() };
    assert_eq!(src(input, &parts[0]), "(a+b)/(c+d)");
    assert_eq!(src(input, &parts[2]), "dy/dx");

    let input = "x^(n+1) + abs(y)";
    let tree = parse_equation(input);
    let EqNode::Seq(parts) = &tree.kind else { panic!() };
    assert_eq!(src(input, &parts[0]), "x^(n+1)");
    assert_eq!(src(input, &parts[2]), "abs(y)");
}

#[test]
fn environment_cell_spans() {
    let input = r"\begin{pmatrix} a & bc \\ d & e \end{pmatrix}";
    let tree = parse_equation(input);
    assert_eq!(tree.span, Span::new(0, input.len()));
    let EqNode::Matrix { rows, .. } = &tree.kind else { panic!() };
    assert_eq!(src(input, &rows[0][1]), "bc");
    assert_eq!(src(input, &rows[1][0]), "d");
}

#[test]
fn stray_token_spans() {
    let input = r"a } \foo";
    let tree = parse_equation(input);
    assert_eq!(src(input, text_node(&tree, "}")), "}");
    assert_eq!(src(input, text_node(&tree, r"\foo")), r"\foo");
}

#[test]
fn made_up_nodes_have_empty_spans() {
    // The `3` of a typed cube root isn't in the input as `3`.
    let tree = parse_equation("∛x");
    let EqNode::Root { index, .. } = &tree.kind else { panic!() };
    assert_eq!(index.kind, EqNode::Text("3".into()));
    // A missing \frac argument.
    let tree = parse_equation(r"\frac{a}");
    let EqNode::Frac(_, denom) = &tree.kind else { panic!() };
    assert!(denom.span.is_empty());
}

#[test]
fn node_at_finds_the_innermost_node() {
    let input = r"a + \frac{x}{y_1}";
    let tree = parse_equation(input);
    let at = |offset: usize| tree.node_at(offset).map(|n| src(input, n).to_string());
    assert_eq!(at(0).as_deref(), Some("a"));
    assert_eq!(at(10).as_deref(), Some("x"));
    assert_eq!(at(15).as_deref(), Some("1"));
    assert_eq!(at(2).as_deref(), Some("+"));
    assert_eq!(tree.node_at(999), None);
}

#[test]
fn equality_ignores_spans() {
    assert_eq!(parse_equation("a+b"), parse_equation("a   +   b"));
    assert_ne!(parse_equation("a+b"), parse_equation("a-b"));
    assert_ne!(parse_equation("a+b").span, parse_equation("a   +   b").span);
}

#[test]
fn children_in_reading_order() {
    let tree = parse_equation(r"\frac{a}{b}");
    let kids: Vec<_> = tree.children().into_iter().map(|n| n.kind.clone()).collect();
    assert_eq!(kids, vec![EqNode::Text("a".into()), EqNode::Text("b".into())]);
    let tree = parse_equation("x_1^2");
    let kids: Vec<_> = tree.children().into_iter().map(|n| n.kind.clone()).collect();
    assert_eq!(kids, vec![EqNode::Text("x".into()), EqNode::Text("1".into()), EqNode::Text("2".into())]);
}

const CORPUS: &[&str] = &[
    r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
    r"\sum_{i=1}^{n} i^2 = \frac{n(n+1)(2n+1)}{6}",
    r"\int_0^\infty e^{-x^2}\,dx",
    r"\begin{cases} x & x \geq 0 \\ -x & \text{otherwise} \end{cases}",
    r"\begin{aligned} f(x) &= (x+1)^2 \\ &= x^2 + 2x + 1 \end{aligned}",
    r"\left[\begin{array}{cc|c} \hline 1 & 2 & 3 \end{array}\right]",
    r"\left\{ x \middle| x > 0 \right\} \boxed{E} \textcolor{red}{y} \color{blue} z",
    r"\overbrace{a+b}^{n} \xrightarrow[a]{b} \not\in \pmod{p} {a \over b}",
    "x = (-b +/- sqrt(b² - 4ac))/(2a) + ∑_(k=1)^n k + ½ + ℝ + f′(x) + \"text\"",
    r"\begin{foo} a & b \end{foo} } & \\ \unknown",
];

#[test]
fn span_invariants_hold_for_corpus() {
    for input in CORPUS {
        check_spans(input, &parse_equation(input));
    }
}

#[test]
fn span_invariants_hold_for_random_input() {
    const FRAGMENTS: &[&str] = &[
        "x", "αβ", "2", " ", "{", "}", "&", "\\\\", "(", ")", "^", "_", "'", "+", "-", "/", "=",
        "\\frac", "\\sqrt[", "]", "\\left(", "\\right)", "\\middle|", "\\begin{pmatrix}",
        "\\end{pmatrix}", "\\begin{foo}", "\\color{red}", "\\over", "\\not", "abs(", "\"q",
        "√", "²", "₁", "½", "∑", "ℝ", "<=", "\\text{é}", "\\unknown",
    ];
    let mut state = 0x5eedu64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    for _ in 0..3000 {
        let len = 1 + next() % 25;
        let input: String = (0..len).map(|_| FRAGMENTS[next() % FRAGMENTS.len()]).collect();
        check_spans(&input, &parse_equation(&input));
    }
}
