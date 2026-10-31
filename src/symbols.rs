//! The symbol table shared by the parser and the output formats.
//!
//! Maps LaTeX command names to Unicode characters (and back), and classifies
//! operator characters for spacing.

use crate::ast::OpKind;

/// Command name → character. When several names map to the same character,
/// the first one listed is the canonical name used by [`unicode_to_latex`].
static SYMBOLS: &[(&str, char)] = &[
    // Greek lowercase
    ("alpha", '\u{03B1}'),
    ("beta", '\u{03B2}'),
    ("gamma", '\u{03B3}'),
    ("delta", '\u{03B4}'),
    ("epsilon", '\u{03B5}'),
    ("varepsilon", '\u{03B5}'),
    ("zeta", '\u{03B6}'),
    ("eta", '\u{03B7}'),
    ("theta", '\u{03B8}'),
    ("vartheta", '\u{03D1}'),
    ("iota", '\u{03B9}'),
    ("kappa", '\u{03BA}'),
    ("varkappa", '\u{03F0}'),
    ("lambda", '\u{03BB}'),
    ("mu", '\u{03BC}'),
    ("nu", '\u{03BD}'),
    ("xi", '\u{03BE}'),
    ("omicron", '\u{03BF}'),
    ("pi", '\u{03C0}'),
    ("varpi", '\u{03D6}'),
    ("rho", '\u{03C1}'),
    ("varrho", '\u{03F1}'),
    ("sigma", '\u{03C3}'),
    ("varsigma", '\u{03C2}'),
    ("tau", '\u{03C4}'),
    ("upsilon", '\u{03C5}'),
    ("phi", '\u{03C6}'),
    ("varphi", '\u{03C6}'),
    ("chi", '\u{03C7}'),
    ("psi", '\u{03C8}'),
    ("omega", '\u{03C9}'),
    ("digamma", '\u{03DD}'),
    // Greek uppercase
    ("Alpha", '\u{0391}'),
    ("Beta", '\u{0392}'),
    ("Gamma", '\u{0393}'),
    ("Delta", '\u{0394}'),
    ("Epsilon", '\u{0395}'),
    ("Zeta", '\u{0396}'),
    ("Eta", '\u{0397}'),
    ("Theta", '\u{0398}'),
    ("Iota", '\u{0399}'),
    ("Kappa", '\u{039A}'),
    ("Lambda", '\u{039B}'),
    ("Mu", '\u{039C}'),
    ("Nu", '\u{039D}'),
    ("Xi", '\u{039E}'),
    ("Pi", '\u{03A0}'),
    ("Rho", '\u{03A1}'),
    ("Sigma", '\u{03A3}'),
    ("Tau", '\u{03A4}'),
    ("Upsilon", '\u{03A5}'),
    ("Phi", '\u{03A6}'),
    ("Chi", '\u{03A7}'),
    ("Psi", '\u{03A8}'),
    ("Omega", '\u{03A9}'),
    // Operators
    ("pm", '\u{00B1}'),
    ("plusminus", '\u{00B1}'),
    ("mp", '\u{2213}'),
    ("minusplus", '\u{2213}'),
    ("times", '\u{00D7}'),
    ("div", '\u{00F7}'),
    ("cdot", '\u{00B7}'),
    ("ast", '\u{2217}'),
    ("star", '\u{22C6}'),
    ("circ", '\u{2218}'),
    ("bullet", '\u{2022}'),
    ("oplus", '\u{2295}'),
    ("otimes", '\u{2297}'),
    ("ominus", '\u{2296}'),
    ("odot", '\u{2299}'),
    ("oslash", '\u{2298}'),
    ("dagger", '\u{2020}'),
    ("dag", '\u{2020}'),
    ("ddagger", '\u{2021}'),
    ("ddag", '\u{2021}'),
    ("sqcup", '\u{2294}'),
    ("sqcap", '\u{2293}'),
    ("uplus", '\u{228E}'),
    ("wr", '\u{2240}'),
    ("diamond", '\u{22C4}'),
    ("amalg", '\u{2A3F}'),
    ("bigcirc", '\u{25EF}'),
    ("triangleleft", '\u{25C1}'),
    ("triangleright", '\u{25B7}'),
    ("lhd", '\u{22B2}'),
    ("rhd", '\u{22B3}'),
    ("ltimes", '\u{22C9}'),
    ("rtimes", '\u{22CA}'),
    // Relations
    ("leq", '\u{2264}'),
    ("le", '\u{2264}'),
    ("geq", '\u{2265}'),
    ("ge", '\u{2265}'),
    ("neq", '\u{2260}'),
    ("ne", '\u{2260}'),
    ("approx", '\u{2248}'),
    ("equiv", '\u{2261}'),
    ("sim", '\u{223C}'),
    ("simeq", '\u{2243}'),
    ("cong", '\u{2245}'),
    ("propto", '\u{221D}'),
    ("ll", '\u{226A}'),
    ("gg", '\u{226B}'),
    ("prec", '\u{227A}'),
    ("succ", '\u{227B}'),
    ("perp", '\u{22A5}'),
    ("parallel", '\u{2225}'),
    ("nparallel", '\u{2226}'),
    ("mid", '\u{2223}'),
    ("nmid", '\u{2224}'),
    ("nleq", '\u{2270}'),
    ("ngeq", '\u{2271}'),
    ("leqslant", '\u{2A7D}'),
    ("geqslant", '\u{2A7E}'),
    ("nless", '\u{226E}'),
    ("ngtr", '\u{226F}'),
    ("nequiv", '\u{2262}'),
    ("nsim", '\u{2241}'),
    ("ncong", '\u{2247}'),
    ("lesssim", '\u{2272}'),
    ("gtrsim", '\u{2273}'),
    ("coloneqq", '\u{2254}'),
    ("doteq", '\u{2250}'),
    ("asymp", '\u{224D}'),
    ("vdash", '\u{22A2}'),
    ("dashv", '\u{22A3}'),
    ("models", '\u{22A8}'),
    ("bowtie", '\u{22C8}'),
    ("smile", '\u{2323}'),
    ("frown", '\u{2322}'),
    // Logic & sets
    ("forall", '\u{2200}'),
    ("exists", '\u{2203}'),
    ("nexists", '\u{2204}'),
    ("neg", '\u{00AC}'),
    ("lnot", '\u{00AC}'),
    ("wedge", '\u{2227}'),
    ("land", '\u{2227}'),
    ("vee", '\u{2228}'),
    ("lor", '\u{2228}'),
    ("in", '\u{2208}'),
    ("notin", '\u{2209}'),
    ("ni", '\u{220B}'),
    ("subset", '\u{2282}'),
    ("supset", '\u{2283}'),
    ("subseteq", '\u{2286}'),
    ("supseteq", '\u{2287}'),
    ("subsetneq", '\u{228A}'),
    ("supsetneq", '\u{228B}'),
    ("nsubseteq", '\u{2288}'),
    ("nsupseteq", '\u{2289}'),
    ("sqsubseteq", '\u{2291}'),
    ("sqsupseteq", '\u{2292}'),
    ("complement", '\u{2201}'),
    ("cup", '\u{222A}'),
    ("cap", '\u{2229}'),
    ("emptyset", '\u{2205}'),
    ("varnothing", '\u{2205}'),
    ("setminus", '\u{2216}'),
    // Arrows
    ("rightarrow", '\u{2192}'),
    ("to", '\u{2192}'),
    ("leftarrow", '\u{2190}'),
    ("gets", '\u{2190}'),
    ("leftrightarrow", '\u{2194}'),
    ("Rightarrow", '\u{21D2}'),
    ("implies", '\u{21D2}'),
    ("Leftarrow", '\u{21D0}'),
    ("Leftrightarrow", '\u{21D4}'),
    ("iff", '\u{21D4}'),
    ("uparrow", '\u{2191}'),
    ("downarrow", '\u{2193}'),
    ("mapsto", '\u{21A6}'),
    ("hookrightarrow", '\u{21AA}'),
    ("hookleftarrow", '\u{21A9}'),
    ("longrightarrow", '\u{27F6}'),
    ("longleftarrow", '\u{27F5}'),
    ("longleftrightarrow", '\u{27F7}'),
    ("Longrightarrow", '\u{27F9}'),
    ("Longleftarrow", '\u{27F8}'),
    ("impliedby", '\u{27F8}'),
    ("Longleftrightarrow", '\u{27FA}'),
    ("longmapsto", '\u{27FC}'),
    ("Uparrow", '\u{21D1}'),
    ("Downarrow", '\u{21D3}'),
    ("updownarrow", '\u{2195}'),
    ("Updownarrow", '\u{21D5}'),
    ("nearrow", '\u{2197}'),
    ("searrow", '\u{2198}'),
    ("nwarrow", '\u{2196}'),
    ("swarrow", '\u{2199}'),
    ("rightleftharpoons", '\u{21CC}'),
    ("leftrightharpoons", '\u{21CB}'),
    ("rightharpoonup", '\u{21C0}'),
    ("rightharpoondown", '\u{21C1}'),
    ("leftharpoonup", '\u{21BC}'),
    ("leftharpoondown", '\u{21BD}'),
    ("nrightarrow", '\u{219B}'),
    ("nleftarrow", '\u{219A}'),
    ("nRightarrow", '\u{21CF}'),
    ("nLeftarrow", '\u{21CD}'),
    ("nLeftrightarrow", '\u{21CE}'),
    // Misc
    ("infty", '\u{221E}'),
    ("inf", '\u{221E}'),
    ("partial", '\u{2202}'),
    ("nabla", '\u{2207}'),
    ("hbar", '\u{210F}'),
    ("ell", '\u{2113}'),
    ("Re", '\u{211C}'),
    ("Im", '\u{2111}'),
    ("wp", '\u{2118}'),
    ("aleph", '\u{2135}'),
    ("angle", '\u{2220}'),
    ("triangle", '\u{25B3}'),
    ("triangledown", '\u{25BD}'),
    ("top", '\u{22A4}'),
    ("bot", '\u{22A5}'),
    ("square", '\u{25A1}'),
    ("Box", '\u{25A1}'),
    ("blacksquare", '\u{25A0}'),
    ("lozenge", '\u{25CA}'),
    ("bigstar", '\u{2605}'),
    ("checkmark", '\u{2713}'),
    ("therefore", '\u{2234}'),
    ("because", '\u{2235}'),
    ("imath", '\u{0131}'),
    ("jmath", '\u{0237}'),
    ("beth", '\u{2136}'),
    ("gimel", '\u{2137}'),
    ("daleth", '\u{2138}'),
    ("mho", '\u{2127}'),
    ("eth", '\u{00F0}'),
    ("flat", '\u{266D}'),
    ("sharp", '\u{266F}'),
    ("natural", '\u{266E}'),
    ("clubsuit", '\u{2663}'),
    ("diamondsuit", '\u{2662}'),
    ("heartsuit", '\u{2661}'),
    ("spadesuit", '\u{2660}'),
    ("surd", '\u{221A}'),
    ("measuredangle", '\u{2221}'),
    ("sphericalangle", '\u{2222}'),
    ("backprime", '\u{2035}'),
    ("copyright", '\u{00A9}'),
    ("pounds", '\u{00A3}'),
    ("S", '\u{00A7}'),
    ("P", '\u{00B6}'),
    ("colon", ':'),
    ("degree", '\u{00B0}'),
    ("deg", '\u{00B0}'),
    ("prime", '\u{2032}'),
    ("ldots", '\u{2026}'),
    ("dots", '\u{2026}'),
    ("dotsc", '\u{2026}'),
    ("dotso", '\u{2026}'),
    ("cdots", '\u{22EF}'),
    ("dotsb", '\u{22EF}'),
    ("dotsm", '\u{22EF}'),
    ("dotsi", '\u{22EF}'),
    ("iddots", '\u{22F0}'),
    ("vdots", '\u{22EE}'),
    ("ddots", '\u{22F1}'),
    // Delimiters
    ("langle", '\u{27E8}'),
    ("rangle", '\u{27E9}'),
    ("lceil", '\u{2308}'),
    ("rceil", '\u{2309}'),
    ("lfloor", '\u{230A}'),
    ("rfloor", '\u{230B}'),
    ("vert", '|'),
    ("lvert", '|'),
    ("rvert", '|'),
    ("Vert", '\u{2016}'),
    ("lVert", '\u{2016}'),
    ("rVert", '\u{2016}'),
    ("lbrace", '{'),
    ("rbrace", '}'),
    ("lbrack", '['),
    ("rbrack", ']'),
    ("backslash", '\\'),
];

