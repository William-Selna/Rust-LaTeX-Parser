//! # rust-latex-parser
//!
//! A LaTeX equation parser that produces an abstract syntax tree.
//!
//! Feed it a string of LaTeX math markup, get back an [`EqNode`] tree. No
//! rendering opinions, no font dependencies, no runtime allocation tricks —
//! just parsing.
//!
//! The tree is yours to walk however you want: render to SVG, convert to
//! MathML, draw on a Skia canvas, dump to a terminal. The crate doesn't care.
//!
//! # Quick start
//!
//! ```
//! use rust_latex_parser::{parse_equation, EqNode};
//!
//! let tree = parse_equation("\\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}");
//! assert!(matches!(tree, EqNode::Frac(_, _)));
//! ```
//!
//! # Bareword shortcuts
//!
//! You don't always need backslashes. The parser recognizes common names as
//! barewords, so `pi` works the same as `\pi`, `sqrt(x)` works like
//! `\sqrt{x}`, and `int_0^1` works like `\int_0^1`.
//!
//! Parentheses after bareword operators act as invisible grouping:
//! `sqrt(x+1)` parses the full `x+1` as the argument.
//!
//! # Supported syntax
//!
//! | Category | Examples |
//! |----------|---------|
//! | Superscripts / subscripts | `x^2`, `x_{i+1}`, `x^2_3` |
//! | Fractions | `a/b`, `\frac{a}{b}`, `\dfrac{a}{b}`, `{a \over b}` |
//! | Square roots | `sqrt(x)`, `\sqrt{x}` |
//! | Greek letters | `pi`, `\alpha`, `\Omega` |
//! | Big operators | `\sum_{i=0}^{n}`, `int_0^1`, `\prod`, `\bigcup`, `\sum\limits_{i}` |
//! | Limit operators | `lim_{x \to 0}`, `sin`, `log_2`, `\operatorname{rank}` |
//! | Accents | `\hat{x}`, `\bar{x}`, `\vec{v}`, `\widehat{xy}` |
//! | Matrices | `\begin{pmatrix} a & b \\\\ c & d \end{pmatrix}` |
//! | Cases | `\begin{cases} x & x>0 \\\\ 0 & x=0 \end{cases}` |
//! | Delimiters | `\left( ... \right)`, `\left\{ ... \middle\| ... \right\}` |
//! | Math fonts | `\mathbb{R}`, `\mathcal{F}`, `\mathbf{v}`, `\textbf{...}` |
//! | Binomials | `\binom{n}{k}`, `{n \choose k}` |
//! | Braces | `\overbrace{a+b}^{n}`, `\underbrace{...}_{text}` |
//! | Stacked | `\overset{def}{=}`, `\underset{lim}{=}`, `\xrightarrow[a]{b}` |
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
//! The only things discarded are presentational commands with no visible
//! output, like `\displaystyle`, `\limits` and `\label{...}`.

pub mod ast;
pub mod parser;

pub use ast::{AccentKind, EqMetrics, EqNode, MathFontKind, MatrixKind};
pub use parser::{latex_to_unicode, parse_equation};
