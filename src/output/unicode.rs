//! Serialize a tree to plain Unicode text.

use super::style::styled_str;
use crate::ast::*;

/// Convert a tree to readable plain text, like `x² + √(y + 1)`.
///
/// Useful for alt text, plain-text clipboard contents, search indexing, and
/// anywhere rich rendering isn't available. Superscripts and subscripts use
/// Unicode script characters when every character has one, and fall back
/// to `^(...)` / `_(...)` otherwise. Math fonts use the Unicode
/// mathematical alphanumerics (`\mathbb{R}` → `ℝ`).
///
/// The output is meant for people. The parser accepts the same notation,
/// so it usually parses back to an equivalent tree, except where separate
/// letters happen to spell a shortcut word (`p i` comes out as `pi`).
///
/// ```
/// use rust_latex_parser::{parse_equation, to_unicode};
///
/// let tree = parse_equation(r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}");
/// assert_eq!(to_unicode(&tree), "(-b ± √(b² - 4ac))/2a");
/// ```
pub fn to_unicode(node: &Node) -> String {
    let mut w = Writer::default();
    w.node(node);
    collapse_spaces(&w.out)
}

#[derive(Default)]
struct Writer {
    out: String,
    font: Option<MathFontKind>,
}

/// Collapse runs of spaces and trim each line.
fn collapse_spaces(s: &str) -> String {
    s.split('\n')
        .map(|line| line.split(' ').filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A sub-tree as text, using the same font context.
fn render(node: &Node, font: Option<MathFontKind>) -> String {
    let mut w = Writer { out: String::new(), font };
    w.node(node);
    collapse_spaces(&w.out)
}

/// Whether a rendered fragment reads as one unit without parentheses.
/// Matches how the parser reads `/`: a run with no spaces is one operand
/// (`2a`, `sin(x)`), and so is anything that brings its own brackets.
fn is_atomic(node: &Node, text: &str) -> bool {
    match &node.kind {
        EqNode::Sqrt(_) | EqNode::Root { .. } | EqNode::Delimited { .. } => true,
        EqNode::Frac(..) | EqNode::Op { .. } => false,
        _ => !text.contains(' ') && !text.contains('/'),
    }
}

/// `(text)` unless it is already a single unit.
fn paren_if_needed(node: &Node, text: String) -> String {
    if is_atomic(node, &text) {
        text
    } else {
        format!("({})", text)
    }
}

fn superscript_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴', '5' => '⁵', '6' => '⁶',
        '7' => '⁷', '8' => '⁸', '9' => '⁹', '+' => '⁺', '-' => '⁻', '=' => '⁼', '(' => '⁽',
        ')' => '⁾', 'n' => 'ⁿ', 'i' => 'ⁱ',
        _ => return None,
    })
}

fn subscript_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀', '1' => '₁', '2' => '₂', '3' => '₃', '4' => '₄', '5' => '₅', '6' => '₆',
        '7' => '₇', '8' => '₈', '9' => '₉', '+' => '₊', '-' => '₋', '=' => '₌', '(' => '₍',
        ')' => '₎', 'a' => 'ₐ', 'e' => 'ₑ', 'o' => 'ₒ', 'x' => 'ₓ', 'h' => 'ₕ', 'k' => 'ₖ',
        'l' => 'ₗ', 'm' => 'ₘ', 'n' => 'ₙ', 'p' => 'ₚ', 's' => 'ₛ', 't' => 'ₜ', 'i' => 'ᵢ',
        'j' => 'ⱼ', 'r' => 'ᵣ', 'u' => 'ᵤ', 'v' => 'ᵥ',
        _ => return None,
    })
}

/// A script as Unicode script characters if possible, else `^(...)`.
fn script(node: &Node, font: Option<MathFontKind>, sup: bool) -> String {
    let text = render(node, font);
    let compact: String = text.chars().filter(|c| *c != ' ').collect();
    let map = if sup { superscript_char } else { subscript_char };
    if !compact.is_empty() && compact.chars().all(|c| map(c).is_some()) {
        return compact.chars().filter_map(map).collect();
    }
    if !compact.is_empty() && compact.chars().all(|c| c == '\u{2032}') {
        return compact; // primes
    }
    let marker = if sup { '^' } else { '_' };
    if compact.chars().count() == 1 {
        format!("{}{}", marker, compact)
    } else {
        format!("{}({})", marker, text)
    }
}