/// Look up the Unicode character for a LaTeX command name.
///
/// Covers 270+ symbols: Greek letters (upper and lowercase, plus `var`
/// variants), operators, relations and their negations, set theory, arrows,
/// delimiters, and miscellaneous symbols. Returns `None` for anything it
/// doesn't recognize.
///
/// This is useful on its own if you just need symbol conversion without
/// parsing a full expression.
///
/// # Examples
///
/// ```
/// use rust_latex_parser::latex_to_unicode;
///
/// assert_eq!(latex_to_unicode("alpha"), Some("α".to_string()));
/// assert_eq!(latex_to_unicode("infty"), Some("∞".to_string()));
/// assert_eq!(latex_to_unicode("rightarrow"), Some("→".to_string()));
/// assert_eq!(latex_to_unicode("notacommand"), None);
/// ```
pub fn latex_to_unicode(name: &str) -> Option<String> {
    SYMBOLS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| c.to_string())
}

/// The canonical LaTeX command name for a symbol character, if it has one.
///
/// This is the inverse of [`latex_to_unicode`]. ASCII characters (`|`, `[`,
/// `:` ...) have no command and return `None`.
///
/// ```
/// use rust_latex_parser::unicode_to_latex;
///
/// assert_eq!(unicode_to_latex('α'), Some("alpha"));
/// assert_eq!(unicode_to_latex('≤'), Some("leq"));
/// assert_eq!(unicode_to_latex('x'), None);
/// ```
pub fn unicode_to_latex(ch: char) -> Option<&'static str> {
    if ch.is_ascii() {
        return None;
    }
    SYMBOLS.iter().find(|(_, c)| *c == ch).map(|(n, _)| *n)
}

