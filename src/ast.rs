//! AST types for the equation layout tree.
//!
//! The core type is [`EqNode`] — a recursive enum representing every structural
//! element the parser can produce. You'll pattern-match on it to build your
//! renderer, converter, or whatever else you need.
//!
//! [`EqMetrics`] is a simple struct for stashing width/ascent/descent
//! measurements if you're building a layout engine. It's not used by the
//! parser itself — it's here for your convenience.

/// A node in the equation layout tree.
///
/// Returned by [`crate::parse_equation`]. Every variant is recursive, so a
/// fraction's numerator can contain another fraction, a matrix cell can hold
/// a summation, and so on — there's no depth limit.
///
/// # Walking the tree
///
/// ```
/// use rust_latex_parser::{parse_equation, EqNode};
///
/// fn count_fractions(node: &EqNode) -> usize {
///     match node {
///         EqNode::Frac(n, d) => 1 + count_fractions(n) + count_fractions(d),
///         EqNode::Seq(nodes) => nodes.iter().map(count_fractions).sum(),
///         EqNode::Sup(base, sup) => count_fractions(base) + count_fractions(sup),
///         EqNode::Sub(base, sub) => count_fractions(base) + count_fractions(sub),
///         EqNode::Sqrt(inner) => count_fractions(inner),
///         _ => 0,
///     }
/// }
///
/// let tree = parse_equation("\\frac{\\frac{a}{b}}{c}");
/// assert_eq!(count_fractions(&tree), 2);
/// ```
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
    Seq(Vec<EqNode>),

    /// Base with superscript. Produced by `x^2` or `x^{2n}`.
    ///
    /// Fields: `(base, superscript)`.
    Sup(Box<EqNode>, Box<EqNode>),

    /// Base with subscript. Produced by `x_1` or `x_{i+1}`.
    ///
    /// Fields: `(base, subscript)`.
    Sub(Box<EqNode>, Box<EqNode>),

    /// Base with both superscript and subscript, stacked vertically to the
    /// right of the base. Produced by `x^2_3` or `x_3^2` (order doesn't matter).
    ///
    /// Fields: `(base, superscript, subscript)`.
    SupSub(Box<EqNode>, Box<EqNode>, Box<EqNode>),

    /// Fraction with numerator over denominator.
    ///
    /// Produced by `\frac{a}{b}` or the shorthand `a/b`.
    ///
    /// Fields: `(numerator, denominator)`.
    Frac(Box<EqNode>, Box<EqNode>),

    /// Square root wrapping its contents.
    ///
    /// Produced by `\sqrt{x}` or the bareword `sqrt(x)`.
    Sqrt(Box<EqNode>),

    /// Root with an explicit index, e.g. a cube root.
    ///
    /// Produced by `\sqrt[3]{x}` or the bareword `sqrt[3](x)`. An empty
    /// index (`\sqrt[]{x}`) produces a plain [`EqNode::Sqrt`] instead.
    Root { index: Box<EqNode>, content: Box<EqNode> },

    /// Big operator — summation, integral, product, etc. — with optional
    /// upper and lower limits.
    ///
    /// The `symbol` field is a Unicode character (e.g. `"∑"`, `"∫"`, `"∏"`).
    /// Produced by `\sum_{i=0}^{n}` or the bareword `sum_{i=0}^{n}`.
    BigOp {
        symbol: String,
        lower: Option<Box<EqNode>>,
        upper: Option<Box<EqNode>>,
    },

    /// An accent mark above (or, for [`AccentKind::Underline`], below) a node.
    ///
    /// Produced by `\hat{x}`, `\bar{x}`, `\vec{v}`, `\widehat{xy}`,
    /// `\underline{x}`, and friends. See [`AccentKind`] for the variants.
    Accent(Box<EqNode>, AccentKind),

    /// A named limit-style operator (`lim`, `sin`, `log`, etc.) with an
    /// optional subscript limit.
    ///
    /// These render as upright text (not italic), with the limit below when
    /// present. Produced by `\lim_{x \to 0}`, barewords like `sin`, `cos`,
    /// and custom operators via `\operatorname{rank}`.
    Limit {
        name: String,
        lower: Option<Box<EqNode>>,
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
    MathFont { kind: MathFontKind, content: Box<EqNode> },

    /// Content wrapped in stretchy delimiters.
    ///
    /// Produced by `\left( ... \right)`. The `left` and `right` strings are
    /// the delimiter characters (e.g. `"("` and `")"`). Command delimiters are
    /// converted to Unicode: `\left\{` gives `"{"`, `\left\langle` gives `"⟨"`,
    /// `\left\|` gives `"‖"`. An invisible delimiter (`\left.` or `\right.`)
    /// produces an empty string.
    Delimited { left: String, right: String, content: Box<EqNode> },

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
    /// Each inner `Vec<EqNode>` is one row, each `EqNode` in a row is one cell.
    /// See [`MatrixKind`] for the delimiter styles.
    Matrix { kind: MatrixKind, rows: Vec<Vec<EqNode>> },

    /// Piecewise / cases environment.
    ///
    /// Produced by `\begin{cases} ... \end{cases}` (or `dcases`). Each row
    /// is a `(value, optional_condition)` pair. The condition comes after `&`.
    Cases { rows: Vec<(EqNode, Option<EqNode>)> },

    /// Multi-line equations aligned at `&` markers.
    ///
    /// Produced by `align`, `align*`, `aligned`, `split`, `alignat`,
    /// `alignedat`, `flalign` and `eqnarray`. Each row is split into cells
    /// at `&`. Following amsmath, odd-numbered columns (1st, 3rd, ...) are
    /// right-aligned and even-numbered columns are left-aligned.
    Align { rows: Vec<Vec<EqNode>> },

    /// Multi-line equations, each line centered.
    ///
    /// Produced by `gather`, `gather*` and `gathered`, and by `equation`
    /// when its body contains `\\`.
    Gather { rows: Vec<EqNode> },

    /// A general array with per-column alignment.
    ///
    /// Produced by `\begin{array}{c|cc} ... \end{array}`, `subarray`, and
    /// `\substack{...}`. `columns` is the parsed column spec. `hlines` lists
    /// where `\hline` rules appear: a value of `i` means a rule above row `i`
    /// (and `rows.len()` means below the last row).
    Array {
        columns: Vec<ArrayColumn>,
        rows: Vec<Vec<EqNode>>,
        hlines: Vec<usize>,
    },

    /// Binomial coefficient. Rendered as a stacked pair in parentheses.
    ///
    /// Produced by `\binom{n}{k}`. Fields: `(top, bottom)`.
    Binom(Box<EqNode>, Box<EqNode>),

    /// Overbrace or underbrace with an optional label.
    ///
    /// Produced by `\overbrace{a+b}^{n}` (over=true) or
    /// `\underbrace{x+y}_{text}` (over=false).
    Brace { content: Box<EqNode>, label: Option<Box<EqNode>>, over: bool },

    /// One expression stacked above or below another.
    ///
    /// Produced by `\overset{def}{=}` (over=true), `\underset{lim}{=}`
    /// (over=false), or `\stackrel{above}{base}` (over=true).
    ///
    /// A `StackRel` takes the spacing class of its base, so `\overset{def}{=}`
    /// is spaced like a relation. Extensible arrows like
    /// `\xrightarrow[below]{above}` are built from nested `StackRel` nodes
    /// around an [`EqNode::Op`] arrow.
    StackRel { base: Box<EqNode>, annotation: Box<EqNode>, over: bool },

    /// Colored subexpression.
    ///
    /// Produced by `\textcolor{red}{x}` (colors just the argument) or
    /// `\color{red}` (a switch that colors everything after it in the
    /// current group, like in LaTeX). `color` is the raw color spec, e.g.
    /// `"red"` or `"#ff0000"`.
    Color { color: String, content: Box<EqNode> },

    /// Content with a box or strike-through drawn over it.
    ///
    /// Produced by `\boxed{x}`, `\fbox{text}`, `\cancel{x}`, `\bcancel{x}`
    /// and `\xcancel{x}`. See [`EncloseKind`].
    Enclose { kind: EncloseKind, content: Box<EqNode> },
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