impl Writer {
    fn node(&mut self, node: &Node) {
        let font = self.font;
        match &node.kind {
            EqNode::Text(t) => match font {
                Some(kind) => self.out.push_str(&styled_str(kind, t)),
                None => self.out.push_str(t),
            },
            EqNode::Op { symbol, .. } => {
                self.out.push(' ');
                self.out.push_str(symbol);
                self.out.push(' ');
            }
            EqNode::Space(w) => {
                if *w > 0.0 {
                    self.out.push(' ');
                }
            }
            EqNode::Seq(nodes) => {
                for (i, n) in nodes.iter().enumerate() {
                    let is_word = matches!(&n.kind, EqNode::Limit { .. } | EqNode::BigOp { .. });
                    let after_open = self.out.ends_with([' ', '(']);
                    if is_word && i > 0 && !after_open {
                        self.out.push(' ');
                    }
                    self.node(n);
                    let is_operator_word = matches!(&n.kind, EqNode::Limit { .. } | EqNode::BigOp { .. });
                    let next_is_paren = matches!(nodes.get(i + 1).map(|n| &n.kind), Some(EqNode::Text(t)) if t == "(");
                    if is_operator_word && !next_is_paren {
                        self.out.push(' ');
                    }
                }
            }
            EqNode::Sup(b, s) => {
                self.base(b);
                self.out.push_str(&script(s, font, true));
            }
            EqNode::Sub(b, s) => {
                self.base(b);
                self.out.push_str(&script(s, font, false));
            }
            EqNode::SupSub(b, sup, sub) => {
                self.base(b);
                self.out.push_str(&script(sub, font, false));
                self.out.push_str(&script(sup, font, true));
            }
            EqNode::Frac(n, d) => {
                let (nt, dt) = (render(n, font), render(d, font));
                if let Some(v) = vulgar(&nt, &dt) {
                    self.out.push(v);
                } else {
                    self.out.push_str(&paren_if_needed(n, nt));
                    self.out.push('/');
                    self.out.push_str(&paren_if_needed(d, dt));
                }
            }
            EqNode::Sqrt(c) => {
                self.out.push('√');
                self.out.push_str(&paren_if_needed(c, render(c, font)));
            }
            EqNode::Root { index, content } => {
                let it = render(index, font);
                let ct = paren_if_needed(content, render(content, font));
                match it.as_str() {
                    "3" => self.out.push('∛'),
                    "4" => self.out.push('∜'),
                    _ => {
                        self.out.push_str(&script(index, font, true));
                        self.out.push('√');
                    }
                }
                self.out.push_str(&ct);
            }
            EqNode::BigOp { symbol, lower, upper } => {
                self.out.push_str(symbol);
                if let Some(l) = lower {
                    self.out.push_str(&script(l, font, false));
                }
                if let Some(u) = upper {
                    self.out.push_str(&script(u, font, true));
                }
            }
            EqNode::Limit { name, lower } => {
                self.out.push_str(name);
                if let Some(l) = lower {
                    self.out.push_str(&script(l, font, false));
                }
            }
            EqNode::Accent(content, kind) => {
                let text = render(content, font);
                let mark = combining_mark(*kind);
                let chars: Vec<char> = text.chars().collect();
                let per_char = matches!(kind, AccentKind::Bar | AccentKind::Underline);
                if chars.len() == 1 || (per_char && !text.contains(' ')) {
                    for c in chars {
                        self.out.push(c);
                        self.out.push(mark);
                    }
                } else {
                    self.out.push_str(&format!("{}({})", accent_name(*kind), text));
                }
            }
            EqNode::TextBlock(t) => self.out.push_str(t),
            EqNode::MathFont { kind, content } => {
                let saved = self.font.replace(*kind);
                self.node(content);
                self.font = saved;
            }
            EqNode::Delimited { left, right, content } => {
                self.out.push_str(left);
                self.out.push_str(&render(content, font));
                self.out.push_str(right);
            }
            EqNode::SizedDelim { delim, .. } | EqNode::Middle(delim) => self.out.push_str(delim),
            EqNode::Matrix { kind, rows } => {
                let (l, r) = match kind {
                    MatrixKind::Paren => ("(", ")"),
                    MatrixKind::VBar => ("|", "|"),
                    MatrixKind::DoubleVBar => ("‖", "‖"),
                    MatrixKind::Brace => ("{", "}"),
                    _ => ("[", "]"),
                };
                self.out.push_str(l);
                self.out.push_str(&rows_text(rows, font, ", ", "; "));
                self.out.push_str(r);
            }
            EqNode::Cases { rows } => {
                let rows: Vec<String> = rows
                    .iter()
                    .map(|(v, c)| match c {
                        Some(c) => format!("{}, {}", render(v, font), render(c, font)),
                        None => render(v, font),
                    })
                    .collect();
                self.out.push_str(&format!("{{ {} }}", rows.join("; ")));
            }
            EqNode::Align { rows } => self.out.push_str(&rows_text(rows, font, " ", "\n")),
            EqNode::Gather { rows } => {
                let lines: Vec<String> = rows.iter().map(|r| render(r, font)).collect();
                self.out.push_str(&lines.join("\n"));
            }
            EqNode::Array { rows, .. } => {
                self.out.push('[');
                self.out.push_str(&rows_text(rows, font, ", ", "; "));
                self.out.push(']');
            }
            EqNode::Binom(t, b) => {
                self.out.push_str(&format!("binom({}, {})", render(t, font), render(b, font)));
            }
            EqNode::Brace { content, label, .. } => {
                self.node(content);
                if let Some(l) = label {
                    self.out.push_str(&format!(" ({})", render(l, font)));
                }
            }
            EqNode::StackRel { base, annotation, over } => {
                let is_op = matches!(&base.kind, EqNode::Op { .. });
                let base_text = render(base, font);
                if is_op {
                    self.out.push(' ');
                }
                self.out.push_str(&base_text);
                self.out.push_str(&script(annotation, font, *over));
                if is_op {
                    self.out.push(' ');
                }
            }
            EqNode::Color { content, .. } => self.node(content),
            EqNode::Enclose { kind, content } => {
                let text = render(content, font);
                match kind {
                    EncloseKind::Box => self.out.push_str(&format!("[{}]", text)),
                    _ => {
                        for c in text.chars() {
                            self.out.push(c);
                            if c != ' ' {
                                self.out.push('\u{0336}');
                            }
                        }
                    }
                }
            }
        }
    }

