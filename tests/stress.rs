//! Stress tests: arbitrary Unicode, scaling behaviour, large inputs,
//! thread safety, determinism, and round-tripping random trees.
//!
//! Everything here must hold for *any* input: the parser is used live in
//! editors, so it sees every half-typed, pasted or adversarial string.

use std::time::{Duration, Instant};

use rust_latex_parser::{parse_equation, EqNode, Node};

// ─── Deterministic random source (no dev-dependencies needed) ───────

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Characters that tend to break text handling: controls, NUL, BOM,
/// combining marks, zero-width joiners, RTL marks, emoji sequences,
/// astral-plane characters, noncharacters, and every ASCII symbol.
const NASTY: &[&str] = &[
    "\0", "\u{1}", "\u{7}", "\u{8}", "\t", "\n", "\r", "\u{B}", "\u{C}", "\u{1B}", "\u{7F}", "\u{85}",
    "\u{A0}", "\u{AD}", "\u{FEFF}", "\u{200B}", "\u{200C}", "\u{200D}", "\u{200E}", "\u{200F}",
    "\u{202E}", "\u{2028}", "\u{2029}", "\u{2060}", "\u{FFFD}", "\u{FFFE}", "\u{FFFF}", "\u{E000}",
    "\u{10FFFF}", "\u{1F600}", "👨‍👩‍👧‍👦", "🏳️‍🌈", "e\u{301}", "\u{301}", "\u{20D7}", "\u{338}",
    "ﷺ", "ש", "ل", "ᄀ", "𝕏", "𝐱", "Ω", "Ω", "ﬁ", "ǅ", "ß", "İ", "ı", "\u{1D400}", "\u{1D7FF}",
    "\\", "{", "}", "[", "]", "(", ")", "^", "_", "&", "%", "$", "#", "~", "'", "\"", "/", "|", "<", ">",
    "=", "+", "-", "*", ",", ".", ";", ":", "!", "?", "@", "`",
];

/// Random fragments: nasty characters mixed with real syntax.
const SYNTAX: &[&str] = &[
    "x", "12", " ", "\\frac", "\\sqrt", "\\sqrt[", "\\left(", "\\right)", "\\left\\{", "\\middle|",
    "\\begin{pmatrix}", "\\end{pmatrix}", "\\begin{array}{c|c}", "\\end{array}", "\\begin{aligned}",
    "\\end{aligned}", "\\begin{cases}", "\\end{cases}", "\\\\", "\\hline", "\\text{", "\\textbf{",
    "\\color{red}", "\\textcolor{#fff}", "\\over", "\\choose", "\\not", "\\big(", "\\Bigg]", "\\sum",
    "\\int", "\\lim", "\\overbrace", "\\underbrace", "\\xrightarrow[", "\\operatorname{", "\\hspace{",
    "\\mkern", "\\label{", "\\substack{", "\\hat", "\\mathbb", "\\boxed", "\\unknown", "abs(", "root(",
    "text(", "sqrt(", "<=", "->", "+/-", "√", "∛", "²", "₁", "½", "∑", "∫", "′", "ℝ", "\"",
];

fn random_input(rng: &mut Rng, max_len: usize) -> String {
    let len = 1 + rng.below(max_len);
    let mut s = String::new();
    for _ in 0..len {
        if rng.below(3) == 0 {
            s.push_str(rng.pick(NASTY));
        } else {
            s.push_str(rng.pick(SYNTAX));
        }
    }
    s
}

/// Every span lies inside the input on character boundaries, and every
/// child lies inside its parent.
fn check_spans(input: &str, node: &Node) {
    let s = node.span;
    assert!(s.start <= s.end && s.end <= input.len(), "span {:?} out of range for {:?}", s, input);
    assert!(input.is_char_boundary(s.start) && input.is_char_boundary(s.end), "span {:?} splits a char in {:?}", s, input);
    for child in node.children() {
        if !child.span.is_empty() && !node.span.is_empty() {
            assert!(
                node.span.start <= child.span.start && child.span.end <= node.span.end,
                "child {:?} outside parent {:?} in {:?}",
                child.span,
                node.span,
                input
            );
        }
        check_spans(input, child);
    }
}

