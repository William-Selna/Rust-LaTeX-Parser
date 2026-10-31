//! Serialize a tree back to canonical LaTeX.

use crate::ast::*;
use crate::parser::{big_op_symbol, is_bareword, is_function_word, BIG_OP_COMMANDS, LIMIT_OPS};
use crate::symbols::{op_kind, unicode_to_latex};

/// Convert a tree to standard LaTeX.
///
/// The output uses plain LaTeX commands only (no bareword shortcuts), so it
/// works in KaTeX, MathJax and real LaTeX. This lets an editor accept
/// whatever the user types and store canonical LaTeX:
///
/// ```
/// use rust_latex_parser::{parse_equation, to_latex};
///
/// let tree = parse_equation("sqrt(x^2 + 1)/2 <= abs(y)");
/// assert_eq!(to_latex(&tree), r"\frac{\sqrt{x^2 + 1}}{2} \leq \left|y\right|");
/// ```
///
/// Parsing the output gives back an equivalent tree, so
/// `to_latex(&parse_equation(&to_latex(&t)))` equals `to_latex(&t)`.
pub fn to_latex(node: &EqNode) -> String {
    let mut w = Writer::default();
    w.node(node);
    w.out.trim().to_string()
}

#[derive(Default)]
struct Writer {
    out: String,
}

impl Writer {
    /// Append a token, adding a space only where gluing it to the previous
    /// output would change its meaning: after a command name (`\alpha x`),
    /// between digits (`1 2`), or where letters would form a shortcut word
    /// (`p i`, not `pi`).
    fn emit(&mut self, token: &str) {
        let (Some(prev), Some(next)) = (self.out.chars().last(), token.chars().next()) else {
            self.out.push_str(token);
            return;
        };
        let numeric = |c: char| c.is_ascii_digit() || c == '.';
        let needs_space = if prev.is_ascii_alphabetic() && (next.is_ascii_alphabetic() || next == '(') {
            self.letters_would_merge(token)
        } else {
            numeric(prev) && numeric(next)
        };
        if needs_space {
            self.out.push(' ');
        }
        self.out.push_str(token);
    }

    /// Whether appending `token` to the trailing run of letters would read
    /// differently from the letters on their own.
    fn letters_would_merge(&self, token: &str) -> bool {
        let run_len = self.out.chars().rev().take_while(|c| c.is_ascii_alphabetic()).count();
        let run: String = self.out.chars().skip(self.out.chars().count() - run_len).collect();
        let before = self.out.chars().rev().nth(run_len);
        if matches!(before, Some('\\') | Some('^') | Some('_')) {
            // After a command name or a one-letter script (`x^n y`), letters
            // need a space; `(` doesn't (`\sin(x)`).
            return !token.starts_with('(');
        }
        if token.starts_with('(') {
            return (0..run.len()).any(|i| is_function_word(&run[i..]) || run[i..] == *"sqrt");
        }
        let joined: String = run.chars().chain(token.chars().take_while(|c| c.is_ascii_alphabetic())).collect();
        // Any suffix of the joined run that starts in the old run and is a
        // shortcut word would be read as that word.
        (0..run.len()).any(|i| {
            let tail = &joined[i..];
            is_bareword(tail) || (1..tail.len()).any(|k| is_bareword(&tail[..k]) && i + k > run.len())
        })
    }

    /// Append a space unless the output already ends with one.
    fn space(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with(' ') {
            self.out.push(' ');
        }
    }

    /// `{...}` around a sub-tree.
    fn group(&mut self, node: &EqNode) {
        let inner = to_latex(node);
        self.emit("{");
        self.out.push_str(&inner);
        self.out.push('}');
    }

    /// A script or argument: bare if it is a single letter or digit,
    /// otherwise braced.
    fn arg(&mut self, node: &EqNode) {
        match node {
            EqNode::Text(t) if t.chars().count() == 1 && t.chars().all(|c| c.is_ascii_alphanumeric()) => {
                self.out.push_str(t)
            }
            _ => self.group(node),
        }
    }

    fn command(&mut self, name: &str) {
        self.emit("\\");
        self.out.push_str(name);
    }

