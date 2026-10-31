//! # rust-latex-parser
//!
//! Math input for Rust: parse LaTeX *or* plain notation into a tree of
//! [`Node`]s, each knowing its source [`Span`], then write it back out as
//! LaTeX, MathML, SVG or Unicode text.
//!
//! Built for equation editors in rich-text apps, where input is usually
//! half-finished and the person typing may not know LaTeX. The parser never
//! fails and never drops what the user typed.
//!
//! # Quick start
//!
//! ```
//! use rust_latex_parser::{parse_equation, EqNode};
//!
//! let tree = parse_equation("x = (-b +/- sqrt(b^2 - 4ac))/(2a)");
//!
//! // Walk the tree. Every node knows where it came from.
//! let EqNode::Seq(parts) = &tree.kind else { unreachable!() };
//! assert!(matches!(parts[2].kind, EqNode::Frac(..)));
//! assert_eq!(parts[2].span.range(), 4..33); // "(-b +/- sqrt(b^2 - 4ac))/(2a)"
//! ```
//!
//! Or write it out in another format (each is a cargo feature, all on by
//! default):
//!
//! ```
//! # #[cfg(all(feature = "latex", feature = "mathml", feature = "unicode", feature = "svg"))] {
//! use rust_latex_parser::*;
//!
//! let tree = parse_equation("x = (-b +/- sqrt(b^2 - 4ac))/(2a)");
//!
//! assert_eq!(to_latex(&tree), r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}");
//! assert_eq!(to_unicode(&tree), "x = (-b ± √(b² - 4ac))/2a");
//! assert!(to_mathml(&tree, MathDisplay::Block).starts_with("<math"));
//! assert!(to_svg(&tree, &SvgOptions::default()).starts_with("<svg"));
//! # }
//! ```
//!
//! # Cargo features
//!
//! | Feature | Adds |
//! |---------|------|
//! | `latex` | `to_latex` |
//! | `mathml` | `to_mathml`, `MathDisplay` |
//! | `unicode` | `to_unicode` |
//! | `svg` | `to_svg`, `SvgOptions`, `SvgFont` (also enables `unicode`) |
//!
//! All are on by default. The parser, the tree, source spans and the symbol
//! lookups are always available; for just those, use
//! `default-features = false`.
//!
//! # Plain notation
//!
//! Alongside LaTeX, the parser accepts what people type when they don't know
//! it. Where a shortcut would clash with a plausible LaTeX reading, LaTeX
//! wins (`n!=1` is "n factorial equals 1").
//!
//! | Type | Get |
//! |------|-----|
//! | `pi`, `alpha`, `infinity`, `leq` | `π`, `α`, `∞`, `≤` |
//! | `<=` `>=` `->` `=>` `<=>` `+/-` | `≤` `≥` `→` `⇒` `⇔` `±` |
//! | `x^(n+1)`, `sum_(i=1)^n` | parentheses group the script |
//! | `(a+b)/(c+d)`, `dy/dx`, `sin(x)/x` | fractions of whole operands |
//! | `sqrt(x)`, `root(3, x)`, `abs(x)`, `floor(x)`, `hat(x)`, `binom(n, k)` | function-style shortcuts |
//! | `"text"`, `text(if)` | upright text |
//! | `x²`, `a₁`, `√x`, `½`, `∑`, `≤`, `ℝ` | typed or pasted Unicode |
//!
//! # Supported syntax
//!
//! | Category | Examples |
//! |----------|---------|
//! | Superscripts / subscripts | `x^2`, `x_{i+1}`, `x^2_3` |
//! | Fractions | `a/b`, `\frac{a}{b}`, `\dfrac{a}{b}`, `{a \over b}` |
//! | Roots | `sqrt(x)`, `\sqrt{x}`, `\sqrt[3]{x}` |
//! | Primes | `f'(x)`, `f''` |
//! | Greek letters | `pi`, `\alpha`, `\Omega` |
//! | Big operators | `\sum_{i=0}^{n}`, `int_0^1`, `\prod`, `\bigcup`, `\sum\limits_{\substack{i \\\\ j}}` |
//! | Limit operators | `lim_{x \to 0}`, `sin`, `log_2`, `\operatorname{rank}` |
//! | Accents | `\hat{x}`, `\bar{x}`, `\vec{v}`, `\widehat{xy}`, `\underline{x}` |
//! | Matrices | `\begin{pmatrix} a & b \\\\ c & d \end{pmatrix}` |
//! | Cases | `\begin{cases} x & x>0 \\\\ 0 & x=0 \end{cases}` |
//! | Aligned equations | `\begin{align} a &= b \\\\ &= c \end{align}`, `gather`, `split` |
//! | Arrays | `\begin{array}{c\|c} \hline a & b \end{array}` |
//! | Delimiters | `\left\{ ... \middle\| ... \right\}`, `\bigl( ... \bigr)` |
//! | Math fonts | `\mathbb{R}`, `\mathcal{F}`, `\mathbf{v}`, `\boldsymbol{x}`, `\textbf{...}` |
//! | Binomials | `\binom{n}{k}`, `{n \choose k}` |
//! | Braces | `\overbrace{a+b}^{n}`, `\underbrace{...}_{text}` |
//! | Stacked | `\overset{def}{=}`, `\underset{lim}{=}`, `\xrightarrow[a]{b}` |
//! | Decorations | `\boxed{x}`, `\cancel{x}`, `\textcolor{red}{x}`, `\color{red}` |
//! | Negation | `\not\in`, `\not=` |
//! | Spacing | `\,`, `\quad`, `\hspace{1em}`, `\mkern3mu` |
//! | 270+ symbols | `\pm`, `\leq`, `\in`, `\rightarrow`, `\infty`, `\mid`, ... |
//!
//! # Error handling
//!
//! The parser never fails. Malformed input produces a best-effort tree, and
//! no input is silently dropped: unknown commands and environments, and stray
//! `}`, `&`, `\\` or `\end{...}`, become literal text nodes and parsing
//! continues after them. This is intentional — it keeps live-preview editors
//! responsive while the user is still typing.
//!
//! The only things discarded are those LaTeX itself treats as invisible:
//! `%` comments and presentational commands like `\displaystyle`,
//! `\limits` and `\label{...}`.
//!
//! # Output
//!
//! See the [`output`] module: `to_latex` for canonical LaTeX, `to_mathml`
//! for native browser rendering, `to_svg` for images, and `to_unicode` for
//! plain text.

pub mod ast;
pub mod output;
pub mod parser;
mod style;
mod symbols;

pub use ast::{
    AccentKind, ArrayColumn, EncloseKind, EqMetrics, EqNode, MathFontKind, MatrixKind, Node,
    OpKind, Span,
};
#[cfg(feature = "latex")]
pub use output::to_latex;
#[cfg(feature = "mathml")]
pub use output::to_mathml;
#[cfg(any(feature = "mathml", feature = "svg"))]
pub use output::MathDisplay;
#[cfg(feature = "svg")]
pub use output::{to_svg, SvgOptions};
#[cfg(feature = "unicode")]
pub use output::to_unicode;
pub use parser::parse_equation;
pub use symbols::{latex_to_unicode, op_kind, symbols, unicode_to_latex};
