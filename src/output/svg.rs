//! Render a tree to SVG with a small TeX-style box layout engine.
//!
//! Glyph widths come from built-in metric tables for a choice of fonts
//! (see [`SvgFont`]), so no font files are needed at render time.
//! Anything that has to stretch or scale — brackets, radicals, fraction
//! bars, big operators, accents, braces — is drawn as vector paths, so it
//! looks right whichever font the viewer ends up using for the letters.
//!
//! All internal lengths are in ems with y pointing *up* from the baseline;
//! they are converted to pixels when the SVG is written.

use super::svg_fonts::{self, FontMetrics};
use super::unicode::to_unicode;
use super::MathDisplay;
use crate::style::styled_char;
use crate::ast::*;

/// Options for [`to_svg`].
#[derive(Debug, Clone)]
pub struct SvgOptions {
    /// Font size in pixels. Default 20.
    pub font_size: f32,
    /// Inline (compact, limits beside big operators) or block (display) style.
    pub display: MathDisplay,
    /// Fill and stroke color. Default `currentColor`, so the equation
    /// takes the surrounding text color when embedded in HTML.
    pub color: String,
    /// The font the layout is measured for. The SVG names this font, so it
    /// should be available to whatever displays the SVG (installed, or
    /// loaded as a web font) for the spacing to come out exactly right.
    /// Default [`SvgFont::Times`], which every platform has.
    pub font: SvgFont,
    /// Override the CSS `font-family` written into the SVG. By default it's
    /// [`SvgFont::css_family`] for `font`. The layout still uses `font`'s
    /// widths.
    pub font_family: Option<String>,
    /// Add `data-span="start-end"` (byte offsets into the input) to every
    /// glyph, so an editor can map clicks back to the source.
    pub span_attributes: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        SvgOptions {
            font_size: 20.0,
            display: MathDisplay::Inline,
            color: "currentColor".to_string(),
            font: SvgFont::Times,
            font_family: None,
            span_attributes: false,
        }
    }
}

/// A font the SVG layout has width tables for.
///
/// The layout needs to know how wide each glyph is to place things; these
/// tables were measured from each font. Pick the one your page or app
/// displays the SVG with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SvgFont {
    /// Times New Roman, or a metric-compatible clone (Liberation Serif,
    /// Nimbus Roman, TeX Gyre Termes). Available almost everywhere.
    #[default]
    Times,
    /// STIX Two Text: a Times-like design made for scientific publishing.
    StixTwo,
    /// Computer Modern (the CMU Serif or Latin Modern Roman fonts): the
    /// classic LaTeX look.
    ComputerModern,
    /// Noto Serif: the default serif on Android and many Linux systems.
    NotoSerif,
    /// DejaVu Serif: a common Linux default.
    DejaVuSerif,
    /// Libertinus Serif: popular in academic publishing.
    Libertinus,
}

impl SvgFont {
    /// The CSS `font-family` value written into the SVG for this font,
    /// with sensible fallbacks.
    pub fn css_family(&self) -> &'static str {
        match self {
            SvgFont::Times => "'Times New Roman',Times,'Liberation Serif','Nimbus Roman',serif",
            SvgFont::StixTwo => "'STIX Two Text','STIX Two Math',serif",
            SvgFont::ComputerModern => "'CMU Serif','Latin Modern Roman','Computer Modern',serif",
            SvgFont::NotoSerif => "'Noto Serif',serif",
            SvgFont::DejaVuSerif => "'DejaVu Serif',serif",
            SvgFont::Libertinus => "'Libertinus Serif','Linux Libertine',serif",
        }
    }

    fn metrics(&self) -> &'static FontMetrics {
        match self {
            SvgFont::Times => &svg_fonts::TIMES,
            SvgFont::StixTwo => &svg_fonts::STIX_TWO,
            SvgFont::ComputerModern => &svg_fonts::COMPUTER_MODERN,
            SvgFont::NotoSerif => &svg_fonts::NOTO_SERIF,
            SvgFont::DejaVuSerif => &svg_fonts::DEJAVU_SERIF,
            SvgFont::Libertinus => &svg_fonts::LIBERTINUS,
        }
    }
}

/// Render a tree as a standalone SVG image.
///
/// The SVG has explicit pixel `width`/`height`, a `vertical-align` style so
/// it sits on the text baseline when placed inline in HTML, and a `<title>`
/// with the plain-text form of the equation for screen readers. Commands
/// the parser didn't recognize are drawn in red.
///
/// ```
/// use rust_latex_parser::{parse_equation, to_svg, SvgOptions};
///
/// let svg = to_svg(&parse_equation(r"\frac{a}{b}"), &SvgOptions::default());
/// assert!(svg.starts_with("<svg"));
/// assert!(svg.contains("<title>a/b</title>"));
/// ```
pub fn to_svg(node: &Node, options: &SvgOptions) -> String {
    let style = Style {
        scale: 1.0,
        display: options.display == MathDisplay::Block,
        font: None,
        color: None,
        middle_owner: 0,
    };
    let mut engine = Engine { next_owner: 1, metrics: options.font.metrics() };
    let mut b = engine.layout(node, &style);
    engine.resolve_stray_middles(&mut b, &style);
    let fs = options.font_size;
    let pad = 0.1;
    let width = (b.w + 2.0 * pad) * fs;
    let height = (b.asc + b.desc + 2.0 * pad) * fs;
    let px = |x: f32| fmt((x + pad) * fs);
    let py = |y: f32| fmt((b.asc + pad - y) * fs);

    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" role=\"img\" style=\"vertical-align:{va}px\">",
        w = fmt(width),
        h = fmt(height),
        va = fmt(-(b.desc + pad) * fs),
    ));
    out.push_str(&format!("<title>{}</title>", escape(&to_unicode(node))));
    out.push_str(&format!(
        "<g fill=\"{c}\" stroke=\"{c}\" stroke-width=\"0\" font-family=\"{f}\">",
        c = escape(&options.color),
        f = escape(options.font_family.as_deref().unwrap_or(options.font.css_family()))
    ));
    for item in &b.items {
        match item {
            Item::Glyph { x, y, chars, scale, italic, bold, family, color, span, error } => {
                // Spaces are left out: SVG would collapse them and shift the
                // explicit positions; their width is already in the layout.
                let visible: Vec<&(char, f32)> = chars.iter().filter(|(c, _)| !c.is_whitespace()).collect();
                if visible.is_empty() {
                    continue;
                }
                let xs: Vec<String> = visible.iter().map(|(_, dx)| px(x + dx)).collect();
                let text: String = visible.iter().map(|(c, _)| *c).collect();
                let mut attrs = format!("x=\"{}\" y=\"{}\" font-size=\"{}\"", xs.join(" "), py(*y), fmt(scale * fs));
                if *italic {
                    attrs.push_str(" font-style=\"italic\"");
                }
                if *bold {
                    attrs.push_str(" font-weight=\"bold\"");
                }
                if let Some(f) = family {
                    attrs.push_str(&format!(" font-family=\"{}\"", f));
                }
                let fill = if *error { Some("#c00") } else { color.as_deref() };
                if let Some(c) = fill {
                    attrs.push_str(&format!(" fill=\"{}\"", escape(c)));
                }
                if options.span_attributes && !span.is_empty() {
                    attrs.push_str(&format!(" data-span=\"{}-{}\"", span.start, span.end));
                }
                out.push_str(&format!("<text {}>{}</text>", attrs, escape(&text)));
            }
            Item::Rect { x, y, w, h, color } => {
                out.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
                    px(*x),
                    py(y + h),
                    fmt(w * fs),
                    fmt(h * fs),
                    color.as_deref().map(|c| format!(" fill=\"{}\"", escape(c))).unwrap_or_default()
                ));
            }
            Item::Path { x, y, cmds, stroke, color } => {
                let mut d = String::new();
                for cmd in cmds {
                    match cmd {
                        Cmd::M(a, b) => d.push_str(&format!("M{} {}", px(x + a), py(y + b))),
                        Cmd::L(a, b) => d.push_str(&format!("L{} {}", px(x + a), py(y + b))),
                        Cmd::Q(a, b, c, e) => {
                            d.push_str(&format!("Q{} {} {} {}", px(x + a), py(y + b), px(x + c), py(y + e)))
                        }
                        Cmd::C(a, b, c, e, f, g) => d.push_str(&format!(
                            "C{} {} {} {} {} {}",
                            px(x + a),
                            py(y + b),
                            px(x + c),
                            py(y + e),
                            px(x + f),
                            py(y + g)
                        )),
                        Cmd::Z => d.push('Z'),
                    }
                }
                let paint = match stroke {
                    Some(sw) => format!(
                        " fill=\"none\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"",
                        fmt(sw * fs)
                    ),
                    None => String::new(),
                };
                let col = color.as_deref().map(|c| format!(" fill=\"{0}\" stroke=\"{0}\"", escape(c))).unwrap_or_default();
                let col = if stroke.is_some() {
                    color.as_deref().map(|c| format!(" stroke=\"{}\"", escape(c))).unwrap_or_default()
                } else {
                    col
                };
                out.push_str(&format!("<path d=\"{}\"{}{}/>", d, paint, col));
            }
            Item::Circle { cx, cy, r, stroke, color } => {
                let paint = match stroke {
                    Some(sw) => format!(" fill=\"none\" stroke-width=\"{}\"", fmt(sw * fs)),
                    None => String::new(),
                };
                let col = color.as_deref().map(|c| format!(" fill=\"{0}\" stroke=\"{0}\"", escape(c))).unwrap_or_default();
                let col = if stroke.is_some() {
                    color.as_deref().map(|c| format!(" stroke=\"{}\"", escape(c))).unwrap_or_default()
                } else {
                    col
                };
                out.push_str(&format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{}{}/>", px(*cx), py(*cy), fmt(r * fs), paint, col));
            }
            Item::PendingMiddle { .. } => {}
        }
    }
    out.push_str("</g></svg>");
    out
}

