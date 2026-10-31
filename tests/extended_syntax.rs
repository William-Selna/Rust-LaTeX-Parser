//! Commonly used LaTeX that the parser previously didn't understand.

mod common;

use common::p;
use rust_latex_parser::{
    latex_to_unicode, parse_equation, AccentKind, ArrayColumn, EncloseKind, EqNode, MathFontKind,
    OpKind,
};

// ─── Roots and fractions ────────────────────────────────────────────

// ─── Roots and fractions ───────────────────────────────────────────

#[test]
fn nth_root() {
    assert_eq!(p("\\sqrt[3]{x}"), "(root 3 x)");
    assert_eq!(p("\\sqrt[n]{x+1}"), "(root n (seq x + 1))");
}

#[test]
fn nth_root_bareword() {
    assert_eq!(p("sqrt[3](x+1)"), "(root 3 (seq x + 1))");
}

#[test]
fn nested_root_index() {
    assert_eq!(p("\\sqrt[\\sqrt[2]{3}]{x}"), "(root (root 2 3) x)");
}

#[test]
fn empty_root_index_is_plain_sqrt() {
    assert_eq!(p("\\sqrt[]{x}"), "(sqrt x)");
}

#[test]
fn root_node_fields() {
    match parse_equation("\\sqrt[3]{8}").kind {
        EqNode::Root { index, content } => {
            assert_eq!(index.kind, EqNode::Text("3".into()));
            assert_eq!(content.kind, EqNode::Text("8".into()));
        }
        other => panic!("expected Root, got {:?}", other),
    }
}

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
fn substack_under_sum() {
    assert_eq!(
        p("\\sum_{\\substack{i<n \\\\ j<m}} a"),
        "(seq (bigop ∑ (array [Center] [] [(seq i < n)] [(seq j < m)]) _) a)"
    );
}

#[test]
fn modular_arithmetic() {
    assert_eq!(p("a \\equiv b \\pmod{n}"), "(seq a ≡ b (seq ( (limit mod _) n )))");
    assert_eq!(p("a \\bmod b"), "(seq a mod b)");
    assert!(matches!(
        parse_equation("a \\bmod b").kind,
        EqNode::Seq(v) if matches!(&v[1].kind, EqNode::Op { symbol, kind: OpKind::Binary } if symbol == "mod")
    ));
    assert_eq!(p("a \\mod{n}"), "(seq a (seq (limit mod _) n))");
}

// ─── Environments ──────────────────────────────────────────────────

#[test]
fn align_environment() {
    assert_eq!(
        p("\\begin{align} a &= b \\\\ c &= d \\end{align}"),
        "(align [a (seq = b)] [c (seq = d)])"
    );
}

#[test]
fn align_variants() {
    for env in ["align*", "aligned", "split", "flalign", "eqnarray*"] {
        let src = format!("\\begin{{{0}}} x &= 1 \\end{{{0}}}", env);
        assert_eq!(p(&src), "(align [x (seq = 1)])", "{}", env);
    }
    assert_eq!(p("\\begin{alignat}{2} x &= 1 \\end{alignat}"), "(align [x (seq = 1)])");
}

#[test]
fn align_with_newlines_comments_and_spacing() {
    let src = "\\begin{aligned}\n  f(x) &= x^2 \\\\[4pt] % first line\n  &= x \\cdot x \\nonumber\n\\end{aligned}";
    assert_eq!(
        p(src),
        "(align [(seq f ( x )) (seq = (sup x 2))] [ (seq = x · x)])"
    );
}

#[test]
fn aligned_inside_delimiters() {
    assert_eq!(
        p("\\left\\{ \\begin{aligned} x &= 1 \\\\ y &= 2 \\end{aligned} \\right."),
        "(delim \"{\" \"\" (align [x (seq = 1)] [y (seq = 2)]))"
    );
}

#[test]
fn gather_environment() {
    assert_eq!(p("\\begin{gather} a = b \\\\ c \\end{gather}"), "(gather (seq a = b) c)");
    assert_eq!(p("\\begin{gathered} a \\end{gathered}"), "(gather a)");
}

#[test]
fn equation_environment_is_transparent() {
    assert_eq!(p("\\begin{equation} E = mc^2 \\label{eq:1} \\end{equation}"), "(seq E = m (sup c 2))");
    assert_eq!(p("\\begin{equation*} a \\\\ b \\end{equation*}"), "(gather a b)");
}

#[test]
fn array_with_column_spec_and_hlines() {
    assert_eq!(
        p("\\begin{array}{c|c} \\hline a & b \\\\ \\hline c & d \\\\ \\hline \\end{array}"),
        "(array [Center, VLine, Center] [0, 1, 2] [a b] [c d])"
    );
}

