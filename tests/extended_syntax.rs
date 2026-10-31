//! Commonly used LaTeX that the parser previously didn't understand.

mod common;

use common::p;
use rust_latex_parser::{latex_to_unicode, parse_equation, EqNode};

// ─── Fractions ─────────────────────────────────────────────────────

#[test]
fn frac_style_variants() {
    for cmd in ["dfrac", "tfrac", "cfrac"] {
        assert_eq!(p(&format!("\\{}{{a}}{{b}}", cmd)), "(frac a b)", "{}", cmd);
    }
    assert_eq!(p("\\cfrac[l]{1}{2}"), "(frac 1 2)");
}

#[test]
fn binom_style_variants() {
    assert_eq!(p("\\dbinom{n}{k}"), "(binom n k)");
    assert_eq!(p("\\tbinom{n}{k}"), "(binom n k)");
}

#[test]
fn infix_over_and_choose() {
    assert_eq!(p("{a \\over b}"), "(frac a b)");
    assert_eq!(p("{n \\choose k}"), "(binom n k)");
    assert_eq!(p("1 + {a+b \\over c}"), "(seq 1 + (frac (seq a + b) c))");
}

#[test]
fn over_does_not_match_overline() {
    assert_eq!(p("\\overline{x}"), "(accent Bar x)");
}

// ─── Operators ─────────────────────────────────────────────────────

#[test]
fn operatorname() {
    assert_eq!(p("\\operatorname{rank}(A)"), "(seq (limit rank _) ( A ))");
}

#[test]
fn operatorname_star_with_limit() {
    assert_eq!(p("\\operatorname*{arg\\,max}_{x} f"), "(seq (limit arg max x) f)");
}

#[test]
fn limits_and_nolimits_are_ignored() {
    assert_eq!(p("\\sum\\limits_{i=1}^{n} i"), "(seq (bigop ∑ (seq i = 1) n) i)");
    assert_eq!(p("\\int\\nolimits_0^1"), "(bigop ∫ 0 1)");
    assert_eq!(p("\\lim\\limits_{x \\to 0}"), "(limit lim (seq x → 0))");
}

#[test]
fn big_set_operators() {
    assert_eq!(p("\\bigcup_{i} A_i"), "(seq (bigop ⋃ i _) (sub A i))");
    assert_eq!(p("\\bigcap_{i=1}^n"), "(bigop ⋂ (seq i = 1) n)");
    assert_eq!(p("\\bigoplus"), "(bigop ⨁ _ _)");
    assert_eq!(p("\\bigwedge"), "(bigop ⋀ _ _)");
    assert_eq!(p("\\oiint"), "(bigop ∯ _ _)");
}

#[test]
fn more_limit_operators() {
    assert_eq!(p("\\Pr(A)"), "(seq (limit Pr _) ( A ))");
    assert_eq!(p("\\coth x"), "(seq (limit coth _) x)");
}

#[test]
fn modular_arithmetic() {
    assert_eq!(p("a \\equiv b \\pmod{n}"), "(seq a ≡ b (seq ( (limit mod _) n )))");
    assert_eq!(p("a \\bmod b"), "(seq a (limit mod _) b)");
    assert_eq!(p("a \\mod{n}"), "(seq a (seq (limit mod _) n))");
}

// ─── Environments ──────────────────────────────────────────────────

#[test]
fn smallmatrix_and_dcases() {
    assert_eq!(p("\\begin{smallmatrix} a & b \\end{smallmatrix}"), "(matrix Plain [a b])");
    assert_eq!(
        p("\\begin{dcases} x & x > 0 \\\\ 0 & \\text{otherwise} \\end{dcases}"),
        "(cases [x (seq x > 0)] [0 (text \"otherwise\")])"
    );
}

#[test]
fn trailing_row_separator_in_matrix() {
    assert_eq!(p("\\begin{pmatrix} a \\\\ b \\\\ \\end{pmatrix}"), "(matrix Paren [a] [b])");
}

#[test]
fn matrix_row_spacing_is_skipped() {
    assert_eq!(
        p("\\begin{pmatrix} a \\\\[4pt] b \\\\\n c \\end{pmatrix}"),
        "(matrix Paren [a] [b] [c])"
    );
}

#[test]
fn row_separator_before_end_on_new_line() {
    // A trailing `\\\\` followed by a newline used to create an extra row.
    assert_eq!(p("\\begin{bmatrix} a \\\\\n\\end{bmatrix}"), "(matrix Bracket [a])");
}