    fn node(&mut self, node: &EqNode) {
        match node {
            EqNode::Text(t) => self.text(t),
            EqNode::Op { symbol, .. } => {
                self.space();
                self.op_symbol(symbol);
                self.out.push(' ');
            }
            EqNode::Space(w) => self.spacing(*w),
            EqNode::Seq(nodes) => nodes.iter().for_each(|n| self.node(n)),
            EqNode::Sup(base, sup) => {
                self.base(base);
                self.superscript(sup);
            }
            EqNode::Sub(base, sub) => {
                self.base(base);
                self.out.push('_');
                self.arg(sub);
            }
            EqNode::SupSub(base, sup, sub) => {
                self.base(base);
                self.out.push('_');
                self.arg(sub);
                self.superscript(sup);
            }
            EqNode::Frac(n, d) => {
                self.command("frac");
                self.group(n);
                self.group(d);
            }
            EqNode::Sqrt(c) => {
                self.command("sqrt");
                self.group(c);
            }
            EqNode::Root { index, content } => {
                self.command("sqrt");
                self.out.push('[');
                self.out.push_str(&to_latex(index));
                self.out.push(']');
                self.group(content);
            }
            EqNode::BigOp { symbol, lower, upper } => {
                let name = BIG_OP_COMMANDS
                    .iter()
                    .find(|n| big_op_symbol(n) == symbol)
                    .copied()
                    .unwrap_or("sum");
                self.command(name);
                if let Some(l) = lower {
                    self.out.push('_');
                    self.arg(l);
                }
                if let Some(u) = upper {
                    self.out.push('^');
                    self.arg(u);
                }
                self.space();
            }
            EqNode::Limit { name, lower } => {
                if LIMIT_OPS.contains(&name.as_str()) {
                    self.command(name);
                } else {
                    self.command("operatorname");
                    self.out.push('{');
                    self.out.push_str(name);
                    self.out.push('}');
                }
                if let Some(l) = lower {
                    self.out.push('_');
                    self.arg(l);
                    self.space();
                }
            }
            EqNode::Accent(content, kind) => {
                let wide = !is_single_char(content);
                let name = match kind {
                    AccentKind::Hat if wide => "widehat",
                    AccentKind::Hat => "hat",
                    AccentKind::Tilde if wide => "widetilde",
                    AccentKind::Tilde => "tilde",
                    AccentKind::Bar if wide => "overline",
                    AccentKind::Bar => "bar",
                    AccentKind::Vec if wide => "overrightarrow",
                    AccentKind::Vec => "vec",
                    AccentKind::Dot => "dot",
                    AccentKind::DoubleDot => "ddot",
                    AccentKind::TripleDot => "dddot",
                    AccentKind::LeftArrow => "overleftarrow",
                    AccentKind::LeftRightArrow => "overleftrightarrow",
                    AccentKind::Acute => "acute",
                    AccentKind::Grave => "grave",
                    AccentKind::Breve => "breve",
                    AccentKind::Check => "check",
                    AccentKind::Ring => "mathring",
                    AccentKind::Underline => "underline",
                };
                self.command(name);
                self.group(content);
            }
            EqNode::TextBlock(t) => {
                self.command("text");
                self.text_arg(t);
            }
            EqNode::MathFont { kind, content } => {
                if let EqNode::TextBlock(t) = content.as_ref() {
                    let name = match kind {
                        MathFontKind::Bold => Some("textbf"),
                        MathFontKind::Italic | MathFontKind::BoldItalic => Some("textit"),
                        MathFontKind::SansSerif => Some("textsf"),
                        MathFontKind::Monospace => Some("texttt"),
                        MathFontKind::Roman => Some("textrm"),
                        _ => None,
                    };
                    if let Some(name) = name {
                        self.command(name);
                        self.text_arg(t);
                        return;
                    }
                }
                let name = match kind {
                    MathFontKind::Bold => "mathbf",
                    MathFontKind::Blackboard => "mathbb",
                    MathFontKind::Calligraphic => "mathcal",
                    MathFontKind::Roman => "mathrm",
                    MathFontKind::Fraktur => "mathfrak",
                    MathFontKind::SansSerif => "mathsf",
                    MathFontKind::Monospace => "mathtt",
                    MathFontKind::Italic => "mathit",
                    MathFontKind::BoldItalic => "boldsymbol",
                };
                self.command(name);
                self.group(content);
            }
            EqNode::Delimited { left, right, content } => {
                self.command("left");
                self.delimiter(left);
                self.out.push_str(&to_latex(content));
                self.command("right");
                self.delimiter(right);
            }
            EqNode::Middle(delim) => {
                self.command("middle");
                self.delimiter(delim);
            }
            EqNode::SizedDelim { delim, size } => {
                let name = match size {
                    1 => "big",
                    2 => "Big",
                    3 => "bigg",
                    _ => "Bigg",
                };
                self.command(name);
                self.delimiter(delim);
            }
            EqNode::Matrix { kind, rows } => {
                let env = match kind {
                    MatrixKind::Plain => "matrix",
                    MatrixKind::Paren => "pmatrix",
                    MatrixKind::Bracket => "bmatrix",
                    MatrixKind::VBar => "vmatrix",
                    MatrixKind::DoubleVBar => "Vmatrix",
                    MatrixKind::Brace => "Bmatrix",
                };
                self.environment(env, "", rows, &[]);
            }
            EqNode::Cases { rows } => {
                let rows: Vec<Vec<EqNode>> = rows
                    .iter()
                    .map(|(v, c)| {
                        let mut r = vec![v.clone()];
                        r.extend(c.iter().cloned());
                        r
                    })
                    .collect();
                self.environment("cases", "", &rows, &[]);
            }
            EqNode::Align { rows } => self.environment("aligned", "", rows, &[]),
            EqNode::Gather { rows } => {
                let rows: Vec<Vec<EqNode>> = rows.iter().map(|r| vec![r.clone()]).collect();
                self.environment("gathered", "", &rows, &[]);
            }
            EqNode::Array { columns, rows, hlines } => {
                let spec: String = columns
                    .iter()
                    .map(|c| match c {
                        ArrayColumn::Left => 'l',
                        ArrayColumn::Center => 'c',
                        ArrayColumn::Right => 'r',
                        ArrayColumn::VLine => '|',
                    })
                    .collect();
                self.environment("array", &format!("{{{}}}", spec), rows, hlines);
            }
            EqNode::Binom(top, bottom) => {
                self.command("binom");
                self.group(top);
                self.group(bottom);
            }
            EqNode::Brace { content, label, over } => {
                self.command(if *over { "overbrace" } else { "underbrace" });
                self.group(content);
                if let Some(l) = label {
                    self.out.push(if *over { '^' } else { '_' });
                    self.group(l);
                }
            }
            EqNode::StackRel { base, annotation, over } => {
                let is_rel = matches!(base.as_ref(), EqNode::Op { .. });
                if is_rel {
                    self.space();
                }
                self.command(if *over { "overset" } else { "underset" });
                self.group(annotation);
                self.group(base);
                if is_rel {
                    self.out.push(' ');
                }
            }
            EqNode::Color { color, content } => {
                self.command("textcolor");
                self.out.push('{');
                self.out.push_str(color);
                self.out.push('}');
                self.group(content);
            }
            EqNode::Enclose { kind, content } => {
                let name = match kind {
                    EncloseKind::Box => "boxed",
                    EncloseKind::Cancel => "cancel",
                    EncloseKind::BackCancel => "bcancel",
                    EncloseKind::XCancel => "xcancel",
                };
                self.command(name);
                self.group(content);
            }
        }
    }

