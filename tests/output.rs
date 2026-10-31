//! Output formats: LaTeX, MathML, Unicode.
#![cfg(all(feature = "latex", feature = "mathml", feature = "unicode"))]

mod common;

use rust_latex_parser::{parse_equation, to_latex, to_mathml, to_unicode, EqNode, MathDisplay, Node};

/// Valid expressions covering every node type.
const CORPUS: &[&str] = &[
    r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
    r"\sum_{i=1}^{n} i^2 = \frac{n(n+1)(2n+1)}{6}",
    r"\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}",
    r"\lim_{x \to 0} \frac{\sin x}{x} = 1",
    r"\mathbb{R}^n \to \mathbf{F} \quad \mathcal{L}\{f\}",
    r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
    r"\begin{bmatrix} 1 & 0 \\ 0 & 1 \end{bmatrix} \begin{vmatrix} a \end{vmatrix} \begin{Vmatrix} b \end{Vmatrix} \begin{Bmatrix} c \end{Bmatrix} \begin{matrix} d \end{matrix}",
    r"|x| = \begin{cases} x & x \geq 0 \\ -x & \text{otherwise} \end{cases}",
    r"\begin{aligned} f(x) &= (x+1)^2 \\ &= x^2 + 2x + 1 \end{aligned}",
    r"\begin{gathered} a = b \\ c \end{gathered}",
    r"\left[\begin{array}{cc|c} \hline 1 & 2 & 3 \\ 4 & 5 & 6 \\ \hline \end{array}\right]",
    r"\hat{x} + \vec{v} + \overline{AB} + \underline{y} + \tilde{n} + \widehat{xy} + \dot{a} + \ddot{b} + \dddot{c}",
    r"\acute{a}\grave{b}\breve{c}\check{d}\mathring{e}\overleftarrow{AB}\overleftrightarrow{CD}\widetilde{xy}\overrightarrow{PQ}",
    r"\boxed{E = mc^2} \cancel{x} \bcancel{y} \xcancel{z} \textcolor{red}{w} \fbox{note}",
    r"\overbrace{a + b}^{n} \underbrace{x + y}_{\text{sum}} \overbrace{c}",
    r"\binom{n}{k} \sqrt[3]{x} \bigl( x \bigr) \Big[ \bigg\{ \Bigg| a \overset{def}{=} b \underset{n}{\to} c",
    r"\xrightarrow[below]{above} \xleftarrow{f}",
    r"\left\{ x \in \mathbb{R} \middle| x > 0 \right\} \lfloor x \rfloor \left\langle u, v \right\rangle \left. \frac{df}{dx} \right|_{x=0}",
    r"\textbf{bold} \textit{it} \texttt{mono} \textsf{sans} \mathit{x} \boldsymbol{\alpha} \mathfrak{g} \mathsf{s} \mathtt{t} \mathrm{d}x",
    r"f'(x) + f''(y) + g'^2 + h'_1",
    r"\operatorname{rank}(A) + \dim \ker A \bmod n + a \pmod{p}",
    r"a \not\in B, \; c \neq d \leq e \; \alpha\beta\gamma \hbar \infty \partial \nabla",
    r"x_{i,j}^{2} \quad x^{2^{3}} \quad {}^{14}C \quad x_1^2",
    r"\{1, 2, 3\} \cup \{4\} \cap A \setminus B \times C",
    r"a \quad b \qquad c \, d \; e \! f \: g \hspace{1em} h \mkern7mu i",
    r"\iint \oint_C \prod_{k} \coprod \bigcup_i \bigcap \bigoplus \bigotimes \bigvee \bigwedge",
    r"\log_2 n + \ln x + \exp(x) + \max_{i} a_i + \min(a,b) + \gcd(a, b) + \Pr(A)",
    r"\#\$\%\&\_\{\} \backslash \|x\|",
    r"-x + (-y) - (+z)",
    r"pi r^2 + sqrt(x+1) + int_0^1 x dx",
    "(a+b)/(c+d) + dy/dx + sin(x)/x + abs(x) + root(3, x) + \"text\" + x² + √y + ½ + x <= y -> z",
    r"\unknown + x",
];

