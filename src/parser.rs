//! The parser. Turns LaTeX math strings into [`EqNode`] trees.
//!
//! The main entry point is [`parse_equation`]. For standalone symbol lookup,
//! use [`latex_to_unicode`].
//!
//! The parser is a hand-written recursive descent parser over a `Vec<char>`.
//! It never fails — malformed input produces a best-effort tree rather than
//! an error. This keeps live-preview editors responsive while the user types.
//!
//! # Recovery rules
//!
//! Structural tokens only act as terminators where they make sense:
//!
//! - `}` closes a `{` group. Outside any group it is kept as literal text.
//! - `&`, `\\` and `\end{...}` end a cell/row/environment only inside an
//!   environment body. Elsewhere they are kept as literal text.
//! - `\right` closes the nearest `\left`. A stray `\right` is kept as text.
//!
//! Every character of the input therefore ends up somewhere in the tree,
//! except for comments (`% ...`), spacing hints like `\\[2pt]`, and purely
//! presentational commands with no visible output (`\displaystyle`,
//! `\limits`, `\nonumber`, `\label{...}`, ...).

use crate::ast::*;
pub use crate::symbols::latex_to_unicode;
use crate::output::unstyle_char;
use crate::symbols::op_kind;

/// Width of `\quad` (1em). Other units are scaled relative to this.
const EM: f32 = 18.0;

pub const BIG_OP_COMMANDS: &[&str] = &[
    "sum", "prod", "int", "iint", "iiint", "oint", "coprod",
    "iiiint", "oiint", "oiiint",
    "bigcup", "bigcap", "bigoplus", "bigotimes", "bigodot", "biguplus",
    "bigsqcup", "bigvee", "bigwedge",
];

/// How deeply structures may nest (atoms inside atoms) before the parser
/// stops building structure and keeps the rest as plain text. Real
/// equations stay far below this; it guarantees deeply nested or malicious
/// input can't overflow the stack.
pub(crate) const MAX_DEPTH: usize = 48;

/// Commands that only affect presentation and have no visible output of
/// their own. They are consumed without producing a node.
const NOOP_COMMANDS: &[&str] = &[
    "displaystyle", "textstyle", "scriptstyle", "scriptscriptstyle",
    "limits", "nolimits", "displaylimits",
    "nonumber", "notag", "label", "hline",
    "nobreak", "allowbreak",
];

/// Limit operators recognized without a backslash (`sin x`, `lim_(x->0)`).
const BARE_LIMIT_OPS: &[&str] = &[
    "liminf", "limsup", "lim",
    "arcsin", "arccos", "arctan",
    "sinh", "cosh", "tanh",
    "sin", "cos", "tan", "cot", "sec", "csc",
    "log", "ln", "exp",
    "min", "max", "det", "dim", "ker", "gcd", "arg",
];

/// Greek letters and symbols recognized without a backslash (`pi`, `alpha`).
/// Sorted longest-first so `epsilon` wins over `eta`.
const BAREWORDS: &[&str] = &[
    "varepsilon", "rightarrow", "leftarrow", "Rightarrow", "Leftarrow",
    "subseteq", "supseteq", "vartheta", "varsigma", "infinity",
    "epsilon", "upsilon", "omicron", "lambda", "Lambda", "implies", "Upsilon",
    "partial", "emptyset", "varphi", "varrho", "subset", "supset",
    "alpha", "beta", "gamma", "delta", "theta", "kappa", "sigma",
    "omega", "Gamma", "Delta", "Theta", "Sigma", "Omega", "varpi",
    "infty", "nabla", "forall", "exists", "approx", "equiv", "notin",
    "times", "cdots", "ldots",
    "cdot", "zeta", "iota", "neq", "leq", "geq",
    "pi", "rho", "tau", "phi", "chi", "psi",
    "eta", "nu", "xi", "mu", "Pi", "Phi", "Psi", "Xi",
    "pm",
];

/// Named limit-style operators (`\lim`, `\sin`, ...), rendered upright.
pub(crate) const LIMIT_OPS: &[&str] = &[
    "lim", "liminf", "limsup",
    "min", "max", "sup", "inf",
    "log", "lg", "ln", "exp",
    "sin", "cos", "tan", "cot", "sec", "csc",
    "arcsin", "arccos", "arctan",
    "sinh", "cosh", "tanh", "coth",
    "det", "dim", "ker", "deg", "gcd", "hom", "arg", "Pr",
];

/// A symbol as a node: an [`EqNode::Op`] if it is a binary operator or
/// relation, otherwise [`EqNode::Text`].
fn symbol_node(symbol: String) -> EqNode {
    match op_kind(&symbol) {
        Some(kind) => EqNode::Op { symbol, kind },
        None => EqNode::Text(symbol),
    }
}

/// Push a node onto a sequence. A binary operator with no operand on its
/// left (`-x`, `a = -b`, `(+1)`) becomes a unary [`EqNode::Text`].
fn push_node(nodes: &mut Vec<Node>, mut node: Node) {
    if let EqNode::Op { symbol, kind: OpKind::Binary } = &node.kind {
        let left = nodes.iter().rev().map(|n| &n.kind).find(|n| !matches!(n, EqNode::Space(_)));
        let has_left_operand = match left {
            None => false,
            Some(EqNode::Op { .. }) | Some(EqNode::BigOp { .. }) | Some(EqNode::Limit { .. }) | Some(EqNode::Middle(_)) => false,
            Some(EqNode::Text(t)) => !matches!(t.as_str(), "(" | "[" | "{" | "," | ";" | "|" | ""),
            Some(_) => true,
        };
        if !has_left_operand {
            node.kind = EqNode::Text(symbol.clone());
        }
    }
    nodes.push(node);
}

/// Whether a run of letters ending in `word` would be read as a bareword
/// shortcut (`pi`, `sin`, `sqrt`, ...) rather than separate letters. Used by
/// the LaTeX writer to decide where letters need a separating space.
pub(crate) fn is_bareword(word: &str) -> bool {
    word == "sqrt"
        || BIG_OP_COMMANDS.contains(&word)
        || BARE_LIMIT_OPS.contains(&word)
        || BAREWORDS.contains(&word)
}

/// Whether `word` followed by `(` is a function-style shortcut like `abs(`.
pub(crate) fn is_function_word(word: &str) -> bool {
    FUNCTION_WORDS.contains(&word)
}

pub fn is_big_op(name: &str) -> bool {
    BIG_OP_COMMANDS.contains(&name)
}

pub fn big_op_symbol(name: &str) -> &'static str {
    match name {
        "sum" => "\u{2211}",
        "prod" => "\u{220F}",
        "coprod" => "\u{2210}",
        "int" => "\u{222B}",
        "iint" => "\u{222C}",
        "iiint" => "\u{222D}",
        "iiiint" => "\u{2A0C}",
        "oint" => "\u{222E}",
        "oiint" => "\u{222F}",
        "oiiint" => "\u{2230}",
        "bigcup" => "\u{22C3}",
        "bigcap" => "\u{22C2}",
        "bigoplus" => "\u{2A01}",
        "bigotimes" => "\u{2A02}",
        "bigodot" => "\u{2A00}",
        "biguplus" => "\u{2A04}",
        "bigsqcup" => "\u{2A06}",
        "bigvee" => "\u{22C1}",
        "bigwedge" => "\u{22C0}",
        _ => "\u{2211}",
    }
}

/// The negated form of a relation symbol, as produced by `\not`.
fn negate_symbol(s: &str) -> String {
    let mut chars = s.chars();
    let (Some(ch), None) = (chars.next(), chars.next()) else {
        return format!("{}\u{0338}", s);
    };
    let negated = match ch {
        '=' => '\u{2260}',
        '<' => '\u{226E}',
        '>' => '\u{226F}',
        '|' | '\u{2223}' => '\u{2224}',
        '\u{2208}' => '\u{2209}',
        '\u{220B}' => '\u{220C}',
        '\u{2261}' => '\u{2262}',
        '\u{2282}' => '\u{2284}',
        '\u{2283}' => '\u{2285}',
        '\u{2286}' => '\u{2288}',
        '\u{2287}' => '\u{2289}',
        '\u{2203}' => '\u{2204}',
        '\u{2264}' => '\u{2270}',
        '\u{2265}' => '\u{2271}',
        '\u{223C}' => '\u{2241}',
        '\u{2243}' => '\u{2244}',
        '\u{2248}' => '\u{2249}',
        '\u{2245}' => '\u{2247}',
        '\u{2225}' => '\u{2226}',
        '\u{227A}' => '\u{2280}',
        '\u{227B}' => '\u{2281}',
        '\u{2192}' => '\u{219B}',
        '\u{2190}' => '\u{219A}',
        '\u{2194}' => '\u{21AE}',
        '\u{21D2}' => '\u{21CF}',
        '\u{21D0}' => '\u{21CD}',
        '\u{21D4}' => '\u{21CE}',
        '\u{22A2}' => '\u{22AC}',
        '\u{22A8}' => '\u{22AD}',
        '\u{2291}' => '\u{22E2}',
        '\u{2292}' => '\u{22E3}',
        '\u{224D}' => '\u{226D}',
        _ => return format!("{}\u{0338}", s),
    };
    negated.to_string()
}

/// Apply `\not` to an already-parsed atom. `not_span` is the span of the
/// `\not` itself, used if it has to be kept as text.
fn negate_node(node: Node, not_span: Span) -> EqNode {
    match node.kind {
        // Ordinary symbols are only negated if they have a precomposed
        // negation (`\not\exists` is `∄`); `\not` before anything else is
        // kept as text.
        EqNode::Text(s) if s.chars().count() == 1 && !negate_symbol(&s).ends_with('\u{0338}') => {
            symbol_node(negate_symbol(&s))
        }
        EqNode::Op { symbol, .. } => symbol_node(negate_symbol(&symbol)),
        EqNode::Text(s) if s.is_empty() => EqNode::Text("\\not".to_string()),
        other => EqNode::Seq(vec![
            Node::new(EqNode::Text("\\not".to_string()), not_span),
            Node::new(other, node.span),
        ]),
    }
}

/// What terminates a sequence in [`EqParser::parse_seq`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum SeqMode {
    /// Top level, `{...}` groups and environment cells.
    Normal,
    /// The body of `\left ... \right`.
    UntilRight,
    /// The inside of a `(...)` argument group: stops at the matching `)`.
    Paren,
    /// Like `Paren`, but also splits at top-level commas.
    ParenArgs,
}

/// Whether a node can be part of a fraction operand. Operators,
/// separators, opening brackets and big operators end an operand.
fn is_operand(node: &Node) -> bool {
    match &node.kind {
        EqNode::Op { .. } | EqNode::Space(_) | EqNode::BigOp { .. } | EqNode::Middle(_) => false,
        EqNode::Text(t) => !matches!(
            t.as_str(),
            "" | "(" | "[" | "{" | "," | ";" | ":" | "|" | "-" | "+" | "\u{00B1}" | "\u{2213}" | "\u{2212}"
        ),
        _ => true,
    }
}