    /// The base of a script: braced unless it's a single atom. Trailing
    /// spaces are dropped so primes stay attached (`\sum'`).
    fn base(&mut self, base: &EqNode) {
        self.base_inner(base);
        let trimmed = self.out.trim_end().len();
        self.out.truncate(trimmed);
    }

    fn base_inner(&mut self, base: &EqNode) {
        match base {
            EqNode::Seq(_) | EqNode::Sup(..) | EqNode::Sub(..) | EqNode::SupSub(..) => self.group(base),
            // An operator with a script (`+^2`) is written bare; braces would
            // make it a unary sign.
            EqNode::Op { symbol, .. } => self.op_symbol(symbol),
            EqNode::Text(t) if t.is_empty() => self.out.push_str("{}"),
            // A unary sign with a script needs braces to stay unary.
            EqNode::Text(t) if op_kind(t).is_some() => self.group(base),
            _ => self.node(base),
        }
    }

    /// A superscript, writing leading primes as `'`.
    fn superscript(&mut self, sup: &EqNode) {
        let primes = |n: &EqNode| match n {
            EqNode::Text(t) if !t.is_empty() && t.chars().all(|c| c == '\u{2032}') => Some(t.chars().count()),
            _ => None,
        };
        if let Some(n) = primes(sup) {
            self.out.push_str(&"'".repeat(n));
            return;
        }
        if let EqNode::Seq(v) = sup {
            if let Some(n) = v.first().and_then(primes) {
                self.out.push_str(&"'".repeat(n));
                let rest = if v.len() == 2 { v[1].clone() } else { EqNode::Seq(v[1..].to_vec()) };
                self.out.push('^');
                self.arg(&rest);
                return;
            }
        }
        self.out.push('^');
        self.arg(sup);
    }

