//! Input from people who don't know LaTeX.
//!
//! Rule: where a shortcut would conflict with a plausible LaTeX reading,
//! LaTeX wins. Otherwise both styles work.

mod common;

use common::p;
use rust_latex_parser::{parse_equation, EqNode, OpKind};

// ─── ASCII operator shortcuts ───────────────────────────────────────

#[test]
fn ascii_relations() {
    assert_eq!(p("x <= y"), "(seq x ≤ y)");
    assert_eq!(p("x >= y"), "(seq x ≥ y)");
    assert_eq!(p("x -> 0"), "(seq x → 0)");
    assert_eq!(p("p => q"), "(seq p ⇒ q)");
    assert_eq!(p("p <=> q"), "(seq p ⇔ q)");
    assert_eq!(p("a <-> b"), "(seq a ↔ b)");
    assert_eq!(p("+/- 3"), "(seq ± 3)");
}

#[test]
fn ascii_shortcuts_are_relation_ops() {
    assert!(matches!(
        parse_equation("a<=b"),
        EqNode::Seq(v) if v[1] == EqNode::Op { symbol: "≤".into(), kind: OpKind::Relation }
    ));
}

#[test]
fn latex_wins_ambiguous_shortcuts() {
    // `n!=1` is "n factorial equals 1" in LaTeX.
    assert_eq!(p("n!=1"), "(seq n ! = 1)");
    // `x<-1` is "x less than minus one".
    assert_eq!(p("x<-1"), "(seq x < - 1)");
    // `a+-b` is "a plus negative b".
    assert_eq!(p("a+-b"), "(seq a + - b)");
}

// ─── Parentheses as grouping ────────────────────────────────────────

#[test]
fn parenthesized_scripts() {
    assert_eq!(p("x^(n+1)"), "(sup x (seq n + 1))");
    assert_eq!(p("e^(i pi)"), "(sup e (seq i π))");
    assert_eq!(p("a_(i,j)"), "(sub a (seq i , j))");
    assert_eq!(p("x^((a))"), "(sup x (seq ( a )))");
}

#[test]
fn parenthesized_big_operator_limits() {
    assert_eq!(p("sum_(i=1)^n i"), "(seq (bigop ∑ (seq i = 1) n) i)");
    assert_eq!(p("int_(-infinity)^0"), "(bigop ∫ (seq - ∞) 0)");
    assert_eq!(p("lim_(x->0) f"), "(seq (limit lim (seq x → 0)) f)");
}

#[test]
fn latex_braces_still_work_in_scripts() {
    assert_eq!(p("x^{(n)}"), "(sup x (seq ( n )))");
}

// ─── Fractions with / ───────────────────────────────────────────────

#[test]
fn slash_with_parenthesized_operands() {
    assert_eq!(p("(a+b)/(c+d)"), "(frac (seq a + b) (seq c + d))");
    assert_eq!(p("1/(x+1)"), "(frac 1 (seq x + 1))");
}

#[test]
fn slash_takes_adjacent_runs() {
    assert_eq!(p("dy/dx"), "(frac (seq d y) (seq d x))");
    assert_eq!(p("2x^2/3y"), "(frac (seq 2 (sup x 2)) (seq 3 y))");
    assert_eq!(p("n!/k!"), "(frac (seq n !) (seq k !))");
    assert_eq!(p("x/2\\pi"), "(frac x (seq 2 π))");
}

#[test]
fn whitespace_ends_a_slash_operand() {
    assert_eq!(p("1/2x"), "(frac 1 (seq 2 x))");
    assert_eq!(p("1/2 x"), "(seq (frac 1 2) x)");
    assert_eq!(p("a b/c"), "(seq a (frac b c))");
}

#[test]
fn slash_stops_at_operators() {
    assert_eq!(p("a + b/c"), "(seq a + (frac b c))");
    assert_eq!(p("a = b/c - d"), "(seq a = (frac b c) - d)");
    assert_eq!(p("-x/2"), "(seq - (frac x 2))");
}

#[test]
fn function_application_stays_in_numerator() {
    assert_eq!(p("sin(x)/x"), "(frac (seq (limit sin _) ( x )) x)");
    assert_eq!(p("2(a+b)/c"), "(frac (seq 2 ( a + b )) c)");
    assert_eq!(p("(a+b)(c+d)/e"), "(frac (seq ( a + b ) ( c + d )) e)");
}

#[test]
fn chained_and_nested_slashes() {
    assert_eq!(p("a/b/c"), "(frac (frac a b) c)");
    assert_eq!(p("sqrt((a+b)/2)"), "(sqrt (frac (seq a + b) 2))");
    assert_eq!(p("\\frac{a}{b}/c"), "(frac (frac a b) c)");
}

// ─── Function-style shortcuts ───────────────────────────────────────