fn fmt(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".to_string() } else { s.to_string() }
}

fn escape(s: &str) -> String {
    super::xml_escape(s)
}

// ─── Layout primitives ──────────────────────────────────────────────

/// Height of the math axis (centre of fraction bars, `+`, `=`) above the baseline.
const AXIS: f32 = 0.25;
/// Default rule thickness (fraction bars, radical overlines).
const RULE: f32 = 0.045;

#[derive(Clone)]
struct Style {
    /// Size relative to the base font: 1.0, 0.7 (script), 0.5 (scriptscript).
    scale: f32,
    /// Display style (big limits, larger operators).
    display: bool,
    font: Option<MathFontKind>,
    color: Option<String>,
    /// Which `\left...\right` a `\middle` belongs to.
    middle_owner: usize,
}

impl Style {
    fn script(&self) -> Style {
        Style { scale: if self.scale > 0.9 { 0.7 } else { 0.5 }, display: false, ..self.clone() }
    }
    fn text(&self) -> Style {
        Style { display: false, ..self.clone() }
    }
    /// Numerator/denominator style.
    fn frac(&self) -> Style {
        if self.display { self.text() } else { self.script() }
    }
    fn is_script(&self) -> bool {
        self.scale < 0.9
    }
}

#[derive(Clone, Debug)]
enum Cmd {
    M(f32, f32),
    L(f32, f32),
    Q(f32, f32, f32, f32),
    C(f32, f32, f32, f32, f32, f32),
    Z,
}

#[derive(Clone, Debug)]
enum Item {
    Glyph {
        x: f32,
        y: f32,
        chars: Vec<(char, f32)>,
        scale: f32,
        italic: bool,
        bold: bool,
        family: Option<&'static str>,
        color: Option<String>,
        span: Span,
        error: bool,
    },
    Rect { x: f32, y: f32, w: f32, h: f32, color: Option<String> },
    /// A path in coordinates relative to `(x, y)`; `stroke` is the line
    /// width, or `None` to fill.
    Path { x: f32, y: f32, cmds: Vec<Cmd>, stroke: Option<f32>, color: Option<String> },
    Circle { cx: f32, cy: f32, r: f32, stroke: Option<f32>, color: Option<String> },
    /// A `\middle` delimiter waiting for its `\left...\right` to know the height.
    PendingMiddle { x: f32, delim: String, owner: usize, scale: f32, color: Option<String> },
}

impl Item {
    fn shift(&mut self, dx: f32, dy: f32) {
        match self {
            Item::Glyph { x, y, .. } | Item::Rect { x, y, .. } | Item::Path { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Item::Circle { cx, cy, .. } => {
                *cx += dx;
                *cy += dy;
            }
            Item::PendingMiddle { x, .. } => *x += dx,
        }
    }
}

/// A laid-out box: width, height above the baseline, depth below it.
#[derive(Clone, Debug, Default)]
struct LBox {
    w: f32,
    asc: f32,
    desc: f32,
    items: Vec<Item>,
}

impl LBox {
    fn empty() -> LBox {
        LBox::default()
    }
    fn space(w: f32) -> LBox {
        LBox { w, ..LBox::default() }
    }
    /// Place another box with its origin at `(dx, dy)` relative to this one,
    /// growing this box's extents to fit.
    fn place(&mut self, mut other: LBox, dx: f32, dy: f32) {
        for item in &mut other.items {
            item.shift(dx, dy);
        }
        self.items.append(&mut other.items);
        self.asc = self.asc.max(other.asc + dy);
        self.desc = self.desc.max(other.desc - dy);
        self.w = self.w.max(dx + other.w);
    }
    /// Append a box to the right.
    fn push(&mut self, other: LBox) {
        let x = self.w;
        self.place(other, x, 0.0);
    }
    fn add(&mut self, item: Item) {
        self.items.push(item);
    }
}

// ─── Glyph metrics ──────────────────────────────────────────────────

/// The advance width of a character in ems: from the font's table when it
/// has the glyph, otherwise a generic estimate (the viewer will fall back to
/// another font for it anyway).
fn char_width(metrics: &FontMetrics, c: char, italic: bool, math: bool) -> f32 {
    let base = metrics
        .width(c, italic)
        .or_else(|| metrics.width(c, !italic))
        .unwrap_or_else(|| fallback_width(c));
    // Math italic letters are a little wider than text italic, and a few
    // overhang to the right (TeX's italic correction).
    let extra = match c {
        _ if !math || !italic => 0.0,
        'f' => 0.14,
        'j' | 'V' | 'W' | 'Y' | 'T' | 'F' | 'P' => 0.08,
        _ if c.is_ascii_alphabetic() => 0.04,
        _ => 0.0,
    };
    base + extra
}

/// Width estimate for glyphs a font's table doesn't cover.
fn fallback_width(c: char) -> f32 {
    match c {
        'α'..='ω' => 0.55,
        'Α'..='Ω' => 0.68,
        '\u{2032}'..='\u{2034}' => 0.28,
        '\u{200B}' => 0.0,
        '·' | '∙' => 0.25,
        '∞' => 0.75,
        '∂' => 0.5,
        '∇' => 0.75,
        '−' | '±' | '∓' | '×' | '÷' | '∗' | '≤' | '≥' | '≠' | '≈' | '≡' | '∼' | '≃' | '≅' | '∝' => 0.6,
        '∈' | '∉' | '∋' | '⊂' | '⊃' | '⊆' | '⊇' | '∪' | '∩' | '∧' | '∨' => 0.6,
        '→' | '←' | '↔' | '⇒' | '⇐' | '⇔' | '↦' | '↪' | '↩' | '⇌' => 0.95,
        '⟶' | '⟵' | '⟷' | '⟹' | '⟸' | '⟺' | '⟼' => 1.5,
        '…' | '⋯' => 0.9,
        '|' | '∣' | '‖' => 0.25,
        '\u{2100}'..='\u{214F}' => 0.72,
        _ if (c as u32) >= 0x2E80 => 1.0,
        _ if (c as u32) >= 0x1D400 => 0.72,
        _ if c.is_ascii() => 0.5,
        _ => 0.6,
    }
}

/// Height above and depth below the baseline of a character, in ems.
fn char_extent(c: char) -> (f32, f32) {
    match c {
        '0'..='9' => (0.68, 0.0),
        'A'..='Z' => (0.66, 0.0),
        'g' | 'p' | 'q' | 'y' => (0.45, 0.22),
        'j' => (0.68, 0.22),
        'f' => (0.68, 0.2),
        'b' | 'd' | 'h' | 'k' | 'l' | 'i' => (0.68, 0.0),
        't' => (0.58, 0.0),
        'a'..='z' => (0.45, 0.0),
        '(' | ')' | '[' | ']' | '{' | '}' | '|' | '‖' | '⟨' | '⟩' | '⌊' | '⌋' | '⌈' | '⌉' | '/' | '\\' => (0.75, 0.25),
        'β' | 'δ' | 'ζ' | 'θ' | 'λ' | 'ξ' | 'ϑ' => (0.7, if matches!(c, 'β' | 'ζ' | 'ξ') { 0.22 } else { 0.0 }),
        'γ' | 'η' | 'μ' | 'ρ' | 'φ' | 'χ' | 'ψ' | 'ϕ' | 'ϱ' => (0.45, 0.22),
        'α'..='ω' => (0.45, 0.0),
        'Α'..='Ω' => (0.68, 0.0),
        '+' | '−' | '-' | '=' | '<' | '>' | '±' | '∓' | '×' | '÷' | '·' | '∗' | '≤' | '≥' | '≠' | '≈' | '≡'
        | '∼' | '→' | '←' | '↔' | '⇒' | '⇐' | '⇔' => (0.55, 0.05),
        ',' | ';' => (0.1, 0.2),
        '.' | ':' => (0.1, 0.0),
        '\u{2032}'..='\u{2034}' => (0.75, 0.0),
        _ => (0.7, 0.12),
    }
}

/// Whether a character is drawn in italic in math (Latin letters and
/// lowercase Greek, following TeX).
fn is_math_italic(c: char) -> bool {
    c.is_ascii_alphabetic() || ('α'..='ω').contains(&c) || matches!(c, 'ϑ' | 'ϕ' | 'ϖ' | 'ϱ' | 'ϵ' | 'ℓ')
}

// ─── Spacing classes ────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Ord,
    Op,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
}