#[test]
fn row_spacing_bracket_that_is_not_a_dimension_is_kept() {
    assert_eq!(
        p("\\begin{matrix} a \\\\[x] \\end{matrix}"),
        "(matrix Plain [a] [(seq [ x ])])"
    );
}

// ─── Delimiters ────────────────────────────────────────────────────

#[test]
fn left_right_with_escaped_braces() {
    assert_eq!(p("\\left\\{ x \\right\\}"), "(delim \"{\" \"}\" x)");
}

#[test]
fn left_right_with_named_delimiters() {
    assert_eq!(p("\\left\\langle v \\right\\rangle"), "(delim \"⟨\" \"⟩\" v)");
    assert_eq!(p("\\left\\lfloor x \\right\\rfloor"), "(delim \"⌊\" \"⌋\" x)");
    assert_eq!(p("\\left\\lvert x \\right\\rvert"), "(delim \"|\" \"|\" x)");
    assert_eq!(p("\\left\\| x \\right\\|"), "(delim \"‖\" \"‖\" x)");
    assert_eq!(p("\\left\\Vert x \\right\\Vert"), "(delim \"‖\" \"‖\" x)");
    assert_eq!(p("\\left< x \\right>"), "(delim \"⟨\" \"⟩\" x)");
}

#[test]
fn left_does_not_stop_on_rightarrow() {
    assert_eq!(p("\\left( a \\rightarrow b \\right)"), "(delim \"(\" \")\" (seq a → b))");
}

#[test]
fn middle_delimiter_in_set_builder() {
    assert_eq!(
        p("\\left\\{ x \\middle| x > 0 \\right\\}"),
        "(delim \"{\" \"}\" (seq x | x > 0))"
    );
}

// ─── Accents, fonts, arrows ────────────────────────────────────────

#[test]
fn wide_accents_map_to_base_kinds() {
    assert_eq!(p("\\widehat{xy}"), "(accent Hat (seq x y))");
    assert_eq!(p("\\widetilde{xy}"), "(accent Tilde (seq x y))");
    assert_eq!(p("\\overrightarrow{AB}"), "(accent Vec (seq A B))");
}

#[test]
fn styled_text_commands() {
    assert_eq!(p("\\textbf{bold}"), "(font Bold (text \"bold\"))");
    assert_eq!(p("\\texttt{code}"), "(font Monospace (text \"code\"))");
    assert_eq!(p("\\mbox{if } x"), "(seq (text \"if \") x)");
    assert_eq!(p("\\textrm{a {b} c}"), "(text \"a {b} c\")");
}

#[test]
fn extensible_arrows() {
    assert_eq!(p("A \\xrightarrow{f} B"), "(seq A (stack true → f) B)");
    assert_eq!(
        p("\\xleftarrow[below]{above}"),
        "(stack true (stack false ← (seq b e l o w)) (seq a b o v e))"
    );
}

// ─── Negation and ignored commands ─────────────────────────────────

#[test]
fn not_negates_relations() {
    assert_eq!(p("a \\not\\in B"), "(seq a ∉ B)");
    assert_eq!(p("a \\not= b"), "(seq a ≠ b)");
    assert_eq!(p("\\not\\subset"), "⊄");
    assert_eq!(p("\\not\\equiv"), "≢");
    assert_eq!(p("\\not<"), "≮");
    // No precomposed form: falls back to a combining long solidus.
    assert_eq!(p("\\not\\perp"), "⊥\u{0338}");
}

#[test]
fn not_at_end_of_input() {
    assert_eq!(p("a \\not"), "(seq a \\not)");
}

#[test]
fn style_commands_are_ignored() {
    assert_eq!(p("\\displaystyle \\frac{a}{b}"), "(frac a b)");
    assert_eq!(p("\\textstyle x"), "x");
}

// ─── Spacing ───────────────────────────────────────────────────────

#[test]
fn hspace_and_kern() {
    fn width(src: &str) -> f32 {
        match parse_equation(src) {
            EqNode::Space(w) => w,
            other => panic!("{}: expected Space, got {:?}", src, other),
        }
    }
    assert_eq!(width("\\hspace{1em}"), 18.0);
    assert_eq!(width("\\hspace*{2em}"), 36.0);
    assert_eq!(width("\\mkern18mu"), 18.0);
    assert_eq!(width("\\kern-3mu"), -3.0);
    assert_eq!(width("\\enspace"), 9.0);
}