    fn base(&mut self, base: &Node) {
        let text = render(base, self.font);
        self.out.push_str(&paren_if_needed(base, text));
    }
}

fn rows_text(rows: &[Vec<Node>], font: Option<MathFontKind>, cell_sep: &str, row_sep: &str) -> String {
    rows.iter()
        .map(|r| r.iter().map(|c| render(c, font)).collect::<Vec<_>>().join(cell_sep))
        .collect::<Vec<_>>()
        .join(row_sep)
}

fn vulgar(n: &str, d: &str) -> Option<char> {
    Some(match (n, d) {
        ("1", "2") => '½', ("1", "3") => '⅓', ("2", "3") => '⅔', ("1", "4") => '¼', ("3", "4") => '¾',
        ("1", "5") => '⅕', ("2", "5") => '⅖', ("3", "5") => '⅗', ("4", "5") => '⅘', ("1", "6") => '⅙',
        ("5", "6") => '⅚', ("1", "7") => '⅐', ("1", "8") => '⅛', ("3", "8") => '⅜', ("5", "8") => '⅝',
        ("7", "8") => '⅞', ("1", "9") => '⅑', ("1", "10") => '⅒',
        _ => return None,
    })
}

fn combining_mark(kind: AccentKind) -> char {
    match kind {
        AccentKind::Hat => '\u{0302}',
        AccentKind::Bar => '\u{0304}',
        AccentKind::Dot => '\u{0307}',
        AccentKind::DoubleDot => '\u{0308}',
        AccentKind::TripleDot => '\u{20DB}',
        AccentKind::Tilde => '\u{0303}',
        AccentKind::Vec => '\u{20D7}',
        AccentKind::LeftArrow => '\u{20D6}',
        AccentKind::LeftRightArrow => '\u{20E1}',
        AccentKind::Acute => '\u{0301}',
        AccentKind::Grave => '\u{0300}',
        AccentKind::Breve => '\u{0306}',
        AccentKind::Check => '\u{030C}',
        AccentKind::Ring => '\u{030A}',
        AccentKind::Underline => '\u{0332}',
    }
}

fn accent_name(kind: AccentKind) -> &'static str {
    match kind {
        AccentKind::Hat => "hat",
        AccentKind::Bar => "bar",
        AccentKind::Dot => "dot",
        AccentKind::DoubleDot => "ddot",
        AccentKind::Tilde => "tilde",
        AccentKind::Vec => "vec",
        AccentKind::Underline => "ul",
        _ => "accent",
    }
}
