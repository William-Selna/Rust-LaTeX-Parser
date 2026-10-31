//! Serialize a tree to MathML.

pub use super::MathDisplay;
use crate::ast::*;
use crate::style::styled_str;

/// Convert a tree to a MathML `<math>` element.
///
/// The output targets [MathML Core], which current Chrome, Edge, Firefox and
/// Safari render natively, so an editor can show equations without a
/// JavaScript math library. Boxes and strike-throughs (`\boxed`, `\cancel`)
/// are drawn with inline CSS, since MathML Core has no `<menclose>`.
/// Commands the parser didn't recognize are wrapped in `<merror>` so an
/// editor can highlight them.
///
/// [MathML Core]: https://www.w3.org/TR/mathml-core/
///
/// ```
/// use rust_latex_parser::{parse_equation, to_mathml, MathDisplay};
///
/// let tree = parse_equation("x^2");
/// assert_eq!(
///     to_mathml(&tree, MathDisplay::Inline),
///     r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><msup><mi>x</mi><mn>2</mn></msup></math>"#
/// );
/// ```
pub fn to_mathml(node: &Node, display: MathDisplay) -> String {
    let mut out = String::from("<math xmlns=\"http://www.w3.org/1998/Math/MathML\"");
    if display == MathDisplay::Block {
        out.push_str(" display=\"block\"");
    }
    out.push('>');
    Writer { out: &mut out, font: None }.node(node);
    out.push_str("</math>");
    out
}

struct Writer<'a> {
    out: &'a mut String,
    /// The enclosing `\mathbb` / `\mathbf` / ... style, applied to letters.
    font: Option<MathFontKind>,
}

/// Escape text for XML content and attribute values.
fn escape(s: &str) -> String {
    super::xml_escape(s)
}

/// Names that take their limit underneath in display style.
const UNDER_LIMIT_NAMES: &[&str] = &[
    "lim", "liminf", "limsup", "max", "min", "sup", "inf", "det", "gcd", "Pr",
];

/// Big operators whose limits sit to the side even in display style.
const INTEGRALS: &str = "\u{222B}\u{222C}\u{222D}\u{2A0C}\u{222E}\u{222F}\u{2230}";

