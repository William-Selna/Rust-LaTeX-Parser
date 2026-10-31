//! SVG output.
#![cfg(feature = "svg")]

use rust_latex_parser::{parse_equation, to_svg, MathDisplay, SvgOptions};

fn svg(input: &str) -> String {
    to_svg(&parse_equation(input), &SvgOptions::default())
}

/// Width and height attributes of the root `<svg>`.
fn size(svg: &str) -> (f32, f32) {
    let attr = |name: &str| -> f32 {
        let key = format!("{}=\"", name);
        let start = svg.find(&key).unwrap() + key.len();
        let end = start + svg[start..].find('"').unwrap();
        svg[start..end].parse().unwrap()
    };
    (attr("width"), attr("height"))
}

/// Tags balance, attributes are quoted, and no number is NaN or infinite.
fn assert_valid(svg: &str) {
    assert!(!svg.contains("NaN") && !svg.contains("inf"), "bad number in {}", svg);
    let mut stack: Vec<&str> = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find('<') {
        let text = &rest[..i];
        assert!(!text.contains('>'), "stray '>' in {}", svg);
        let end = rest[i..].find('>').expect("unclosed tag") + i;
        let tag = &rest[i + 1..end];
        if let Some(name) = tag.strip_prefix('/') {
            assert_eq!(stack.pop(), Some(name), "mismatched </{}>", name);
        } else if !tag.ends_with('/') {
            stack.push(tag.split_whitespace().next().unwrap());
        }
        assert_eq!(tag.matches('"').count() % 2, 0, "unbalanced quotes in <{}>", tag);
        rest = &rest[end + 1..];
    }
    assert!(stack.is_empty(), "unclosed {:?}", stack);
}