/// XML well-formedness, including that no character is illegal in XML 1.0
/// (NUL and most C0 controls are, even escaped).
#[cfg(any(feature = "mathml", feature = "svg"))]
fn check_xml(xml: &str) {
    for c in xml.chars() {
        let legal = matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && !matches!(c, '\u{FFFE}' | '\u{FFFF}'));
        assert!(legal, "illegal XML character {:?} in output", c);
    }
    let mut stack: Vec<&str> = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find('<') {
        assert!(!rest[..i].contains('>'), "stray '>' in text");
        let end = rest[i..].find('>').expect("unclosed tag") + i;
        let tag = &rest[i + 1..end];
        if let Some(name) = tag.strip_prefix('/') {
            assert_eq!(stack.pop(), Some(name), "mismatched tag");
        } else if !tag.ends_with('/') {
            stack.push(tag.split_whitespace().next().expect("empty tag"));
        }
        rest = &rest[end + 1..];
    }
    assert!(stack.is_empty(), "unclosed tags {:?}", stack);
}

/// Every numeric attribute in the SVG is a finite number.
#[cfg(feature = "svg")]
fn check_svg_numbers(svg: &str, input: &str) {
    const NUMERIC: &[&str] = &["x", "y", "width", "height", "viewBox", "font-size", "r", "cx", "cy", "stroke-width"];
    let mut rest = svg;
    while let Some(eq) = rest.find("=\"") {
        let name_start = rest[..eq].rfind([' ', '<']).map_or(0, |i| i + 1);
        let name = &rest[name_start..eq];
        let value_end = eq + 2 + rest[eq + 2..].find('"').unwrap();
        let value = &rest[eq + 2..value_end];
        if NUMERIC.contains(&name) {
            for part in value.split(' ') {
                let v: f64 = part.parse().unwrap_or_else(|_| panic!("{}={:?} is not a number for {:?}", name, value, input));
                assert!(v.is_finite(), "{}={} for {:?}", name, value, input);
            }
        }
        rest = &rest[value_end + 1..];
    }
}

/// Run every output format and check its invariants.
fn check_outputs(input: &str, tree: &Node) {
    let _ = (input, tree);
    // At the nesting limit, `\frac{..}{..}` written back out nests deeper
    // than the `a/b/...` it came from, so the re-parse cuts off at a
    // different point. Exact round-tripping is promised below the limit.
    #[cfg(feature = "latex")]
    if tree_depth(tree) < 30 {
        let latex = rust_latex_parser::to_latex(tree);
        // Re-parsing canonical LaTeX settles after at most one more round.
        let again = rust_latex_parser::to_latex(&parse_equation(&latex));
        assert_eq!(again, rust_latex_parser::to_latex(&parse_equation(&again)), "LaTeX not stable for {:?}", input);
    }
    #[cfg(feature = "mathml")]
    for display in [rust_latex_parser::MathDisplay::Inline, rust_latex_parser::MathDisplay::Block] {
        check_xml(&rust_latex_parser::to_mathml(tree, display));
    }
    #[cfg(feature = "unicode")]
    let _ = rust_latex_parser::to_unicode(tree);
    #[cfg(feature = "svg")]
    {
        let svg = rust_latex_parser::to_svg(tree, &rust_latex_parser::SvgOptions::default());
        check_xml(&svg);
        check_svg_numbers(&svg, input);
    }
}

// ─── Arbitrary input ────────────────────────────────────────────────

#[test]
fn arbitrary_unicode_never_panics_and_keeps_invariants() {
    let mut rng = Rng(0x9E3779B97F4A7C15);
    for _ in 0..4000 {
        let input = random_input(&mut rng, 30);
        let tree = parse_equation(&input);
        check_spans(&input, &tree);
        check_outputs(&input, &tree);
    }
}

#[test]
fn every_single_char_and_pair() {
    // Each nasty character on its own, and every pair of them.
    for a in NASTY {
        let tree = parse_equation(a);
        check_spans(a, &tree);
        check_outputs(a, &tree);
        for b in NASTY {
            let input = format!("{}{}", a, b);
            check_spans(&input, &parse_equation(&input));
        }
    }
}

#[test]
fn every_prefix_of_real_expressions() {
    // An editor parses every prefix as the user types.
    let exprs = [
        r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
        r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}^{-1}",
        r"\left\{ x \in \mathbb{R} \middle| \sum_{i=1}^{n} x_i^2 < \infty \right\}",
        r"\begin{array}{c|c} \hline \text{a} & \overbrace{b+c}^{n} \\ \end{array}",
        "lim_(x->0) sin(x)/x = 1 + \"done\" + abs(root(3, x²))",
        "αβγ 𝐱 ℝ ∑ ∫ √ ½ ≤ ≠ → 👍",
    ];
    for expr in exprs {
        for (i, _) in expr.char_indices().chain(std::iter::once((expr.len(), ' '))) {
            let prefix = &expr[..i];
            let tree = parse_equation(prefix);
            check_spans(prefix, &tree);
            check_outputs(prefix, &tree);
        }
    }
}

