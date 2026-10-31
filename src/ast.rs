//! AST types for the equation tree.
//!
//! The parser returns a [`Node`]: what the node *is* ([`EqNode`], a recursive
//! enum you pattern-match on) and where it came from in the input
//! ([`Span`]). Children are `Node`s too, so every part of the tree knows its
//! source position — an editor can map the cursor to the tree, highlight the
//! part being edited, or underline an unknown command.
//!
//! [`EqMetrics`] is a simple struct for stashing width/ascent/descent
//! measurements if you're building a layout engine.

use std::ops::Range;

/// A range of the input string, as byte offsets: `&input[span.range()]` is
/// the source text of a node.
///
/// Nodes the parser makes up rather than reads (the `3` of a typed `∛`, or
/// the empty text of a missing `\frac` argument) have an empty span at the
/// position where they would be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    /// Byte offset of the first character.
    pub start: usize,
    /// Byte offset just past the last character.
    pub end: usize,
}

impl Span {
    /// A span from `start` to `end` (byte offsets).
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// The span as a `Range`, for slicing the input.
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Whether the span covers no input.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Whether the byte offset `offset` falls inside the span. A cursor at
    /// the very end counts as inside.
    pub fn contains(&self, offset: usize) -> bool {
        self.start <= offset && offset <= self.end
    }