#[test]
fn array_column_spec_forms() {
    match parse_equation("\\begin{array}{*{3}{c}|l@{\\hspace{1em}}rp{2cm}} \\end{array}").kind {
        EqNode::Array { columns, .. } => assert_eq!(
            columns,
            vec![
                ArrayColumn::Center,
                ArrayColumn::Center,
                ArrayColumn::Center,
                ArrayColumn::VLine,
                ArrayColumn::Left,
                ArrayColumn::Right,
                ArrayColumn::Left,
            ]
        ),
        other => panic!("expected Array, got {:?}", other),
    }
}

#[test]
fn augmented_matrix() {
    assert_eq!(
        p("\\left[ \\begin{array}{cc|c} 1 & 2 & 3 \\\\ 4 & 5 & 6 \\end{array} \\right]"),
        "(delim \"[\" \"]\" (array [Center, Center, VLine, Center] [] [1 2 3] [4 5 6]))"
    );
}

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
        "(delim \"{\" \"}\" (seq x (middle \"|\") x > 0))"
    );
}

#[test]
fn sized_delimiters() {
    assert_eq!(p("\\bigl( x \\bigr)"), "(seq (sized 1 \"(\") x (sized 1 \")\"))");
    assert_eq!(p("\\Big["), "(sized 2 \"[\")");
    assert_eq!(p("\\bigg\\{"), "(sized 3 \"{\")");
    assert_eq!(p("\\Biggm\\vert"), "(sized 4 \"|\")");
}

#[test]
fn sized_delimiter_does_not_eat_big_operators() {
    assert_eq!(p("\\bigcup"), "(bigop ⋃ _ _)");
}

// ─── Accents, fonts, decorations ───────────────────────────────────

#[test]
fn wide_accents_map_to_base_kinds() {
    assert_eq!(p("\\widehat{xy}"), "(accent Hat (seq x y))");
    assert_eq!(p("\\widetilde{xy}"), "(accent Tilde (seq x y))");
    assert_eq!(p("\\overrightarrow{AB}"), "(accent Vec (seq A B))");
}

#[test]
fn new_accent_kinds() {
    let cases = [
        ("overleftarrow", AccentKind::LeftArrow),
        ("overleftrightarrow", AccentKind::LeftRightArrow),
        ("underline", AccentKind::Underline),
        ("acute", AccentKind::Acute),
        ("grave", AccentKind::Grave),
        ("breve", AccentKind::Breve),
        ("check", AccentKind::Check),
        ("mathring", AccentKind::Ring),
        ("dddot", AccentKind::TripleDot),
    ];
    for (cmd, kind) in cases {
        match parse_equation(&format!("\\{}{{x}}", cmd)).kind {
            EqNode::Accent(_, k) => assert_eq!(k, kind, "{}", cmd),
            other => panic!("\\{}: expected Accent, got {:?}", cmd, other),
        }
    }
}

#[test]
fn new_math_fonts() {
    let cases = [
        ("mathit", MathFontKind::Italic),
        ("boldsymbol", MathFontKind::BoldItalic),
        ("bm", MathFontKind::BoldItalic),
        ("mathscr", MathFontKind::Calligraphic),
    ];
    for (cmd, kind) in cases {
        match parse_equation(&format!("\\{}{{x}}", cmd)).kind {
            EqNode::MathFont { kind: k, .. } => assert_eq!(k, kind, "{}", cmd),
            other => panic!("\\{}: expected MathFont, got {:?}", cmd, other),
        }
    }
    assert_eq!(p("\\boldsymbol{\\alpha}"), "(font BoldItalic α)");
}

#[test]
fn styled_text_commands() {
    assert_eq!(p("\\textbf{bold}"), "(font Bold (text \"bold\"))");
    assert_eq!(p("\\textit{it}"), "(font Italic (text \"it\"))");
    assert_eq!(p("\\texttt{code}"), "(font Monospace (text \"code\"))");
    assert_eq!(p("\\mbox{if } x"), "(seq (text \"if \") x)");
    assert_eq!(p("\\textrm{a {b} c}"), "(text \"a {b} c\")");
}

#[test]
fn boxed_and_cancel() {
    assert_eq!(p("\\boxed{E = mc^2}"), "(enclose Box (seq E = m (sup c 2)))");
    assert_eq!(p("\\fbox{note}"), "(enclose Box (text \"note\"))");
    assert_eq!(p("\\cancel{x}"), "(enclose Cancel x)");
    assert_eq!(p("\\bcancel{x}"), "(enclose BackCancel x)");
    assert_eq!(p("\\xcancel{x}"), "(enclose XCancel x)");
    assert!(matches!(
        parse_equation("\\cancel{x}").kind,
        EqNode::Enclose { kind: EncloseKind::Cancel, .. }
    ));
}

#[test]
fn textcolor_applies_to_argument_only() {
    assert_eq!(p("\\textcolor{red}{x} + y"), "(seq (color red x) + y)");
}

#[test]
fn color_switch_applies_to_rest_of_group() {
    assert_eq!(p("a + \\color{blue} b + c"), "(seq a + (color blue (seq b + c)))");
    assert_eq!(p("{\\color{red} x} + y"), "(seq (color red x) + y)");
    assert_eq!(p("\\color{#ff0000}{x}"), "(color #ff0000 x)");
}