/// Recursively flatten nested sequences so trees that differ only in
/// grouping compare equal (`{a b} c` vs `a b c` print the same).
fn normalize(node: &Node) -> Node {
    fn flat(node: &Node, out: &mut Vec<Node>) {
        match &node.kind {
            EqNode::Seq(v) => v.iter().for_each(|n| flat(n, out)),
            _ => out.push(normalize_inner(node)),
        }
    }
    let mut out = Vec::new();
    flat(node, &mut out);
    if out.len() == 1 { out.remove(0) } else { EqNode::Seq(out).into() }
}

fn normalize_inner(node: &Node) -> Node {
    let b = |n: &Node| Box::new(normalize(n));
    let ob = |n: &Option<Box<Node>>| n.as_ref().map(|n| b(n));
    let rows = |r: &Vec<Vec<Node>>| r.iter().map(|row| row.iter().map(normalize).collect()).collect();
    let kind = match &node.kind {
        EqNode::Sup(x, y) => EqNode::Sup(b(x), b(y)),
        EqNode::Sub(x, y) => EqNode::Sub(b(x), b(y)),
        EqNode::SupSub(x, y, z) => EqNode::SupSub(b(x), b(y), b(z)),
        EqNode::Frac(x, y) => EqNode::Frac(b(x), b(y)),
        EqNode::Binom(x, y) => EqNode::Binom(b(x), b(y)),
        EqNode::Sqrt(x) => EqNode::Sqrt(b(x)),
        EqNode::Root { index, content } => EqNode::Root { index: b(index), content: b(content) },
        EqNode::Accent(x, k) => EqNode::Accent(b(x), *k),
        EqNode::MathFont { kind, content } => EqNode::MathFont { kind: *kind, content: b(content) },
        EqNode::Delimited { left, right, content } => EqNode::Delimited { left: left.clone(), right: right.clone(), content: b(content) },
        EqNode::Color { color, content } => EqNode::Color { color: color.clone(), content: b(content) },
        EqNode::Enclose { kind, content } => EqNode::Enclose { kind: *kind, content: b(content) },
        EqNode::StackRel { base, annotation, over } => EqNode::StackRel { base: b(base), annotation: b(annotation), over: *over },
        EqNode::Brace { content, label, over } => EqNode::Brace { content: b(content), label: ob(label), over: *over },
        EqNode::BigOp { symbol, lower, upper } => EqNode::BigOp { symbol: symbol.clone(), lower: ob(lower), upper: ob(upper) },
        EqNode::Limit { name, lower } => EqNode::Limit { name: name.clone(), lower: ob(lower) },
        EqNode::Matrix { kind, rows: r } => EqNode::Matrix { kind: *kind, rows: rows(r) },
        EqNode::Align { rows: r } => EqNode::Align { rows: rows(r) },
        EqNode::Array { columns, rows: r, hlines } => EqNode::Array { columns: columns.clone(), rows: rows(r), hlines: hlines.clone() },
        EqNode::Gather { rows: r } => EqNode::Gather { rows: r.iter().map(normalize).collect() },
        EqNode::Cases { rows: r } => EqNode::Cases { rows: r.iter().map(|(v, c)| (normalize(v), c.as_ref().map(normalize))).collect() },
        other => other.clone(),
    };
    Node::new(kind, node.span)
}

// ─── LaTeX ──────────────────────────────────────────────────────────

#[test]
fn latex_round_trips_to_the_same_tree() {
    for src in CORPUS {
        let tree = parse_equation(src);
        let latex = to_latex(&tree);
        let reparsed = parse_equation(&latex);
        assert_eq!(normalize(&reparsed), normalize(&tree), "\ninput: {}\nlatex: {}", src, latex);
    }
}

#[test]
fn latex_output_is_stable() {
    for src in CORPUS {
        let once = to_latex(&parse_equation(src));
        let twice = to_latex(&parse_equation(&once));
        assert_eq!(once, twice, "input: {}", src);
    }
}

#[test]
fn latex_from_natural_input() {
    let cases = [
        ("sqrt(x^2 + 1)/2 <= abs(y)", r"\frac{\sqrt{x^2 + 1}}{2} \leq \left|y\right|"),
        ("lim_(x->0) sin(x)/x = 1", r"\lim_{x \rightarrow 0} \frac{\sin(x)}{x} = 1"),
        ("x² + y² = r²", "x^2 + y^2 = r^2"),
        ("√(a+b) + ½", r"\sqrt{a + b} + \frac{1}{2}"),
        ("root(3, x) + binom(n, k)", r"\sqrt[3]{x} + \binom{n}{k}"),
        ("\"for all\" x in A", r"\text{for all}x inA"),
        ("dy/dx", r"\frac{dy}{dx}"),
    ];
    for (src, expected) in cases {
        assert_eq!(to_latex(&parse_equation(src)), expected, "{}", src);
    }
}

