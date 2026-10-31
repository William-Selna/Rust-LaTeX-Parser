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
#[derive(Debug, Clone)]
pub enum EqNode {
    /// Plain text — a variable name, number, operator character, or Unicode symbol.
    ///
    /// Examples: `"x"`, `"42"`, `"+"`, `"α"`, `"∑"`.
    Text(String),

    /// Horizontal space, measured in points.
    ///
    /// Inserted automatically around binary operators (`+`, `=`, `\leq`, etc.)
    /// and by explicit spacing commands (`\quad`, `\,`, `\;`).
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

    /// An accent mark above a node.
    ///
    /// Produced by `\hat{x}`, `\bar{x}`, `\dot{x}`, `\ddot{x}`, `\tilde{x}`,
    /// or `\vec{v}`. See [`AccentKind`] for the variants.
    Accent(Box<EqNode>, AccentKind),

    /// A named limit-style operator (`lim`, `sin`, `log`, etc.) with an
    /// optional subscript limit.
    ///
    /// These render as upright text (not italic), with the limit below when
    /// present. Produced by `\lim_{x \to 0}`, barewords like `sin`, `cos`,
    /// and custom operators via `\operatorname{rank}`. `\bmod` and `\pmod`
    /// also use a `Limit` named `"mod"`.
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
    /// Produced by `\mathbb{R}`, `\mathbf{v}`, `\mathcal{F}`, etc. Styled
    /// text commands (`\textbf`, `\textsf`, `\texttt`) produce a `MathFont`
    /// wrapping a [`EqNode::TextBlock`].
    /// See [`MathFontKind`] for the supported font families.
    MathFont { kind: MathFontKind, content: Box<EqNode> },

    /// Content wrapped in stretchy delimiters.
    ///
    /// Produced by `\left( ... \right)`. The `left` and `right` strings are
    /// the delimiter characters (e.g. `"("` and `")"`). Command delimiters are
    /// converted to Unicode: `\left\{` gives `"{"`, `\left\langle` gives `"⟨"`,
    /// `\left\|` gives `"‖"`. An invisible delimiter (`\left.` or `\right.`)
    /// produces an empty string.
    Delimited { left: String, right: String, content: Box<EqNode> },

    /// Matrix or matrix-like environment.
    ///
    /// Produced by `\begin{pmatrix}`, `\begin{bmatrix}`, `\begin{smallmatrix}`, etc.
    /// Each inner `Vec<EqNode>` is one row, each `EqNode` in a row is one cell.
    /// See [`MatrixKind`] for the delimiter styles.
    Matrix { kind: MatrixKind, rows: Vec<Vec<EqNode>> },

    /// Piecewise / cases environment.
    ///
    /// Produced by `\begin{cases} ... \end{cases}` (or `dcases`). Each row
    /// is a `(value, optional_condition)` pair. The condition comes after `&`;
    /// any further `&`-separated cells are kept in the condition.
    Cases { rows: Vec<(EqNode, Option<EqNode>)> },

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
    /// (over=false), or `\stackrel{above}{base}` (over=true). Extensible
    /// arrows like `\xrightarrow[below]{above}` are built from nested
    /// `StackRel` nodes around the arrow symbol.
    StackRel { base: Box<EqNode>, annotation: Box<EqNode>, over: bool },
}

/// The kind of accent mark in an [`EqNode::Accent`] node.
#[derive(Debug, Clone, Copy)]
pub enum AccentKind {
    /// `\hat{x}` or `\widehat{xy}` — circumflex above.
    Hat,
    /// `\bar{x}` or `\overline{x}` — horizontal bar above.
    Bar,
    /// `\dot{x}` — single dot above.
    Dot,
    /// `\ddot{x}` — double dot (diaeresis) above.
    DoubleDot,
    /// `\tilde{x}` or `\widetilde{xy}` — tilde above.
    Tilde,
    /// `\vec{v}` or `\overrightarrow{AB}` — right arrow above.
    Vec,
}

/// The font family in a [`EqNode::MathFont`] node.
#[derive(Debug, Clone, Copy)]
pub enum MathFontKind {
    /// `\mathbf{...}` or `\textbf{...}` — bold.
    Bold,
    /// `\mathbb{...}` — double-struck / blackboard bold.
    Blackboard,
    /// `\mathcal{...}` or `\mathscr{...}` — calligraphic / script.
    Calligraphic,
    /// `\mathrm{...}` — upright roman.
    Roman,
    /// `\mathfrak{...}` — Fraktur / blackletter.
    Fraktur,
    /// `\mathsf{...}` or `\textsf{...}` — sans-serif.
    SansSerif,
    /// `\mathtt{...}` or `\texttt{...}` — monospace / typewriter.
    Monospace,
}

/// The delimiter style for a [`EqNode::Matrix`] node.
#[derive(Debug, Clone, Copy)]
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
#[derive(Debug, Clone, Copy)]
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