fn class_of(node: &Node) -> Class {
    match &node.kind {
        EqNode::Op { kind: OpKind::Binary, .. } => Class::Bin,
        EqNode::Op { kind: OpKind::Relation, .. } => Class::Rel,
        EqNode::StackRel { base, .. } => match class_of(base) {
            Class::Bin | Class::Rel => class_of(base),
            _ => Class::Ord,
        },
        EqNode::BigOp { .. } | EqNode::Limit { .. } => Class::Op,
        EqNode::Text(t) => match t.as_str() {
            "(" | "[" | "{" | "\u{27E8}" | "\u{230A}" | "\u{2308}" => Class::Open,
            ")" | "]" | "}" | "\u{27E9}" | "\u{230B}" | "\u{2309}" | "!" => Class::Close,
            "," | ";" => Class::Punct,
            _ => Class::Ord,
        },
        EqNode::Space(_) => Class::Ord,
        _ => Class::Ord,
    }
}

/// Space (in mu, 18 per em) between two adjacent atoms, as in TeX.
fn space_between(left: Class, right: Class, script: bool) -> f32 {
    use Class::*;
    let (thin, med, thick) = (3.0, 4.0, 5.0);
    let s = match (left, right) {
        (Op, Ord) | (Ord, Op) | (Op, Op) | (Close, Op) => thin,
        (Bin, _) | (_, Bin) => {
            if script {
                0.0
            } else {
                med
            }
        }
        (Rel, Rel) => 0.0,
        (Rel, _) | (_, Rel) => {
            if script {
                0.0
            } else {
                thick
            }
        }
        (Punct, _) => {
            if script {
                0.0
            } else {
                thin
            }
        }
        _ => 0.0,
    };
    s / 18.0
}

// ─── The engine ─────────────────────────────────────────────────────

struct Engine {
    /// Counter for `\left...\right` pairs, to match `\middle`s.
    next_owner: usize,
    /// Glyph widths for the chosen font.
    metrics: &'static FontMetrics,
}

const LIMIT_UNDER: &[&str] = &["lim", "liminf", "limsup", "max", "min", "sup", "inf", "det", "gcd", "Pr"];
const INTEGRALS: &str = "∫∬∭⨌∮∯∰";

impl Engine {
    fn layout(&mut self, node: &Node, st: &Style) -> LBox {
        let s = st.scale;
        match &node.kind {
            EqNode::Text(t) => self.text(t, node.span, st),
            EqNode::Op { symbol, .. } => {
                let shown = match symbol.as_str() {
                    "-" => "\u{2212}",
                    "*" => "\u{2217}",
                    other => other,
                };
                if shown == "mod" {
                    return self.upright(shown, node.span, st);
                }
                self.text(shown, node.span, st)
            }
            EqNode::Space(w) => LBox::space(w / 18.0 * s),
            EqNode::Seq(nodes) => self.hlist(nodes, st),
            EqNode::Sup(base, sup) => self.scripts(base, Some(sup), None, st),
            EqNode::Sub(base, sub) => self.scripts(base, None, Some(sub), st),
            EqNode::SupSub(base, sup, sub) => self.scripts(base, Some(sup), Some(sub), st),
            EqNode::Frac(n, d) => self.fraction(n, d, true, st),
            EqNode::Binom(n, d) => {
                let inner = self.fraction(n, d, false, st);
                self.delimit("(", ")", inner, st)
            }
            EqNode::Sqrt(c) => self.radical(None, c, st),
            EqNode::Root { index, content } => self.radical(Some(index), content, st),
            EqNode::BigOp { symbol, lower, upper } => self.big_op(symbol, lower.as_deref(), upper.as_deref(), st),
            EqNode::Limit { name, lower } => {
                let word = self.upright(name, node.span, st);
                match lower {
                    Some(l) if st.display && LIMIT_UNDER.contains(&name.as_str()) => {
                        let low = self.layout(l, &st.script());
                        self.over_under(word, None, Some(low), st)
                    }
                    Some(l) => {
                        let low = self.layout(l, &st.script());
                        self.attach(word, None, Some(low), false, st)
                    }
                    None => word,
                }
            }
            EqNode::Accent(c, kind) => self.accent(c, *kind, st),
            EqNode::TextBlock(t) => self.text_block(t, node.span, st),
            EqNode::MathFont { kind, content } => {
                let inner = Style { font: Some(*kind), ..st.clone() };
                self.layout(content, &inner)
            }
            EqNode::Delimited { left, right, content } => {
                let owner = self.next_owner;
                self.next_owner += 1;
                let inner_style = Style { middle_owner: owner, ..st.clone() };
                let inner = self.layout(content, &inner_style);
                let mut b = self.delimit(left, right, inner, st);
                self.resolve_middles(&mut b, owner, st);
                b
            }
            EqNode::Middle(d) => {
                let w = delim_width(d, 1.0) * s;
                let mut b = LBox { w, asc: 0.75 * s, desc: 0.25 * s, items: vec![] };
                b.add(Item::PendingMiddle {
                    x: 0.0,
                    delim: d.clone(),
                    owner: st.middle_owner,
                    scale: s,
                    color: st.color.clone(),
                });
                // Leave room around the bar like a relation.
                let mut out = LBox::space(0.15 * s);
                out.push(b);
                out.push(LBox::space(0.15 * s));
                out
            }
            EqNode::SizedDelim { delim, size } => {
                let half = [0.6, 0.9, 1.2, 1.5][(*size as usize).clamp(1, 4) - 1] * s;
                self.delim_box(delim, AXIS * s + half, AXIS * s - half, st)
            }
            EqNode::Matrix { kind, rows } => {
                let mut table = self.table(rows, |_| Align::Center, 1.0, 0.0, &st.text(), &[], &[]);
                let (l, r) = match kind {
                    MatrixKind::Plain => ("", ""),
                    MatrixKind::Paren => ("(", ")"),
                    MatrixKind::Bracket => ("[", "]"),
                    MatrixKind::VBar => ("|", "|"),
                    MatrixKind::DoubleVBar => ("\u{2016}", "\u{2016}"),
                    MatrixKind::Brace => ("{", "}"),
                };
                if l.is_empty() {
                    table
                } else {
                    // A little air between the brackets and the entries.
                    let pad = 0.08 * st.scale;
                    let mut padded = LBox::space(pad);
                    padded.push(std::mem::take(&mut table));
                    padded.push(LBox::space(pad));
                    self.delimit(l, r, padded, st)
                }
            }
            EqNode::Cases { rows } => {
                let rows: Vec<Vec<Node>> = rows
                    .iter()
                    .map(|(v, c)| {
                        let mut r = vec![v.clone()];
                        r.extend(c.iter().cloned());
                        r
                    })
                    .collect();
                let table = self.table(&rows, |_| Align::Left, 1.0, 0.0, &st.text(), &[], &[]);
                let mut padded = LBox::space(0.12 * st.scale);
                padded.push(table);
                self.delimit("{", "", padded, st)
            }
            EqNode::Align { rows } => {
                let display = Style { display: true, ..st.clone() };
                self.table(rows, |j| if j % 2 == 0 { Align::Right } else { Align::Left }, 0.0, 2.0, &display, &[], &[])
            }
            EqNode::Gather { rows } => {
                let rows: Vec<Vec<Node>> = rows.iter().map(|r| vec![r.clone()]).collect();
                let display = Style { display: true, ..st.clone() };
                self.table(&rows, |_| Align::Center, 1.0, 0.0, &display, &[], &[])
            }
            EqNode::Array { columns, rows, hlines } => {
                let aligns: Vec<Align> = columns
                    .iter()
                    .filter_map(|c| match c {
                        ArrayColumn::Left => Some(Align::Left),
                        ArrayColumn::Center => Some(Align::Center),
                        ArrayColumn::Right => Some(Align::Right),
                        _ => None,
                    })
                    .collect();
                let mut vlines = Vec::new();
                let mut col = 0;
                for c in columns {
                    match c {
                        ArrayColumn::VLine => vlines.push(col),
                        _ => col += 1,
                    }
                }
                let align = move |j: usize| aligns.get(j).copied().unwrap_or(Align::Center);
                self.table(rows, align, 1.0, 0.0, &st.text(), &vlines, hlines)
            }
            EqNode::Brace { content, label, over } => self.brace(content, label.as_deref(), *over, st),
            EqNode::StackRel { base, annotation, over } => {
                let b = self.layout(base, st);
                let a = self.layout(annotation, &st.script());
                if *over {
                    self.over_under(b, Some(a), None, st)
                } else {
                    self.over_under(b, None, Some(a), st)
                }
            }
            EqNode::Color { color, content } => {
                let inner = Style { color: Some(color.clone()), ..st.clone() };
                self.layout(content, &inner)
            }
            EqNode::Enclose { kind, content } => self.enclose(*kind, content, st),
        }
    }

