//! Malformed / half-typed input must never lose content.
//!
//! The parser used to stop at a stray `}`, `&`, `\\` or `\end{...}` and
//! silently drop everything after it. These tests pin down the recovery
//! behavior and check, over thousands of generated inputs, that every
//! letter and digit the user typed survives into the tree.

mod common;

use common::{all_text, p};
use rust_latex_parser::{parse_equation, EqNode};

// ─── Stray structural tokens are kept, and parsing continues ────────

#[test]
fn stray_ampersand_at_top_level() {
    assert_eq!(p("a & b"), "(seq a & b)");
}

#[test]
fn stray_close_brace_at_top_level() {
    assert_eq!(p("a } b"), "(seq a } b)");
    assert_eq!(p("x}}}"), "(seq x } } })");
}

#[test]
fn stray_row_separator_at_top_level() {
    assert_eq!(p("a \\\\ b"), "(seq a \\\\ b)");
}

#[test]
fn stray_end_at_top_level() {
    assert_eq!(p("x \\end{foo} y"), "(seq x \\end{foo} y)");
    assert_eq!(p("x \\end y"), "(seq x \\end y)");
}

#[test]
fn stray_right_at_top_level() {
    assert_eq!(p("x + y \\right) z"), "(seq x + y \\right ) z)");
}

#[test]
fn close_brace_after_group_keeps_rest() {
    // The first `}` closes the frac denominator; the second is stray.
    assert_eq!(p("\\frac{a}{b}} + c"), "(seq (frac a b) } + c)");
}

#[test]
fn ampersand_inside_braces_is_literal() {
    assert_eq!(p("\\frac{a & b}{c}"), "(frac (seq a & b) c)");
}

#[test]
fn ampersand_inside_braces_within_matrix_cell_is_literal() {
    assert_eq!(
        p("\\begin{matrix} {a & b} & c \\end{matrix}"),
        "(matrix Plain [(seq a & b) c])"
    );
}

#[test]
fn unknown_environment_is_kept_verbatim() {
    assert_eq!(
        p("\\begin{foo} a & b \\\\ c \\end{foo} d"),
        "(seq (seq \\begin{foo} a & b \\\\ c \\end{foo}) d)"
    );
}

#[test]
fn unterminated_unknown_environment_keeps_content() {
    assert_eq!(p("\\begin{foo} a & b"), "(seq \\begin{foo} a & b)");
}

#[test]
fn mismatched_end_is_kept() {
    assert_eq!(
        p("\\begin{pmatrix} a \\end{bmatrix} b"),
        "(seq (matrix Paren [a]) \\end{bmatrix} b)"
    );
}

#[test]
fn environment_inside_group_ends_at_closing_brace() {
    // Half-typed: the user hasn't written \end{pmatrix} yet.
    assert_eq!(p("\\frac{\\begin{pmatrix} a}{b}"), "(frac (matrix Paren [a]) b)");
}

#[test]
fn begin_without_name() {
    assert_eq!(p("\\begin x"), "(seq \\begin x)");
}

#[test]
fn row_separator_inside_braces_in_environment() {
    let node = parse_equation("\\begin{matrix} \\frac{a \\\\ b}{c} & d \\end{matrix}");
    assert_eq!(common::sx(&node), "(matrix Plain [(frac (seq a \\\\ b) c) d])");
}

#[test]
fn left_right_inside_matrix_isolates_ampersand() {
    assert_eq!(
        p("\\begin{pmatrix} \\left( a & b \\right) & c \\end{pmatrix}"),
        "(matrix Paren [(delim \"(\" \")\" (seq a & b)) c])"
    );
}

#[test]
fn cases_with_extra_columns_keeps_them() {
    assert_eq!(
        p("\\begin{cases} a & b & c \\end{cases}"),
        "(cases [a (seq b & c)])"
    );
}