#[test]
fn latex_separates_letters_only_when_needed() {
    // Glued letters are fine unless they would spell a shortcut word.
    assert_eq!(to_latex(&parse_equation("4ac")), "4ac");
    assert_eq!(to_latex(&parse_equation("p i")), "p i");
    assert_eq!(to_latex(&parse_equation("s i n x")), "si nx");
    assert_eq!(to_latex(&parse_equation(r"\alpha x")), r"\alpha x");
    assert_eq!(to_latex(&parse_equation(r"\sin(x)")), r"\sin(x)");
    assert_eq!(to_latex(&parse_equation("a b s(x)")), "abs (x)");
    assert_eq!(to_latex(&parse_equation("1 2")), "1 2");
}

#[test]
fn latex_escapes_special_characters() {
    assert_eq!(to_latex(&parse_equation(r"\{a\} \& \% \# \_ \$")), r"\{a\}\&\%\#\_\$");
    assert_eq!(to_latex(&parse_equation("a } b")), r"a\}b");
}

// ─── Unicode ────────────────────────────────────────────────────────

#[test]
fn unicode_output() {
    let cases = [
        (r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}", "(-b ± √(b² - 4ac))/2a"),
        (r"\mathbb{R}^n \to \mathbf{F}", "ℝⁿ → 𝐅"),
        (r"\sum_{i=1}^n x_i", "∑ᵢ₌₁ⁿ xᵢ"),
        (r"\lim_{x \to 0} \frac{\sin(x)}{x}", "lim_(x → 0) sin(x)/x"),
        (r"\sqrt[3]{x} + \frac{1}{2}", "∛x + ½"),
        (r"\hat{x} + \bar{y}", "x\u{302} + y\u{304}"),
        (r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}", "(a, b; c, d)"),
        (r"e^{-x^2}", "e^(-x²)"),
        (r"f'(x)", "f′(x)"),
        (r"\left| x \right|", "|x|"),
        (r"\textcolor{red}{x} + \boxed{y}", "x + [y]"),
        (r"\begin{aligned} a &= b \\ &= c \end{aligned}", "a = b\n= c"),
    ];
    for (src, expected) in cases {
        assert_eq!(to_unicode(&parse_equation(src)), expected, "{}", src);
    }
}

#[test]
fn unicode_output_parses_back() {
    for src in [r"\frac{a+b}{c}", r"x^2 + y_1", r"\sqrt{x+1}", r"\mathbb{R}", "sin(x)/x", r"\int_0^1 f"] {
        let tree = parse_equation(src);
        let text = to_unicode(&tree);
        assert_eq!(
            to_latex(&parse_equation(&text)),
            to_latex(&tree),
            "\ninput: {}\nunicode: {}",
            src,
            text
        );
    }
}

// ─── MathML ─────────────────────────────────────────────────────────

/// Minimal well-formedness check: tags balance and text has no raw `<`/`&`.
fn assert_well_formed(xml: &str) {
    let mut stack: Vec<String> = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find('<') {
        let text = &rest[..i];
        assert!(!text.contains('>'), "stray '>' in text: {}", xml);
        for (j, _) in text.match_indices('&') {
            let entity = &text[j..];
            assert!(
                ["&amp;", "&lt;", "&gt;", "&quot;"].iter().any(|e| entity.starts_with(e)),
                "unescaped '&' in: {}",
                xml
            );
        }
        let end = rest[i..].find('>').expect("unclosed tag") + i;
        let tag = &rest[i + 1..end];
        if let Some(name) = tag.strip_prefix('/') {
            assert_eq!(stack.pop().as_deref(), Some(name), "mismatched </{}> in {}", name, xml);
        } else if !tag.ends_with('/') {
            let name = tag.split_whitespace().next().unwrap().to_string();
            stack.push(name);
        }
        rest = &rest[end + 1..];
    }
    assert!(stack.is_empty(), "unclosed tags {:?} in {}", stack, xml);
}