    /// The smallest span covering both.
    pub fn to(&self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

/// A node in the equation tree: what it is, and where it came from.
///
/// Returned by [`crate::parse_equation`]. Match on [`Node::kind`] to see what
/// the node is; its children are `Node`s too.
///
/// Comparing nodes with `==` compares their structure and ignores spans, so
/// trees parsed from `a+b` and `a + b` are equal.
///
/// # Walking the tree
///
/// ```
/// use rust_latex_parser::{parse_equation, EqNode, Node};
///
/// fn count_fractions(node: &Node) -> usize {
///     let here = matches!(node.kind, EqNode::Frac(..)) as usize;
///     here + node.children().into_iter().map(count_fractions).sum::<usize>()
/// }
///
/// let input = "\\frac{\\frac{a}{b}}{c}";
/// let tree = parse_equation(input);
/// assert_eq!(count_fractions(&tree), 2);
///
/// // Every node knows where it came from.
/// let numerator = &tree.children()[0];
/// assert_eq!(&input[numerator.span.range()], "\\frac{a}{b}");
/// ```
#[derive(Debug, Clone)]
pub struct Node {
    /// What the node is.
    pub kind: EqNode,
    /// Where it came from in the input.
    pub span: Span,
}

impl PartialEq for Node {
    /// Structural equality: spans are ignored.
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl From<EqNode> for Node {
    /// A node with an empty span, for building trees by hand.
    fn from(kind: EqNode) -> Self {
        Node { kind, span: Span::default() }
    }
}

impl Node {
    /// A node of the given kind covering `span`.
    pub fn new(kind: EqNode, span: Span) -> Self {
        Node { kind, span }
    }

    /// The direct children, in reading order: a fraction's numerator then
    /// denominator, a script's base then its scripts, a matrix's cells row
    /// by row, and so on.
    pub fn children(&self) -> Vec<&Node> {
        let mut out: Vec<&Node> = Vec::new();
        match &self.kind {
            EqNode::Text(_) | EqNode::Op { .. } | EqNode::Space(_) | EqNode::TextBlock(_)
            | EqNode::Middle(_) | EqNode::SizedDelim { .. } => {}
            EqNode::Seq(v) | EqNode::Gather { rows: v } => out.extend(v),
            EqNode::Sup(a, b) | EqNode::Sub(a, b) | EqNode::Frac(a, b) | EqNode::Binom(a, b) => {
                out.push(a);
                out.push(b);
            }
            EqNode::SupSub(base, sup, sub) => {
                out.push(base);
                out.push(sub);
                out.push(sup);
            }
            EqNode::Sqrt(c)
            | EqNode::Accent(c, _)
            | EqNode::MathFont { content: c, .. }
            | EqNode::Delimited { content: c, .. }
            | EqNode::Color { content: c, .. }
            | EqNode::Enclose { content: c, .. } => out.push(c),
            EqNode::Root { index, content } => {
                out.push(index);
                out.push(content);
            }
            EqNode::BigOp { lower, upper, .. } => {
                out.extend(lower.as_deref());
                out.extend(upper.as_deref());
            }
            EqNode::Limit { lower, .. } => out.extend(lower.as_deref()),
            EqNode::Matrix { rows, .. } | EqNode::Align { rows } | EqNode::Array { rows, .. } => {
                out.extend(rows.iter().flatten())
            }
            EqNode::Cases { rows } => {
                for (value, condition) in rows {
                    out.push(value);
                    out.extend(condition);
                }
            }
            EqNode::Brace { content, label, .. } => {
                out.push(content);
                out.extend(label.as_deref());
            }
            EqNode::StackRel { base, annotation, .. } => {
                out.push(annotation);
                out.push(base);
            }
        }
        out
    }

    /// The innermost node whose span contains the byte offset `offset`, for
    /// mapping a cursor or click position to the tree. Nodes with empty
    /// spans (made up by the parser) are skipped.
    ///
    /// ```
    /// use rust_latex_parser::{parse_equation, EqNode};
    ///
    /// let input = "a + \\sqrt{x}";
    /// let tree = parse_equation(input);
    /// let at = tree.node_at(10).unwrap(); // the `x`
    /// assert_eq!(at.kind, EqNode::Text("x".into()));
    /// ```
    pub fn node_at(&self, offset: usize) -> Option<&Node> {
        if !self.span.contains(offset) {
            return None;
        }
        for child in self.children() {
            if !child.span.is_empty() {
                if let Some(found) = child.node_at(offset) {
                    return Some(found);
                }
            }
        }
        Some(self)
    }
}

/// What a [`Node`] is.
///
/// Every variant is recursive, so a fraction's numerator can contain another
/// fraction, a matrix cell can hold a summation, and so on — there's no
/// depth limit. Children are [`Node`]s, which also carry their source spans.
///
/// The enum is `#[non_exhaustive]`: new LaTeX constructs may add variants in
/// future releases, so include a wildcard arm when matching.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum EqNode {
    /// Plain text — a variable name, number, punctuation, or Unicode symbol.
    ///
    /// Examples: `"x"`, `"42"`, `"("`, `"α"`, `"∞"`. A unary minus (as in
    /// `-x`) is `Text("-")`.
    ///
    /// Commands the parser doesn't recognize, and stray structural tokens,
    /// are kept as text too (`"\\foo"`, `"}"`), so no input is lost.
    Text(String),

    /// A binary operator or relation, such as `+`, `=`, `≤` or `→`.
    ///
    /// The tree records what the symbol *is*; spacing is up to the renderer.
    /// See [`OpKind`] for the classes. Produced by `+ - * = < >`, commands
    /// like `\leq` and `\cup`, typed Unicode symbols like `≤`, and ASCII
    /// shortcuts like `<=` and `->`.
    Op { symbol: String, kind: OpKind },

    /// Explicit horizontal space, in units where `\quad` (1em) is 18.
    ///
    /// Only produced by spacing commands (`\quad`, `\,`, `\;`, `\hspace{..}`),
    /// never for ordinary whitespace in the input, which is insignificant.
    /// Can be negative (e.g. `\!` produces `Space(-3.0)`).
    Space(f32),

    /// A horizontal sequence of nodes. This is the most common container —
    /// any expression with more than one piece gets wrapped in a `Seq`.
    Seq(Vec<Node>),

    /// Base with superscript. Produced by `x^2` or `x^{2n}`.
    ///
    /// Fields: `(base, superscript)`.
    Sup(Box<Node>, Box<Node>),

    /// Base with subscript. Produced by `x_1` or `x_{i+1}`.
    ///
    /// Fields: `(base, subscript)`.
    Sub(Box<Node>, Box<Node>),

    /// Base with both superscript and subscript, stacked vertically to the
    /// right of the base. Produced by `x^2_3` or `x_3^2` (order doesn't matter).
    ///
    /// Fields: `(base, superscript, subscript)`.
    SupSub(Box<Node>, Box<Node>, Box<Node>),

    /// Fraction with numerator over denominator.
    ///
    /// Produced by `\frac{a}{b}` or the shorthand `a/b`.
    ///
    /// Fields: `(numerator, denominator)`.
    Frac(Box<Node>, Box<Node>),

    /// Square root wrapping its contents.
    ///
    /// Produced by `\sqrt{x}` or the bareword `sqrt(x)`.
    Sqrt(Box<Node>),

    /// Root with an explicit index, e.g. a cube root.
    ///
    /// Produced by `\sqrt[3]{x}` or the bareword `sqrt[3](x)`. An empty
    /// index (`\sqrt[]{x}`) produces a plain [`EqNode::Sqrt`] instead.
    Root { index: Box<Node>, content: Box<Node> },

    /// Big operator — summation, integral, product, etc. — with optional
    /// upper and lower limits.
    ///
    /// The `symbol` field is a Unicode character (e.g. `"∑"`, `"∫"`, `"∏"`).
    /// Produced by `\sum_{i=0}^{n}` or the bareword `sum_{i=0}^{n}`.
    BigOp {
        symbol: String,
        lower: Option<Box<Node>>,
        upper: Option<Box<Node>>,
    },

    /// An accent mark above (or, for [`AccentKind::Underline`], below) a node.
    ///
    /// Produced by `\hat{x}`, `\bar{x}`, `\vec{v}`, `\widehat{xy}`,
    /// `\underline{x}`, and friends. See [`AccentKind`] for the variants.
    Accent(Box<Node>, AccentKind),

    /// A named limit-style operator (`lim`, `sin`, `log`, etc.) with an
    /// optional subscript limit.
    ///
    /// These render as upright text (not italic), with the limit below when
    /// present. Produced by `\lim_{x \to 0}`, barewords like `sin`, `cos`,
    /// and custom operators via `\operatorname{rank}`.
    Limit {
        name: String,
        lower: Option<Box<Node>>,
    },

    /// Upright text block within an equation.
    ///
    /// Produced by `\text{hello world}`. The content is not parsed as math —
    /// it's passed through as-is.
    TextBlock(String),

    /// Math font override for a subexpression.
    ///
    /// Produced by `\mathbb{R}`, `\mathbf{v}`, `\mathcal{F}`, `\boldsymbol{x}`,
    /// etc. Styled text commands like `\textbf{...}` produce a `MathFont`
    /// wrapping a [`EqNode::TextBlock`]. See [`MathFontKind`] for the font
    /// families.
    MathFont { kind: MathFontKind, content: Box<Node> },

    /// Content wrapped in stretchy delimiters.
    ///
    /// Produced by `\left( ... \right)`. The `left` and `right` strings are
    /// the delimiter characters (e.g. `"("` and `")"`). Command delimiters are
    /// converted to Unicode: `\left\{` gives `"{"`, `\left\langle` gives `"⟨"`,
    /// `\left\|` gives `"‖"`. An invisible delimiter (`\left.` or `\right.`)
    /// produces an empty string.
    Delimited { left: String, right: String, content: Box<Node> },

    /// A delimiter between `\left` and `\right` that stretches to the same
    /// height, as in set-builder notation `\left\{ x \middle| x > 0 \right\}`.
    ///
    /// Produced by `\middle`. The string is converted the same way as in
    /// [`EqNode::Delimited`].
    Middle(String),

    /// A manually sized delimiter.
    ///
    /// Produced by `\big(`, `\Big[`, `\bigg\{`, `\Bigg|` and their
    /// `l`/`r`/`m` forms (`\bigl(`, `\bigr)`, ...). `size` runs from 1
    /// (`\big`) to 4 (`\Bigg`). `delim` is converted the same way as in
    /// [`EqNode::Delimited`].
    SizedDelim { delim: String, size: u8 },

    /// Matrix or matrix-like environment.
    ///
    /// Produced by `\begin{pmatrix}`, `\begin{bmatrix}`, `\begin{smallmatrix}`, etc.
    /// Each inner `Vec<Node>` is one row, each `Node` in a row is one cell.
    /// See [`MatrixKind`] for the delimiter styles.
    Matrix { kind: MatrixKind, rows: Vec<Vec<Node>> },

    /// Piecewise / cases environment.
    ///
    /// Produced by `\begin{cases} ... \end{cases}` (or `dcases`). Each row
    /// is a `(value, optional_condition)` pair. The condition comes after `&`.
    Cases { rows: Vec<(Node, Option<Node>)> },

    /// Multi-line equations aligned at `&` markers.
    ///
    /// Produced by `align`, `align*`, `aligned`, `split`, `alignat`,
    /// `alignedat`, `flalign` and `eqnarray`. Each row is split into cells
    /// at `&`. Following amsmath, odd-numbered columns (1st, 3rd, ...) are
    /// right-aligned and even-numbered columns are left-aligned.
    Align { rows: Vec<Vec<Node>> },

    /// Multi-line equations, each line centered.
    ///
    /// Produced by `gather`, `gather*` and `gathered`, and by `equation`
    /// when its body contains `\\`.
    Gather { rows: Vec<Node> },

    /// A general array with per-column alignment.
    ///
    /// Produced by `\begin{array}{c|cc} ... \end{array}`, `subarray`, and
    /// `\substack{...}`. `columns` is the parsed column spec. `hlines` lists
    /// where `\hline` rules appear: a value of `i` means a rule above row `i`
    /// (and `rows.len()` means below the last row).
    Array {
        columns: Vec<ArrayColumn>,
        rows: Vec<Vec<Node>>,
        hlines: Vec<usize>,
    },

    /// Binomial coefficient. Rendered as a stacked pair in parentheses.
    ///
    /// Produced by `\binom{n}{k}`. Fields: `(top, bottom)`.
    Binom(Box<Node>, Box<Node>),

    /// Overbrace or underbrace with an optional label.
    ///
    /// Produced by `\overbrace{a+b}^{n}` (over=true) or
    /// `\underbrace{x+y}_{text}` (over=false).
    Brace { content: Box<Node>, label: Option<Box<Node>>, over: bool },

    /// One expression stacked above or below another.
    ///
    /// Produced by `\overset{def}{=}` (over=true), `\underset{lim}{=}`
    /// (over=false), or `\stackrel{above}{base}` (over=true).
    ///
    /// A `StackRel` takes the spacing class of its base, so `\overset{def}{=}`
    /// is spaced like a relation. Extensible arrows like
    /// `\xrightarrow[below]{above}` are built from nested `StackRel` nodes
    /// around an [`EqNode::Op`] arrow.
    StackRel { base: Box<Node>, annotation: Box<Node>, over: bool },

    /// Colored subexpression.
    ///
    /// Produced by `\textcolor{red}{x}` (colors just the argument) or
    /// `\color{red}` (a switch that colors everything after it in the
    /// current group, like in LaTeX). `color` is the raw color spec, e.g.
    /// `"red"` or `"#ff0000"`.
    Color { color: String, content: Box<Node> },

    /// Content with a box or strike-through drawn over it.
    ///
    /// Produced by `\boxed{x}`, `\fbox{text}`, `\cancel{x}`, `\bcancel{x}`
    /// and `\xcancel{x}`. See [`EncloseKind`].
    Enclose { kind: EncloseKind, content: Box<Node> },
}

/// The spacing class of an [`EqNode::Op`].
///
/// Following TeX: binary operators get a medium space on each side,
/// relations a slightly wider one. A binary operator with nothing on its
/// left (like the `-` in `-x`) is parsed as a unary [`EqNode::Text`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OpKind {
    /// `+`, `-`, `×`, `·`, `∪`, `∧`, `⊕`, `\bmod`, ...
    Binary,
    /// `=`, `<`, `≤`, `≠`, `∈`, `⊂`, `→`, `⇒`, `∣`, ...
    Relation,
}

/// The kind of accent mark in an [`EqNode::Accent`] node.
///
/// Wide variants (`\widehat`, `\widetilde`, `\overrightarrow`) map to the
/// same kind as their narrow counterparts; stretch them to the content width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AccentKind {
    /// `\hat{x}` or `\widehat{xy}` — circumflex above.
    Hat,
    /// `\bar{x}` or `\overline{x}` — horizontal bar above.
    Bar,
    /// `\dot{x}` — single dot above.
    Dot,
    /// `\ddot{x}` — double dot (diaeresis) above.
    DoubleDot,
    /// `\dddot{x}` — triple dot above.
    TripleDot,
    /// `\tilde{x}` or `\widetilde{xy}` — tilde above.
    Tilde,
    /// `\vec{v}` or `\overrightarrow{AB}` — right arrow above.
    Vec,
    /// `\overleftarrow{AB}` — left arrow above.
    LeftArrow,
    /// `\overleftrightarrow{AB}` — double-headed arrow above.
    LeftRightArrow,
    /// `\acute{x}` — acute accent above.
    Acute,
    /// `\grave{x}` — grave accent above.
    Grave,
    /// `\breve{x}` — breve above.
    Breve,
    /// `\check{x}` or `\widecheck{x}` — caron above.
    Check,
    /// `\mathring{x}` — ring above.
    Ring,
    /// `\underline{x}` — horizontal line *below* the content.
    Underline,
}