#[test]
fn invalid_hspace_is_kept_as_text() {
    assert_eq!(p("\\hspace{wide}"), "\\hspace{wide}");
}

#[test]
fn spaces_before_scripts_are_ignored() {
    assert_eq!(p("x ^ 2"), "(sup x 2)");
    assert_eq!(p("\\sum _{i} ^{n}"), "(bigop ∑ i n)");
}

#[test]
fn arguments_after_whitespace() {
    // These used to take the space itself as the argument.
    assert_eq!(p("\\frac{a} {b}"), "(frac a b)");
    assert_eq!(p("sqrt x"), "(sqrt x)");
    assert_eq!(p("a/ b"), "(frac a b)");
    assert_eq!(p("x^ 2"), "(sup x 2)");
}

// ─── Symbols ───────────────────────────────────────────────────────

#[test]
fn new_symbols() {
    let cases = [
        ("mid", "∣"), ("nmid", "∤"), ("leqslant", "⩽"), ("geqslant", "⩾"),
        ("subsetneq", "⊊"), ("nsubseteq", "⊈"), ("longrightarrow", "⟶"),
        ("Longleftrightarrow", "⟺"), ("longmapsto", "⟼"), ("rightleftharpoons", "⇌"),
        ("dagger", "†"), ("odot", "⊙"), ("vdash", "⊢"), ("models", "⊨"),
        ("therefore", "∴"), ("because", "∵"), ("top", "⊤"), ("bot", "⊥"),
        ("square", "□"), ("checkmark", "✓"), ("lbrace", "{"), ("Vert", "‖"),
        ("cdots", "⋯"), ("iddots", "⋰"), ("colon", ":"),
    ];
    for (name, expected) in cases {
        assert_eq!(latex_to_unicode(name).as_deref(), Some(expected), "{}", name);
    }
}

#[test]
fn new_relations_get_operator_spacing() {
    for src in ["a \\mid b", "a \\leqslant b", "A \\subsetneq B", "x \\longrightarrow y", "p \\models q"] {
        match parse_equation(src) {
            EqNode::Seq(nodes) => assert!(
                nodes.iter().any(|n| matches!(n, EqNode::Seq(inner) if matches!(inner.first(), Some(EqNode::Space(_))))),
                "{} has no spaced operator: {:?}",
                src,
                nodes
            ),
            other => panic!("{}: {:?}", src, other),
        }
    }
}

// ─── Compatibility ─────────────────────────────────────────────────

/// 0.1.1 is a patch release: output for input that already parsed correctly
/// must not change. (0.2 deliberately changes these.)
#[test]
fn existing_output_is_unchanged() {
    assert_eq!(latex_to_unicode("ldots").as_deref(), Some("⋯"));
    assert_eq!(latex_to_unicode("varsigma").as_deref(), Some("σ"));
    assert_eq!(p("x'"), "(seq x ')");
    assert_eq!(p("50%"), "(seq 50 %)");
    assert!(matches!(parse_equation("\\notin"), EqNode::Text(ref s) if s == "∉"));
    assert!(matches!(parse_equation("\\|"), EqNode::Text(ref s) if s == "|"));
    assert!(matches!(parse_equation("\\sqrt[3]{x}"), EqNode::Seq(_)));
    // One space node per space character.
    match parse_equation("a  b") {
        EqNode::Seq(v) => assert_eq!(v.len(), 4),
        other => panic!("{:?}", other),
    }
}

// ─── Real-world expressions ────────────────────────────────────────

#[test]
fn real_world_expressions() {
    let cases = [
        // Matrix rank-nullity
        ("\\operatorname{rank}(A) + \\dim\\ker A = n",
         "(seq (limit rank _) ( A ) + (limit dim _) (limit ker _) A = n)"),
        // Set-builder
        ("\\{ x \\in \\mathbb{R} \\mid x > 0 \\}",
         "(seq { x ∈ (font Blackboard R) ∣ x > 0 })"),
        // Derivative
        ("\\lim_{h \\to 0} \\dfrac{f(x+h) - f(x)}{h}",
         "(seq (limit lim (seq h → 0)) (frac (seq f ( x + h ) - f ( x )) h))"),
    ];
    for (src, expected) in cases {
        assert_eq!(p(src), expected, "{}", src);
    }
}