impl Writer<'_> {
    fn open(&mut self, tag: &str) {
        self.out.push('<');
        self.out.push_str(tag);
        self.out.push('>');
    }

    fn close(&mut self, tag: &str) {
        self.out.push_str("</");
        let name = tag.split_whitespace().next().unwrap_or(tag);
        self.out.push_str(name);
        self.out.push('>');
    }

    fn leaf(&mut self, tag: &str, text: &str) {
        self.open(tag);
        self.out.push_str(&escape(text));
        self.close(tag);
    }

    /// Wrap the output of `f` in `<tag>...</tag>`.
    fn wrap(&mut self, tag: &str, f: impl FnOnce(&mut Self)) {
        self.open(tag);
        f(self);
        self.close(tag);
    }

    fn mo(&mut self, symbol: &str) {
        self.leaf("mo", &display_symbol(symbol));
    }

    fn node(&mut self, node: &Node) {
        match &node.kind {
            EqNode::Text(t) => self.text(t),
            EqNode::Op { symbol, .. } => self.mo(symbol),
            EqNode::Space(w) => {
                self.out.push_str(&format!("<mspace width=\"{}em\"/>", trim_float(*w / 18.0)));
            }
            EqNode::Seq(nodes) => self.wrap("mrow", |w| nodes.iter().for_each(|n| w.node(n))),
            EqNode::Sup(b, s) => self.wrap("msup", |w| {
                w.node(b);
                w.node(s);
            }),
            EqNode::Sub(b, s) => self.wrap("msub", |w| {
                w.node(b);
                w.node(s);
            }),
            EqNode::SupSub(b, sup, sub) => self.wrap("msubsup", |w| {
                w.node(b);
                w.node(sub);
                w.node(sup);
            }),
            EqNode::Frac(n, d) => self.wrap("mfrac", |w| {
                w.node(n);
                w.node(d);
            }),
            EqNode::Sqrt(c) => self.wrap("msqrt", |w| w.node(c)),
            EqNode::Root { index, content } => self.wrap("mroot", |w| {
                w.node(content);
                w.node(index);
            }),
            EqNode::BigOp { symbol, lower, upper } => {
                let integral = INTEGRALS.contains(symbol.as_str());
                let op = if integral {
                    format!("<mo largeop=\"true\">{}</mo>", escape(symbol))
                } else {
                    format!("<mo largeop=\"true\" movablelimits=\"true\">{}</mo>", escape(symbol))
                };
                let (both, low, high) = if integral { ("msubsup", "msub", "msup") } else { ("munderover", "munder", "mover") };
                match (lower, upper) {
                    (None, None) => self.out.push_str(&op),
                    (Some(l), Some(u)) => self.wrap(both, |w| {
                        w.out.push_str(&op);
                        w.node(l);
                        w.node(u);
                    }),
                    (Some(l), None) => self.wrap(low, |w| {
                        w.out.push_str(&op);
                        w.node(l);
                    }),
                    (None, Some(u)) => self.wrap(high, |w| {
                        w.out.push_str(&op);
                        w.node(u);
                    }),
                }
            }
            EqNode::Limit { name, lower } => match lower {
                Some(l) if UNDER_LIMIT_NAMES.contains(&name.as_str()) => self.wrap("munder", |w| {
                    w.out.push_str(&format!("<mo movablelimits=\"true\">{}</mo>", escape(name)));
                    w.node(l);
                }),
                Some(l) => self.wrap("mrow", |w| {
                    w.wrap("msub", |w| {
                        w.leaf("mi", name);
                        w.node(l);
                    });
                    w.leaf("mo", "\u{2061}");
                }),
                None => self.wrap("mrow", |w| {
                    w.leaf("mi", name);
                    w.leaf("mo", "\u{2061}");
                }),
            },
            EqNode::Accent(content, kind) => {
                let stretchy = !matches!(&content.kind, EqNode::Text(t) if t.chars().count() <= 1);
                let mark = match kind {
                    AccentKind::Bar if stretchy => "\u{203E}",
                    _ => accent_mark(*kind),
                };
                let mo = format!("<mo stretchy=\"{}\">{}</mo>", stretchy, escape(mark));
                if *kind == AccentKind::Underline {
                    self.wrap("munder accentunder=\"true\"", |w| {
                        w.node(content);
                        w.out.push_str(&mo);
                    });
                } else {
                    self.wrap("mover accent=\"true\"", |w| {
                        w.node(content);
                        w.out.push_str(&mo);
                    });
                }
            }
            EqNode::TextBlock(t) => {
                let style = match self.font {
                    Some(MathFontKind::Bold) => " style=\"font-weight:bold\"",
                    Some(MathFontKind::Italic) => " style=\"font-style:italic\"",
                    Some(MathFontKind::BoldItalic) => " style=\"font-weight:bold;font-style:italic\"",
                    Some(MathFontKind::SansSerif) => " style=\"font-family:sans-serif\"",
                    Some(MathFontKind::Monospace) => " style=\"font-family:monospace\"",
                    _ => "",
                };
                self.out.push_str(&format!("<mtext{}>{}</mtext>", style, escape(t)));
            }
            EqNode::MathFont { kind, content } => {
                let saved = self.font.replace(*kind);
                self.node(content);
                self.font = saved;
            }
            EqNode::Delimited { left, right, content } => self.wrap("mrow", |w| {
                if !left.is_empty() {
                    w.fence(left, "prefix");
                }
                w.node(content);
                if !right.is_empty() {
                    w.fence(right, "postfix");
                }
            }),
            EqNode::Middle(delim) => self.fence(delim, "infix"),
            EqNode::SizedDelim { delim, size } => {
                let em = match size {
                    1 => "1.2",
                    2 => "1.8",
                    3 => "2.4",
                    _ => "3",
                };
                self.out.push_str(&format!(
                    "<mo stretchy=\"true\" symmetric=\"true\" minsize=\"{em}em\" maxsize=\"{em}em\">{}</mo>",
                    escape(delim)
                ));
            }
            EqNode::Matrix { kind, rows } => {
                let (l, r) = match kind {
                    MatrixKind::Plain => ("", ""),
                    MatrixKind::Paren => ("(", ")"),
                    MatrixKind::Bracket => ("[", "]"),
                    MatrixKind::VBar => ("|", "|"),
                    MatrixKind::DoubleVBar => ("\u{2016}", "\u{2016}"),
                    MatrixKind::Brace => ("{", "}"),
                };
                self.wrap("mrow", |w| {
                    if !l.is_empty() {
                        w.fence(l, "prefix");
                    }
                    w.table(rows, |_| "center", &[], &[]);
                    if !r.is_empty() {
                        w.fence(r, "postfix");
                    }
                });
            }
            EqNode::Cases { rows } => {
                let rows: Vec<Vec<Node>> = rows
                    .iter()
                    .map(|(v, c)| {
                        let mut r = vec![v.clone()];
                        r.extend(c.iter().cloned());
                        r
                    })
                    .collect();
                self.wrap("mrow", |w| {
                    w.fence("{", "prefix");
                    w.table(&rows, |_| "left", &[], &[]);
                });
            }
            EqNode::Align { rows } => {
                self.table(rows, |j| if j % 2 == 0 { "right" } else { "left" }, &[], &[]);
            }
            EqNode::Gather { rows } => {
                let rows: Vec<Vec<Node>> = rows.iter().map(|r| vec![r.clone()]).collect();
                self.table(&rows, |_| "center", &[], &[]);
            }
            EqNode::Array { columns, rows, hlines } => {
                let aligns: Vec<&str> = columns
                    .iter()
                    .filter_map(|c| match c {
                        ArrayColumn::Left => Some("left"),
                        ArrayColumn::Center => Some("center"),
                        ArrayColumn::Right => Some("right"),
                        _ => None,
                    })
                    .collect();
                // Vertical rules: which column boundaries (0 = left edge) have a line.
                let mut vlines = Vec::new();
                let mut col = 0;
                for c in columns {
                    match c {
                        ArrayColumn::VLine => vlines.push(col),
                        _ => col += 1,
                    }
                }
                self.table(rows, |j| aligns.get(j).copied().unwrap_or("center"), &vlines, hlines);
            }
            EqNode::Binom(top, bottom) => self.wrap("mrow", |w| {
                w.fence("(", "prefix");
                w.wrap("mfrac linethickness=\"0\"", |w| {
                    w.node(top);
                    w.node(bottom);
                });
                w.fence(")", "postfix");
            }),
            EqNode::Brace { content, label, over } => {
                let (outer, inner, mark) = if *over {
                    ("mover", "mover", "\u{23DE}")
                } else {
                    ("munder", "munder", "\u{23DF}")
                };
                let draw = |w: &mut Self| {
                    w.wrap(inner, |w| {
                        w.node(content);
                        w.out.push_str(&format!("<mo stretchy=\"true\">{}</mo>", mark));
                    })
                };
                match label {
                    Some(l) => self.wrap(outer, |w| {
                        draw(w);
                        w.node(l);
                    }),
                    None => draw(self),
                }
            }
            EqNode::StackRel { base, annotation, over } => {
                self.wrap(if *over { "mover" } else { "munder" }, |w| {
                    w.node(base);
                    w.node(annotation);
                });
            }
            EqNode::Color { color, content } => {
                self.out.push_str(&format!("<mrow mathcolor=\"{}\">", escape(color)));
                self.node(content);
                self.out.push_str("</mrow>");
            }
            EqNode::Enclose { kind, content } => {
                let style = match kind {
                    EncloseKind::Box => "border:1px solid currentColor;padding:0.1em 0.2em",
                    EncloseKind::Cancel => "background:linear-gradient(to top right,transparent calc(50% - 0.6px),currentColor calc(50% - 0.6px),currentColor calc(50% + 0.6px),transparent calc(50% + 0.6px))",
                    EncloseKind::BackCancel => "background:linear-gradient(to bottom right,transparent calc(50% - 0.6px),currentColor calc(50% - 0.6px),currentColor calc(50% + 0.6px),transparent calc(50% + 0.6px))",
                    EncloseKind::XCancel => "background:linear-gradient(to top right,transparent calc(50% - 0.6px),currentColor calc(50% - 0.6px),currentColor calc(50% + 0.6px),transparent calc(50% + 0.6px)),linear-gradient(to bottom right,transparent calc(50% - 0.6px),currentColor calc(50% - 0.6px),currentColor calc(50% + 0.6px),transparent calc(50% + 0.6px))",
                };
                self.out.push_str(&format!("<mrow style=\"{}\">", style));
                self.node(content);
                self.out.push_str("</mrow>");
            }
        }
    }

    fn fence(&mut self, symbol: &str, form: &str) {
        self.out.push_str(&format!(
            "<mo fence=\"true\" stretchy=\"true\" form=\"{}\">{}</mo>",
            form,
            escape(symbol)
        ));
    }

    /// An `<mtable>`. `align(j)` gives column j's alignment; `vlines` are
    /// column boundaries with a vertical rule; `hlines` rows with a rule above.
    fn table(&mut self, rows: &[Vec<Node>], align: impl Fn(usize) -> &'static str, vlines: &[usize], hlines: &[usize]) {
        let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
        self.out.push_str("<mtable>");
        for (i, row) in rows.iter().enumerate() {
            self.out.push_str("<mtr>");
            for j in 0..cols {
                let mut style = format!("text-align:{}", align(j));
                if vlines.contains(&j) {
                    style.push_str(";border-left:1px solid currentColor");
                }
                if j + 1 == cols && vlines.contains(&cols) {
                    style.push_str(";border-right:1px solid currentColor");
                }
                if hlines.contains(&i) {
                    style.push_str(";border-top:1px solid currentColor");
                }
                if i + 1 == rows.len() && hlines.contains(&rows.len()) {
                    style.push_str(";border-bottom:1px solid currentColor");
                }
                self.out.push_str(&format!("<mtd style=\"{}\">", style));
                if let Some(cell) = row.get(j) {
                    self.node(cell);
                }
                self.out.push_str("</mtd>");
            }
            self.out.push_str("</mtr>");
        }
        self.out.push_str("</mtable>");
    }

    fn text(&mut self, t: &str) {
        if t.is_empty() {
            self.out.push_str("<mrow></mrow>");
            return;
        }
        if t.starts_with('\\') && t.chars().count() > 1 {
            // An unknown command or stray token: flag it for the editor.
            self.out.push_str(&format!("<merror><mtext>{}</mtext></merror>", escape(t)));
            return;
        }
        let styled = match self.font {
            Some(kind) => styled_str(kind, t),
            None => t.to_string(),
        };
        if t.chars().all(|c| c.is_ascii_digit() || c == '.') {
            self.leaf("mn", &styled);
        } else if t.chars().count() == 1 && is_bracket(t.chars().next().unwrap()) {
            // Plain brackets don't stretch (only \left...\right ones do).
            self.out.push_str(&format!(
                "<mo stretchy=\"false\" lspace=\"0\" rspace=\"0\">{}</mo>",
                escape(t)
            ));
        } else if t.chars().count() == 1 && is_fence_or_punct(t.chars().next().unwrap()) {
            self.mo(t);
        } else if self.font == Some(MathFontKind::Roman) {
            self.out.push_str(&format!("<mi mathvariant=\"normal\">{}</mi>", escape(&styled)));
        } else {
            self.leaf("mi", &styled);
        }
    }
}