/// Every command name the symbol table knows, with its character. Handy for
/// building an autocomplete list or a symbol palette.
///
/// ```
/// use rust_latex_parser::symbols;
///
/// assert!(symbols().any(|(name, ch)| name == "alpha" && ch == 'α'));
/// ```
pub fn symbols() -> impl Iterator<Item = (&'static str, char)> {
    SYMBOLS.iter().copied()
}

/// The spacing class of an operator symbol, or `None` for ordinary symbols.
pub fn op_kind(symbol: &str) -> Option<OpKind> {
    let mut chars = symbol.chars();
    let first = chars.next()?;
    // Negations built with a combining long solidus (U+0338) keep the class
    // of the base symbol.
    match chars.next() {
        None => {}
        Some('\u{0338}') if chars.next().is_none() => {}
        Some(_) => return None,
    }
    if is_relation(first) {
        Some(OpKind::Relation)
    } else if is_binary(first) {
        Some(OpKind::Binary)
    } else {
        None
    }
}

fn is_binary(ch: char) -> bool {
    matches!(ch,
        '+' | '-' | '\u{2212}' | '\u{00B1}' | '\u{2213}' | '\u{00D7}' | '\u{00F7}' | '\u{00B7}' | // + - − ± ∓ × ÷ ·
        '*' | '\u{2217}' | '\u{22C6}' | '\u{2218}' | '\u{2022}' | // * ∗ ⋆ ∘ •
        '\u{2295}' | '\u{2296}' | '\u{2297}' | '\u{2298}' | '\u{2299}' | // ⊕ ⊖ ⊗ ⊘ ⊙
        '\u{222A}' | '\u{2229}' | '\u{2294}' | '\u{2293}' | '\u{228E}' | // ∪ ∩ ⊔ ⊓ ⊎
        '\u{2227}' | '\u{2228}' | '\u{2216}' | // ∧ ∨ ∖
        '\u{2020}' | '\u{2021}' | '\u{2240}' | '\u{22C4}' | '\u{2A3F}' | '\u{25EF}' | // † ‡ ≀ ⋄ ⨿ ◯
        '\u{25C1}' | '\u{25B7}' | '\u{22B2}' | '\u{22B3}' | '\u{22C9}' | '\u{22CA}' // ◁ ▷ ⊲ ⊳ ⋉ ⋊
    )
}