    /// The `{...}` argument of `\text` and friends, with braces escaped.
    fn text_arg(&mut self, t: &str) {
        self.out.push('{');
        self.out.push_str(&t.replace('{', "\\{").replace('}', "\\}"));
        self.out.push('}');
    }

    fn text(&mut self, t: &str) {
        if t.is_empty() {
            self.out.push_str("{}");
            return;
        }
        if t.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
            // A number or plain letters: one token.
            self.emit(t);
            return;
        }
        if t.starts_with('\\') && t.len() > 1 {
            // An unknown command or stray token kept verbatim.
            self.emit(t);
            return;
        }
        for c in t.chars() {
            match c {
                '{' | '}' | '&' | '%' | '#' | '$' | '_' | '/' | '"' | '\'' | '^' => {
                    self.emit("\\");
                    self.out.push(c);
                }
                '\\' => self.command("backslash"),
                // As text (not an operator), `⊥` is `\bot`, not `\perp`.
                '\u{22A5}' => self.command("bot"),
                _ => match unicode_to_latex(c) {
                    Some(name) => self.command(name),
                    None => {
                        let mut buf = [0u8; 4];
                        self.emit(c.encode_utf8(&mut buf));
                    }
                },
            }
        }
    }

    fn op_symbol(&mut self, symbol: &str) {
        if symbol == "mod" {
            self.command("bmod");
            return;
        }
        let mut chars = symbol.chars();
        let first = chars.next().unwrap_or(' ');
        if chars.next() == Some('\u{0338}') {
            self.command("not");
        }
        match unicode_to_latex(first) {
            Some(name) => self.command(name),
            None => self.out.push(first),
        }
    }

    fn spacing(&mut self, w: f32) {
        let named = [(3.0, ","), (4.0, ":"), (5.0, ";"), (-3.0, "!"), (18.0, "quad"), (36.0, "qquad")];
        match named.iter().find(|(width, _)| *width == w) {
            Some((_, name)) => self.command(name),
            None => self.command(&format!("mkern{}mu", w)),
        }
        if !self.out.ends_with(|c: char| c.is_ascii_alphabetic()) {
            return;
        }
        self.out.push(' ');
    }

    fn delimiter(&mut self, d: &str) {
        match d {
            "" => self.out.push('.'),
            "{" => self.out.push_str("\\{"),
            "}" => self.out.push_str("\\}"),
            "\u{2016}" => self.out.push_str("\\|"),
            "\\" => self.out.push_str("\\backslash "),
            _ => {
                let c = d.chars().next().unwrap_or('.');
                match unicode_to_latex(c) {
                    Some(name) => {
                        self.out.push('\\');
                        self.out.push_str(name);
                        self.out.push(' ');
                    }
                    None => self.out.push_str(d),
                }
            }
        }
    }

    fn environment(&mut self, env: &str, args: &str, rows: &[Vec<EqNode>], hlines: &[usize]) {
        self.command("begin");
        self.out.push('{');
        self.out.push_str(env);
        self.out.push('}');
        self.out.push_str(args);
        let hline_at = |i: usize| hlines.iter().filter(|&&h| h == i).count();
        for (i, row) in rows.iter().enumerate() {
            if i > 0 {
                self.out.push_str(" \\\\");
            }
            for _ in 0..hline_at(i) {
                self.out.push_str(" \\hline");
            }
            self.out.push(' ');
            let cells: Vec<String> = row.iter().map(to_latex).collect();
            self.out.push_str(&cells.join(" & "));
        }
        if hline_at(rows.len()) > 0 {
            self.out.push_str(" \\\\");
            for _ in 0..hline_at(rows.len()) {
                self.out.push_str(" \\hline");
            }
        }
        self.out.push(' ');
        self.command("end");
        self.out.push('{');
        self.out.push_str(env);
        self.out.push('}');
    }
}

fn is_single_char(node: &EqNode) -> bool {
    matches!(node, EqNode::Text(t) if t.chars().count() == 1)
}