    // ─── Text ──

    fn text(&mut self, t: &str, span: Span, st: &Style) -> LBox {
        self.text_run(t, span, st, true)
    }

    /// Lay out a run of characters. `math` adds the extra room math italic
    /// letters need; prose in `\text{}` doesn't get it.
    fn text_run(&mut self, t: &str, span: Span, st: &Style, math: bool) -> LBox {
        if t.is_empty() {
            return LBox::empty();
        }
        let error = t.starts_with('\\') && t.chars().count() > 1;
        let font = st.font;
        let mut chars = Vec::new();
        let (mut x, mut asc, mut desc) = (0.0f32, 0.0f32, 0.0f32);
        let mut italic_run = false;
        let mut bold = false;
        let mut family = None;
        for c in t.chars() {
            let c = match c {
                '-' => '\u{2212}',
                '*' => '\u{2217}',
                _ => c,
            };
            let (shown, italic) = match font {
                _ if error => (c, false),
                Some(MathFontKind::Roman) => (c, false),
                Some(MathFontKind::Bold) => {
                    bold = true;
                    (c, false)
                }
                Some(MathFontKind::BoldItalic) => {
                    bold = true;
                    (c, is_math_italic(c))
                }
                Some(MathFontKind::Italic) => (c, c.is_alphanumeric()),
                Some(MathFontKind::SansSerif) => {
                    family = Some("sans-serif");
                    (c, false)
                }
                Some(MathFontKind::Monospace) => {
                    family = Some("monospace");
                    (c, false)
                }
                Some(kind) => (styled_char(kind, c), false),
                None => (c, is_math_italic(c)),
            };
            italic_run |= italic;
            let w = char_width(self.metrics, c, italic, math) * if family == Some("monospace") { 1.2 } else { 1.0 };
            let (a, d) = char_extent(c);
            chars.push((shown, x * st.scale));
            x += w;
            asc = asc.max(a);
            desc = desc.max(d);
        }
        let s = st.scale;
        let mut b = LBox { w: x * s, asc: asc * s, desc: desc * s, items: vec![] };
        b.add(Item::Glyph {
            x: 0.0,
            y: 0.0,
            chars,
            scale: s,
            italic: italic_run && !error,
            bold,
            family,
            color: st.color.clone(),
            span,
            error,
        });
        b
    }

    /// Upright text such as operator names (`sin`, `lim`).
    fn upright(&mut self, t: &str, span: Span, st: &Style) -> LBox {
        let roman = Style { font: Some(MathFontKind::Roman), ..st.clone() };
        self.text(t, span, &roman)
    }

    fn text_block(&mut self, t: &str, span: Span, st: &Style) -> LBox {
        // Text keeps its spaces and is upright unless a text font says otherwise.
        let font = match st.font {
            Some(MathFontKind::Italic) | Some(MathFontKind::BoldItalic) | Some(MathFontKind::Bold)
            | Some(MathFontKind::SansSerif) | Some(MathFontKind::Monospace) => st.font,
            _ => Some(MathFontKind::Roman),
        };
        let inner = Style { font, ..st.clone() };
        let mut b = self.text_run(t, span, &inner, false);
        b.asc = b.asc.max(0.68 * st.scale);
        b
    }

    // ─── Horizontal lists ──

    fn hlist(&mut self, nodes: &[Node], st: &Style) -> LBox {
        let mut out = LBox::empty();
        let mut prev: Option<Class> = None;
        for (i, n) in nodes.iter().enumerate() {
            let mut class = class_of(n);
            // A binary operator with nothing usable on its left or right
            // behaves like an ordinary symbol (TeX's rule).
            if class == Class::Bin {
                let next = nodes.get(i + 1).map(class_of);
                if matches!(prev, None | Some(Class::Bin | Class::Op | Class::Rel | Class::Open | Class::Punct))
                    || matches!(next, None | Some(Class::Rel | Class::Close | Class::Punct))
                {
                    class = Class::Ord;
                }
            }
            if let Some(p) = prev {
                out.push(LBox::space(space_between(p, class, st.is_script()) * st.scale));
            }
            let b = self.layout(n, st);
            out.push(b);
            prev = Some(class);
        }
        out
    }

    // ─── Scripts ──

    fn scripts(&mut self, base: &Node, sup: Option<&Node>, sub: Option<&Node>, st: &Style) -> LBox {
        let base_box = self.layout(base, st);
        let sup_box = sup.map(|n| self.layout(n, &st.script()));
        let sub_box = sub.map(|n| self.layout(n, &st.script()));
        let is_char = matches!(&base.kind, EqNode::Text(t) if t.chars().count() == 1);
        self.attach(base_box, sup_box, sub_box, is_char, st)
    }