#[test]
fn delimiter_functions() {
    assert_eq!(p("abs(x)"), "(delim \"|\" \"|\" x)");
    assert_eq!(p("norm(v)"), "(delim \"‖\" \"‖\" v)");
    assert_eq!(p("floor(x/2)"), "(delim \"⌊\" \"⌋\" (frac x 2))");
    assert_eq!(p("ceil(n)"), "(delim \"⌈\" \"⌉\" n)");
}

#[test]
fn root_and_binom_functions() {
    assert_eq!(p("root(3, x)"), "(root 3 x)");
    assert_eq!(p("root(x)"), "(sqrt x)");
    assert_eq!(p("cbrt(27)"), "(root 3 27)");
    assert_eq!(p("binom(n, k)"), "(binom n k)");
}

#[test]
fn accent_functions() {
    assert_eq!(p("hat(x) + vec(v)"), "(seq (accent Hat x) + (accent Vec v))");
    assert_eq!(p("bar(z)"), "(accent Bar z)");
    assert_eq!(p("ul(x)"), "(accent Underline x)");
}

#[test]
fn text_function_and_quotes() {
    assert_eq!(p("text(for all) x"), "(seq (text \"for all\") x)");
    assert_eq!(p("\"if\" x > 0"), "(seq (text \"if\") x > 0)");
    // Half-typed: an unclosed quote runs to the end.
    assert_eq!(p("x \"otherw"), "(seq x (text \"otherw\"))");
}

#[test]
fn function_words_need_an_immediate_paren() {
    assert_eq!(p("abs"), "(seq a b s)");
    assert_eq!(p("abs (x)"), "(seq a b s ( x ))");
}

// ─── Words ──────────────────────────────────────────────────────────

#[test]
fn more_barewords() {
    assert_eq!(p("infinity"), "∞");
    assert_eq!(p("a neq b"), "(seq a ≠ b)");
    assert_eq!(p("x leq y"), "(seq x ≤ y)");
    assert_eq!(p("A subseteq B"), "(seq A ⊆ B)");
    assert_eq!(p("Xi Upsilon varphi"), "(seq Ξ Υ φ)");
}

#[test]
fn barewords_respect_word_boundaries() {
    assert_eq!(p("pin"), "(seq p i n)");
    assert_eq!(p("beta"), "β");
}

// ─── Unicode input ──────────────────────────────────────────────────

#[test]
fn unicode_superscripts_and_subscripts() {
    assert_eq!(p("x²"), "(sup x 2)");
    assert_eq!(p("x²³"), "(sup x 23)");
    assert_eq!(p("eⁱⁿ"), "(sup e (seq i n))");
    assert_eq!(p("a₁₂"), "(sub a 12)");
    assert_eq!(p("xₙ²"), "(supsub x 2 n)");
    assert_eq!(p("x⁻¹"), "(sup x (seq - 1))");
}

#[test]
fn unicode_roots_and_fractions() {
    assert_eq!(p("√x"), "(sqrt x)");
    assert_eq!(p("√(x+1)"), "(sqrt (seq x + 1))");
    assert_eq!(p("∛8"), "(root 3 8)");
    assert_eq!(p("∜x"), "(root 4 x)");
    assert_eq!(p("½"), "(frac 1 2)");
    assert_eq!(p("¾x"), "(seq (frac 3 4) x)");
}

#[test]
fn unicode_operators_and_big_operators() {
    assert_eq!(p("x ≤ y"), "(seq x ≤ y)");
    assert!(matches!(
        parse_equation("a × b"),
        EqNode::Seq(v) if v[1] == EqNode::Op { symbol: "×".into(), kind: OpKind::Binary }
    ));
    assert_eq!(p("∑_(k=0)^n k"), "(seq (bigop ∑ (seq k = 0) n) k)");
    assert_eq!(p("∫_0^1"), "(bigop ∫ 0 1)");
}

#[test]
fn unicode_primes() {
    assert_eq!(p("f′(x)"), "(seq (sup f ′) ( x ))");
    assert_eq!(p("f″"), "(sup f ′′)");
}

// ─── Mixed input ────────────────────────────────────────────────────

#[test]
fn mixed_latex_and_natural_input() {
    assert_eq!(
        p("lim_(x->0) sin(x)/x = 1"),
        "(seq (limit lim (seq x → 0)) (frac (seq (limit sin _) ( x )) x) = 1)"
    );
    assert_eq!(
        p("\\int_0^1 x² dx = 1/3"),
        "(seq (bigop ∫ 0 1) (sup x 2) d x = (frac 1 3))"
    );
    assert_eq!(
        p("E = mc²"),
        "(seq E = m (sup c 2))"
    );
    assert_eq!(
        p("x = (-b +/- sqrt(b² - 4ac))/(2a)"),
        "(seq x = (frac (seq - b ± (sqrt (seq (sup b 2) - 4 a c))) (seq 2 a)))"
    );
}