// ─── Scaling ────────────────────────────────────────────────────────

/// Time `f` on inputs of size `n` and `4n`. Linear work grows ~4x; flag
/// anything that grows much faster (quadratic would be ~16x).
fn assert_linear(name: &str, make: impl Fn(usize) -> String, f: impl Fn(&str)) {
    let time = |n: usize| {
        let input = make(n);
        f(&input); // warm up
        let mut best = Duration::MAX;
        for _ in 0..3 {
            let start = Instant::now();
            f(&input);
            best = best.min(start.elapsed());
        }
        best
    };
    let small = time(1_000);
    let large = time(4_000);
    let ratio = large.as_secs_f64() / small.as_secs_f64().max(1e-6);
    assert!(ratio < 9.0, "{}: 4x input took {:.1}x longer ({:?} -> {:?})", name, ratio, small, large);
}

/// Inputs that stress particular code paths, by repeat count.
type Shape = (&'static str, fn(usize) -> String);

fn shapes() -> Vec<Shape> {
    vec![
        ("flat sum", |n| "x + ".repeat(n)),
        ("letters", |n| "xy".repeat(n)),
        ("digits", |n| "1".repeat(n)),
        ("whitespace", |n| format!("x{}y", " ".repeat(n))),
        ("fractions", |n| "\\frac{a}{b}".repeat(n)),
        ("slashes", |n| "a/".repeat(n) + "b"),
        ("glued groups then slash", |n| "(a)".repeat(n) + "/b"),
        ("unmatched close parens then slash", |n| ")".repeat(n) + "/b"),
        ("open parens", |n| "(".repeat(n)),
        ("paren args", |n| format!("root({})", "x,".repeat(n))),
        ("matrix cells", |n| format!("\\begin{{pmatrix}}{}\\end{{pmatrix}}", "a&".repeat(n))),
        ("matrix rows", |n| format!("\\begin{{pmatrix}}{}\\end{{pmatrix}}", "a\\\\".repeat(n))),
        ("stray braces", |n| "}".repeat(n)),
        ("stray ampersands", |n| "&".repeat(n)),
        ("unknown commands", |n| "\\foo".repeat(n)),
        ("text block", |n| format!("\\text{{{}}}", "word ".repeat(n))),
        ("quoted text", |n| format!("\"{}", "word ".repeat(n))),
        ("comments", |n| "x % comment\n".repeat(n)),
        ("unicode scripts", |n| "x²".repeat(n)),
        ("long superscript run", |n| format!("x{}", "²".repeat(n))),
        ("primes", |n| format!("f{}", "'".repeat(n))),
        ("colors", |n| "\\color{red}x".repeat(n)),
        ("over", |n| "a \\over ".repeat(n) + "b"),
        ("middles", |n| format!("\\left({}\\right)", "x\\middle|".repeat(n))),
        ("big ops", |n| "\\sum_{i}^{n}".repeat(n)),
        ("greek", |n| "\\alpha".repeat(n)),
        ("barewords", |n| "pi ".repeat(n)),
        ("near-barewords", |n| "p".repeat(n) + "i"),
        ("spaces commands", |n| "\\,".repeat(n) + "-x"),
        ("nasty chars", |n| "\u{200D}\u{301}👨‍👩‍👧".repeat(n)),
    ]
}

#[test]
fn parsing_scales_linearly() {
    for (name, make) in shapes() {
        assert_linear(name, make, |s| {
            let _ = parse_equation(s);
        });
    }
}

#[cfg(all(feature = "latex", feature = "mathml", feature = "unicode", feature = "svg"))]
#[test]
fn outputs_scale_linearly() {
    use rust_latex_parser::*;
    for (name, make) in shapes() {
        let tree_of = |s: &str| parse_equation(s);
        assert_linear(&format!("latex/{}", name), make, |s| {
            let _ = to_latex(&tree_of(s));
        });
        assert_linear(&format!("mathml/{}", name), make, |s| {
            let _ = to_mathml(&tree_of(s), MathDisplay::Block);
        });
        assert_linear(&format!("unicode/{}", name), make, |s| {
            let _ = to_unicode(&tree_of(s));
        });
        assert_linear(&format!("svg/{}", name), make, |s| {
            let _ = to_svg(&tree_of(s), &SvgOptions::default());
        });
    }
}

#[test]
fn megabyte_input() {
    // A 1 MB paste parses in well under a second, even in a debug build.
    let input = r"\frac{a_1 + b^2}{\sqrt{c}} + \alpha \cdot x_{i,j} - ".repeat(20_000);
    assert!(input.len() > 1_000_000);
    let start = Instant::now();
    let tree = parse_equation(&input);
    let took = start.elapsed();
    assert!(took < Duration::from_secs(3), "1 MB took {:?}", took);
    // The root spans all the content (trailing whitespace isn't content).
    assert_eq!(tree.span.end, input.trim_end().len());
}

// ─── Nesting ────────────────────────────────────────────────────────

/// Depth of a tree, computed without recursion.
fn tree_depth(root: &Node) -> usize {
    let mut max = 0;
    let mut stack = vec![(root, 1usize)];
    while let Some((node, d)) = stack.pop() {
        max = max.max(d);
        stack.extend(node.children().into_iter().map(|c| (c, d + 1)));
    }
    max
}

#[test]
fn chains_that_used_to_overflow_the_stack() {
    // Each of these built a tree as deep as the input is long, and crashed
    // the process (even in release builds) when parsed or dropped.
    for input in ["a/".repeat(100_000) + "b", "x^(".repeat(100_000) + "1", "1/(".repeat(100_000) + "1"] {
        let tree = parse_equation(&input);
        assert!(tree_depth(&tree) < 200, "depth {}", tree_depth(&tree));
        check_outputs(&input[..2_000], &parse_equation(&input[..2_000]));
    }
}

#[test]
fn no_construct_nests_without_bound() {
    // Repeat every fragment, and a sample of fragment pairs, thousands of
    // times: the tree depth must stay bounded however long the input is.
    let mut patterns: Vec<String> = SYNTAX.iter().chain(NASTY).map(|s| s.to_string()).collect();
    let mut rng = Rng(99);
    for _ in 0..600 {
        patterns.push(format!("{}{}", rng.pick(SYNTAX), rng.pick(SYNTAX)));
    }
    for p in patterns {
        let input = p.repeat(1_500);
        let depth = tree_depth(&parse_equation(&input));
        assert!(depth < 200, "{:?} repeated nests {} deep", p, depth);
    }
}

// ─── API guarantees ─────────────────────────────────────────────────

#[test]
fn types_are_send_sync_and_static() {
    fn assert_traits<T: Send + Sync + 'static>() {}
    assert_traits::<Node>();
    assert_traits::<EqNode>();
    assert_traits::<rust_latex_parser::Span>();
    #[cfg(feature = "svg")]
    assert_traits::<rust_latex_parser::SvgOptions>();
}

#[test]
fn parsing_is_deterministic_and_thread_safe() {
    let inputs: Vec<String> = {
        let mut rng = Rng(42);
        (0..200).map(|_| random_input(&mut rng, 20)).collect()
    };
    let expected: Vec<String> = inputs.iter().map(|s| format!("{:?}", parse_equation(s))).collect();
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let inputs = inputs.clone();
            std::thread::spawn(move || inputs.iter().map(|s| format!("{:?}", parse_equation(s))).collect::<Vec<_>>())
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), expected);
    }
}