    /// Attach scripts to the right of a base.
    fn attach(&mut self, base: LBox, sup: Option<LBox>, sub: Option<LBox>, simple_base: bool, st: &Style) -> LBox {
        let s = st.scale;
        let mut out = LBox::empty();
        let base_asc = base.asc;
        let base_desc = base.desc;
        let x = base.w + 0.03 * s;
        out.push(base);
        let drop = if simple_base { 0.0 } else { 1.0 };
        let mut sup_shift = sup.as_ref().map(|b| {
            let min = if st.display { 0.41 } else { 0.36 } * s;
            min.max(drop * (base_asc - 0.39 * s)).max(b.desc + 0.11 * s)
        });
        let mut sub_shift = sub.as_ref().map(|b| (0.15 * s).max(drop * (base_desc + 0.05 * s)).max(b.asc - 0.35 * s));
        if let (Some(up), Some(down), Some(sp), Some(sb)) = (sup_shift, sub_shift, sup.as_ref(), sub.as_ref()) {
            let gap = (up - sp.desc) - (sb.asc - down);
            let min_gap = 0.16 * s;
            if gap < min_gap {
                sub_shift = Some(down + (min_gap - gap));
            }
            sup_shift = Some(up);
        }
        if let (Some(b), Some(up)) = (sup, sup_shift) {
            out.place(b, x, up);
        }
        if let (Some(b), Some(down)) = (sub, sub_shift) {
            out.place(b, x, -down);
        }
        out.w += 0.04 * s;
        out
    }

    /// Put limits (or annotations) centred above and below a base.
    fn over_under(&mut self, base: LBox, over: Option<LBox>, under: Option<LBox>, st: &Style) -> LBox {
        let s = st.scale;
        let w = base.w.max(over.as_ref().map_or(0.0, |b| b.w)).max(under.as_ref().map_or(0.0, |b| b.w));
        let mut out = LBox::empty();
        let (base_asc, base_desc, base_w) = (base.asc, base.desc, base.w);
        out.place(base, (w - base_w) / 2.0, 0.0);
        let gap = 0.12 * s;
        if let Some(b) = over {
            let y = base_asc + gap + b.desc;
            let bw = b.w;
            out.place(b, (w - bw) / 2.0, y);
        }
        if let Some(b) = under {
            let y = -(base_desc + gap + b.asc);
            let bw = b.w;
            out.place(b, (w - bw) / 2.0, y);
        }
        out.w = w;
        out
    }

    // ─── Fractions and radicals ──

    fn fraction(&mut self, n: &Node, d: &Node, rule: bool, st: &Style) -> LBox {
        let s = st.scale;
        let inner = st.frac();
        let num = self.layout(n, &inner);
        let den = self.layout(d, &inner);
        let t = if rule { RULE * s } else { 0.0 };
        let gap = if st.display { 0.15 } else { 0.08 } * s;
        let axis = AXIS * s;
        let num_shift = (axis + t / 2.0 + gap + num.desc).max(if st.display { 0.68 } else { 0.39 } * s);
        let den_shift = (den.asc + gap + t / 2.0 - axis).max(if st.display { 0.69 } else { 0.35 } * s);
        let pad = 0.12 * s;
        let w = num.w.max(den.w) + 2.0 * pad;
        let mut out = LBox::empty();
        let (nw, dw) = (num.w, den.w);
        out.place(num, (w - nw) / 2.0, num_shift);
        out.place(den, (w - dw) / 2.0, -den_shift);
        if rule {
            out.add(Item::Rect { x: pad * 0.5, y: axis - t / 2.0, w: w - pad, h: t, color: st.color.clone() });
        }
        out.w = w;
        out
    }

    fn radical(&mut self, index: Option<&Node>, content: &Node, st: &Style) -> LBox {
        let s = st.scale;
        let body = self.layout(content, &Style { ..st.clone() });
        let t = RULE * s;
        let clearance = if st.display { 0.15 } else { 0.08 } * s;
        let top = body.asc.max(0.62 * s) + clearance + t;
        let bottom = -(body.desc.max(0.0) + 0.1 * s);
        let height = top - bottom;
        let sign_w = (0.55 + 0.08 * (height / s - 1.0).max(0.0)).min(1.0) * s;
        let idx = index.map(|i| self.layout(i, &Style { scale: if s > 0.6 { 0.5 } else { 0.4 }, display: false, ..st.clone() }));
        let shift = idx.as_ref().map_or(0.0, |b| (b.w - 0.5 * sign_w).max(0.0));
        let mut out = LBox::empty();
        let x0 = shift;
        let cmds = vec![
            Cmd::M(0.0, bottom + 0.45 * height),
            Cmd::L(0.17 * sign_w, bottom + 0.52 * height),
            Cmd::L(0.45 * sign_w, bottom),
            Cmd::L(sign_w, top - t / 2.0),
            Cmd::L(sign_w + body.w + 0.08 * s, top - t / 2.0),
        ];
        out.add(Item::Path { x: x0, y: 0.0, cmds, stroke: Some(t * 1.15), color: st.color.clone() });
        let body_w = body.w;
        out.place(body, x0 + sign_w + 0.04 * s, 0.0);
        if let Some(b) = idx {
            let y = bottom + 0.6 * height + b.desc;
            out.place(b, 0.0, y);
        }
        out.w = x0 + sign_w + body_w + 0.12 * s;
        out.asc = out.asc.max(top + 0.05 * s);
        out.desc = out.desc.max(-bottom);
        out
    }

    // ─── Big operators ──

    fn big_op(&mut self, symbol: &str, lower: Option<&Node>, upper: Option<&Node>, st: &Style) -> LBox {
        let s = st.scale;
        let integral = INTEGRALS.contains(symbol);
        let height = match (integral, st.display) {
            (true, true) => 2.2,
            (true, false) => 1.25,
            (false, true) => 1.55,
            (false, false) => 1.05,
        } * s;
        let op = self.op_shape(symbol, height, st);
        let low = lower.map(|n| self.layout(n, &st.script()));
        let up = upper.map(|n| self.layout(n, &st.script()));
        if st.display && !integral {
            self.over_under(op, up, low, st)
        } else if integral {
            // Limits beside the integral, the lower one tucked in.
            let mut out = LBox::empty();
            let (op_asc, op_desc, op_w) = (op.asc, op.desc, op.w);
            out.push(op);
            if let Some(b) = up {
                let y = op_asc - b.asc * 0.6;
                out.place(b, op_w + 0.02 * s, y);
            }
            if let Some(b) = low {
                let y = -(op_desc - b.desc * 0.4);
                out.place(b, op_w - 0.18 * s * (height / s / 2.2), y);
            }
            out.w += 0.06 * s;
            out
        } else {
            self.attach(op, up, low, false, st)
        }
    }

