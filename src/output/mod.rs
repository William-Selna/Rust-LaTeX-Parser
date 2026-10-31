//! Output formats: turn a [`Node`](crate::Node) tree into LaTeX, MathML,
//! SVG or plain Unicode text.
//!
//! | Function | Cargo feature | Use it for |
//! |----------|---------------|-----------|
//! | `to_latex` | `latex` | Storing canonical LaTeX, interop with KaTeX/MathJax/LaTeX |
//! | `to_mathml` | `mathml` | Rendering in the browser with no JavaScript library |
//! | `to_unicode` | `unicode` | Alt text, plain-text clipboard, search indexing |
//! | `to_svg` | `svg` | Images: anywhere MathML isn't available, exports, thumbnails |
//!
//! All four are enabled by default. Each lives behind its own feature, so a
//! crate that only needs the parser and tree can turn them off with
//! `default-features = false`.

#[cfg(feature = "latex")]
mod latex;
#[cfg(feature = "mathml")]
mod mathml;
#[cfg(feature = "svg")]
mod svg;
#[cfg(feature = "svg")]
mod svg_fonts;
#[cfg(feature = "unicode")]
mod unicode;

#[cfg(feature = "latex")]
pub use latex::to_latex;
#[cfg(feature = "mathml")]
pub use mathml::to_mathml;
#[cfg(feature = "svg")]
pub use svg::{to_svg, SvgFont, SvgOptions};
#[cfg(feature = "unicode")]
pub use unicode::to_unicode;

/// Whether an equation is set inline with text or as a display (block)
/// equation on its own line. Used by `to_mathml` and `to_svg`.
#[cfg(any(feature = "mathml", feature = "svg"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MathDisplay {
    /// Inline with surrounding text: big-operator limits go to the side.
    #[default]
    Inline,
    /// A display equation on its own line: limits go above and below.
    Block,
}