// ─── Random trees round-trip through LaTeX ──────────────────────────

/// Build a random, valid tree from the node types `to_latex` can express,
/// using only content that means the same thing when re-parsed.
#[cfg(feature = "latex")]
fn random_tree(rng: &mut Rng, depth: usize) -> Node {
    use rust_latex_parser::*;
    let leaf = |rng: &mut Rng| -> EqNode {
        match rng.below(6) {
            0 => EqNode::Text(rng.pick(&["x", "y", "a", "b", "n"]).to_string()),
            1 => EqNode::Text(rng.pick(&["1", "2", "42", "3.5"]).to_string()),
            2 => EqNode::Text(rng.pick(&["α", "π", "∞", "∂", "ℏ"]).to_string()),
            3 => EqNode::Op { symbol: rng.pick(&["=", "<", "≤", "→", "∈"]).to_string(), kind: OpKind::Relation },
            4 => EqNode::TextBlock(rng.pick(&["if", "for all", "otherwise"]).to_string()),
            _ => EqNode::Limit { name: rng.pick(&["sin", "log", "rank"]).to_string(), lower: None },
        }
    };
    if depth == 0 {
        return leaf(rng).into();
    }
    let sub = |rng: &mut Rng| Box::new(random_tree(rng, depth - 1));
    let kind = match rng.below(14) {
        0 | 1 => {
            // Sequences alternate operands and binary operators so unary
            // signs don't change meaning.
            let n = 2 + rng.below(3);
            let mut v = Vec::new();
            for i in 0..n {
                if i > 0 {
                    v.push(EqNode::Op { symbol: rng.pick(&["+", "-", "·", "×"]).to_string(), kind: OpKind::Binary }.into());
                }
                v.push(Node::from(EqNode::Frac(sub(rng), sub(rng))));
            }
            EqNode::Seq(v)
        }
        2 => EqNode::Frac(sub(rng), sub(rng)),
        3 => EqNode::Sqrt(sub(rng)),
        4 => EqNode::Root { index: Box::new(EqNode::Text("3".into()).into()), content: sub(rng) },
        5 => EqNode::Sup(Box::new(EqNode::Text("x".into()).into()), sub(rng)),
        6 => EqNode::Sub(Box::new(EqNode::Text("a".into()).into()), sub(rng)),
        7 => EqNode::Binom(sub(rng), sub(rng)),
        8 => EqNode::Delimited { left: "(".into(), right: ")".into(), content: sub(rng) },
        9 => EqNode::Accent(Box::new(EqNode::Text("x".into()).into()), *rng.pick(&[AccentKind::Hat, AccentKind::Bar, AccentKind::Vec, AccentKind::Dot])),
        10 => EqNode::MathFont { kind: *rng.pick(&[MathFontKind::Bold, MathFontKind::Blackboard, MathFontKind::Calligraphic]), content: Box::new(EqNode::Text("R".into()).into()) },
        11 => EqNode::Matrix { kind: MatrixKind::Paren, rows: vec![vec![random_tree(rng, depth - 1), random_tree(rng, depth - 1)], vec![random_tree(rng, depth - 1), random_tree(rng, depth - 1)]] },
        12 => EqNode::BigOp { symbol: "∑".into(), lower: Some(sub(rng)), upper: Some(sub(rng)) },
        _ => EqNode::Enclose { kind: EncloseKind::Box, content: sub(rng) },
    };
    kind.into()
}