fn is_relation(ch: char) -> bool {
    matches!(ch,
        '=' | '<' | '>' |
        '\u{2264}' | '\u{2265}' | '\u{2260}' | '\u{2248}' | '\u{2261}' | // ≤ ≥ ≠ ≈ ≡
        '\u{2208}' | '\u{2209}' | '\u{220B}' | '\u{220C}' | // ∈ ∉ ∋ ∌
        '\u{2282}' | '\u{2283}' | '\u{2286}' | '\u{2287}' | // ⊂ ⊃ ⊆ ⊇
        '\u{228A}' | '\u{228B}' | '\u{2284}' | '\u{2285}' | '\u{2288}' | '\u{2289}' | // ⊊ ⊋ ⊄ ⊅ ⊈ ⊉
        '\u{2291}' | '\u{2292}' | // ⊑ ⊒
        '\u{227A}' | '\u{227B}' | '\u{2280}' | '\u{2281}' | // ≺ ≻ ⊀ ⊁
        '\u{223C}' | '\u{2241}' | '\u{2243}' | '\u{2244}' | '\u{2245}' | '\u{2247}' | // ∼ ≁ ≃ ≄ ≅ ≇
        '\u{2249}' | '\u{2262}' | '\u{221D}' | '\u{226A}' | '\u{226B}' | // ≉ ≢ ∝ ≪ ≫
        '\u{226E}' | '\u{226F}' | '\u{2270}' | '\u{2271}' | '\u{2A7D}' | '\u{2A7E}' | // ≮ ≯ ≰ ≱ ⩽ ⩾
        '\u{2272}' | '\u{2273}' | // ≲ ≳
        '\u{2223}' | '\u{2224}' | '\u{2225}' | '\u{2226}' | // ∣ ∤ ∥ ∦
        '\u{2254}' | '\u{2250}' | '\u{224D}' | '\u{22C8}' | '\u{2323}' | '\u{2322}' | // ≔ ≐ ≍ ⋈ ⌣ ⌢
        '\u{22A2}' | '\u{22A3}' | '\u{22A8}' | '\u{22AC}' | '\u{22AD}' | // ⊢ ⊣ ⊨ ⊬ ⊭
        '\u{22E2}' | '\u{22E3}' | '\u{226D}' | // ⋢ ⋣ ≭
        // Arrows
        '\u{2190}'..='\u{21FF}' | '\u{27F5}'..='\u{27FF}'
    )
}
