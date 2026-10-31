//! Output formats: turn an [`EqNode`](crate::EqNode) tree into LaTeX,
//! MathML or plain Unicode text.
//!
//! | Function | Use it for |
//! |----------|-----------|
//! | [`to_latex`] | Storing canonical LaTeX, interop with KaTeX/MathJax/LaTeX |
//! | [`to_mathml`] | Rendering in the browser with no JavaScript library |
//! | [`to_unicode`] | Alt text, plain-text clipboard, search indexing |
//! | [`to_svg`] | Images: anywhere MathML isn't available, exports, thumbnails |

mod latex;
mod mathml;
mod style;
mod svg;
mod unicode;

pub use latex::to_latex;
pub use mathml::{to_mathml, MathDisplay};
pub use svg::{to_svg, SvgOptions};
pub use unicode::to_unicode;

pub(crate) use style::unstyle_char;