#[cfg(feature = "latex")]
#[test]
fn random_trees_round_trip_through_latex() {
    use rust_latex_parser::to_latex;
    /// Flatten nested sequences: `{a b} c` and `a b c` mean the same.
    fn flat(node: &Node) -> String {
        let mut s = format!("{:?}", node.kind);
        // Debug output includes spans; strip them for comparison.
        while let Some(i) = s.find("span: Span {") {
            let j = s[i..].find('}').unwrap() + i + 1;
            s.replace_range(i..j, "");
        }
        s
    }
    let mut rng = Rng(7);
    for i in 0..2000 {
        let tree = random_tree(&mut rng, 1 + i % 4);
        let latex = to_latex(&tree);
        let reparsed = parse_equation(&latex);
        assert_eq!(
            to_latex(&reparsed),
            latex,
            "\ntree: {}\nlatex: {}\nreparsed: {}",
            flat(&tree),
            latex,
            flat(&reparsed)
        );
    }
}

#[cfg(feature = "latex")]
#[test]
fn any_text_content_round_trips_exactly() {
    // Text blocks are raw, so they can hold backslashes and braces in any
    // arrangement; the LaTeX writer must encode them so they read back
    // exactly.
    const PIECES: &[&str] = &["a", " ", "\\", "{", "}", "\\{", "\\}", "\\textbackslash{}", "textbackslash", "%", "&", "$", "\u{0}", "é"];
    let mut rng = Rng(5);
    for _ in 0..3000 {
        let len = rng.below(12);
        let content: String = (0..len).map(|_| *rng.pick(PIECES)).collect();
        let tree: Node = EqNode::TextBlock(content.clone()).into();
        let latex = rust_latex_parser::to_latex(&tree);
        assert_eq!(parse_equation(&latex).kind, EqNode::TextBlock(content.clone()), "{:?} -> {}", content, latex);
    }
}