    /// Draw a big operator symbol centred on the math axis.
    fn op_shape(&mut self, symbol: &str, height: f32, st: &Style) -> LBox {
        let s = st.scale;
        let axis = AXIS * s;
        let (top, bot) = (axis + height / 2.0, axis - height / 2.0);
        let mid = axis;
        let sw = (0.05 + 0.015 * (height / s - 1.0)) * s;
        let color = st.color.clone();
        let path = |x: f32, cmds: Vec<Cmd>| Item::Path { x, y: 0.0, cmds, stroke: Some(sw), color: color.clone() };
        let mut b = LBox { w: 0.0, asc: top, desc: -bot, items: vec![] };
        let first = symbol.chars().next().unwrap_or('∑');
        let count = match first {
            '∬' | '∯' => 2,
            '∭' | '∰' => 3,
            '⨌' => 4,
            _ => 1,
        };
        match first {
            '∫' | '∬' | '∭' | '⨌' | '∮' | '∯' | '∰' => {
                let w = 0.42 * height.powf(0.6) * s.powf(0.4);
                let step = w * 0.55;
                for k in 0..count {
                    let x = k as f32 * step;
                    b.add(path(x, vec![
                        Cmd::M(w * 0.95, top - 0.07 * height),
                        Cmd::C(w * 0.85, top + 0.01 * height, w * 0.64, top, w * 0.58, top - 0.16 * height),
                        Cmd::L(w * 0.42, bot + 0.16 * height),
                        Cmd::C(w * 0.36, bot, w * 0.15, bot - 0.01 * height, w * 0.05, bot + 0.07 * height),
                    ]));
                }
                let total = w + step * (count as f32 - 1.0);
                if matches!(first, '∮' | '∯' | '∰') {
                    b.add(Item::Circle { cx: total / 2.0, cy: mid, r: 0.12 * height.min(1.2 * s), stroke: Some(sw * 0.8), color: color.clone() });
                }
                b.w = total + 0.05 * s;
            }
            '∏' | '∐' => {
                let w = 0.72 * height.powf(0.5) * s.powf(0.5);
                let (a, z) = if first == '∏' { (top, bot) } else { (bot, top) };
                b.add(path(0.0, vec![
                    Cmd::M(0.04 * w, a), Cmd::L(0.96 * w, a),
                    Cmd::M(0.22 * w, a), Cmd::L(0.22 * w, z),
                    Cmd::M(0.78 * w, a), Cmd::L(0.78 * w, z),
                    Cmd::M(0.08 * w, z), Cmd::L(0.36 * w, z),
                    Cmd::M(0.64 * w, z), Cmd::L(0.92 * w, z),
                ]));
                b.w = w + 0.08 * s;
            }
            '⋃' | '⋂' | '⨄' | '⨆' | '⋁' | '⋀' => {
                let w = 0.68 * height.powf(0.5) * s.powf(0.5);
                let cmds = match first {
                    '⋃' | '⨄' => vec![Cmd::M(0.1 * w, top), Cmd::L(0.1 * w, mid), Cmd::C(0.1 * w, bot - 0.05 * height, 0.9 * w, bot - 0.05 * height, 0.9 * w, mid), Cmd::L(0.9 * w, top)],
                    '⋂' => vec![Cmd::M(0.1 * w, bot), Cmd::L(0.1 * w, mid), Cmd::C(0.1 * w, top + 0.05 * height, 0.9 * w, top + 0.05 * height, 0.9 * w, mid), Cmd::L(0.9 * w, bot)],
                    '⨆' => vec![Cmd::M(0.1 * w, top), Cmd::L(0.1 * w, bot), Cmd::L(0.9 * w, bot), Cmd::L(0.9 * w, top)],
                    '⋁' => vec![Cmd::M(0.08 * w, top), Cmd::L(0.5 * w, bot), Cmd::L(0.92 * w, top)],
                    _ => vec![Cmd::M(0.08 * w, bot), Cmd::L(0.5 * w, top), Cmd::L(0.92 * w, bot)],
                };
                b.add(path(0.0, cmds));
                if first == '⨄' {
                    let c = mid - 0.05 * height;
                    b.add(path(0.0, vec![Cmd::M(0.32 * w, c), Cmd::L(0.68 * w, c), Cmd::M(0.5 * w, c - 0.18 * w), Cmd::L(0.5 * w, c + 0.18 * w)]));
                }
                b.w = w + 0.08 * s;
            }
            '⨁' | '⨂' | '⨀' => {
                let r = height * 0.42;
                let cx = r + 0.05 * s;
                b.add(Item::Circle { cx, cy: mid, r, stroke: Some(sw), color: color.clone() });
                match first {
                    '⨁' => b.add(path(0.0, vec![Cmd::M(cx - r, mid), Cmd::L(cx + r, mid), Cmd::M(cx, mid - r), Cmd::L(cx, mid + r)])),
                    '⨂' => {
                        let k = r * 0.707;
                        b.add(path(0.0, vec![Cmd::M(cx - k, mid - k), Cmd::L(cx + k, mid + k), Cmd::M(cx - k, mid + k), Cmd::L(cx + k, mid - k)]))
                    }
                    _ => b.add(Item::Circle { cx, cy: mid, r: sw * 1.2, stroke: None, color: color.clone() }),
                }
                b.w = 2.0 * r + 0.1 * s;
            }
            _ => {
                // ∑ (and anything unrecognized)
                let w = 0.78 * height.powf(0.55) * s.powf(0.45);
                b.add(path(0.0, vec![
                    Cmd::M(0.94 * w, top - 0.12 * height),
                    Cmd::L(0.9 * w, top),
                    Cmd::L(0.06 * w, top),
                    Cmd::L(0.52 * w, mid),
                    Cmd::L(0.06 * w, bot),
                    Cmd::L(0.9 * w, bot),
                    Cmd::L(0.94 * w, bot + 0.12 * height),
                ]));
                b.w = w + 0.06 * s;
            }
        }
        b
    }

    // ─── Delimiters ──

    /// Wrap content in stretchy delimiters sized to it.
    fn delimit(&mut self, left: &str, right: &str, inner: LBox, st: &Style) -> LBox {
        let s = st.scale;
        let axis = AXIS * s;
        let half = (inner.asc - axis).max(inner.desc + axis).max(0.5 * s) + 0.08 * s;
        let (top, bot) = (axis + half, axis - half);
        let pad = 0.06 * s;
        let mut out = LBox::empty();
        if !left.is_empty() {
            out.push(self.delim_box(left, top, bot, st));
            out.push(LBox::space(pad));
        }
        out.push(inner);
        if !right.is_empty() {
            out.push(LBox::space(pad));
            out.push(self.delim_box(right, top, bot, st));
        }
        out
    }

    /// Replace pending `\middle`s owned by `owner` with delimiters as tall
    /// as the box.
    fn resolve_middles(&mut self, b: &mut LBox, owner: usize, st: &Style) {
        let axis = AXIS * st.scale;
        let half = (b.asc - axis).max(b.desc + axis);
        let mut extra = Vec::new();
        b.items.retain(|item| match item {
            Item::PendingMiddle { x, delim, owner: o, .. } if *o == owner => {
                extra.push((*x, delim.clone()));
                false
            }
            _ => true,
        });
        for (x, delim) in extra {
            let mut d = self.delim_box(&delim, axis + half, axis - half, st);
            for item in &mut d.items {
                item.shift(x, 0.0);
            }
            b.items.extend(d.items);
        }
    }

    /// Draw any `\middle` that wasn't inside a `\left...\right` at normal size.
    fn resolve_stray_middles(&mut self, b: &mut LBox, st: &Style) {
        let mut stray = Vec::new();
        b.items.retain(|item| match item {
            Item::PendingMiddle { x, delim, scale, color, .. } => {
                stray.push((*x, delim.clone(), *scale, color.clone()));
                false
            }
            _ => true,
        });
        for (x, delim, scale, color) in stray {
            let s = Style { scale, color, ..st.clone() };
            let mut d = self.delim_box(&delim, (AXIS + 0.5) * scale, (AXIS - 0.5) * scale, &s);
            for item in &mut d.items {
                item.shift(x, 0.0);
            }
            b.items.extend(d.items);
        }
    }