/// Index of the `(` matching the `)` at `close`, if any.
fn matching_open(nodes: &[Node], close: usize) -> Option<usize> {
    let mut depth = 0usize;
    for i in (0..=close).rev() {
        match &nodes[i].kind {
            EqNode::Text(t) if t == ")" => depth += 1,
            EqNode::Text(t) if t == "(" => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Remove and return the numerator for a `/` from the end of `nodes`.
///
/// The numerator is the run of adjacent operands before the slash
/// (`dy/dx`, `2x^2/3`, `n!/k`), where a parenthesized group counts as one
/// operand (`sin(x)/x`, `2(a+b)/c`). If the whole numerator is a single
/// `(...)` group, its parentheses are dropped: `(a+b)/(c+d)`.
///
/// Also returns whether the numerator was glued to the node before it, and
/// the source span it was taken from (including any dropped parentheses).
fn take_numerator(nodes: &mut Vec<Node>, glued: &mut Vec<bool>, at: Span) -> (Node, bool, Span) {
    let Some(last) = nodes.len().checked_sub(1) else {
        return (Node::new(EqNode::Text(String::new()), at), false, at);
    };
    let mut start = last;
    let is_close = |n: &Node| matches!(&n.kind, EqNode::Text(t) if t == ")");
    if is_close(&nodes[last]) {
        if let Some(open) = matching_open(nodes, last) {
            start = open;
        }
    } else if !is_operand(&nodes[last]) {
        let node = nodes.pop().unwrap();
        let g = glued.pop().unwrap_or(false);
        let span = node.span;
        return (node, g, span);
    }
    let single_group = start != last && start == matching_open(nodes, last).unwrap_or(usize::MAX);
    while start > 0 && glued.get(start).copied().unwrap_or(false) && is_operand(&nodes[start - 1]) {
        start -= 1;
        if is_close(&nodes[start]) {
            match matching_open(nodes, start) {
                Some(open) => start = open,
                None => break,
            }
        }
    }
    let was_glued = glued.get(start).copied().unwrap_or(false);
    glued.truncate(start);
    let mut taken: Vec<Node> = nodes.drain(start..).collect();
    let source = cover(&taken, at);
    let opens_group = matches!(taken.first().map(|n| &n.kind), Some(EqNode::Text(t)) if t == "(");
    if single_group && opens_group && matching_open(&taken, taken.len() - 1) == Some(0) {
        taken.pop();
        taken.remove(0);
    }
    (finish_nodes(taken, at), was_glued, source)
}

/// Join comma-separated argument parts back together with `,` text.
fn join_parts(parts: Vec<Node>, at: Span) -> Node {
    let mut out: Vec<Node> = Vec::new();
    for (i, part) in parts.into_iter().enumerate() {
        if i > 0 {
            out.push(between(out.last(), Some(&part), ","));
        }
        match part.kind {
            EqNode::Seq(v) => out.extend(v),
            EqNode::Text(ref t) if t.is_empty() => {}
            _ => out.push(part),
        }
    }
    finish_nodes(out, at)
}

/// A separator text node (`,` or `&`) placed in the gap between two nodes.
fn between(prev: Option<&Node>, next: Option<&Node>, text: &str) -> Node {
    let start = prev.map(|n| n.span.end).unwrap_or(0);
    let end = next.map(|n| n.span.start).unwrap_or(start).max(start);
    Node::new(EqNode::Text(text.to_string()), Span::new(start, end))
}

/// A Unicode superscript or subscript character, as `(is_superscript, plain char)`.
fn unicode_script(c: char) -> Option<(bool, char)> {
    const SUP: &[(char, char)] = &[
        ('⁰', '0'), ('¹', '1'), ('²', '2'), ('³', '3'), ('⁴', '4'), ('⁵', '5'), ('⁶', '6'),
        ('⁷', '7'), ('⁸', '8'), ('⁹', '9'), ('⁺', '+'), ('⁻', '-'), ('⁼', '='), ('⁽', '('),
        ('⁾', ')'), ('ⁿ', 'n'), ('ⁱ', 'i'),
    ];
    const SUB: &[(char, char)] = &[
        ('₀', '0'), ('₁', '1'), ('₂', '2'), ('₃', '3'), ('₄', '4'), ('₅', '5'), ('₆', '6'),
        ('₇', '7'), ('₈', '8'), ('₉', '9'), ('₊', '+'), ('₋', '-'), ('₌', '='), ('₍', '('),
        ('₎', ')'), ('ₐ', 'a'), ('ₑ', 'e'), ('ₒ', 'o'), ('ₓ', 'x'), ('ₕ', 'h'), ('ₖ', 'k'),
        ('ₗ', 'l'), ('ₘ', 'm'), ('ₙ', 'n'), ('ₚ', 'p'), ('ₛ', 's'), ('ₜ', 't'), ('ᵢ', 'i'),
        ('ⱼ', 'j'), ('ᵣ', 'r'), ('ᵤ', 'u'), ('ᵥ', 'v'),
    ];
    SUP.iter().find(|(u, _)| *u == c).map(|(_, p)| (true, *p))
        .or_else(|| SUB.iter().find(|(u, _)| *u == c).map(|(_, p)| (false, *p)))
}

/// A Unicode vulgar fraction character like `½`, as `(numerator, denominator)`.
fn vulgar_fraction(c: char) -> Option<(&'static str, &'static str)> {
    Some(match c {
        '½' => ("1", "2"), '⅓' => ("1", "3"), '⅔' => ("2", "3"), '¼' => ("1", "4"),
        '¾' => ("3", "4"), '⅕' => ("1", "5"), '⅖' => ("2", "5"), '⅗' => ("3", "5"),
        '⅘' => ("4", "5"), '⅙' => ("1", "6"), '⅚' => ("5", "6"), '⅐' => ("1", "7"),
        '⅛' => ("1", "8"), '⅜' => ("3", "8"), '⅝' => ("5", "8"), '⅞' => ("7", "8"),
        '⅑' => ("1", "9"), '⅒' => ("1", "10"),
        _ => return None,
    })
}

/// The command name for a typed big-operator character like `∑`.
fn big_op_for_char(c: char) -> Option<&'static str> {
    BIG_OP_COMMANDS.iter().copied().find(|name| big_op_symbol(name).starts_with(c))
}

/// ASCII shortcuts for people who don't know LaTeX. Only sequences with no
/// sensible LaTeX reading are included (`<=` would be "less than, equals"
/// in LaTeX); ambiguous ones like `!=` (`n! = 1`) and `<-` (`x < -1`) are
/// left to LaTeX.
const ASCII_SHORTCUTS: &[(&str, &str)] = &[
    ("<=>", "\u{21D4}"), // ⇔
    ("<->", "\u{2194}"), // ↔
    ("+/-", "\u{00B1}"), // ±
    ("<=", "\u{2264}"),  // ≤
    (">=", "\u{2265}"),  // ≥
    ("->", "\u{2192}"),  // →
    ("=>", "\u{21D2}"),  // ⇒
];

/// Function-style shortcuts that take a parenthesized argument list, like
/// `abs(x)` or `root(3, x)`. They only apply when `(` follows immediately.
const FUNCTION_WORDS: &[&str] = &[
    "abs", "norm", "floor", "ceil", "cbrt", "root", "binom", "text",
    "hat", "bar", "vec", "dot", "ddot", "tilde", "ul",
];

/// The span covering a list of nodes (ignoring empty, made-up ones), or
/// `fallback` if there are none.
fn cover(nodes: &[Node], fallback: Span) -> Span {
    nodes
        .iter()
        .map(|n| n.span)
        .filter(|s| !s.is_empty())
        .reduce(|a, b| a.to(b))
        .unwrap_or(fallback)
}

/// Collapse a node list into a single node. An empty list becomes empty
/// text at `at`.
fn finish_nodes(mut nodes: Vec<Node>, at: Span) -> Node {
    match nodes.len() {
        0 => Node::new(EqNode::Text(String::new()), at),
        1 => nodes.remove(0),
        _ => {
            let span = cover(&nodes, at);
            Node::new(EqNode::Seq(nodes), span)
        }
    }
}

/// Join environment cells back into one node, keeping `&` separators as text.
fn join_cells(mut cells: Vec<Node>) -> Node {
    if cells.len() == 1 {
        return cells.remove(0);
    }
    let mut out: Vec<Node> = Vec::new();
    for (i, cell) in cells.into_iter().enumerate() {
        if i > 0 {
            out.push(between(out.last(), Some(&cell), "&"));
        }
        out.push(cell);
    }
    let span = cover(&out, Span::default());
    Node::new(EqNode::Seq(out), span)
}

/// Set every span in a tree to `span` (for sub-trees parsed from a
/// rewritten string, like Unicode superscripts).
fn respan(node: &mut Node, span: Span) {
    node.span = span;
    match &mut node.kind {
        EqNode::Seq(v) | EqNode::Gather { rows: v } => v.iter_mut().for_each(|n| respan(n, span)),
        EqNode::Sup(a, b) | EqNode::Sub(a, b) | EqNode::Frac(a, b) | EqNode::Binom(a, b) => {
            respan(a, span);
            respan(b, span);
        }
        EqNode::SupSub(a, b, c) => {
            respan(a, span);
            respan(b, span);
            respan(c, span);
        }
        EqNode::Delimited { content, .. } | EqNode::Sqrt(content) => respan(content, span),
        _ => {}
    }
}

/// Convert a TeX dimension like `1em`, `-3mu`, `2.5pt` or `1cm` to the
/// parser's spacing units (where `\quad` = 1em = 18). Returns `None` if the
/// string isn't a dimension.
fn parse_dimension(s: &str) -> Option<f32> {
    let s = s.trim();
    let num_end = s
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || ((c == '-' || c == '+') && i == 0)))
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let value: f32 = s[..num_end].trim().parse().ok()?;
    let pt = EM / 10.0; // assume a 10pt font, so 1em = 10pt
    let factor = match s[num_end..].trim() {
        "em" => EM,
        "ex" => EM * 0.431,
        "mu" => EM / 18.0,
        "pt" | "px" | "bp" => pt,
        "mm" => 2.845 * pt,
        "cm" => 28.45 * pt,
        "in" => 72.27 * pt,
        _ => return None,
    };
    Some(value * factor)
}

/// Read a balanced `{...}` group from `chars` starting at `*i` (which must
/// point at `{`). Returns the inner text and advances `*i` past the group.
/// Escaped braces (`\{`, `\}`) don't count towards the nesting.
fn take_braced(chars: &[char], i: &mut usize) -> String {
    let mut out = String::new();
    if chars.get(*i) != Some(&'{') {
        return out;
    }
    *i += 1;
    let mut depth = 1;
    while let Some(&c) = chars.get(*i) {
        *i += 1;
        match c {
            '\\' => {
                // Keep the escape and the escaped character together.
                out.push(c);
                if let Some(&next) = chars.get(*i) {
                    out.push(next);
                    *i += 1;
                }
                continue;
            }
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        out.push(c);
    }
    out
}

/// Parse an `array` column spec such as `c|cc`, `*{3}{c}` or `l@{}r`.
fn parse_column_spec(spec: &str) -> Vec<ArrayColumn> {
    let chars: Vec<char> = spec.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        match c {
            'l' => out.push(ArrayColumn::Left),
            'c' => out.push(ArrayColumn::Center),
            'r' => out.push(ArrayColumn::Right),
            '|' => out.push(ArrayColumn::VLine),
            'p' | 'm' | 'b' => {
                out.push(ArrayColumn::Left);
                take_braced(&chars, &mut i);
            }
            '@' | '!' | '>' | '<' => {
                take_braced(&chars, &mut i);
            }
            '*' => {
                let count: usize = take_braced(&chars, &mut i).trim().parse().unwrap_or(1);
                let inner = parse_column_spec(&take_braced(&chars, &mut i));
                for _ in 0..count.min(256) {
                    out.extend_from_slice(&inner);
                }
            }
            _ => {}
        }
    }
    out
}

/// Parse a LaTeX (or plain-notation) math string into a [`Node`] tree.
///
/// Accepts standard LaTeX math-mode markup as well as bareword shortcuts
/// (`pi`, `sqrt(x)`, `int_0^1`, etc.). Never returns an error — malformed
/// input produces a best-effort tree, and stray structural tokens (`}`, `&`,
/// `\\`, `\end{...}`) are kept as literal text instead of being dropped.
///
/// # Examples
///
/// ```
/// use rust_latex_parser::{parse_equation, EqNode};
///
/// // Standard LaTeX
/// let tree = parse_equation("\\frac{a}{b}");
/// assert!(matches!(tree.kind, EqNode::Frac(_, _)));
///
/// // Bareword shortcuts
/// let tree = parse_equation("pi r^2");
///
/// // Complex expressions
/// let tree = parse_equation("\\int_0^\\infty e^{-x^2} dx = \\sqrt{\\pi}");
/// ```
pub fn parse_equation(input: &str) -> Node {
    let mut parser = EqParser::new(input);
    let mut parts = Vec::new();
    loop {
        parts.push(parser.parse_sequence());
        // The top-level sequence has no terminators, so this only triggers
        // if a future change introduces one. Never lose input regardless.
        let start = parser.pos;
        match parser.advance() {
            Some(ch) => parts.push(Node::new(EqNode::Text(ch.to_string()), parser.span_from(start))),
            None => break,
        }
    }
    if parts.len() == 1 {
        return parts.remove(0);
    }
    let mut nodes = Vec::new();
    for part in parts {
        match part.kind {
            EqNode::Seq(inner) => nodes.extend(inner),
            EqNode::Text(ref s) if s.is_empty() => {}
            _ => nodes.push(part),
        }
    }
    finish_nodes(nodes, Span::default())
}