#[test]
fn extensible_arrows() {
    assert_eq!(p("A \\xrightarrow{f} B"), "(seq A (stack true → f) B)");
    assert_eq!(
        p("\\xleftarrow[below]{above}"),
        "(stack true (stack false ← (seq b e l o w)) (seq a b o v e))"
    );
}

// ─── Primes, negation, comments ────────────────────────────────────

#[test]
fn primes() {
    assert_eq!(p("x'"), "(sup x ′)");
    assert_eq!(p("f''(x)"), "(seq (sup f ′′) ( x ))");
    assert_eq!(p("y'''"), "(sup y ′′′)");
}

#[test]
fn primes_combine_with_scripts() {
    assert_eq!(p("x'^2"), "(sup x (seq ′ 2))");
    assert_eq!(p("x'_1"), "(supsub x ′ 1)");
    assert_eq!(p("f'^{(n)}"), "(sup f (seq ′ ( n )))");
}

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
fn comments_are_skipped() {
    assert_eq!(p("a + b % the rest is ignored"), "(seq a + b)");
    assert_eq!(p("a % comment\n+ b"), "(seq a + b)");
    assert_eq!(p("50\\%"), "(seq 50 %)");
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
        match parse_equation(src).kind {
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
    // The command is kept as text and its argument parsed normally.
    assert_eq!(p("\\hspace{wide}"), "(seq \\hspace (seq w i d e))");
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

#[test]
fn whitespace_produces_no_nodes() {
    assert_eq!(
        parse_equation("a \n\t b").kind,
        EqNode::Seq(vec![EqNode::Text("a".into()).into(), EqNode::Text("b".into()).into()])
    );
}

#[test]
fn new_symbols() {
    let cases = [
        ("mid", "∣"), ("nmid", "∤"), ("leqslant", "⩽"), ("geqslant", "⩾"),
        ("subsetneq", "⊊"), ("nsubseteq", "⊈"), ("longrightarrow", "⟶"),
        ("Longleftrightarrow", "⟺"), ("longmapsto", "⟼"), ("rightleftharpoons", "⇌"),
        ("dagger", "†"), ("odot", "⊙"), ("vdash", "⊢"), ("models", "⊨"),
        ("therefore", "∴"), ("because", "∵"), ("top", "⊤"), ("bot", "⊥"),
        ("square", "□"), ("checkmark", "✓"), ("lbrace", "{"), ("Vert", "‖"),
        ("varsigma", "ς"), ("vartheta", "ϑ"), ("varpi", "ϖ"), ("varrho", "ϱ"),
        ("ldots", "…"), ("cdots", "⋯"), ("iddots", "⋰"), ("colon", ":"),
    ];
    for (name, expected) in cases {
        assert_eq!(latex_to_unicode(name).as_deref(), Some(expected), "{}", name);
    }
}

#[test]
fn new_relations_are_relation_ops() {
    for (src, sym) in [("a \\mid b", "∣"), ("a \\leqslant b", "⩽"), ("A \\subsetneq B", "⊊"), ("x \\longrightarrow y", "⟶"), ("p \\models q", "⊨")] {
        match parse_equation(src).kind {
            EqNode::Seq(nodes) => assert_eq!(
                nodes[1].kind,
                EqNode::Op { symbol: sym.to_string(), kind: OpKind::Relation },
                "{}",
                src
            ),
            other => panic!("{}: {:?}", src, other),
        }
    }
}

#[test]
fn escaped_double_bar() {
    assert_eq!(p("\\|x\\|"), "(seq ‖ x ‖)");
}

// ─── Real-world expressions ────────────────────────────────────────

#[test]
fn real_world_expressions() {
    let cases = [
        // Matrix rank-nullity
        ("\\operatorname{rank}(A) + \\dim\\ker A = n",
         "(seq (limit rank _) ( A ) + (limit dim _) (limit ker _) A = n)"),
        // Norm with sized delimiters
        ("\\Bigl\\| x \\Bigr\\|_2", "(seq (sized 2 \"‖\") x (sub (sized 2 \"‖\") 2))"),
        // Cube root
        ("\\sqrt[3]{27} = 3", "(seq (root 3 27) = 3)"),
        // Set-builder
        ("\\{ x \\in \\mathbb{R} \\mid x > 0 \\}",
         "(seq { x ∈ (font Blackboard R) ∣ x > 0 })"),
        // Derivative
        ("f'(x) = \\lim_{h \\to 0} \\dfrac{f(x+h) - f(x)}{h}",
         "(seq (sup f ′) ( x ) = (limit lim (seq h → 0)) (frac (seq f ( x + h ) - f ( x )) h))"),
    ];
    for (src, expected) in cases {
        assert_eq!(p(src), expected, "{}", src);
    }
}