    /// A single delimiter drawn to span `bot..top`.
    fn delim_box(&mut self, d: &str, top: f32, bot: f32, st: &Style) -> LBox {
        let s = st.scale;
        let h = top - bot;
        let w = delim_width(d, h / s) * s;
        let mid = (top + bot) / 2.0;
        let sw = 0.045 * s;
        let color = st.color.clone();
        let stroke = |cmds: Vec<Cmd>| Item::Path { x: 0.0, y: 0.0, cmds, stroke: Some(sw), color: color.clone() };
        let mut b = LBox { w, asc: top, desc: -bot, items: vec![] };
        let flip = |x: f32| w - x;
        match d {
            "(" | ")" => {
                let left = d == "(";
                let fx = |x: f32| if left { x } else { flip(x) };
                let bulge = (0.08 + 0.04 * (h / s)).min(0.3) * s;
                let thick = 0.05 * s * (1.0 + 0.15 * (h / s - 1.0).max(0.0)).min(1.6);
                let end = fx(0.85 * w);
                let ctrl_o = fx(0.18 * w - bulge * 0.6);
                let ctrl_i = fx(0.18 * w - bulge * 0.6 + thick * 2.0);
                b.add(Item::Path {
                    x: 0.0,
                    y: 0.0,
                    cmds: vec![Cmd::M(end, top), Cmd::Q(ctrl_o, mid, end, bot), Cmd::Q(ctrl_i, mid, end, top), Cmd::Z],
                    stroke: None,
                    color: color.clone(),
                });
                b.add(stroke(vec![Cmd::M(end, top), Cmd::Q(ctrl_o, mid, end, bot)]));
            }
            "[" | "]" | "\u{230A}" | "\u{230B}" | "\u{2308}" | "\u{2309}" => {
                let left = matches!(d, "[" | "\u{230A}" | "\u{2308}");
                let fx = |x: f32| if left { x } else { flip(x) };
                let (stem, arm) = (fx(0.3 * w), fx(0.85 * w));
                let has_top = matches!(d, "[" | "]" | "\u{2308}" | "\u{2309}");
                let has_bot = matches!(d, "[" | "]" | "\u{230A}" | "\u{230B}");
                let mut cmds = vec![];
                if has_top {
                    cmds.extend([Cmd::M(arm, top), Cmd::L(stem, top)]);
                } else {
                    cmds.push(Cmd::M(stem, top));
                }
                cmds.push(Cmd::L(stem, bot));
                if has_bot {
                    cmds.push(Cmd::L(arm, bot));
                }
                b.add(stroke(cmds));
            }
            "{" | "}" => {
                let left = d == "{";
                let fx = |x: f32| if left { x } else { flip(x) };
                let (tip, stem, end) = (fx(0.1 * w), fx(0.5 * w), fx(0.9 * w));
                let r = (0.12 * s).min(h / 6.0);
                b.add(stroke(vec![
                    Cmd::M(end, top),
                    Cmd::Q(stem, top, stem, top - r),
                    Cmd::L(stem, mid + r),
                    Cmd::Q(stem, mid, tip, mid),
                    Cmd::Q(stem, mid, stem, mid - r),
                    Cmd::L(stem, bot + r),
                    Cmd::Q(stem, bot, end, bot),
                ]));
            }
            "|" | "\u{2223}" => b.add(stroke(vec![Cmd::M(w / 2.0, top), Cmd::L(w / 2.0, bot)])),
            "\u{2016}" => b.add(stroke(vec![
                Cmd::M(w * 0.3, top),
                Cmd::L(w * 0.3, bot),
                Cmd::M(w * 0.7, top),
                Cmd::L(w * 0.7, bot),
            ])),
            "\u{27E8}" | "\u{27E9}" | "<" | ">" => {
                let left = matches!(d, "\u{27E8}" | "<");
                let fx = |x: f32| if left { x } else { flip(x) };
                b.add(stroke(vec![Cmd::M(fx(0.8 * w), top), Cmd::L(fx(0.2 * w), mid), Cmd::L(fx(0.8 * w), bot)]));
            }
            "/" => b.add(stroke(vec![Cmd::M(0.85 * w, top), Cmd::L(0.15 * w, bot)])),
            "\\" => b.add(stroke(vec![Cmd::M(0.15 * w, top), Cmd::L(0.85 * w, bot)])),
            _ => {
                // Unknown delimiter: draw it as text, scaled to the height.
                let scale = (h / 1.0).max(s);
                let t = self.text(d, Span::default(), &Style { scale, ..st.text() });
                let tw = t.w;
                b.place(t, 0.0, mid - AXIS * scale);
                b.w = tw;
            }
        }
        b
    }

    // ─── Tables ──