pub struct EqParser {
    chars: Vec<char>,
    /// Byte offset of each char (plus one past the end), for spans.
    offsets: Vec<usize>,
    pos: usize,
    /// Inside a `{...}` group: `}` terminates the current sequence.
    in_group: bool,
    /// Inside an environment body: `&`, `\\` and `\end` terminate the current sequence.
    in_env: bool,
    /// Current nesting depth, capped at [`MAX_DEPTH`].
    depth: usize,
}

impl EqParser {
    pub fn new(input: &str) -> Self {
        let mut offsets: Vec<usize> = input.char_indices().map(|(i, _)| i).collect();
        offsets.push(input.len());
        Self {
            chars: input.chars().collect(),
            offsets,
            pos: 0,
            in_group: false,
            in_env: false,
            depth: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// The span from char index `start` to the current position.
    fn span_from(&self, start: usize) -> Span {
        Span::new(self.offsets[start], self.offsets[self.pos])
    }

    /// An empty span at the current position.
    fn here(&self) -> Span {
        self.span_from(self.pos)
    }

    /// A node the parser makes up (not read from the input), with an empty
    /// span at the current position.
    fn leaf(&self, kind: EqNode) -> Node {
        Node::new(kind, self.here())
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    /// Skip a `%` comment up to and including the end of the line.
    fn skip_comment(&mut self) {
        while let Some(c) = self.advance() {
            if c == '\n' {
                break;
            }
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            self.skip_whitespace();
            if self.peek() == Some('%') {
                self.skip_comment();
            } else {
                break;
            }
        }
    }

    /// Peek at the first non-whitespace character without consuming anything.
    fn peek_past_whitespace(&self) -> Option<char> {
        self.chars[self.pos..].iter().copied().find(|c| !c.is_whitespace())
    }

    /// The name of the command at the current position (`\name`), if any.
    fn peek_command_name(&self) -> Option<String> {
        if self.peek() != Some('\\') {
            return None;
        }
        let name: String = self.chars[self.pos + 1..]
            .iter()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if name.is_empty() { None } else { Some(name) }
    }

    /// Consume a presentational no-op command (`\displaystyle`, `\limits`,
    /// `\label{...}`, ...) if one is at the current position.
    fn skip_noop_command(&mut self) -> bool {
        let Some(name) = self.peek_command_name() else { return false };
        if !NOOP_COMMANDS.contains(&name.as_str()) {
            return false;
        }
        self.pos += 1 + name.chars().count();
        self.skip_whitespace();
        if name == "label" {
            self.parse_brace_arg_opt();
            self.skip_whitespace();
        }
        true
    }

    fn parse_sequence(&mut self) -> Node {
        self.parse_sequence_until_right(false)
    }

    /// Parse a sequence, optionally stopping at `\right` (for `\left...\right`).
    fn parse_sequence_until_right(&mut self, stop_on_right: bool) -> Node {
        let mode = if stop_on_right { SeqMode::UntilRight } else { SeqMode::Normal };
        self.parse_seq(mode).remove(0)
    }

    /// Whether the current position terminates the sequence being parsed.
    fn at_sequence_stop(&self, stop_on_right: bool) -> bool {
        match self.peek() {
            None => true,
            Some('}') => self.in_group,
            Some('&') => self.in_env,
            Some('\\') => {
                (self.in_env && (self.is_row_separator() || self.lookahead_command("end")))
                    || (stop_on_right && self.lookahead_command("right"))
            }
            _ => false,
        }
    }

    /// The core sequence loop shared by top-level input, `{...}` groups,
    /// environment cells, `\left...\right` bodies and `(...)` argument groups.
    ///
    /// Returns one node per comma-separated part in [`SeqMode::ParenArgs`]
    /// mode, and exactly one node otherwise.
    fn parse_seq(&mut self, mode: SeqMode) -> Vec<Node> {
        let in_parens = matches!(mode, SeqMode::Paren | SeqMode::ParenArgs);
        let stop_on_right = matches!(mode, SeqMode::UntilRight);
        let mut parts = Vec::new();
        let mut nodes: Vec<Node> = Vec::new();
        // glued[i]: no whitespace between nodes[i - 1] and nodes[i].
        let mut glued: Vec<bool> = Vec::new();
        let mut after_space = false;
        let mut depth = 0usize;
        while let Some(ch) = self.peek() {
            if ch == '%' {
                self.skip_comment();
                continue;
            }
            if in_parens {
                match ch {
                    ')' if depth == 0 => break,
                    ')' | '(' => {
                        if ch == '(' { depth += 1 } else { depth -= 1 }
                        let start = self.pos;
                        self.advance();
                        nodes.push(Node::new(EqNode::Text(ch.to_string()), self.span_from(start)));
                        glued.push(!after_space);
                        after_space = false;
                        continue;
                    }
                    ',' if depth == 0 && mode == SeqMode::ParenArgs => {
                        let at = self.here();
                        self.advance();
                        parts.push(finish_nodes(std::mem::take(&mut nodes), at));
                        glued.clear();
                        after_space = false;
                        continue;
                    }
                    _ => {}
                }
            }
            if self.at_sequence_stop(stop_on_right) {
                break;
            }
            if ch.is_whitespace() {
                self.skip_whitespace();
                after_space = true;
                continue;
            }
            if self.skip_noop_command() {
                after_space = true;
                continue;
            }
            if ch == '\\' && !in_parens && self.depth < MAX_DEPTH {
                // \color{c} is a switch: it colors the rest of the current group.
                if self.lookahead_command("color") {
                    let start = self.pos;
                    self.pos += "\\color".len();
                    self.skip_whitespace();
                    let color = self.parse_brace_arg_opt().unwrap_or_default();
                    self.depth += 1;
                    let rest = self.parse_seq(mode).remove(0);
                    self.depth -= 1;
                    let kind = EqNode::Color {
                        color: color.trim().to_string(),
                        content: Box::new(rest),
                    };
                    nodes.push(Node::new(kind, self.span_from(start)));
                    break;
                }
                // Infix {a \over b} and {n \choose k}: everything before is the
                // top, everything after (to the end of the group) the bottom.
                let infix = if self.lookahead_command("over") {
                    Some("over")
                } else if self.lookahead_command("choose") {
                    Some("choose")
                } else {
                    None
                };
                if let Some(cmd) = infix {
                    let at = self.here();
                    self.pos += 1 + cmd.len();
                    let top = finish_nodes(std::mem::take(&mut nodes), at);
                    self.depth += 1;
                    let bottom = self.parse_seq(mode).remove(0);
                    self.depth -= 1;
                    let span = top.span.to(bottom.span).to(at);
                    let kind = if cmd == "over" {
                        EqNode::Frac(Box::new(top), Box::new(bottom))
                    } else {
                        EqNode::Binom(Box::new(top), Box::new(bottom))
                    };
                    nodes.push(Node::new(kind, span));
                    break;
                }
            }
            if ch == '/' {
                let slash = self.here();
                self.advance();
                let (numer, numer_glued, source) = take_numerator(&mut nodes, &mut glued, slash);
                let denom = self.parse_denominator();
                let span = source.to(slash).to(denom.span).to(self.here());
                nodes.push(Node::new(EqNode::Frac(Box::new(numer), Box::new(denom)), span));
                glued.push(numer_glued && !after_space);
                after_space = false;
                continue;
            }
            let atom = self.parse_atom();
            let node = self.parse_postfix(atom);
            push_node(&mut nodes, node);
            glued.push(!after_space);
            after_space = false;
        }
        parts.push(finish_nodes(nodes, self.here()));
        parts
    }

    /// Parse the denominator after `/`: a `(...)` group (parentheses
    /// dropped), or a run of adjacent letters, digits and symbols, so that
    /// `dy/dx` is `dy` over `dx`. Whitespace ends the run: `1/2 x` is
    /// one half times x.
    fn parse_denominator(&mut self) -> Node {
        self.skip_whitespace();
        if self.peek() == Some('(') {
            let group = self.parse_group_atom();
            return self.parse_postfix(group);
        }
        let first = self.parse_atom();
        let mut parts = vec![self.parse_postfix(first)];
        while self.continues_run() {
            let atom = self.parse_atom();
            let node = self.parse_postfix(atom);
            parts.push(node);
        }
        finish_nodes(parts, self.here())
    }

    /// Whether the next token continues a run of adjacent operands
    /// (a letter, digit, or symbol command like `\alpha`).
    fn continues_run(&self) -> bool {
        match self.peek() {
            Some(c) if c.is_alphanumeric() && unicode_script(c).is_none() && vulgar_fraction(c).is_none() => true,
            Some('!') => true, // factorial: n!/k!
            Some('\\') => self
                .peek_command_name()
                .and_then(|name| latex_to_unicode(&name))
                .is_some_and(|sym| op_kind(&sym).is_none()),
            _ => false,
        }
    }

    /// Check if the next two chars are `\\` (row separator in matrices/cases).
    fn is_row_separator(&self) -> bool {
        self.pos + 1 < self.chars.len()
            && self.chars[self.pos] == '\\'
            && self.chars[self.pos + 1] == '\\'
    }

    /// Peek ahead to see if the next command after `\` is the given name,
    /// without consuming anything.
    fn lookahead_command(&self, name: &str) -> bool {
        if self.pos >= self.chars.len() || self.chars[self.pos] != '\\' {
            return false;
        }
        let name_chars: Vec<char> = name.chars().collect();
        let start = self.pos + 1; // skip the backslash
        if start + name_chars.len() > self.chars.len() {
            return false;
        }
        for (i, &nc) in name_chars.iter().enumerate() {
            if self.chars[start + i] != nc {
                return false;
            }
        }
        // Make sure it's not a prefix of a longer command
        let after = start + name_chars.len();
        if after < self.chars.len() && self.chars[after].is_ascii_alphabetic() {
            return false;
        }
        true
    }

    /// Parse an atom that treats `(...)` as invisible grouping (no parens shown).
    /// Used by sqrt, frac, scripts and other constructs that take arguments.
    /// Inner parentheses stay visible: `((3x+3))` keeps one pair.
    fn parse_group_atom(&mut self) -> Node {
        self.skip_whitespace();
        if self.peek() == Some('(') {
            self.advance();
            let inner = self.parse_seq(SeqMode::Paren).remove(0);
            if self.peek() == Some(')') {
                self.advance();
            }
            return inner;
        }
        self.parse_atom()
    }

    /// Parse `( ... )` and split the contents at top-level commas, for
    /// function-style input like `root(3, x)` or `binom(n, k)`.
    /// Assumes the current char is `(`.
    fn parse_paren_args(&mut self) -> Vec<Node> {
        self.advance(); // '('
        let parts = self.parse_seq(SeqMode::ParenArgs);
        if self.peek() == Some(')') {
            self.advance();
        }
        parts
    }

    /// Parse a `{...}` group as an isolated sequence. Inside braces, `&` and
    /// `\\` are literal even when the group sits in an environment cell.
    fn parse_brace_group(&mut self) -> Node {
        self.advance(); // '{'
        let saved = (self.in_group, self.in_env);
        self.in_group = true;
        self.in_env = false;
        let inner = self.parse_sequence();
        (self.in_group, self.in_env) = saved;
        if self.peek() == Some('}') {
            self.advance();
        }
        inner
    }

    /// Parse an optional `[...]` argument. Assumes the current char is `[`.
    fn parse_bracket_arg(&mut self) -> Node {
        self.advance(); // '['
        let mut nodes = Vec::new();
        while let Some(ch) = self.peek() {
            match ch {
                ']' => break,
                '}' if self.in_group => break,
                '%' => self.skip_comment(),
                c if c.is_whitespace() => self.skip_whitespace(),
                _ => {
                    let atom = self.parse_atom();
                    let node = self.parse_postfix(atom);
                    push_node(&mut nodes, node);
                }
            }
        }
        let at = self.here();
        if self.peek() == Some(']') {
            self.advance();
        }
        finish_nodes(nodes, at)
    }

    /// Parse the argument of `\sqrt` / `sqrt`, with an optional `[index]`.
    fn parse_root(&mut self) -> EqNode {
        self.skip_whitespace();
        let index = if self.peek() == Some('[') { Some(self.parse_bracket_arg()) } else { None };
        let content = self.parse_group_atom();
        match index {
            Some(Node { kind: EqNode::Text(ref s), .. }) if s.trim().is_empty() => EqNode::Sqrt(Box::new(content)),
            Some(index) => EqNode::Root { index: Box::new(index), content: Box::new(content) },
            None => EqNode::Sqrt(Box::new(content)),
        }
    }

    /// Consume a run of Unicode superscript (or subscript) characters and
    /// parse their plain equivalents: `²³` is `23`, `₍ᵢ₊₁₎` is `(i+1)`.
    fn take_unicode_script(&mut self, superscript: bool) -> Node {
        let start = self.pos;
        let mut plain = String::new();
        while let Some((is_sup, p)) = self.peek().and_then(unicode_script) {
            if is_sup != superscript {
                break;
            }
            plain.push(p);
            self.advance();
        }
        let mut node = parse_equation(&plain);
        respan(&mut node, self.span_from(start));
        node
    }

    /// Parse the argument of a `^` or `_`. A parenthesized group is taken
    /// whole, with the parentheses dropped: `x^(n+1)`, `sum_(i=1)^n`.
    fn parse_script_arg(&mut self) -> Node {
        self.skip_whitespace_and_comments();
        if self.peek() == Some('(') {
            return self.parse_group_atom();
        }
        self.parse_atom()
    }

    /// The operand of a typed `√`: a `(...)` group or a single atom with
    /// its scripts (`√x²` is the root of x²).
    fn parse_root_operand(&mut self) -> Node {
        let atom = self.parse_group_atom();
        self.parse_postfix(atom)
    }

    /// An ASCII shortcut like `<=` or `->` at the current position, as
    /// `(length, symbol)`.
    fn peek_ascii_shortcut(&self) -> Option<(usize, &'static str)> {
        ASCII_SHORTCUTS.iter().find_map(|(ascii, symbol)| {
            let len = ascii.len();
            let matches = self.chars.len() >= self.pos + len
                && ascii.chars().zip(&self.chars[self.pos..]).all(|(a, b)| a == *b);
            matches.then_some((len, *symbol))
        })
    }

    /// `"quoted text"` is upright text, like `\text{...}`. An unclosed quote
    /// runs to the end of the input, so half-typed text still previews.
    fn parse_quoted_text(&mut self) -> EqNode {
        self.advance(); // opening quote
        let mut text = String::new();
        while let Some(c) = self.advance() {
            if c == '"' {
                break;
            }
            text.push(c);
        }
        EqNode::TextBlock(text)
    }

    /// Match a function-style shortcut word, but only if `(` follows
    /// immediately (so `abs` on its own is still three letters).
    fn try_function_word(&mut self, kw: &str) -> bool {
        let len = kw.chars().count();
        let followed_by_paren = self.chars.get(self.pos + len) == Some(&'(');
        followed_by_paren && self.try_keyword(kw)
    }

    /// Build the node for a function-style shortcut. The current char is `(`.
    fn parse_function_word(&mut self, kw: &str) -> EqNode {
        if kw == "text" {
            // Raw text up to the matching `)`.
            self.advance();
            let mut depth = 0;
            let mut text = String::new();
            while let Some(c) = self.advance() {
                match c {
                    '(' => depth += 1,
                    ')' if depth == 0 => break,
                    ')' => depth -= 1,
                    _ => {}
                }
                text.push(c);
            }
            return EqNode::TextBlock(text);
        }
        let mut args = self.parse_paren_args();
        let at = self.here();
        let delimited = |l: &str, r: &str, content: Node| EqNode::Delimited {
            left: l.to_string(),
            right: r.to_string(),
            content: Box::new(content),
        };
        match kw {
            "root" if args.len() >= 2 => {
                let index = args.remove(0);
                let rest = join_parts(args, at);
                EqNode::Root { index: Box::new(index), content: Box::new(rest) }
            }
            "binom" if args.len() >= 2 => {
                let top = args.remove(0);
                let bottom = join_parts(args, at);
                EqNode::Binom(Box::new(top), Box::new(bottom))
            }
            _ => {
                let content = join_parts(args, at);
                match kw {
                    "abs" => delimited("|", "|", content),
                    "norm" => delimited("\u{2016}", "\u{2016}", content),
                    "floor" => delimited("\u{230A}", "\u{230B}", content),
                    "ceil" => delimited("\u{2308}", "\u{2309}", content),
                    "cbrt" => EqNode::Root {
                        index: Box::new(self.leaf(EqNode::Text("3".to_string()))),
                        content: Box::new(content),
                    },
                    "root" => EqNode::Sqrt(Box::new(content)),
                    "binom" => EqNode::Binom(Box::new(content), Box::new(self.leaf(EqNode::Text(String::new())))),
                    _ => {
                        let kind = match kw {
                            "hat" => AccentKind::Hat,
                            "bar" => AccentKind::Bar,
                            "vec" => AccentKind::Vec,
                            "dot" => AccentKind::Dot,
                            "ddot" => AccentKind::DoubleDot,
                            "tilde" => AccentKind::Tilde,
                            _ => AccentKind::Underline,
                        };
                        EqNode::Accent(Box::new(content), kind)
                    }
                }
            }
        }
    }

    /// Collect `^upper` / `_lower` limits after a big operator, skipping
    /// `\limits` / `\nolimits`.
    fn parse_big_op_limits(&mut self) -> (Option<Node>, Option<Node>) {
        let mut upper = None;
        let mut lower = None;
        loop {
            if self.peek_past_whitespace() == Some('\\') {
                let save = self.pos;
                self.skip_whitespace();
                if self.skip_noop_command() {
                    continue;
                }
                self.pos = save;
            }
            if let Some((is_sup, _)) = self.peek().and_then(unicode_script) {
                // Typed Unicode limits: ∫₀¹, ∑ᵢ₌₁ⁿ
                if (is_sup && upper.is_none()) || (!is_sup && lower.is_none()) {
                    let script = self.take_unicode_script(is_sup);
                    if is_sup { upper = Some(script) } else { lower = Some(script) }
                    continue;
                }
            }
            match self.peek_past_whitespace() {
                Some('^') if upper.is_none() => {
                    self.skip_whitespace();
                    self.advance();
                    upper = Some(self.parse_script_arg());
                }
                Some('_') if lower.is_none() => {
                    self.skip_whitespace();
                    self.advance();
                    lower = Some(self.parse_script_arg());
                }
                _ => break,
            }
        }
        (upper, lower)
    }

    /// Collect the `_lower` limit after a named operator like `\lim`.
    fn parse_limit_lower(&mut self) -> Option<Node> {
        loop {
            if self.peek_past_whitespace() == Some('\\') {
                let save = self.pos;
                self.skip_whitespace();
                if self.skip_noop_command() {
                    continue;
                }
                self.pos = save;
            }
            break;
        }
        if self.peek_past_whitespace() == Some('_') {
            self.skip_whitespace();
            self.advance();
            self.skip_whitespace();
            return Some(self.parse_group_atom());
        }
        None
    }

    fn big_op(&mut self, name: &str) -> EqNode {
        let symbol = big_op_symbol(name).to_string();
        let (upper, lower) = self.parse_big_op_limits();
        EqNode::BigOp {
            symbol,
            lower: lower.map(Box::new),
            upper: upper.map(Box::new),
        }
    }

    /// Parse one atom (a letter, number, command, group, ...), recording
    /// its span.
    fn parse_atom(&mut self) -> Node {
        self.skip_whitespace_and_comments();
        let start = self.pos;
        if self.depth >= MAX_DEPTH {
            // Too deep: keep the input as plain text, one character at a time.
            let kind = match self.advance() {
                Some(c) => EqNode::Text(c.to_string()),
                None => EqNode::Text(String::new()),
            };
            return Node::new(kind, self.span_from(start));
        }
        self.depth += 1;
        let node = if self.peek() == Some('{') {
            // Braces are transparent: the group's node spans its content,
            // so the `x` in `\sqrt{x}` spans just `x`.
            self.parse_brace_group()
        } else {
            let kind = self.parse_atom_kind(start);
            Node::new(kind, self.span_from(start))
        };
        self.depth -= 1;
        node
    }

    fn parse_atom_kind(&mut self, start: usize) -> EqNode {
        match self.peek() {
            None => EqNode::Text(String::new()),
            Some('{') => self.parse_brace_group().kind, // (handled in parse_atom)
            Some('\\') => {
                self.advance();
                self.parse_command(start)
            }
            Some(ch) if ch.is_ascii_alphabetic() => {
                // Check for "sqrt" keyword
                if self.try_keyword("sqrt") {
                    return self.parse_root();
                }
                // Function-style shortcuts: abs(x), root(3, x), text(if), ...
                for &kw in FUNCTION_WORDS {
                    if self.try_function_word(kw) {
                        return self.parse_function_word(kw);
                    }
                }
                // Try bare-word big operators (int, sum, prod, etc.)
                for &kw in BIG_OP_COMMANDS {
                    if self.try_keyword(kw) {
                        let op = self.big_op(kw);
                        // If followed by (...), consume as the integrand/summand body
                        if self.peek() == Some('(') {
                            let op = Node::new(op, self.span_from(start));
                            let body = self.parse_group_atom();
                            return EqNode::Seq(vec![op, body]);
                        }
                        return op;
                    }
                }
                // Try bare-word limit operators (lim, sin, cos, log, etc.)

                for &kw in BARE_LIMIT_OPS {
                    if self.try_keyword(kw) {
                        let lower = self.parse_limit_lower();
                        return EqNode::Limit {
                            name: kw.to_string(),
                            lower: lower.map(Box::new),
                        };
                    }
                }
                // Try bare-word Greek letters / symbols (pi, alpha, theta, etc.)
                if let Some(node) = self.try_bareword_symbol() {
                    return node;
                }
                // Single letter
                self.advance();
                EqNode::Text(ch.to_string())
            }
            Some(ch) if ch.is_ascii_digit() || ch == '.' => {
                let mut s = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() || c == '.' {
                        s.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                EqNode::Text(s)
            }
            Some('*') => {
                self.advance();
                symbol_node("\u{00B7}".to_string())
            }
            Some('"') => self.parse_quoted_text(),
            Some('\u{221A}') => {
                // √x, √(x+1)
                self.advance();
                EqNode::Sqrt(Box::new(self.parse_root_operand()))
            }
            Some(c @ ('\u{221B}' | '\u{221C}')) => {
                // ∛x, ∜x
                self.advance();
                let index = if c == '\u{221B}' { "3" } else { "4" };
                let index = Node::new(EqNode::Text(index.to_string()), self.span_from(start));
                EqNode::Root { index: Box::new(index), content: Box::new(self.parse_root_operand()) }
            }
            Some(c) if vulgar_fraction(c).is_some() => {
                self.advance();
                let (n, d) = vulgar_fraction(c).unwrap();
                let span = self.span_from(start);
                EqNode::Frac(
                    Box::new(Node::new(EqNode::Text(n.to_string()), span)),
                    Box::new(Node::new(EqNode::Text(d.to_string()), span)),
                )
            }
            Some(c) if big_op_for_char(c).is_some() => {
                self.advance();
                self.big_op(big_op_for_char(c).unwrap())
            }
            Some(_) if self.peek_ascii_shortcut().is_some() => {
                let (len, symbol) = self.peek_ascii_shortcut().unwrap();
                self.pos += len;
                symbol_node(symbol.to_string())
            }
            Some(ch) if !ch.is_ascii() && unstyle_char(ch).is_some() => {
                // Typed or pasted math letters like `ℝ` or `𝐱`.
                self.advance();
                let (kind, plain) = unstyle_char(ch).unwrap();
                let content = Node::new(EqNode::Text(plain.to_string()), self.span_from(start));
                EqNode::MathFont { kind, content: Box::new(content) }
            }
            Some(ch) => {
                // Operators (`+ - = < >` and typed Unicode like `≤`) become
                // Op nodes; everything else is text.
                self.advance();
                symbol_node(ch.to_string())
            }
        }
    }

    fn parse_postfix(&mut self, base: Node) -> Node {
        // Collect both ^ and _ in any order to combine into SupSub if both present.
        // Primes (x', x'') become a superscript ′.
        let mut sup: Option<Node> = None;
        let mut sub: Option<Node> = None;
        let mut primes = 0usize;
        let mut prime_span: Option<Span> = None;
        let mut add_prime_span = |span: Span| {
            prime_span = Some(prime_span.map_or(span, |s| s.to(span)));
        };

        loop {
            if let Some((count, _)) = self.peek().and_then(|c| match c {
                '\u{2032}' => Some((1, c)),
                '\u{2033}' => Some((2, c)),
                '\u{2034}' => Some((3, c)),
                _ => None,
            }) {
                // Typed prime characters: x′, f″
                let at = self.pos;
                self.advance();
                add_prime_span(self.span_from(at));
                primes += count;
                continue;
            }
            if let Some((is_sup, _)) = self.peek().and_then(unicode_script) {
                // Typed Unicode scripts: x², a₁, eⁱⁿ
                let slot = if is_sup { &sup } else { &sub };
                if slot.is_none() {
                    let node = self.take_unicode_script(is_sup);
                    if is_sup { sup = Some(node) } else { sub = Some(node) }
                    continue;
                }
            }
            match self.peek_past_whitespace() {
                Some('\'') if self.peek() == Some('\'') => {
                    let at = self.pos;
                    self.advance();
                    add_prime_span(self.span_from(at));
                    primes += 1;
                }
                Some('^') if sup.is_none() => {
                    self.skip_whitespace();
                    self.advance();
                    sup = Some(self.parse_script_arg());
                }
                Some('_') if sub.is_none() => {
                    self.skip_whitespace();
                    self.advance();
                    sub = Some(self.parse_script_arg());
                }
                _ => break,
            }
        }

        if primes > 0 {
            let prime_span = prime_span.unwrap_or_else(|| self.here());
            let prime = Node::new(EqNode::Text("\u{2032}".repeat(primes)), prime_span);
            sup = Some(match sup {
                None => prime,
                Some(Node { kind: EqNode::Seq(mut v), span }) => {
                    v.insert(0, prime);
                    Node::new(EqNode::Seq(v), span.to(prime_span))
                }
                Some(s) => {
                    let span = prime_span.to(s.span);
                    Node::new(EqNode::Seq(vec![prime, s]), span)
                }
            });
        }

        let span = base.span.to(self.here());
        let kind = match (sup, sub) {
            (Some(s), Some(b)) => EqNode::SupSub(Box::new(base), Box::new(s), Box::new(b)),
            (Some(s), None) => EqNode::Sup(Box::new(base), Box::new(s)),
            (None, Some(b)) => EqNode::Sub(Box::new(base), Box::new(b)),
            (None, None) => return base,
        };
        Node::new(kind, span)
    }

    fn try_keyword(&mut self, kw: &str) -> bool {
        // Keywords are ASCII, so bytes and chars line up.
        let len = kw.len();
        let rest = &self.chars[self.pos..];
        if rest.len() < len || !kw.bytes().zip(rest).all(|(k, &c)| k as char == c) {
            return false;
        }
        if rest.get(len).is_some_and(|c| c.is_ascii_alphabetic()) {
            return false;
        }
        self.pos += len;
        true
    }

    /// Try to match a bare-word Greek letter or common symbol at the current position.
    /// Tries longest match first to avoid ambiguity (e.g. "epsilon" before "eta").
    fn try_bareword_symbol(&mut self) -> Option<EqNode> {

        for &word in BAREWORDS {
            if self.try_keyword(word) {
                let name = if word == "infinity" { "infty" } else { word };
                let symbol = latex_to_unicode(name).unwrap_or_else(|| word.to_string());
                return Some(symbol_node(symbol));
            }
        }
        None
    }

    /// Parse a command after its backslash. `start` is the position of the
    /// backslash, for spans of nodes the command makes up.
    fn parse_command(&mut self, start: usize) -> EqNode {
        // Handle single-char (non-alpha) commands first:
        // \, \; \: \> \! spacing, escaped characters, and a stray \\.
        if let Some(ch) = self.peek() {
            if !ch.is_ascii_alphabetic() {
                self.advance();
                return match ch {
                    ',' => EqNode::Space(3.0),   // thin space
                    ':' | '>' => EqNode::Space(4.0), // medium space
                    ';' => EqNode::Space(5.0),   // thick space
                    '!' => EqNode::Space(-3.0),  // negative thin space
                    // A row separator outside an environment: keep it visible.
                    '\\' => EqNode::Text("\\\\".to_string()),
                    '|' => EqNode::Text("\u{2016}".to_string()), // ‖
                    c if c.is_whitespace() => EqNode::Space(4.0), // backslash-space
                    _ => EqNode::Text(ch.to_string()),
                };
            }
        }

        let mut name = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphabetic() {
                name.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        // TeX ignores whitespace after a control word.
        self.skip_whitespace();

        if NOOP_COMMANDS.contains(&name.as_str()) {
            if name == "label" {
                self.parse_brace_arg_opt();
            }
            return EqNode::Text(String::new());
        }

        // Named spacing commands
        let space = match name.as_str() {
            "quad" => Some(EM),
            "qquad" => Some(2.0 * EM),
            "thinspace" => Some(3.0),
            "medspace" => Some(4.0),
            "thickspace" => Some(5.0),
            "enspace" => Some(EM / 2.0),
            "negthinspace" => Some(-3.0),
            "negmedspace" => Some(-4.0),
            "negthickspace" => Some(-5.0),
            _ => None,
        };
        if let Some(width) = space {
            return EqNode::Space(width);
        }

        match name.as_str() {
            // \hspace{1em}, \hspace*{2pt}
            "hspace" => {
                if self.peek() == Some('*') {
                    self.advance();
                    self.skip_whitespace();
                }
                return match self.parse_brace_arg_opt() {
                    Some(dim) => match parse_dimension(&dim) {
                        Some(w) => EqNode::Space(w),
                        None => EqNode::Text(format!("\\hspace{{{}}}", dim)),
                    },
                    None => EqNode::Text("\\hspace".to_string()),
                };
            }
            // \kern3mu, \mkern-2mu, \mskip 4mu, \hskip 1em
            "kern" | "mkern" | "mskip" | "hskip" => {
                let start = self.pos;
                let mut dim = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() || c == '.' || ((c == '-' || c == '+') && dim.is_empty()) {
                        dim.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.skip_whitespace();
                for _ in 0..2 {
                    match self.peek() {
                        Some(c) if c.is_ascii_alphabetic() => {
                            dim.push(c);
                            self.advance();
                        }
                        _ => break,
                    }
                }
                return match parse_dimension(&dim) {
                    Some(w) => EqNode::Space(w),
                    None => {
                        self.pos = start;
                        EqNode::Text(format!("\\{}", name))
                    }
                };
            }

            // Upright text blocks
            "text" | "textrm" | "textnormal" | "textup" | "mbox" | "hbox" => {
                return self.parse_text_block();
            }
            // Styled text blocks
            "textbf" | "textit" | "emph" | "textsl" | "textsf" | "texttt" => {
                let kind = match name.as_str() {
                    "textbf" => MathFontKind::Bold,
                    "textsf" => MathFontKind::SansSerif,
                    "texttt" => MathFontKind::Monospace,
                    _ => MathFontKind::Italic,
                };
                let content = self.parse_text_node();
                return EqNode::MathFont { kind, content: Box::new(content) };
            }
            "fbox" => {
                let content = self.parse_text_node();
                return EqNode::Enclose { kind: EncloseKind::Box, content: Box::new(content) };
            }

            // \operatorname{rank}, \operatorname*{argmax}_x
            "operatorname" => {
                if self.peek() == Some('*') {
                    self.advance();
                    self.skip_whitespace();
                }
                let raw = match self.parse_brace_arg_opt() {
                    Some(raw) => raw,
                    None => self.advance().map(|c| c.to_string()).unwrap_or_default(),
                };
                let op_name: String = raw
                    .replace("\\,", " ")
                    .replace("\\;", " ")
                    .replace("\\:", " ")
                    .replace("\\ ", " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                let lower = self.parse_limit_lower();
                return EqNode::Limit { name: op_name, lower: lower.map(Box::new) };
            }

            // \left ... \right — stretchy delimiters
            "left" => return self.parse_left_right(),
            // \middle| inside \left...\right
            "middle" => return EqNode::Middle(self.parse_delimiter()),

            // \big( \Bigl[ \biggr\} ...
            "big" | "bigl" | "bigr" | "bigm"
            | "Big" | "Bigl" | "Bigr" | "Bigm"
            | "bigg" | "biggl" | "biggr" | "biggm"
            | "Bigg" | "Biggl" | "Biggr" | "Biggm" => {
                let base = name.trim_end_matches(['l', 'r', 'm']);
                let size = match base {
                    "big" => 1,
                    "Big" => 2,
                    "bigg" => 3,
                    _ => 4,
                };
                let delim = self.parse_delimiter();
                return EqNode::SizedDelim { delim, size };
            }

            // \begin{env} ... \end{env}
            "begin" => return self.parse_begin_env(start),
            // A stray \end{...} outside any environment: keep it as text.
            "end" => {
                return match self.parse_brace_arg_opt() {
                    Some(env) => EqNode::Text(format!("\\end{{{}}}", env)),
                    None => EqNode::Text("\\end".to_string()),
                };
            }

            // Binomials
            "binom" | "dbinom" | "tbinom" => {
                let top = self.parse_group_atom();
                let bot = self.parse_group_atom();
                return EqNode::Binom(Box::new(top), Box::new(bot));
            }

            // \overbrace{...}^{label}
            "overbrace" => {
                let content = self.parse_atom();
                let mut label = None;
                if self.peek_past_whitespace() == Some('^') {
                    self.skip_whitespace();
                    self.advance();
                    label = Some(Box::new(self.parse_script_arg()));
                }
                return EqNode::Brace { content: Box::new(content), label, over: true };
            }

            // \underbrace{...}_{label}
            "underbrace" => {
                let content = self.parse_atom();
                let mut label = None;
                if self.peek_past_whitespace() == Some('_') {
                    self.skip_whitespace();
                    self.advance();
                    label = Some(Box::new(self.parse_script_arg()));
                }
                return EqNode::Brace { content: Box::new(content), label, over: false };
            }

            // \overset{annotation}{base} and \stackrel{annotation}{base}
            "overset" | "stackrel" | "underset" => {
                let annotation = self.parse_group_atom();
                let base = self.parse_group_atom();
                return EqNode::StackRel {
                    base: Box::new(base),
                    annotation: Box::new(annotation),
                    over: name != "underset",
                };
            }

            // Extensible arrows: \xrightarrow[below]{above}
            "xrightarrow" | "xleftarrow" | "xRightarrow" | "xLeftarrow"
            | "xleftrightarrow" | "xLeftrightarrow" | "xmapsto"
            | "xhookrightarrow" | "xhookleftarrow" | "xrightleftharpoons" => {
                let arrow = match name.as_str() {
                    "xrightarrow" => "\u{2192}",
                    "xleftarrow" => "\u{2190}",
                    "xRightarrow" => "\u{21D2}",
                    "xLeftarrow" => "\u{21D0}",
                    "xleftrightarrow" => "\u{2194}",
                    "xLeftrightarrow" => "\u{21D4}",
                    "xmapsto" => "\u{21A6}",
                    "xhookrightarrow" => "\u{21AA}",
                    "xhookleftarrow" => "\u{21A9}",
                    _ => "\u{21CC}",
                };
                let arrow_span = self.span_from(start);
                let below = if self.peek() == Some('[') { Some(self.parse_bracket_arg()) } else { None };
                let above = self.parse_group_atom();
                let mut node = Node::new(symbol_node(arrow.to_string()), arrow_span);
                if let Some(below) = below {
                    let kind = EqNode::StackRel { base: Box::new(node), annotation: Box::new(below), over: false };
                    node = Node::new(kind, self.span_from(start));
                }
                return EqNode::StackRel { base: Box::new(node), annotation: Box::new(above), over: true };
            }

            // Fractions (display/text style variants map to the same node)
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                if name == "cfrac" && self.peek() == Some('[') {
                    self.parse_bracket_arg(); // [l] / [r] numerator alignment
                }
                let numer = self.parse_group_atom();
                let denom = self.parse_group_atom();
                return EqNode::Frac(Box::new(numer), Box::new(denom));
            }

            // Square root, optionally with an index: \sqrt[3]{x}
            "sqrt" => return self.parse_root(),

            // Boxes and strike-throughs
            "boxed" | "cancel" | "bcancel" | "xcancel" => {
                let kind = match name.as_str() {
                    "boxed" => EncloseKind::Box,
                    "cancel" => EncloseKind::Cancel,
                    "bcancel" => EncloseKind::BackCancel,
                    _ => EncloseKind::XCancel,
                };
                let content = self.parse_atom();
                return EqNode::Enclose { kind, content: Box::new(content) };
            }

            // Color. \color as a switch is handled in parse_seq;
            // here it only applies to the following atom.
            "textcolor" | "color" => {
                let color = self.parse_brace_arg_opt().unwrap_or_default();
                let content = if name == "textcolor" { self.parse_group_atom() } else { self.parse_atom() };
                return EqNode::Color { color: color.trim().to_string(), content: Box::new(content) };
            }

            // \not\in, \not=, \not\subset, ...
            "not" => {
                let not_span = self.span_from(start);
                let next = self.parse_atom();
                return negate_node(next, not_span);
            }

            // Modular arithmetic
            "bmod" => {
                return EqNode::Op { symbol: "mod".to_string(), kind: OpKind::Binary };
            }
            "pmod" => {
                let cmd = |kind| Node::new(kind, Span::new(self.offsets[start], self.offsets[start]));
                let (space, open, word, gap) = (
                    cmd(EqNode::Space(8.0)),
                    cmd(EqNode::Text("(".to_string())),
                    cmd(EqNode::Limit { name: "mod".to_string(), lower: None }),
                    cmd(EqNode::Space(6.0)),
                );
                let arg = self.parse_group_atom();
                let close = self.leaf(EqNode::Text(")".to_string()));
                return EqNode::Seq(vec![space, open, word, gap, arg, close]);
            }
            "mod" => {
                let cmd = |kind| Node::new(kind, Span::new(self.offsets[start], self.offsets[start]));
                let (space, word, gap) = (
                    cmd(EqNode::Space(12.0)),
                    cmd(EqNode::Limit { name: "mod".to_string(), lower: None }),
                    cmd(EqNode::Space(6.0)),
                );
                let arg = self.parse_group_atom();
                return EqNode::Seq(vec![space, word, gap, arg]);
            }

            // \substack{i < n \\ j < m}
            "substack" => {
                if self.peek() != Some('{') {
                    return EqNode::Text("\\substack".to_string());
                }
                self.advance();
                let (rows, hlines) = self.parse_env_body(true);
                if self.peek() == Some('}') {
                    self.advance();
                }
                return EqNode::Array { columns: vec![ArrayColumn::Center], rows, hlines };
            }

            _ => {}
        }

        // Math fonts
        if let Some(kind) = match name.as_str() {
            "mathbf" => Some(MathFontKind::Bold),
            "mathbb" | "Bbb" => Some(MathFontKind::Blackboard),
            "mathcal" | "mathscr" => Some(MathFontKind::Calligraphic),
            "mathrm" | "mathup" => Some(MathFontKind::Roman),
            "mathfrak" => Some(MathFontKind::Fraktur),
            "mathsf" => Some(MathFontKind::SansSerif),
            "mathtt" => Some(MathFontKind::Monospace),
            "mathit" | "mathnormal" => Some(MathFontKind::Italic),
            "boldsymbol" | "bm" | "pmb" => Some(MathFontKind::BoldItalic),
            _ => None,
        } {
            let arg = self.parse_atom();
            return EqNode::MathFont { kind, content: Box::new(arg) };
        }

        // Accents
        if let Some(kind) = match name.as_str() {
            "hat" | "widehat" => Some(AccentKind::Hat),
            "bar" | "overline" => Some(AccentKind::Bar),
            "dot" => Some(AccentKind::Dot),
            "ddot" => Some(AccentKind::DoubleDot),
            "dddot" => Some(AccentKind::TripleDot),
            "tilde" | "widetilde" => Some(AccentKind::Tilde),
            "vec" | "overrightarrow" => Some(AccentKind::Vec),
            "overleftarrow" => Some(AccentKind::LeftArrow),
            "overleftrightarrow" => Some(AccentKind::LeftRightArrow),
            "acute" => Some(AccentKind::Acute),
            "grave" => Some(AccentKind::Grave),
            "breve" => Some(AccentKind::Breve),
            "check" | "widecheck" => Some(AccentKind::Check),
            "mathring" => Some(AccentKind::Ring),
            "underline" => Some(AccentKind::Underline),
            _ => None,
        } {
            let arg = self.parse_atom();
            return EqNode::Accent(Box::new(arg), kind);
        }

        // Named limit operators (lim, min, max, sup, inf, log, ln, sin, cos, tan, etc.)
        if LIMIT_OPS.contains(&name.as_str()) {
            let lower = self.parse_limit_lower();
            return EqNode::Limit {
                name: name.clone(),
                lower: lower.map(Box::new),
            };
        }

        // Big operators -- parse as BigOp node with limits
        if is_big_op(&name) {
            return self.big_op(&name);
        }

        // Greek letter / symbol lookup
        match latex_to_unicode(&name) {
            // `⊥` is a relation as `\perp` but an ordinary symbol as `\bot`.
            Some(symbol) if name == "perp" => EqNode::Op { symbol, kind: OpKind::Relation },
            Some(symbol) => symbol_node(symbol),
            None => EqNode::Text(format!("\\{}", name)),
        }
    }

    /// Parse \text{...} — consume braces and return raw text (no math parsing).
    fn parse_text_block(&mut self) -> EqNode {
        let raw = self.parse_brace_arg_opt().unwrap_or_default();
        // `\{` and `\}` are literal braces in text; other content is kept raw.
        EqNode::TextBlock(raw.replace("\\{", "{").replace("\\}", "}"))
    }

    /// [`Self::parse_text_block`] as a node with its span.
    fn parse_text_node(&mut self) -> Node {
        let start = self.pos;
        let kind = self.parse_text_block();
        Node::new(kind, self.span_from(start))
    }

    /// Parse a delimiter after `\left`, `\right`, `\middle` or `\big`.
    fn parse_delimiter(&mut self) -> String {
        self.skip_whitespace();
        match self.peek() {
            None => String::new(),
            Some('.') => {
                self.advance();
                String::new()
            }
            Some('<') => {
                self.advance();
                "\u{27E8}".to_string()
            }
            Some('>') => {
                self.advance();
                "\u{27E9}".to_string()
            }
            Some('\\') => {
                self.advance();
                match self.peek() {
                    Some(c) if c.is_ascii_alphabetic() => {
                        let mut name = String::new();
                        while let Some(c) = self.peek() {
                            if c.is_ascii_alphabetic() {
                                name.push(c);
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        match name.as_str() {
                            "vert" | "lvert" | "rvert" => "|".to_string(),
                            "Vert" | "lVert" | "rVert" => "\u{2016}".to_string(),
                            "lbrace" => "{".to_string(),
                            "rbrace" => "}".to_string(),
                            "lbrack" => "[".to_string(),
                            "rbrack" => "]".to_string(),
                            "backslash" => "\\".to_string(),
                            other => latex_to_unicode(other).unwrap_or_else(|| format!("\\{}", other)),
                        }
                    }
                    Some('|') => {
                        self.advance();
                        "\u{2016}".to_string()
                    }
                    Some(c) => {
                        self.advance();
                        c.to_string()
                    }
                    None => "\\".to_string(),
                }
            }
            Some(c) => {
                self.advance();
                c.to_string()
            }
        }
    }

    /// Parse \left<delim> ... \right<delim>.
    fn parse_left_right(&mut self) -> EqNode {
        let left_delim = self.parse_delimiter();

        // Parse content until \right. Like braces, \left...\right forms a
        // group, so `&` and `\\` inside it are not environment separators.
        let saved_env = self.in_env;
        self.in_env = false;
        let content = self.parse_sequence_until_right(true);
        self.in_env = saved_env;

        // Consume \right and its delimiter
        let right_delim = if self.lookahead_command("right") {
            self.pos += "\\right".len();
            self.parse_delimiter()
        } else {
            String::new()
        };

        EqNode::Delimited {
            left: left_delim,
            right: right_delim,
            content: Box::new(content),
        }
    }

    /// Parse \begin{env_name} ... \end{env_name}.
    fn parse_begin_env(&mut self, start: usize) -> EqNode {
        let env_name = match self.parse_brace_arg_opt() {
            Some(name) => name.trim().to_string(),
            None => return EqNode::Text("\\begin".to_string()),
        };

        match env_name.as_str() {
            "matrix" | "pmatrix" | "bmatrix" | "vmatrix" | "Bmatrix" | "Vmatrix" | "smallmatrix" => {
                let kind = match env_name.as_str() {
                    "pmatrix" => MatrixKind::Paren,
                    "bmatrix" => MatrixKind::Bracket,
                    "vmatrix" => MatrixKind::VBar,
                    "Vmatrix" => MatrixKind::DoubleVBar,
                    "Bmatrix" => MatrixKind::Brace,
                    _ => MatrixKind::Plain,
                };
                let (rows, _) = self.parse_env_body(false);
                self.consume_end_env(&env_name);
                EqNode::Matrix { kind, rows }
            }
            "cases" | "dcases" => {
                let (rows, _) = self.parse_env_body(false);
                self.consume_end_env(&env_name);
                let rows = rows
                    .into_iter()
                    .map(|mut cells| {
                        let value = cells.remove(0);
                        let condition = if cells.is_empty() { None } else { Some(join_cells(cells)) };
                        (value, condition)
                    })
                    .collect();
                EqNode::Cases { rows }
            }
            "align" | "align*" | "aligned" | "split" | "flalign" | "flalign*"
            | "eqnarray" | "eqnarray*" | "alignat" | "alignat*" | "alignedat" => {
                if env_name.starts_with("alignat") || env_name == "alignedat" {
                    self.skip_whitespace();
                    self.parse_brace_arg_opt(); // number of column pairs
                }
                let (rows, _) = self.parse_env_body(false);
                self.consume_end_env(&env_name);
                EqNode::Align { rows }
            }
            "gather" | "gather*" | "gathered" => {
                let (rows, _) = self.parse_env_body(false);
                self.consume_end_env(&env_name);
                EqNode::Gather { rows: rows.into_iter().map(join_cells).collect() }
            }
            "array" | "subarray" => {
                self.skip_whitespace();
                if self.peek() == Some('[') {
                    self.parse_bracket_arg(); // vertical position [t]/[b]
                    self.skip_whitespace();
                }
                let spec = self.parse_brace_arg_opt().unwrap_or_default();
                let (rows, hlines) = self.parse_env_body(false);
                self.consume_end_env(&env_name);
                EqNode::Array { columns: parse_column_spec(&spec), rows, hlines }
            }
            "equation" | "equation*" | "displaymath" | "math" => {
                let (mut rows, _) = self.parse_env_body(false);
                self.consume_end_env(&env_name);
                if rows.len() == 1 {
                    join_cells(rows.remove(0)).kind
                } else {
                    EqNode::Gather { rows: rows.into_iter().map(join_cells).collect() }
                }
            }
            _ => {
                // Unknown environment — keep everything as literal text so
                // nothing the user typed disappears.
                let begin = Node::new(EqNode::Text(format!("\\begin{{{}}}", env_name)), self.span_from(start));
                let (rows, hlines) = self.parse_env_body(false);
                let mut parts = vec![begin];
                for (i, row) in rows.into_iter().enumerate() {
                    if i > 0 {
                        let sep = between(parts.last(), row.first(), "\\\\");
                        parts.push(sep);
                    }
                    for _ in hlines.iter().filter(|&&h| h == i) {
                        let at = parts.last().map(|n| n.span.end).unwrap_or(0);
                        parts.push(Node::new(EqNode::Text("\\hline".to_string()), Span::new(at, at)));
                    }
                    for (j, cell) in row.into_iter().enumerate() {
                        if j > 0 {
                            parts.push(between(parts.last(), Some(&cell), "&"));
                        }
                        parts.push(cell);
                    }
                }
                let end_start = self.pos;
                if self.consume_end_env(&env_name) {
                    let end = Node::new(EqNode::Text(format!("\\end{{{}}}", env_name)), self.span_from(end_start));
                    parts.push(end);
                }
                EqNode::Seq(parts)
            }
        }
    }

    /// Consume `{text}` and return the text inside the (balanced) braces, or
    /// `None` if the next character isn't `{`.
    fn parse_brace_arg_opt(&mut self) -> Option<String> {
        if self.peek() != Some('{') {
            return None;
        }
        let mut pos = self.pos;
        let s = take_braced(&self.chars, &mut pos);
        self.pos = pos;
        Some(s)
    }

    /// Consume any `\hline`s at the current position, recording them as
    /// appearing above row `row_index`.
    fn skip_hlines(&mut self, hlines: &mut Vec<usize>, row_index: usize) {
        loop {
            let save = self.pos;
            self.skip_whitespace_and_comments();
            if self.lookahead_command("hline") {
                self.pos += "\\hline".len();
                hlines.push(row_index);
            } else {
                self.pos = save;
                break;
            }
        }
    }

    /// After a `\\`, skip an optional spacing argument like `[2pt]`.
    fn skip_row_spacing(&mut self) {
        if self.peek() != Some('[') {
            return;
        }
        let rest: String = self.chars[self.pos + 1..].iter().take_while(|&&c| c != ']').collect();
        let closed = self.pos + 1 + rest.chars().count() < self.chars.len();
        if closed && parse_dimension(&rest).is_some() {
            self.pos += rest.chars().count() + 2;
        }
    }

    /// Parse an environment body: rows separated by `\\`, cells by `&`.
    /// Stops at `\end` (or at `}` if `brace_delimited`). Returns the rows and
    /// the positions of any `\hline`s.
    fn parse_env_body(&mut self, brace_delimited: bool) -> (Vec<Vec<Node>>, Vec<usize>) {
        let saved = (self.in_group, self.in_env);
        // An environment opened inside a `{...}` group also ends at that
        // group's `}`, so half-typed input like `\frac{\begin{matrix} a}{b}`
        // doesn't swallow the rest of the expression.
        self.in_group = brace_delimited || saved.0;
        self.in_env = true;

        let mut rows = Vec::new();
        let mut hlines = Vec::new();
        self.skip_hlines(&mut hlines, 0);
        loop {
            let mut row = Vec::new();
            loop {
                row.push(self.parse_sequence());
                if self.peek() == Some('&') {
                    self.advance();
                    continue;
                }
                break;
            }
            rows.push(row);
            if self.is_row_separator() {
                self.pos += 2;
                self.skip_row_spacing();
                self.skip_whitespace_and_comments();
                self.skip_hlines(&mut hlines, rows.len());
                // Leading whitespace in a row (e.g. between `\hline` and
                // `\end`) is insignificant.
                self.skip_whitespace_and_comments();
                let at_end = self.peek().is_none()
                    || self.lookahead_command("end")
                    || (self.in_group && self.peek() == Some('}'));
                if at_end {
                    break;
                }
                continue;
            }
            break;
        }

        (self.in_group, self.in_env) = saved;
        (rows, hlines)
    }

    /// Consume `\end{env_name}` if it is next and the name matches.
    /// A mismatched `\end{other}` is left in place for an outer environment
    /// (or, at the top level, kept as literal text).
    fn consume_end_env(&mut self, expected: &str) -> bool {
        if !self.lookahead_command("end") {
            return false;
        }
        let save = self.pos;
        self.pos += "\\end".len();
        self.skip_whitespace();
        match self.parse_brace_arg_opt() {
            Some(name) if name.trim() == expected => true,
            _ => {
                self.pos = save;
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Helper matchers ────────────────────────────────────────────

    fn is_text(node: &Node, expected: &str) -> bool {
        matches!(&node.kind, EqNode::Text(s) if s == expected)
    }

    /// Collect all Text contents from a tree (depth-first), ignoring spaces.
    fn collect_text(node: &Node) -> String {
        match &node.kind {
            EqNode::Text(s) => s.clone(),
            EqNode::Op { symbol, .. } | EqNode::Middle(symbol) => symbol.clone(),
            EqNode::Space(_) => String::new(),
            EqNode::Seq(nodes) => nodes.iter().map(collect_text).collect(),
            EqNode::Sup(base, sup) => format!("{}^{}", collect_text(base), collect_text(sup)),
            EqNode::Sub(base, sub) => format!("{}_{}",collect_text(base), collect_text(sub)),
            EqNode::SupSub(base, sup, sub) => format!("{}^{}_{}", collect_text(base), collect_text(sup), collect_text(sub)),
            EqNode::Frac(n, d) => format!("({})/({})", collect_text(n), collect_text(d)),
            EqNode::Sqrt(inner) => format!("sqrt({})", collect_text(inner)),
            EqNode::BigOp { symbol, lower, upper } => {
                let mut s = symbol.clone();
                if let Some(l) = lower { s += &format!("_{}", collect_text(l)); }
                if let Some(u) = upper { s += &format!("^{}", collect_text(u)); }
                s
            }
            EqNode::Limit { name, lower } => {
                let mut s = name.clone();
                if let Some(l) = lower { s += &format!("_{}", collect_text(l)); }
                s
            }
            EqNode::Accent(inner, _) => collect_text(inner),
            EqNode::TextBlock(s) => s.clone(),
            EqNode::MathFont { content, .. } => collect_text(content),
            EqNode::Delimited { content, .. } => collect_text(content),
            EqNode::Matrix { rows, .. } => {
                rows.iter().map(|r| r.iter().map(collect_text).collect::<Vec<_>>().join("&")).collect::<Vec<_>>().join("\\\\")
            }
            EqNode::Cases { rows, .. } => {
                rows.iter().map(|(v, c)| {
                    let mut s = collect_text(v);
                    if let Some(cond) = c { s += &format!("&{}", collect_text(cond)); }
                    s
                }).collect::<Vec<_>>().join("\\\\")
            }
            EqNode::Binom(a, b) => format!("binom({},{})", collect_text(a), collect_text(b)),
            EqNode::Brace { content, .. } => collect_text(content),
            EqNode::StackRel { base, annotation, .. } => format!("stack({},{})", collect_text(base), collect_text(annotation)),
            EqNode::Root { index, content } => format!("root({},{})", collect_text(index), collect_text(content)),
            EqNode::Color { content, .. } => collect_text(content),
            EqNode::Enclose { content, .. } => collect_text(content),
            EqNode::SizedDelim { delim, .. } => delim.clone(),
            EqNode::Align { rows } | EqNode::Array { rows, .. } => {
                rows.iter().map(|r| r.iter().map(collect_text).collect::<Vec<_>>().join("&")).collect::<Vec<_>>().join("\\\\")
            }
            EqNode::Gather { rows } => rows.iter().map(collect_text).collect::<Vec<_>>().join("\\\\"),
        }
    }

    // ─── Basic parsing ──────────────────────────────────────────────

    #[test]
    fn parse_single_letter() {
        let node = parse_equation("x");
        assert!(is_text(&node, "x"));
    }

    #[test]
    fn parse_number() {
        let node = parse_equation("42");
        assert!(is_text(&node, "42"));
    }

    #[test]
    fn parse_decimal_number() {
        let node = parse_equation("3.14");
        assert!(is_text(&node, "3.14"));
    }

    #[test]
    fn parse_empty_input() {
        let node = parse_equation("");
        assert!(is_text(&node, ""));
    }

    // ─── Superscripts & subscripts ──────────────────────────────────

    #[test]
    fn parse_superscript() {
        let node = parse_equation("x^2");
        assert!(matches!(node.kind, EqNode::Sup(_, _)));
        assert_eq!(collect_text(&node), "x^2");
    }

    #[test]
    fn parse_subscript() {
        let node = parse_equation("x_1");
        assert!(matches!(node.kind, EqNode::Sub(_, _)));
        assert_eq!(collect_text(&node), "x_1");
    }

    #[test]
    fn parse_combined_sup_sub() {
        let node = parse_equation("x^2_3");
        assert!(matches!(node.kind, EqNode::SupSub(_, _, _)));
        assert_eq!(collect_text(&node), "x^2_3");
    }

    #[test]
    fn parse_combined_sub_sup_order() {
        // Sub then sup should also produce SupSub
        let node = parse_equation("x_3^2");
        assert!(matches!(node.kind, EqNode::SupSub(_, _, _)));
    }

    #[test]
    fn parse_braced_superscript() {
        let node = parse_equation("x^{2n}");
        assert!(matches!(node.kind, EqNode::Sup(_, _)));
        if let EqNode::Sup(_, sup) = &node.kind {
            assert_eq!(collect_text(sup), "2n");
        }
    }

    // ─── Fractions ──────────────────────────────────────────────────

    #[test]
    fn parse_inline_fraction() {
        let node = parse_equation("a/b");
        assert!(matches!(node.kind, EqNode::Frac(_, _)));
        assert_eq!(collect_text(&node), "(a)/(b)");
    }

    #[test]
    fn parse_frac_command() {
        let node = parse_equation("\\frac{a}{b}");
        assert!(matches!(node.kind, EqNode::Frac(_, _)));
        assert_eq!(collect_text(&node), "(a)/(b)");
    }

    #[test]
    fn parse_frac_with_parens() {
        // (x+1)/(x-1) — parens are invisible grouping for /
        let node = parse_equation("(x+1)/(x-1)");
        // Should be a sequence ending with a Frac
        let text = collect_text(&node);
        assert!(text.contains("x+1"));
        assert!(text.contains("x-1"));
    }

    #[test]
    fn parse_nested_paren_fraction() {
        // 5/((3x+3)) — inner parens should be visible
        let node = parse_equation("5/((3x+3))");
        let text = collect_text(&node);
        assert!(text.contains("(3x+3)"));
    }

    // ─── Square root ────────────────────────────────────────────────

    #[test]
    fn parse_sqrt_bareword() {
        let node = parse_equation("sqrt(x)");
        assert!(matches!(node.kind, EqNode::Sqrt(_)));
        assert_eq!(collect_text(&node), "sqrt(x)");
    }

    #[test]
    fn parse_sqrt_command() {
        let node = parse_equation("\\sqrt{x}");
        assert!(matches!(node.kind, EqNode::Sqrt(_)));
    }

    #[test]
    fn parse_sqrt_no_visible_parens() {
        // sqrt(x^2) — parens are invisible grouping, not in the AST
        let node = parse_equation("sqrt(x^2)");
        assert!(matches!(node.kind, EqNode::Sqrt(_)));
        // The inner content should be Sup(x, 2), not Seq with parens
        if let EqNode::Sqrt(inner) = &node.kind {
            assert!(matches!(inner.kind, EqNode::Sup(_, _)), "Inner should be Sup, got: {:?}", inner);
        }
    }

    #[test]
    fn parse_sqrt_with_visible_parens() {
        // sqrt((x^2)) — double parens means inner parens are visible
        let node = parse_equation("sqrt((x^2))");
        let text = collect_text(&node);
        assert!(text.contains("("));
    }

    // ─── Greek letters ──────────────────────────────────────────────

    #[test]
    fn parse_greek_bareword() {
        let node = parse_equation("pi");
        assert!(is_text(&node, "\u{03C0}"));
    }

    #[test]
    fn parse_greek_backslash() {
        let node = parse_equation("\\alpha");
        assert!(is_text(&node, "\u{03B1}"));
    }

    #[test]
    fn parse_greek_uppercase() {
        let node = parse_equation("\\Omega");
        assert!(is_text(&node, "\u{03A9}"));
    }

    #[test]
    fn parse_greek_in_expression() {
        let node = parse_equation("pi r^2");
        let text = collect_text(&node);
        assert!(text.contains("\u{03C0}"));
        assert!(text.contains("r^2"));
    }

    // ─── Big operators ──────────────────────────────────────────────

    #[test]
    fn parse_sum_with_limits() {
        let node = parse_equation("\\sum_{i=0}^{n}");
        assert!(matches!(node.kind, EqNode::BigOp { .. }));
        if let EqNode::BigOp { lower, upper, .. } = &node.kind {
            assert!(lower.is_some());
            assert!(upper.is_some());
        }
    }

    #[test]
    fn parse_int_bareword() {
        let node = parse_equation("int_0^1");
        assert!(matches!(node.kind, EqNode::BigOp { .. }));
    }

    #[test]
    fn parse_int_with_body_parens() {
        // int(x^2) — parens should be consumed as invisible grouping
        let tree = parse_equation("int(x^2)");
        let text = collect_text(&tree);
        // Should contain the integral symbol and x^2 but not parens
        assert!(text.contains("x^2"));
        assert!(!text.contains("("));
    }

    // ─── Limit operators ────────────────────────────────────────────

    #[test]
    fn parse_lim_bareword() {
        let node = parse_equation("lim_{x \\to 0}");
        assert!(matches!(node.kind, EqNode::Limit { .. }));
        if let EqNode::Limit { name, lower } = &node.kind {
            assert_eq!(name, "lim");
            assert!(lower.is_some());
        }
    }

    #[test]
    fn parse_sin_bareword() {
        let node = parse_equation("sin(x)");
        // sin is a Limit node, (x) is the argument as a separate Seq member
        let text = collect_text(&node);
        assert!(text.contains("sin"));
        assert!(text.contains("x"));
    }

    #[test]
    fn parse_log_with_subscript() {
        let node = parse_equation("log_2");
        assert!(matches!(node.kind, EqNode::Limit { .. }));
        if let EqNode::Limit { name, lower } = &node.kind {
            assert_eq!(name, "log");
            assert!(lower.is_some());
        }
    }

    // ─── Operator spacing ───────────────────────────────────────────

    fn op(symbol: &str, kind: OpKind) -> Node {
        EqNode::Op { symbol: symbol.to_string(), kind }.into()
    }

    fn t(text: &str) -> Node {
        EqNode::Text(text.to_string()).into()
    }

    fn seq(nodes: Vec<Node>) -> Node {
        EqNode::Seq(nodes).into()
    }

    #[test]
    fn parse_plus_is_binary_op() {
        assert_eq!(
            parse_equation("a + b"),
            seq(vec![t("a"), op("+", OpKind::Binary), t("b")])
        );
    }

    #[test]
    fn parse_unary_minus_is_text() {
        assert_eq!(
            parse_equation("-x"),
            seq(vec![t("-"), t("x")])
        );
        // After a relation, minus is unary too.
        assert_eq!(
            parse_equation("a = -b"),
            seq(vec![
                t("a"),
                op("=", OpKind::Relation),
                t("-"),
                t("b"),
            ])
        );
    }

    #[test]
    fn parse_binary_minus_is_op() {
        assert_eq!(
            parse_equation("a - b"),
            seq(vec![t("a"), op("-", OpKind::Binary), t("b")])
        );
    }

    // ─── Relation symbols ───────────────────────────────────────────

    #[test]
    fn parse_neq_is_relation() {
        assert_eq!(
            parse_equation("a \\neq b"),
            seq(vec![t("a"), op("≠", OpKind::Relation), t("b")])
        );
    }

    #[test]
    fn parse_leq_is_relation() {
        assert!(matches!(
            parse_equation("x \\leq y").kind,
            EqNode::Seq(v) if v[1] == op("≤", OpKind::Relation)
        ));
    }

    #[test]
    fn whitespace_produces_no_nodes() {
        assert_eq!(parse_equation("a  \n b"), parse_equation("ab"));
        assert_eq!(parse_equation("   "), t(""));
    }

    // ─── Environments ───────────────────────────────────────────────

    #[test]
    fn parse_pmatrix() {
        let node = parse_equation("\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}");
        assert!(matches!(node.kind, EqNode::Matrix { .. }));
        if let EqNode::Matrix { kind, rows } = &node.kind {
            assert!(matches!(kind, MatrixKind::Paren));
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].len(), 2);
        }
    }

    #[test]
    fn parse_cases() {
        let node = parse_equation("\\begin{cases} x^2 & x > 0 \\\\ 0 & x = 0 \\end{cases}");
        assert!(matches!(node.kind, EqNode::Cases { .. }));
        if let EqNode::Cases { rows } = &node.kind {
            assert_eq!(rows.len(), 2);
        }
    }

    // ─── Special commands ───────────────────────────────────────────

    #[test]
    fn parse_text_block() {
        let node = parse_equation("\\text{hello world}");
        assert!(matches!(node.kind, EqNode::TextBlock(_)));
        if let EqNode::TextBlock(s) = &node.kind {
            assert_eq!(s, "hello world");
        }
    }

    #[test]
    fn parse_mathbb() {
        let node = parse_equation("\\mathbb{R}");
        assert!(matches!(node.kind, EqNode::MathFont { .. }));
        if let EqNode::MathFont { kind, .. } = &node.kind {
            assert!(matches!(kind, MathFontKind::Blackboard));
        }
    }

    #[test]
    fn parse_binom() {
        let node = parse_equation("\\binom{n}{k}");
        assert!(matches!(node.kind, EqNode::Binom(_, _)));
    }

    #[test]
    fn parse_left_right_delimiters() {
        let node = parse_equation("\\left( x \\right)");
        assert!(matches!(node.kind, EqNode::Delimited { .. }));
    }

    #[test]
    fn parse_overbrace() {
        let node = parse_equation("\\overbrace{a+b}^{n}");
        assert!(matches!(node.kind, EqNode::Brace { over: true, .. }));
    }

    #[test]
    fn parse_overset() {
        let node = parse_equation("\\overset{def}{=}");
        assert!(matches!(node.kind, EqNode::StackRel { over: true, .. }));
    }

    // ─── Spacing commands ───────────────────────────────────────────

    #[test]
    fn parse_quad_spacing() {
        let node = parse_equation("a \\quad b");
        if let EqNode::Seq(nodes) = &node.kind {
            let has_wide_space = nodes.iter().any(|n| matches!(n.kind, EqNode::Space(w) if w >= 18.0));
            assert!(has_wide_space, "\\quad should produce 18pt space");
        }
    }

    #[test]
    fn parse_thin_space() {
        let node = parse_equation("a\\,b");
        if let EqNode::Seq(nodes) = &node.kind {
            let has_thin_space = nodes.iter().any(|n| matches!(n.kind, EqNode::Space(w) if w > 0.0 && w < 5.0));
            assert!(has_thin_space, "\\, should produce thin space");
        }
    }

    // ─── Accents ────────────────────────────────────────────────────

    #[test]
    fn parse_hat_accent() {
        let node = parse_equation("\\hat{x}");
        assert!(matches!(node.kind, EqNode::Accent(_, AccentKind::Hat)));
    }

    #[test]
    fn parse_vec_accent() {
        let node = parse_equation("\\vec{v}");
        assert!(matches!(node.kind, EqNode::Accent(_, AccentKind::Vec)));
    }

    // ─── latex_to_unicode ───────────────────────────────────────────

    #[test]
    fn unicode_greek_lowercase() {
        assert_eq!(latex_to_unicode("alpha"), Some("\u{03B1}".to_string()));
        assert_eq!(latex_to_unicode("omega"), Some("\u{03C9}".to_string()));
    }

    #[test]
    fn unicode_greek_uppercase() {
        assert_eq!(latex_to_unicode("Gamma"), Some("\u{0393}".to_string()));
        assert_eq!(latex_to_unicode("Sigma"), Some("\u{03A3}".to_string()));
    }

    #[test]
    fn unicode_operators() {
        assert_eq!(latex_to_unicode("leq"), Some("\u{2264}".to_string()));
        assert_eq!(latex_to_unicode("geq"), Some("\u{2265}".to_string()));
        assert_eq!(latex_to_unicode("neq"), Some("\u{2260}".to_string()));
        assert_eq!(latex_to_unicode("infty"), Some("\u{221E}".to_string()));
    }

    #[test]
    fn unicode_arrows() {
        assert_eq!(latex_to_unicode("rightarrow"), Some("\u{2192}".to_string()));
        assert_eq!(latex_to_unicode("implies"), Some("\u{21D2}".to_string()));
    }

    #[test]
    fn unicode_unknown_returns_none() {
        assert_eq!(latex_to_unicode("notacommand"), None);
    }

    // ─── Edge cases ─────────────────────────────────────────────────

    #[test]
    fn parse_nested_fractions() {
        let node = parse_equation("\\frac{\\frac{a}{b}}{c}");
        assert!(matches!(node.kind, EqNode::Frac(_, _)));
        if let EqNode::Frac(numer, _) = &node.kind {
            assert!(matches!(numer.kind, EqNode::Frac(_, _)));
        }
    }

    #[test]
    fn parse_nested_superscripts() {
        let node = parse_equation("x^{2^3}");
        assert!(matches!(node.kind, EqNode::Sup(_, _)));
        if let EqNode::Sup(_, sup) = &node.kind {
            assert!(matches!(sup.kind, EqNode::Sup(_, _)));
        }
    }

    #[test]
    fn parse_complex_expression() {
        // Should not panic
        let _node = parse_equation("\\frac{-b \\pm sqrt(b^2 - 4ac)}{2a}");
    }

    #[test]
    fn parse_complex_integral() {
        let _node = parse_equation("\\int_0^\\infty e^{-x^2} dx");
    }

    #[test]
    fn parse_complex_matrix() {
        let _node = parse_equation("\\begin{pmatrix} \\cos\\theta & -\\sin\\theta \\\\ \\sin\\theta & \\cos\\theta \\end{pmatrix}");
    }

    #[test]
    fn parse_euler_identity() {
        let _node = parse_equation("e^{i pi} + 1 = 0");
    }

    #[test]
    fn parse_epsilon_delta() {
        let _node = parse_equation("\\forall \\epsilon > 0, \\exists \\delta > 0 : |x - a| < \\delta \\implies |f(x) - L| < \\epsilon");
    }
}
