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
use crate::symbols::op_kind;

/// Width of `\quad` (1em). Other units are scaled relative to this.
const EM: f32 = 18.0;

pub const BIG_OP_COMMANDS: &[&str] = &[
    "sum", "prod", "int", "iint", "iiint", "oint", "coprod",
    "iiiint", "oiint", "oiiint",
    "bigcup", "bigcap", "bigoplus", "bigotimes", "bigodot", "biguplus",
    "bigsqcup", "bigvee", "bigwedge",
];

/// Commands that only affect presentation and have no visible output of
/// their own. They are consumed without producing a node.
const NOOP_COMMANDS: &[&str] = &[
    "displaystyle", "textstyle", "scriptstyle", "scriptscriptstyle",
    "limits", "nolimits", "displaylimits",
    "nonumber", "notag", "label", "hline",
    "nobreak", "allowbreak",
];

/// Named limit-style operators (`\lim`, `\sin`, ...), rendered upright.
const LIMIT_OPS: &[&str] = &[
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
fn push_node(nodes: &mut Vec<EqNode>, node: EqNode) {
    if let EqNode::Op { symbol, kind: OpKind::Binary } = &node {
        let left = nodes.iter().rev().find(|n| !matches!(n, EqNode::Space(_)));
        let has_left_operand = match left {
            None => false,
            Some(EqNode::Op { .. }) | Some(EqNode::BigOp { .. }) | Some(EqNode::Limit { .. }) => false,
            Some(EqNode::Text(t)) => !matches!(t.as_str(), "(" | "[" | "{" | "," | ";" | "|" | ""),
            Some(_) => true,
        };
        if !has_left_operand {
            nodes.push(EqNode::Text(symbol.clone()));
            return;
        }
    }
    nodes.push(node);
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
        _ => return format!("{}\u{0338}", s),
    };
    negated.to_string()
}

/// Apply `\not` to an already-parsed atom.
fn negate_node(node: EqNode) -> EqNode {
    match node {
        EqNode::Text(s) if !s.is_empty() => symbol_node(negate_symbol(&s)),
        EqNode::Op { symbol, .. } => symbol_node(negate_symbol(&symbol)),
        EqNode::Text(_) => EqNode::Text("\\not".to_string()),
        other => EqNode::Seq(vec![EqNode::Text("\\not".to_string()), other]),
    }
}

/// Collapse a node list into a single node.
fn finish_nodes(mut nodes: Vec<EqNode>) -> EqNode {
    match nodes.len() {
        0 => EqNode::Text(String::new()),
        1 => nodes.remove(0),
        _ => EqNode::Seq(nodes),
    }
}