    #[allow(clippy::too_many_arguments)]
    fn table(
        &mut self,
        rows: &[Vec<Node>],
        align: impl Fn(usize) -> Align,
        col_gap: f32,
        pair_gap: f32,
        st: &Style,
        vlines: &[usize],
        hlines: &[usize],
    ) -> LBox {
        let s = st.scale;
        let cells: Vec<Vec<LBox>> = rows
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .map(|(j, c)| {
                        if pair_gap > 0.0 && j % 2 == 1 {
                            // Like amsmath's `&=`: the cell continues after an
                            // (empty) left side, so a leading `=` is spaced.
                            let mut v = vec![Node::from(EqNode::Text("\u{200B}".to_string()))];
                            match &c.kind {
                                EqNode::Seq(nodes) => v.extend(nodes.iter().cloned()),
                                _ => v.push(c.clone()),
                            }
                            self.hlist(&v, st)
                        } else {
                            self.layout(c, st)
                        }
                    })
                    .collect()
            })
            .collect();
        let cols = cells.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths = vec![0.0f32; cols];
        for row in &cells {
            for (j, c) in row.iter().enumerate() {
                widths[j] = widths[j].max(c.w);
            }
        }
        // Gaps between columns: `col_gap` everywhere, or for aligned
        // equations no gap inside a right/left pair and `pair_gap` between pairs.
        let gap_after = |j: usize| -> f32 {
            let em = if pair_gap > 0.0 {
                if j % 2 == 1 { pair_gap } else { 0.0 }
            } else {
                col_gap
            };
            em * s
        };
        let outer = if vlines.is_empty() && hlines.is_empty() { 0.0 } else { 0.5 * s };
        let mut xs = Vec::with_capacity(cols);
        let mut x = outer;
        for (j, width) in widths.iter().enumerate() {
            xs.push(x);
            x += width;
            if j + 1 < cols {
                x += gap_after(j);
            }
        }
        let total_w = x + outer;
        // Row heights, with struts so rows don't crowd.
        let strut = (0.7 * s, 0.3 * s);
        let row_gap = if pair_gap > 0.0 || st.display { 0.25 } else { 0.12 } * s;
        let metrics: Vec<(f32, f32)> = cells
            .iter()
            .map(|row| {
                let a = row.iter().map(|c| c.asc).fold(strut.0, f32::max);
                let d = row.iter().map(|c| c.desc).fold(strut.1, f32::max);
                (a, d)
            })
            .collect();
        let total_h: f32 = metrics.iter().map(|(a, d)| a + d).sum::<f32>() + row_gap * (metrics.len().saturating_sub(1)) as f32;
        let axis = AXIS * s;
        let top = axis + total_h / 2.0;
        let mut out = LBox { w: total_w, asc: top, desc: total_h - top, items: vec![] };
        let mut y = top;
        let mut row_tops = Vec::new();
        for (i, row) in cells.into_iter().enumerate() {
            let (a, d) = metrics[i];
            row_tops.push(y + if i == 0 { 0.0 } else { row_gap / 2.0 });
            let baseline = y - a;
            for (j, c) in row.into_iter().enumerate() {
                let cw = c.w;
                let dx = match align(j) {
                    Align::Left => 0.0,
                    Align::Center => (widths[j] - cw) / 2.0,
                    Align::Right => widths[j] - cw,
                };
                out.place(c, xs[j] + dx, baseline);
            }
            y = baseline - d - row_gap;
        }
        let bottom = y + row_gap;
        let t = RULE * s * 0.8;
        for &i in hlines {
            let yy = if i < row_tops.len() { row_tops[i] } else { bottom };
            out.add(Item::Rect { x: 0.0, y: yy - t / 2.0, w: total_w, h: t, color: st.color.clone() });
        }
        for &j in vlines {
            let xx = if j == 0 {
                0.0
            } else if j >= cols {
                total_w
            } else {
                xs[j] - gap_after(j - 1) / 2.0
            };
            out.add(Item::Rect { x: xx - t / 2.0, y: bottom, w: t, h: top - bottom, color: st.color.clone() });
        }
        out.asc = out.asc.max(top);
        out.desc = out.desc.max(-bottom);
        out
    }

    // ─── Accents, braces, boxes ──

    fn accent(&mut self, content: &Node, kind: AccentKind, st: &Style) -> LBox {
        let s = st.scale;
        let body = self.layout(content, st);
        let single = matches!(&content.kind, EqNode::Text(t) if t.chars().count() == 1);
        let italic = matches!(&content.kind, EqNode::Text(t) if t.chars().all(is_math_italic)) && st.font.is_none();
        let skew = if single && italic { 0.08 * s } else { 0.0 };
        let aw = if single { (0.32 * s).min(body.w.max(0.2 * s)) } else { body.w };
        let x0 = (body.w - aw) / 2.0 + skew;
        let y0 = body.asc.max(0.45 * s) + 0.07 * s;
        let c = x0 + aw / 2.0;
        let sw = 0.04 * s;
        let color = st.color.clone();
        let stroke = |cmds: Vec<Cmd>| Item::Path { x: 0.0, y: 0.0, cmds, stroke: Some(sw), color: color.clone() };
        let dot = |cx: f32| Item::Circle { cx, cy: y0 + 0.06 * s, r: 0.045 * s, stroke: None, color: color.clone() };
        let (body_asc, body_desc, body_w) = (body.asc, body.desc, body.w);
        let mut out = LBox::empty();
        out.push(body);
        let mark_h = 0.14 * s;
        let arrow = |out: &mut LBox, right: bool, left: bool| {
            let y = y0 + 0.07 * s;
            let (a, z) = (x0, x0 + aw.max(0.35 * s));
            out.add(stroke(vec![Cmd::M(a, y), Cmd::L(z, y)]));
            if right {
                out.add(stroke(vec![Cmd::M(z - 0.1 * s, y + 0.06 * s), Cmd::L(z, y), Cmd::L(z - 0.1 * s, y - 0.06 * s)]));
            }
            if left {
                out.add(stroke(vec![Cmd::M(a + 0.1 * s, y + 0.06 * s), Cmd::L(a, y), Cmd::L(a + 0.1 * s, y - 0.06 * s)]));
            }
        };
        match kind {
            AccentKind::Hat => out.add(stroke(vec![Cmd::M(x0, y0), Cmd::L(c, y0 + mark_h), Cmd::L(x0 + aw, y0)])),
            AccentKind::Check => out.add(stroke(vec![Cmd::M(x0, y0 + mark_h), Cmd::L(c, y0), Cmd::L(x0 + aw, y0 + mark_h)])),
            AccentKind::Tilde => out.add(stroke(vec![
                Cmd::M(x0, y0 + 0.03 * s),
                Cmd::C(x0 + aw * 0.3, y0 + 0.16 * s, x0 + aw * 0.7, y0 - 0.04 * s, x0 + aw, y0 + 0.1 * s),
            ])),
            AccentKind::Bar => out.add(stroke(vec![Cmd::M(x0, y0 + 0.06 * s), Cmd::L(x0 + aw, y0 + 0.06 * s)])),
            AccentKind::Vec => arrow(&mut out, true, false),
            AccentKind::LeftArrow => arrow(&mut out, false, true),
            AccentKind::LeftRightArrow => arrow(&mut out, true, true),
            AccentKind::Dot => out.add(dot(c)),
            AccentKind::DoubleDot => {
                out.add(dot(c - 0.09 * s));
                out.add(dot(c + 0.09 * s));
            }
            AccentKind::TripleDot => {
                out.add(dot(c - 0.15 * s));
                out.add(dot(c));
                out.add(dot(c + 0.15 * s));
            }
            AccentKind::Acute => out.add(stroke(vec![Cmd::M(c - 0.04 * s, y0), Cmd::L(c + 0.08 * s, y0 + mark_h)])),
            AccentKind::Grave => out.add(stroke(vec![Cmd::M(c + 0.04 * s, y0), Cmd::L(c - 0.08 * s, y0 + mark_h)])),
            AccentKind::Breve => out.add(stroke(vec![Cmd::M(c - 0.1 * s, y0 + mark_h), Cmd::Q(c, y0 - 0.04 * s, c + 0.1 * s, y0 + mark_h)])),
            AccentKind::Ring => out.add(Item::Circle { cx: c, cy: y0 + 0.08 * s, r: 0.065 * s, stroke: Some(sw), color: color.clone() }),
            AccentKind::Underline => {
                let y = -(body_desc + 0.1 * s);
                out.add(stroke(vec![Cmd::M(0.0, y), Cmd::L(body_w, y)]));
                out.desc = out.desc.max(-y + sw);
                return out;
            }
        }
        out.asc = out.asc.max(y0 + 0.2 * s).max(body_asc);
        out
    }

    fn brace(&mut self, content: &Node, label: Option<&Node>, over: bool, st: &Style) -> LBox {
        let s = st.scale;
        let body = self.layout(content, st);
        let w = body.w;
        let h = 0.2 * s;
        let (asc, desc) = (body.asc, body.desc);
        let mut out = LBox::empty();
        out.push(body);
        let (y, dir) = if over { (asc + 0.08 * s, 1.0) } else { (-(desc + 0.08 * s), -1.0) };
        let r = (0.15 * s).min(w / 6.0);
        let m = y + dir * h / 2.0;
        out.add(Item::Path {
            x: 0.0,
            y: 0.0,
            cmds: vec![
                Cmd::M(0.0, y),
                Cmd::Q(0.0, m, r, m),
                Cmd::L(w / 2.0 - r, m),
                Cmd::Q(w / 2.0, m, w / 2.0, y + dir * h),
                Cmd::Q(w / 2.0, m, w / 2.0 + r, m),
                Cmd::L(w - r, m),
                Cmd::Q(w, m, w, y),
            ],
            stroke: Some(0.04 * s),
            color: st.color.clone(),
        });
        let edge = y + dir * h;
        if over {
            out.asc = out.asc.max(edge + 0.04 * s);
        } else {
            out.desc = out.desc.max(-edge + 0.04 * s);
        }
        if let Some(l) = label {
            let lb = self.layout(l, &st.script());
            let lw = lb.w;
            let ly = if over { edge + 0.08 * s + lb.desc } else { edge - 0.08 * s - lb.asc };
            out.place(lb, (w - lw) / 2.0, ly);
            out.w = out.w.max(w);
        }
        out
    }

    fn enclose(&mut self, kind: EncloseKind, content: &Node, st: &Style) -> LBox {
        let s = st.scale;
        let pad = if kind == EncloseKind::Box { 0.15 * s } else { 0.0 };
        let body = self.layout(content, st);
        let (w, asc, desc) = (body.w + 2.0 * pad, body.asc + pad, body.desc + pad);
        let mut out = LBox { w, asc, desc, items: vec![] };
        out.place(body, pad, 0.0);
        let sw = 0.04 * s;
        let color = st.color.clone();
        let line = |cmds: Vec<Cmd>| Item::Path { x: 0.0, y: 0.0, cmds, stroke: Some(sw), color: color.clone() };
        let (l, r, t, b) = (0.0, w, asc, -desc);
        match kind {
            EncloseKind::Box => out.add(line(vec![Cmd::M(l, b), Cmd::L(r, b), Cmd::L(r, t), Cmd::L(l, t), Cmd::Z])),
            EncloseKind::Cancel => out.add(line(vec![Cmd::M(l, b), Cmd::L(r, t)])),
            EncloseKind::BackCancel => out.add(line(vec![Cmd::M(l, t), Cmd::L(r, b)])),
            EncloseKind::XCancel => out.add(line(vec![Cmd::M(l, b), Cmd::L(r, t), Cmd::M(l, t), Cmd::L(r, b)])),
        }
        out
    }
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Center,
    Right,
}

/// Width (in ems at size 1) of a stretchy delimiter of the given height.
fn delim_width(d: &str, height: f32) -> f32 {
    let grow = 0.04 * (height - 1.0).max(0.0);
    match d {
        "(" | ")" => 0.36 + grow,
        "[" | "]" | "\u{230A}" | "\u{230B}" | "\u{2308}" | "\u{2309}" => 0.3,
        "{" | "}" => 0.42 + grow,
        "|" | "\u{2223}" => 0.22,
        "\u{2016}" => 0.32,
        "\u{27E8}" | "\u{27E9}" | "<" | ">" => 0.36 + grow,
        "/" | "\\" => 0.4 + grow,
        "" => 0.12,
        _ => 0.5,
    }
}