#[test]
fn half_typed_inputs_do_not_panic() {
    for input in [
        "\\", "\\\\", "{", "}", "&", "^", "_", "'", "%", "\\left", "\\right", "\\left\\",
        "\\begin{", "\\begin{pmatrix", "\\begin{pmatrix}", "\\end{", "\\sqrt[", "\\sqrt[3",
        "\\frac{", "\\frac{a}{", "\\color{", "\\color{red}", "\\textcolor{red}", "\\not",
        "\\operatorname", "\\operatorname{", "\\big", "\\substack{", "\\hspace{", "\\kern",
        "\\xrightarrow[", "\\begin{array}{", "\\begin{array}{c", "x^{", "a \\over", "\\choose",
        "\\begin{align} a &= \\\\", "\\begin{cases} 1 & \\text{if", "\\left\\{ x \\middle",
    ] {
        let _ = format!("{:?}", parse_equation(input));
    }
}

// ─── Randomized "nothing is dropped" property ───────────────────────

/// Small deterministic PRNG so the test is reproducible without dependencies.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as usize
    }
}

/// Fragments that exercise every recovery path. Content characters are
/// limited to `xyz123` so they can't accidentally combine into a bareword
/// like `pi` or a dimension like `1em`. Fragments starting with `\` are
/// commands; any of those letters inside them aren't counted as content.
const FRAGMENTS: &[&str] = &[
    "x", "y", "z", "1", "2", "3", "x", "y", "12", " ", "  ",
    "{", "}", "}", "&", "&", "\\\\", "(", ")", "[", "]", "^", "_", "'", "+", "-", "/", "=",
    "\\frac", "\\sqrt", "\\sqrt[", "\\left(", "\\right)", "\\left\\{", "\\right\\}", "\\left.",
    "\\middle|", "\\right", "\\left",
    "\\begin{pmatrix}", "\\end{pmatrix}", "\\begin{cases}", "\\end{cases}",
    "\\begin{align}", "\\end{align}", "\\begin{array}{c|c}", "\\end{array}",
    "\\begin{foo}", "\\end{foo}", "\\end{bar}", "\\begin", "\\end", "\\hline",
    "\\over", "\\choose", "\\color{red}", "\\textcolor{red}", "\\not", "\\bigl", "\\Bigr",
    "\\substack{", "\\operatorname{", "\\text{", "\\boxed", "\\hat", "\\mathbb",
    "\\sum", "\\int", "\\lim", "\\limits", "\\displaystyle", "\\binom", "\\overbrace",
    "\\underbrace", "\\xrightarrow", "\\alpha", "\\in", "\\,", "\\",
];

fn content_counts(s: &str) -> [usize; 6] {
    let mut counts = [0; 6];
    for c in s.chars() {
        if let Some(i) = "xyz123".find(c) {
            counts[i] += 1;
        }
    }
    counts
}

#[test]
fn random_inputs_never_drop_content() {
    let mut rng = Lcg(0x5eed);
    for case in 0..5000 {
        let len = 1 + rng.next() % 40;
        let fragments: Vec<&str> = (0..len).map(|_| FRAGMENTS[rng.next() % FRAGMENTS.len()]).collect();
        let input: String = fragments.concat();
        let tree = parse_equation(&input);
        // Only count content we inserted, not letters inside command or
        // environment names (e.g. the `x` in `pmatrix`).
        let content: String = fragments.iter().filter(|f| !f.starts_with('\\')).copied().collect();
        let expected = content_counts(&content);
        let actual = content_counts(&all_text(&tree));
        for i in 0..6 {
            assert!(
                actual[i] >= expected[i],
                "case {}: lost '{}' (input has {}, tree has {})\ninput: {:?}\ntree:  {:?}",
                case,
                &"xyz123"[i..i + 1],
                expected[i],
                actual[i],
                input,
                tree
            );
        }
    }
}

#[test]
fn every_top_level_char_survives_plain_input() {
    // With no commands at all, every non-space character should appear.
    let input = "x}y&z\\\\1}2&&3";
    let text = all_text(&parse_equation(input));
    for c in input.chars().filter(|c| !c.is_whitespace() && *c != '\\') {
        assert!(text.contains(c), "lost {:?} from {:?}", c, text);
    }
}

#[test]
fn deeply_nested_environments_terminate() {
    let input = "\\begin{pmatrix}".repeat(100) + "x" + &"\\end{pmatrix}".repeat(100);
    let node = parse_equation(&input);
    assert!(matches!(node, EqNode::Matrix { .. }));
}