/// Join environment cells back into one node, keeping `&` separators as text.
fn join_cells(mut cells: Vec<EqNode>) -> EqNode {
    if cells.len() == 1 {
        return cells.remove(0);
    }
    let mut out = Vec::new();
    for (i, cell) in cells.into_iter().enumerate() {
        if i > 0 {
            out.push(EqNode::Text("&".to_string()));
        }
        out.push(cell);
    }
    EqNode::Seq(out)
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

/// Parse a LaTeX math string into an [`EqNode`] tree.
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
/// assert!(matches!(tree, EqNode::Frac(_, _)));
///
/// // Bareword shortcuts
/// let tree = parse_equation("pi r^2");
///
/// // Complex expressions
/// let tree = parse_equation("\\int_0^\\infty e^{-x^2} dx = \\sqrt{\\pi}");
/// ```
pub fn parse_equation(input: &str) -> EqNode {
    let mut parser = EqParser::new(input);
    let mut parts = Vec::new();
    loop {
        parts.push(parser.parse_sequence());
        // The top-level sequence has no terminators, so this only triggers
        // if a future change introduces one. Never lose input regardless.
        match parser.advance() {
            Some(ch) => parts.push(EqNode::Text(ch.to_string())),
            None => break,
        }
    }
    if parts.len() == 1 {
        return parts.remove(0);
    }
    let mut nodes = Vec::new();
    for part in parts {
        match part {
            EqNode::Seq(inner) => nodes.extend(inner),
            EqNode::Text(ref s) if s.is_empty() => {}
            other => nodes.push(other),
        }
    }
    finish_nodes(nodes)
}

pub struct EqParser {
    chars: Vec<char>,
    pos: usize,
    /// Inside a `{...}` group: `}` terminates the current sequence.
    in_group: bool,
    /// Inside an environment body: `&`, `\\` and `\end` terminate the current sequence.
    in_env: bool,
}

impl EqParser {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            in_group: false,
            in_env: false,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
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

    fn parse_sequence(&mut self) -> EqNode {
        self.parse_sequence_until(false)
    }

    fn parse_sequence_until(&mut self, stop_on_paren: bool) -> EqNode {
        self.parse_sequence_until_ex(stop_on_paren, false)
    }

    /// Whether the current position terminates the sequence being parsed.
    fn at_sequence_stop(&self, stop_on_paren: bool, stop_on_right: bool) -> bool {
        match self.peek() {
            None => true,
            Some('}') => self.in_group,
            Some(')') => stop_on_paren,
            Some('&') => self.in_env,
            Some('\\') => {
                (self.in_env && (self.is_row_separator() || self.lookahead_command("end")))
                    || (stop_on_right && self.lookahead_command("right"))
            }
            _ => false,
        }
    }

    fn parse_sequence_until_ex(&mut self, stop_on_paren: bool, stop_on_right: bool) -> EqNode {
        let mut nodes = Vec::new();
        while let Some(ch) = self.peek() {
            if ch == '%' {
                self.skip_comment();
                continue;
            }
            if self.at_sequence_stop(stop_on_paren, stop_on_right) {
                break;
            }
            if ch.is_whitespace() {
                self.skip_whitespace();
                continue;
            }
            if self.skip_noop_command() {
                continue;
            }
            if ch == '\\' {
                // \color{c} is a switch: it colors the rest of the current group.
                if self.lookahead_command("color") {
                    self.pos += "\\color".len();
                    self.skip_whitespace();
                    let color = self.parse_brace_arg_opt().unwrap_or_default();
                    let rest = self.parse_sequence_until_ex(stop_on_paren, stop_on_right);
                    nodes.push(EqNode::Color {
                        color: color.trim().to_string(),
                        content: Box::new(rest),
                    });
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
                    self.pos += 1 + cmd.len();
                    let top = finish_nodes(std::mem::take(&mut nodes));
                    let bottom = self.parse_sequence_until_ex(stop_on_paren, stop_on_right);
                    nodes.push(if cmd == "over" {
                        EqNode::Frac(Box::new(top), Box::new(bottom))
                    } else {
                        EqNode::Binom(Box::new(top), Box::new(bottom))
                    });
                    break;
                }
            }
            if ch == '/' {
                self.advance();
                // Turn last node into fraction numerator
                let numer = if let Some(n) = nodes.pop() { n } else { EqNode::Text(String::new()) };
                let denom = self.parse_group_atom();
                let denom = self.parse_postfix(denom);
                nodes.push(EqNode::Frac(Box::new(numer), Box::new(denom)));
                continue;
            }
            let atom = self.parse_atom();
            let node = self.parse_postfix(atom);
            push_node(&mut nodes, node);
        }
        finish_nodes(nodes)
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
    /// Used by sqrt, frac, and other constructs that take arguments.
    /// Tracks paren nesting so `((3x+3))` correctly distinguishes inner visible parens from grouping parens.
    fn parse_group_atom(&mut self) -> EqNode {
        self.skip_whitespace();
        if self.peek() == Some('(') {
            self.advance();
            // Parse content, but track nested parens so we only stop
            // on the `)` that matches our opening `(`
            let mut nodes = Vec::new();
            let mut depth = 0i32;
            while let Some(ch) = self.peek() {
                match ch {
                    ')' if depth == 0 => break,
                    ')' => {
                        depth -= 1;
                        self.advance();
                        nodes.push(EqNode::Text(")".into()));
                    }
                    '(' => {
                        depth += 1;
                        self.advance();
                        nodes.push(EqNode::Text("(".into()));
                    }
                    '%' => self.skip_comment(),
                    _ if self.at_sequence_stop(false, false) => break,
                    c if c.is_whitespace() => self.skip_whitespace(),
                    _ => {
                        if self.skip_noop_command() {
                            continue;
                        }
                        let atom = self.parse_atom();
                        let node = self.parse_postfix(atom);
                        push_node(&mut nodes, node);
                    }
                }
            }
            if self.peek() == Some(')') {
                self.advance();
            }
            return finish_nodes(nodes);
        }
        self.parse_atom()
    }

    /// Parse a `{...}` group as an isolated sequence. Inside braces, `&` and
    /// `\\` are literal even when the group sits in an environment cell.
    fn parse_brace_group(&mut self) -> EqNode {
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
    fn parse_bracket_arg(&mut self) -> EqNode {
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
        if self.peek() == Some(']') {
            self.advance();
        }
        finish_nodes(nodes)
    }

    /// Parse the argument of `\sqrt` / `sqrt`, with an optional `[index]`.
    fn parse_root(&mut self) -> EqNode {
        self.skip_whitespace();
        let index = if self.peek() == Some('[') { Some(self.parse_bracket_arg()) } else { None };
        let content = self.parse_group_atom();
        match index {
            Some(EqNode::Text(ref s)) if s.trim().is_empty() => EqNode::Sqrt(Box::new(content)),
            Some(index) => EqNode::Root { index: Box::new(index), content: Box::new(content) },
            None => EqNode::Sqrt(Box::new(content)),
        }
    }

    /// Parse the argument of a `^` or `_`.
    fn parse_script_arg(&mut self) -> EqNode {
        self.skip_whitespace_and_comments();
        self.parse_atom()
    }

    /// Collect `^upper` / `_lower` limits after a big operator, skipping
    /// `\limits` / `\nolimits`.
    fn parse_big_op_limits(&mut self) -> (Option<EqNode>, Option<EqNode>) {
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
    fn parse_limit_lower(&mut self) -> Option<EqNode> {
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

    fn parse_atom(&mut self) -> EqNode {
        self.skip_whitespace_and_comments();
        match self.peek() {
            None => EqNode::Text(String::new()),
            Some('{') => self.parse_brace_group(),
            Some('\\') => {
                self.advance();
                self.parse_command()
            }
            Some(ch) if ch.is_ascii_alphabetic() => {
                // Check for "sqrt" keyword
                if self.try_keyword("sqrt") {
                    return self.parse_root();
                }
                // Try bare-word big operators (int, sum, prod, etc.)
                for &kw in BIG_OP_COMMANDS {
                    if self.try_keyword(kw) {
                        let op = self.big_op(kw);
                        // If followed by (...), consume as the integrand/summand body
                        if self.peek() == Some('(') {
                            let body = self.parse_group_atom();
                            return EqNode::Seq(vec![op, body]);
                        }
                        return op;
                    }
                }
                // Try bare-word limit operators (lim, sin, cos, log, etc.)
                static BARE_LIMIT_OPS: &[&str] = &[
                    "liminf", "limsup", "lim",
                    "arcsin", "arccos", "arctan",
                    "sinh", "cosh", "tanh",
                    "sin", "cos", "tan", "cot", "sec", "csc",
                    "log", "ln", "exp",
                    "min", "max", "det", "dim", "ker", "gcd", "arg",
                ];
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
            Some(ch) => {
                // Operators (`+ - = < >` and typed Unicode like `≤`) become
                // Op nodes; everything else is text.
                self.advance();
                symbol_node(ch.to_string())
            }
        }
    }

    fn parse_postfix(&mut self, base: EqNode) -> EqNode {
        // Collect both ^ and _ in any order to combine into SupSub if both present.
        // Primes (x', x'') become a superscript ′.
        let mut sup: Option<EqNode> = None;
        let mut sub: Option<EqNode> = None;
        let mut primes = 0usize;

        loop {
            match self.peek_past_whitespace() {
                Some('\'') if self.peek() == Some('\'') => {
                    self.advance();
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
            let prime = EqNode::Text("\u{2032}".repeat(primes));
            sup = Some(match sup {
                None => prime,
                Some(EqNode::Seq(mut v)) => {
                    v.insert(0, prime);
                    EqNode::Seq(v)
                }
                Some(s) => EqNode::Seq(vec![prime, s]),
            });
        }

        match (sup, sub) {
            (Some(s), Some(b)) => EqNode::SupSub(Box::new(base), Box::new(s), Box::new(b)),
            (Some(s), None) => EqNode::Sup(Box::new(base), Box::new(s)),
            (None, Some(b)) => EqNode::Sub(Box::new(base), Box::new(b)),
            (None, None) => base,
        }
    }

    fn try_keyword(&mut self, kw: &str) -> bool {
        let kw_chars: Vec<char> = kw.chars().collect();
        if self.pos + kw_chars.len() > self.chars.len() {
            return false;
        }
        for (i, &kc) in kw_chars.iter().enumerate() {
            if self.chars[self.pos + i] != kc {
                return false;
            }
        }
        let after = self.pos + kw_chars.len();
        if after < self.chars.len() && self.chars[after].is_ascii_alphabetic() {
            return false;
        }
        self.pos += kw_chars.len();
        true
    }

    /// Try to match a bare-word Greek letter or common symbol at the current position.
    /// Tries longest match first to avoid ambiguity (e.g. "epsilon" before "eta").
    fn try_bareword_symbol(&mut self) -> Option<EqNode> {
        // Sorted longest-first so we match "epsilon" before "eta", "theta" before "the", etc.
        static BAREWORDS: &[&str] = &[
            "varepsilon", "rightarrow", "leftarrow", "Rightarrow", "Leftarrow",
            "epsilon", "upsilon", "omicron", "lambda", "Lambda", "implies",
            "partial", "emptyset",
            "alpha", "beta", "gamma", "delta", "theta", "kappa", "sigma",
            "omega", "Gamma", "Delta", "Theta", "Sigma", "Omega",
            "infty", "nabla", "forall", "exists", "approx", "equiv",
            "times", "cdot",
            "zeta", "iota", "pi", "rho", "tau", "phi", "chi", "psi",
            "eta", "nu", "xi", "mu", "Pi", "Phi", "Psi",
            "pm",
        ];
        for &word in BAREWORDS {
            if self.try_keyword(word) {
                let symbol = latex_to_unicode(word)
                    .unwrap_or_else(|| word.to_string());
                return Some(symbol_node(symbol));
            }
        }
        None
    }

    fn parse_command(&mut self) -> EqNode {
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
                let content = self.parse_text_block();
                return EqNode::MathFont { kind, content: Box::new(content) };
            }
            "fbox" => {
                let content = self.parse_text_block();
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
            "middle" => return EqNode::Text(self.parse_delimiter()),

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
            "begin" => return self.parse_begin_env(),
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
                let below = if self.peek() == Some('[') { Some(self.parse_bracket_arg()) } else { None };
                let above = self.parse_group_atom();
                let mut node = symbol_node(arrow.to_string());
                if let Some(below) = below {
                    node = EqNode::StackRel { base: Box::new(node), annotation: Box::new(below), over: false };
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

            // Color. \color as a switch is handled in parse_sequence_until_ex;
            // here it only applies to the following atom.
            "textcolor" | "color" => {
                let color = self.parse_brace_arg_opt().unwrap_or_default();
                let content = if name == "textcolor" { self.parse_group_atom() } else { self.parse_atom() };
                return EqNode::Color { color: color.trim().to_string(), content: Box::new(content) };
            }

            // \not\in, \not=, \not\subset, ...
            "not" => {
                let next = self.parse_atom();
                return negate_node(next);
            }

            // Modular arithmetic
            "bmod" => {
                return EqNode::Op { symbol: "mod".to_string(), kind: OpKind::Binary };
            }
            "pmod" => {
                let arg = self.parse_group_atom();
                return EqNode::Seq(vec![
                    EqNode::Space(8.0),
                    EqNode::Text("(".to_string()),
                    EqNode::Limit { name: "mod".to_string(), lower: None },
                    EqNode::Space(6.0),
                    arg,
                    EqNode::Text(")".to_string()),
                ]);
            }
            "mod" => {
                let arg = self.parse_group_atom();
                return EqNode::Seq(vec![
                    EqNode::Space(12.0),
                    EqNode::Limit { name: "mod".to_string(), lower: None },
                    EqNode::Space(6.0),
                    arg,
                ]);
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
            Some(symbol) => symbol_node(symbol),
            None => EqNode::Text(format!("\\{}", name)),
        }
    }

    /// Parse \text{...} — consume braces and return raw text (no math parsing).
    fn parse_text_block(&mut self) -> EqNode {
        EqNode::TextBlock(self.parse_brace_arg_opt().unwrap_or_default())
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
        let content = self.parse_sequence_until_ex(false, true);
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
    fn parse_begin_env(&mut self) -> EqNode {
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
                    join_cells(rows.remove(0))
                } else {
                    EqNode::Gather { rows: rows.into_iter().map(join_cells).collect() }
                }
            }
            _ => {
                // Unknown environment — keep everything as literal text so
                // nothing the user typed disappears.
                let (rows, hlines) = self.parse_env_body(false);
                let mut parts = vec![EqNode::Text(format!("\\begin{{{}}}", env_name))];
                for (i, row) in rows.into_iter().enumerate() {
                    if i > 0 {
                        parts.push(EqNode::Text("\\\\".to_string()));
                    }
                    for _ in hlines.iter().filter(|&&h| h == i) {
                        parts.push(EqNode::Text("\\hline".to_string()));
                    }
                    for (j, cell) in row.into_iter().enumerate() {
                        if j > 0 {
                            parts.push(EqNode::Text("&".to_string()));
                        }
                        parts.push(cell);
                    }
                }
                if self.consume_end_env(&env_name) {
                    parts.push(EqNode::Text(format!("\\end{{{}}}", env_name)));
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
    fn parse_env_body(&mut self, brace_delimited: bool) -> (Vec<Vec<EqNode>>, Vec<usize>) {
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
                row.push(self.parse_sequence_until_ex(false, false));
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

    fn is_text(node: &EqNode, expected: &str) -> bool {
        matches!(node, EqNode::Text(s) if s == expected)
    }

    /// Collect all Text contents from a tree (depth-first), ignoring spaces.
    fn collect_text(node: &EqNode) -> String {
        match node {
            EqNode::Text(s) => s.clone(),
            EqNode::Op { symbol, .. } => symbol.clone(),
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
        assert!(matches!(node, EqNode::Sup(_, _)));
        assert_eq!(collect_text(&node), "x^2");
    }

    #[test]
    fn parse_subscript() {
        let node = parse_equation("x_1");
        assert!(matches!(node, EqNode::Sub(_, _)));
        assert_eq!(collect_text(&node), "x_1");
    }

    #[test]
    fn parse_combined_sup_sub() {
        let node = parse_equation("x^2_3");
        assert!(matches!(node, EqNode::SupSub(_, _, _)));
        assert_eq!(collect_text(&node), "x^2_3");
    }

    #[test]
    fn parse_combined_sub_sup_order() {
        // Sub then sup should also produce SupSub
        let node = parse_equation("x_3^2");
        assert!(matches!(node, EqNode::SupSub(_, _, _)));
    }

    #[test]
    fn parse_braced_superscript() {
        let node = parse_equation("x^{2n}");
        assert!(matches!(node, EqNode::Sup(_, _)));
        if let EqNode::Sup(_, sup) = &node {
            assert_eq!(collect_text(sup), "2n");
        }
    }

    // ─── Fractions ──────────────────────────────────────────────────

    #[test]
    fn parse_inline_fraction() {
        let node = parse_equation("a/b");
        assert!(matches!(node, EqNode::Frac(_, _)));
        assert_eq!(collect_text(&node), "(a)/(b)");
    }

    #[test]
    fn parse_frac_command() {
        let node = parse_equation("\\frac{a}{b}");
        assert!(matches!(node, EqNode::Frac(_, _)));
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
        assert!(matches!(node, EqNode::Sqrt(_)));
        assert_eq!(collect_text(&node), "sqrt(x)");
    }

    #[test]
    fn parse_sqrt_command() {
        let node = parse_equation("\\sqrt{x}");
        assert!(matches!(node, EqNode::Sqrt(_)));
    }

    #[test]
    fn parse_sqrt_no_visible_parens() {
        // sqrt(x^2) — parens are invisible grouping, not in the AST
        let node = parse_equation("sqrt(x^2)");
        assert!(matches!(node, EqNode::Sqrt(_)));
        // The inner content should be Sup(x, 2), not Seq with parens
        if let EqNode::Sqrt(inner) = &node {
            assert!(matches!(**inner, EqNode::Sup(_, _)), "Inner should be Sup, got: {:?}", inner);
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
        assert!(matches!(node, EqNode::BigOp { .. }));
        if let EqNode::BigOp { lower, upper, .. } = &node {
            assert!(lower.is_some());
            assert!(upper.is_some());
        }
    }

    #[test]
    fn parse_int_bareword() {
        let node = parse_equation("int_0^1");
        assert!(matches!(node, EqNode::BigOp { .. }));
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
        assert!(matches!(node, EqNode::Limit { .. }));
        if let EqNode::Limit { name, lower } = &node {
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
        assert!(matches!(node, EqNode::Limit { .. }));
        if let EqNode::Limit { name, lower } = &node {
            assert_eq!(name, "log");
            assert!(lower.is_some());
        }
    }

    // ─── Operator spacing ───────────────────────────────────────────

    fn op(symbol: &str, kind: OpKind) -> EqNode {
        EqNode::Op { symbol: symbol.to_string(), kind }
    }

    #[test]
    fn parse_plus_is_binary_op() {
        assert_eq!(
            parse_equation("a + b"),
            EqNode::Seq(vec![EqNode::Text("a".into()), op("+", OpKind::Binary), EqNode::Text("b".into())])
        );
    }

    #[test]
    fn parse_unary_minus_is_text() {
        assert_eq!(
            parse_equation("-x"),
            EqNode::Seq(vec![EqNode::Text("-".into()), EqNode::Text("x".into())])
        );
        // After a relation, minus is unary too.
        assert_eq!(
            parse_equation("a = -b"),
            EqNode::Seq(vec![
                EqNode::Text("a".into()),
                op("=", OpKind::Relation),
                EqNode::Text("-".into()),
                EqNode::Text("b".into()),
            ])
        );
    }

    #[test]
    fn parse_binary_minus_is_op() {
        assert_eq!(
            parse_equation("a - b"),
            EqNode::Seq(vec![EqNode::Text("a".into()), op("-", OpKind::Binary), EqNode::Text("b".into())])
        );
    }

    // ─── Relation symbols ───────────────────────────────────────────

    #[test]
    fn parse_neq_is_relation() {
        assert_eq!(
            parse_equation("a \\neq b"),
            EqNode::Seq(vec![EqNode::Text("a".into()), op("≠", OpKind::Relation), EqNode::Text("b".into())])
        );
    }

    #[test]
    fn parse_leq_is_relation() {
        assert!(matches!(
            parse_equation("x \\leq y"),
            EqNode::Seq(v) if v[1] == op("≤", OpKind::Relation)
        ));
    }

    #[test]
    fn whitespace_produces_no_nodes() {
        assert_eq!(parse_equation("a  \n b"), parse_equation("ab"));
        assert_eq!(parse_equation("   "), EqNode::Text(String::new()));
    }

    // ─── Environments ───────────────────────────────────────────────

    #[test]
    fn parse_pmatrix() {
        let node = parse_equation("\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}");
        assert!(matches!(node, EqNode::Matrix { .. }));
        if let EqNode::Matrix { kind, rows } = &node {
            assert!(matches!(kind, MatrixKind::Paren));
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].len(), 2);
        }
    }

    #[test]
    fn parse_cases() {
        let node = parse_equation("\\begin{cases} x^2 & x > 0 \\\\ 0 & x = 0 \\end{cases}");
        assert!(matches!(node, EqNode::Cases { .. }));
        if let EqNode::Cases { rows } = &node {
            assert_eq!(rows.len(), 2);
        }
    }

    // ─── Special commands ───────────────────────────────────────────

    #[test]
    fn parse_text_block() {
        let node = parse_equation("\\text{hello world}");
        assert!(matches!(node, EqNode::TextBlock(_)));
        if let EqNode::TextBlock(s) = &node {
            assert_eq!(s, "hello world");
        }
    }

    #[test]
    fn parse_mathbb() {
        let node = parse_equation("\\mathbb{R}");
        assert!(matches!(node, EqNode::MathFont { .. }));
        if let EqNode::MathFont { kind, .. } = &node {
            assert!(matches!(kind, MathFontKind::Blackboard));
        }
    }

    #[test]
    fn parse_binom() {
        let node = parse_equation("\\binom{n}{k}");
        assert!(matches!(node, EqNode::Binom(_, _)));
    }

    #[test]
    fn parse_left_right_delimiters() {
        let node = parse_equation("\\left( x \\right)");
        assert!(matches!(node, EqNode::Delimited { .. }));
    }

    #[test]
    fn parse_overbrace() {
        let node = parse_equation("\\overbrace{a+b}^{n}");
        assert!(matches!(node, EqNode::Brace { over: true, .. }));
    }

    #[test]
    fn parse_overset() {
        let node = parse_equation("\\overset{def}{=}");
        assert!(matches!(node, EqNode::StackRel { over: true, .. }));
    }

    // ─── Spacing commands ───────────────────────────────────────────

    #[test]
    fn parse_quad_spacing() {
        let node = parse_equation("a \\quad b");
        if let EqNode::Seq(nodes) = &node {
            let has_wide_space = nodes.iter().any(|n| matches!(n, EqNode::Space(w) if *w >= 18.0));
            assert!(has_wide_space, "\\quad should produce 18pt space");
        }
    }

    #[test]
    fn parse_thin_space() {
        let node = parse_equation("a\\,b");
        if let EqNode::Seq(nodes) = &node {
            let has_thin_space = nodes.iter().any(|n| matches!(n, EqNode::Space(w) if *w > 0.0 && *w < 5.0));
            assert!(has_thin_space, "\\, should produce thin space");
        }
    }

    // ─── Accents ────────────────────────────────────────────────────

    #[test]
    fn parse_hat_accent() {
        let node = parse_equation("\\hat{x}");
        assert!(matches!(node, EqNode::Accent(_, AccentKind::Hat)));
    }

    #[test]
    fn parse_vec_accent() {
        let node = parse_equation("\\vec{v}");
        assert!(matches!(node, EqNode::Accent(_, AccentKind::Vec)));
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
        assert!(matches!(node, EqNode::Frac(_, _)));
        if let EqNode::Frac(numer, _) = &node {
            assert!(matches!(**numer, EqNode::Frac(_, _)));
        }
    }

    #[test]
    fn parse_nested_superscripts() {
        let node = parse_equation("x^{2^3}");
        assert!(matches!(node, EqNode::Sup(_, _)));
        if let EqNode::Sup(_, sup) = &node {
            assert!(matches!(**sup, EqNode::Sup(_, _)));
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