#[test]
fn basic_document() {
    let out = svg("x^2");
    assert!(out.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(out.contains("role=\"img\""));
    assert!(out.contains("<title>x²</title>"));
    assert!(out.contains("style=\"vertical-align:-"));
    assert!(out.contains(">x</text>"));
    assert!(out.contains(">2</text>"));
    assert_valid(&out);
}

#[test]
fn letters_are_italic_and_digits_upright() {
    let out = svg("x1");
    assert!(out.contains("font-style=\"italic\">x</text>"));
    assert!(out.contains(">1</text>") && !out.contains("italic\">1<"));
}

#[test]
fn shapes_are_drawn_as_paths() {
    assert!(svg(r"\frac{a}{b}").contains("<rect"), "fraction bar");
    assert!(svg(r"\sqrt{x}").contains("<path"), "radical");
    assert!(svg(r"\sum_{i=1}^n i").contains("<path"), "sigma");
    assert!(svg(r"\left( \frac{a}{b} \right)").matches("<path").count() >= 2, "stretchy parens");
    assert!(svg(r"\hat{x}").contains("<path"), "hat");
    assert!(svg(r"\dot{x}").contains("<circle"), "dot");
}

#[test]
fn fractions_are_taller_than_text() {
    let (_, h_text) = size(&svg("x"));
    let (_, h_frac) = size(&svg(r"\frac{a}{b}"));
    assert!(h_frac > h_text * 1.4, "{} vs {}", h_frac, h_text);
}

#[test]
fn display_style_enlarges_operators() {
    let tree = parse_equation(r"\sum_{i=1}^n i");
    let inline = to_svg(&tree, &SvgOptions::default());
    let block = to_svg(&tree, &SvgOptions { display: MathDisplay::Block, ..SvgOptions::default() });
    assert!(size(&block).1 > size(&inline).1);
}

#[test]
fn font_size_scales_everything() {
    let tree = parse_equation("a + b");
    let small = to_svg(&tree, &SvgOptions { font_size: 10.0, ..SvgOptions::default() });
    let large = to_svg(&tree, &SvgOptions { font_size: 40.0, ..SvgOptions::default() });
    let (ws, hs) = size(&small);
    let (wl, hl) = size(&large);
    assert!((wl / ws - 4.0).abs() < 0.05 && (hl / hs - 4.0).abs() < 0.05);
}

#[test]
fn operators_get_space() {
    // `a+b` is wider than `ab` by roughly the + sign plus two medium spaces.
    let (w_ab, _) = size(&svg("ab"));
    let (w_sum, _) = size(&svg("a+b"));
    let plus = 0.564 * 20.0;
    let spaces = 2.0 * 4.0 / 18.0 * 20.0;
    assert!((w_sum - w_ab - plus - spaces).abs() < 1.0, "{} vs {}", w_sum - w_ab, plus + spaces);
}

#[test]
fn color_and_errors() {
    let out = to_svg(&parse_equation("x"), &SvgOptions { color: "#123456".into(), ..SvgOptions::default() });
    assert!(out.contains("fill=\"#123456\""));
    assert!(svg(r"\textcolor{red}{x}").contains("fill=\"red\""));
    assert!(svg(r"\unknown").contains("fill=\"#c00\""));
}

#[test]
fn span_attributes() {
    let input = r"a + \sqrt{x}";
    let tree = parse_equation(input);
    let plain = to_svg(&tree, &SvgOptions::default());
    assert!(!plain.contains("data-span"));
    let tagged = to_svg(&tree, &SvgOptions { span_attributes: true, ..SvgOptions::default() });
    assert!(tagged.contains("data-span=\"0-1\">a</text>"));
    assert!(tagged.contains("data-span=\"10-11\""));
}

#[test]
fn escapes_text() {
    let out = svg(r"\text{a < b & c}");
    assert!(out.contains("a &lt; b &amp; c"));
    assert_valid(&out);
}

const CORPUS: &[&str] = &[
    r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
    r"\sum_{i=1}^{n} i^2 = \frac{n(n+1)(2n+1)}{6}",
    r"\int_0^\infty e^{-x^2}\,dx \iint \oint_C \prod_k \coprod \bigcup_i \bigcap \bigoplus \bigotimes \bigodot \biguplus \bigsqcup \bigvee \bigwedge",
    r"\lim_{x \to 0} \frac{\sin x}{x} \log_2 n",
    r"\mathbb{R}^n \mathbf{F} \mathcal{L} \mathfrak{g} \mathsf{s} \mathtt{t} \mathrm{d} \mathit{x} \boldsymbol{\alpha}",
    r"\begin{pmatrix} a & b \\ c & d \end{pmatrix} \begin{bmatrix} 1 \end{bmatrix} \begin{vmatrix} a \end{vmatrix} \begin{Vmatrix} b \end{Vmatrix} \begin{Bmatrix} c \end{Bmatrix} \begin{matrix} d \end{matrix}",
    r"\begin{cases} x & x \geq 0 \\ -x & \text{otherwise} \end{cases}",
    r"\begin{aligned} f(x) &= (x+1)^2 \\ &= x^2 + 2x + 1 \end{aligned} \begin{gathered} a \\ b \end{gathered}",
    r"\left[\begin{array}{cc|c} \hline 1 & 2 & 3 \\ 4 & 5 & 6 \\ \hline \end{array}\right]",
    r"\hat{x}\vec{v}\overline{AB}\underline{y}\tilde{n}\widehat{xy}\dot{a}\ddot{b}\dddot{c}\acute{a}\grave{b}\breve{c}\check{d}\mathring{e}\overleftarrow{AB}\overleftrightarrow{CD}",
    r"\boxed{E = mc^2} \cancel{x} \bcancel{y} \xcancel{z} \fbox{note}",
    r"\overbrace{a + b}^{n} \underbrace{x + y}_{\text{sum}}",
    r"\binom{n}{k} \sqrt[3]{x} \bigl( x \bigr) \Big[ \bigg\{ \Bigg| a \overset{def}{=} b \xrightarrow[a]{f} c",
    r"\left\{ x \middle| x > 0 \right\} \left\langle u \right\rangle \left\lfloor x \right\rfloor \left\lceil y \right\rceil \left\| v \right\| \left/ a \right\backslash \left. b \right|",
    r"f'(x) + f''' \quad a \, b \; c \! d \hspace{2em} e",
    "x² + √y + ½ + ∑_(k=1)^n k + \"text\" + ℝ + 漢字 + 🎉",
    r"\middle| alone",
];

#[test]
fn corpus_renders_valid_svg() {
    for src in CORPUS {
        for display in [MathDisplay::Inline, MathDisplay::Block] {
            let out = to_svg(&parse_equation(src), &SvgOptions { display, ..SvgOptions::default() });
            assert_valid(&out);
            let (w, h) = size(&out);
            assert!(w > 0.0 && h > 0.0, "{}", src);
        }
    }
}

#[test]
fn random_input_renders_valid_svg() {
    const FRAGMENTS: &[&str] = &[
        "x", "2", " ", "{", "}", "&", "\\\\", "(", ")", "^", "_", "'", "+", "-", "/", "=", "\\frac",
        "\\sqrt", "\\sqrt[", "]", "\\left(", "\\right)", "\\middle|", "\\begin{pmatrix}", "\\end{pmatrix}",
        "\\begin{cases}", "\\end{cases}", "\\begin{array}{c|c}", "\\end{array}", "\\hline",
        "\\begin{aligned}", "\\end{aligned}", "\\color{red}", "\\boxed", "\\hat", "\\sum", "\\int",
        "\\lim", "\\overbrace", "\\xrightarrow", "\\bigl(", "\\text{a<b}", "abs(", "√", "²", "½",
    ];
    let mut state = 0xabcdu64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    for _ in 0..2000 {
        let len = 1 + next() % 25;
        let input: String = (0..len).map(|_| FRAGMENTS[next() % FRAGMENTS.len()]).collect();
        assert_valid(&to_svg(&parse_equation(&input), &SvgOptions::default()));
    }
}

#[test]
fn deep_input_renders() {
    for input in [
        "{".repeat(3000) + "x" + &"}".repeat(3000),
        "\\frac{".repeat(3000) + "x",
        "\\left(".repeat(3000) + "x" + &"\\right)".repeat(3000),
        "\\begin{pmatrix}".repeat(3000) + "x",
        "x^{".repeat(3000) + "2",
    ] {
        assert_valid(&svg(&input));
    }
}