#[test]
fn mathml_basic_structure() {
    let ml = |s: &str| to_mathml(&parse_equation(s), MathDisplay::Inline);
    assert_eq!(ml("x^2"), r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><msup><mi>x</mi><mn>2</mn></msup></math>"#);
    assert!(ml(r"\frac{a}{b}").contains("<mfrac><mi>a</mi><mi>b</mi></mfrac>"));
    assert!(ml(r"\sqrt[3]{x}").contains("<mroot><mi>x</mi><mn>3</mn></mroot>"));
    assert!(ml("a + b").contains("<mo>+</mo>"));
    assert!(ml("a - b").contains("<mo>\u{2212}</mo>"), "minus sign");
    assert!(ml(r"\mathbb{R}").contains("<mi>ℝ</mi>"));
    assert!(ml(r"\text{if}").contains("<mtext>if</mtext>"));
    assert!(ml(r"\textcolor{red}{x}").contains(r#"<mrow mathcolor="red">"#));
    assert!(ml(r"\unknown").contains("<merror>"));
}

#[test]
fn mathml_display_modes() {
    let t = parse_equation(r"\sum_{i=1}^n i");
    let inline = to_mathml(&t, MathDisplay::Inline);
    let block = to_mathml(&t, MathDisplay::Block);
    assert!(!inline.contains("display="));
    assert!(block.contains(r#"display="block""#));
    assert!(block.contains("<munderover>"));
    // Integrals keep their limits to the side.
    assert!(to_mathml(&parse_equation(r"\int_0^1"), MathDisplay::Block).contains("<msubsup>"));
}

#[test]
fn mathml_fences() {
    let ml = |s: &str| to_mathml(&parse_equation(s), MathDisplay::Inline);
    // \left...\right and \middle stretch; plain brackets don't.
    assert!(ml(r"\left( x \middle| y \right)").matches(r#"stretchy="true""#).count() == 3);
    assert!(ml("(x)").contains(r#"<mo stretchy="false" lspace="0" rspace="0">(</mo>"#));
}

#[test]
fn mathml_escapes() {
    let ml = to_mathml(&parse_equation(r"a < b \text{ & <tag>}"), MathDisplay::Inline);
    assert!(ml.contains("<mo>&lt;</mo>"));
    assert!(ml.contains("<mtext> &amp; &lt;tag&gt;</mtext>"));
    assert_well_formed(&ml);
}

#[test]
fn mathml_is_well_formed_for_corpus() {
    for src in CORPUS {
        for display in [MathDisplay::Inline, MathDisplay::Block] {
            assert_well_formed(&to_mathml(&parse_equation(src), display));
        }
    }
}

// ─── Random input ───────────────────────────────────────────────────

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as usize
    }
}

#[test]
fn outputs_handle_random_input() {
    const FRAGMENTS: &[&str] = &[
        "x", "y", "2", " ", "{", "}", "&", "\\\\", "(", ")", "^", "_", "'", "+", "-", "/", "=", "<",
        "\\frac", "\\sqrt", "\\sqrt[", "]", "\\left(", "\\right)", "\\middle|", "\\begin{pmatrix}",
        "\\end{pmatrix}", "\\begin{cases}", "\\end{cases}", "\\begin{array}{c|c}", "\\end{array}",
        "\\hline", "\\color{red}", "\\textcolor{blue}", "\\boxed", "\\hat", "\\mathbb", "\\sum",
        "\\lim", "\\text{a b}", "\\alpha", "\\not", "\\bigl", "\\overbrace", "\\xrightarrow",
        "abs(", "root(", "\"q", "√", "²", "½", "<=", "->", "\\unknown",
    ];
    let mut rng = Lcg(0xfeed);
    for case in 0..3000 {
        let len = 1 + rng.next() % 30;
        let input: String = (0..len).map(|_| FRAGMENTS[rng.next() % FRAGMENTS.len()]).collect();
        let tree = parse_equation(&input);
        let latex = to_latex(&tree);
        assert_well_formed(&to_mathml(&tree, MathDisplay::Block));
        let _ = to_unicode(&tree);
        // Malformed input can contain literal tokens LaTeX can't escape (a
        // stray `\right` inside `\left...\right`), so the first rewrite may
        // regroup them; after that the output must be a fixed point.
        let again = to_latex(&parse_equation(&latex));
        assert_eq!(again, to_latex(&parse_equation(&again)), "case {}: input {:?}", case, input);
    }
}