/// Bracket characters, which render as non-stretchy `<mo>` in plain text.
fn is_bracket(c: char) -> bool {
    matches!(
        c,
        '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\u{2016}' | '\u{27E8}' | '\u{27E9}' | '\u{230A}'
            | '\u{230B}' | '\u{2308}' | '\u{2309}'
    )
}

/// Characters that are `<mo>` rather than identifiers when they appear as
/// plain text: brackets, punctuation, and unary signs.
fn is_fence_or_punct(c: char) -> bool {
    matches!(
        c,
        '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\u{2016}' | ',' | ';' | ':' | '!' | '.' | '\'' | '/'
            | '-' | '+' | '\u{00B1}' | '\u{2213}' | '\u{2212}' | '\u{27E8}' | '\u{27E9}' | '\u{230A}'
            | '\u{230B}' | '\u{2308}' | '\u{2309}' | '\u{2032}'
    )
}

/// The character to display for an operator: ASCII `-` becomes a real
/// minus sign, `*` a centered asterisk.
fn display_symbol(symbol: &str) -> String {
    match symbol {
        "-" => "\u{2212}".to_string(),
        "*" => "\u{2217}".to_string(),
        _ => symbol.to_string(),
    }
}

fn accent_mark(kind: AccentKind) -> &'static str {
    match kind {
        AccentKind::Hat => "^",
        AccentKind::Bar => "\u{00AF}",
        AccentKind::Dot => "\u{02D9}",
        AccentKind::DoubleDot => "\u{00A8}",
        AccentKind::TripleDot => "\u{20DB}",
        AccentKind::Tilde => "~",
        AccentKind::Vec => "\u{2192}",
        AccentKind::LeftArrow => "\u{2190}",
        AccentKind::LeftRightArrow => "\u{2194}",
        AccentKind::Acute => "\u{00B4}",
        AccentKind::Grave => "`",
        AccentKind::Breve => "\u{02D8}",
        AccentKind::Check => "\u{02C7}",
        AccentKind::Ring => "\u{02DA}",
        AccentKind::Underline => "_",
    }
}

/// Format a float without trailing zeros (`0.5`, `1`, `0.1667`).
fn trim_float(v: f32) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" { "0".to_string() } else { s.to_string() }
}