/// The font family in a [`EqNode::MathFont`] node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MathFontKind {
    /// `\mathbf{...}` or `\textbf{...}` — bold.
    Bold,
    /// `\mathbb{...}` — double-struck / blackboard bold.
    Blackboard,
    /// `\mathcal{...}` or `\mathscr{...}` — calligraphic / script.
    Calligraphic,
    /// `\mathrm{...}` — upright roman.
    Roman,
    /// `\mathit{...}` or `\textit{...}` — italic.
    Italic,
    /// `\boldsymbol{...}` or `\bm{...}` — bold italic for letters, bold for
    /// symbols.
    BoldItalic,
    /// `\mathfrak{...}` — Fraktur / blackletter.
    Fraktur,
    /// `\mathsf{...}` or `\textsf{...}` — sans-serif.
    SansSerif,
    /// `\mathtt{...}` or `\texttt{...}` — monospace / typewriter.
    Monospace,
}

/// The delimiter style for a [`EqNode::Matrix`] node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MatrixKind {
    /// `\begin{matrix}` or `\begin{smallmatrix}` — no delimiters.
    Plain,
    /// `\begin{pmatrix}` — parentheses `( )`.
    Paren,
    /// `\begin{bmatrix}` — square brackets `[ ]`.
    Bracket,
    /// `\begin{vmatrix}` — single vertical bars `| |`.
    VBar,
    /// `\begin{Vmatrix}` — double vertical bars `‖ ‖`.
    DoubleVBar,
    /// `\begin{Bmatrix}` — curly braces `{ }`.
    Brace,
}

/// The decoration in an [`EqNode::Enclose`] node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EncloseKind {
    /// `\boxed{...}` or `\fbox{...}` — a frame around the content.
    Box,
    /// `\cancel{...}` — a diagonal strike from bottom-left to top-right.
    Cancel,
    /// `\bcancel{...}` — a diagonal strike from top-left to bottom-right.
    BackCancel,
    /// `\xcancel{...}` — both diagonal strikes (an X).
    XCancel,
}

/// One entry in an [`EqNode::Array`] column spec.
///
/// `p{width}`, `m{width}` and `b{width}` columns become [`ArrayColumn::Left`];
/// `@{...}` and `!{...}` inserts are skipped; `*{n}{spec}` is expanded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ArrayColumn {
    /// `l` — left-aligned column.
    Left,
    /// `c` — centered column.
    Center,
    /// `r` — right-aligned column.
    Right,
    /// `|` — a vertical rule between columns (not a column itself).
    VLine,
}

/// Measured dimensions of a laid-out equation node.
///
/// This struct isn't produced by the parser — it's here for your convenience
/// if you're building a renderer and need somewhere to store measurements
/// during the layout pass.
///
/// All values are in the same unit as your font size (typically points).
///
/// ```text
///          ┬
///          │ ascent (above baseline)
///  ───────────── baseline
///          │ descent (below baseline)
///          ┴
///  ├─────────────┤
///       width
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqMetrics {
    /// Horizontal extent of the node.
    pub width: f32,
    /// Distance from the baseline to the top of the node. Always non-negative.
    pub ascent: f32,
    /// Distance from the baseline to the bottom of the node. Always non-negative.
    pub descent: f32,
}

impl EqMetrics {
    /// Total height: `ascent + descent`.
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}
