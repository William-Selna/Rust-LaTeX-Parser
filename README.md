# Rust-LaTeX-Parser

Math input for Rust. Takes what the user typed, whether that's LaTeX or plain notation like `sqrt(x)/2 <= abs(y)`, and gives you an AST where every node knows where it came from. Then writes it back out as LaTeX, MathML, SVG or plain Unicode. No dependencies.

Built for equation editors in rich-text apps, where the input is half-finished most of the time and the person typing may not know LaTeX.

## Install

```toml
[dependencies]
rust-latex-parser = "0.2"
```

### Features

Each output format is a cargo feature, all on by default:

| Feature | Adds |
|---|---|
| `latex` | `to_latex` |
| `mathml` | `to_mathml` |
| `unicode` | `to_unicode` |
| `svg` | `to_svg`, `SvgOptions`, `SvgFont` (also turns on `unicode`) |

The parser, the tree, source spans and the symbol lookups are always there. If that's all you need, turn the rest off; the crate builds about a third faster and half the size:

```toml
rust-latex-parser = { version = "0.2", default-features = false }
# or just the formats you use:
rust-latex-parser = { version = "0.2", default-features = false, features = ["mathml"] }
```

Upgrading from 0.1? See [CHANGELOG.md](CHANGELOG.md). The short version: `parse_equation` returns a `Node`, so `match tree.kind` instead of `match tree`; add a `_ =>` arm to matches on `EqNode`; and expect `Op` nodes where you used to get `Space` padding around operators.

## Usage

```rust
use rust_latex_parser::{parse_equation, to_latex, to_mathml, to_svg, to_unicode, MathDisplay, SvgOptions};

let tree = parse_equation("x = (-b +/- sqrt(b^2 - 4ac))/(2a)");

to_latex(&tree);   // x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}
to_unicode(&tree); // x = (-b ± √(b² - 4ac))/2a
to_mathml(&tree, MathDisplay::Block);    // <math display="block">…</math>
to_svg(&tree, &SvgOptions::default());   // <svg …>…</svg>
```

Or walk the tree yourself and render it however you like:

```rust
use rust_latex_parser::{parse_equation, EqNode};

let input = r"a + \sqrt{x}";
let tree = parse_equation(input);

match &tree.kind {
    EqNode::Seq(parts) => { /* a, +, √x */ }
    _ => {}
}

// Every node knows where it came from (byte offsets into the input)...
let sqrt = &tree.children()[2];
assert_eq!(&input[sqrt.span.range()], r"\sqrt{x}");

// ...so you can find what's under the cursor.
let under_cursor = tree.node_at(10).unwrap(); // the `x`
```

## Input

### LaTeX

- **Fractions and roots:** `\frac{a}{b}`, `\dfrac`, `\tfrac`, `\cfrac`, `{a \over b}`, `\sqrt{x}`, `\sqrt[3]{x}`
- **Scripts:** `x^2`, `x_{i+1}`, primes (`f'(x)`, `f''`)
- **Big operators with limits:** `\sum`, `\int`, `\prod`, `\bigcup`, `\bigoplus`, ... including `\limits`/`\nolimits` and `\substack`
- **Named operators:** `\sin`, `\log`, `\lim`, `\Pr`, ... plus custom ones with `\operatorname{rank}`
- **Environments:** `matrix`/`pmatrix`/`bmatrix`/`vmatrix`/`Bmatrix`/`Vmatrix`/`smallmatrix`, `cases`/`dcases`, `align`/`aligned`/`split`/`alignat`/`eqnarray`, `gather`/`gathered`, `array` (column specs and `\hline`), `equation`
- **Delimiters:** `\left...\middle...\right` with any delimiter (`\{`, `\langle`, `\lfloor`, `\|`, ...), and manual sizes (`\big(`, `\Bigl[`, `\bigg\{`, ...)
- **Accents:** `\hat`, `\bar`, `\vec`, `\dot`, `\tilde`, wide versions (`\widehat`, `\overrightarrow`), `\underline`, and more
- **Fonts and text:** `\mathbb`, `\mathcal`, `\mathbf`, `\boldsymbol`, `\mathit`, ..., `\text{}`, `\textbf{}`, `\mbox{}`
- **Decorations:** `\boxed`, `\cancel`/`\bcancel`/`\xcancel`, `\textcolor{red}{x}`, `\color{red}`, over/underbraces, `\overset`/`\underset`, `\xrightarrow[below]{above}`
- **Negation:** `\not\in` → `∉`, `\not=` → `≠`, etc.
- **Spacing:** `\,` `\;` `\quad`, `\hspace{1em}`, `\mkern3mu`, ...
- **Comments:** `% ...` is ignored, as in LaTeX (use `\%` for a literal percent)
- **270+ symbols:** Greek (including `var` forms), arrows, relations and their negations, set theory, logic, the usual

Purely presentational commands with no output of their own (`\displaystyle`, `\limits`, `\nonumber`, `\label{...}`) are accepted and skipped.

### Plain notation

For people who don't know LaTeX. Both styles work everywhere and can be mixed. Where a shortcut would clash with something a LaTeX user could plausibly mean, LaTeX wins.

| Type | Get |
|---|---|
| `pi`, `alpha`, `infinity`, `leq`, `subseteq` | `π`, `α`, `∞`, `≤`, `⊆` |
| `<=` `>=` `->` `=>` `<=>` `<->` `+/-` | `≤` `≥` `→` `⇒` `⇔` `↔` `±` |
| `x^(n+1)`, `sum_(i=1)^n` | parentheses group the script |
| `(a+b)/(c+d)`, `dy/dx`, `sin(x)/x` | a fraction of the whole operands (a space ends an operand: `1/2 x` is ½·x) |
| `sqrt(x)`, `root(3, x)`, `cbrt(x)` | roots |
| `abs(x)`, `norm(v)`, `floor(x)`, `ceil(x)` | `\|x\|`, `‖v‖`, `⌊x⌋`, `⌈x⌉` |
| `hat(x)`, `bar(x)`, `vec(v)`, `dot(x)`, `tilde(x)`, `ul(x)` | accents |
| `binom(n, k)` | binomial |
| `"if"` or `text(if)` | upright text |
| `sin x`, `lim_(x->0)`, `int_0^1`, `sum`, `prod` | operators |
| `x²`, `a₁`, `f′`, `√x`, `∛x`, `½`, `∑`, `≤`, `ℝ`, `𝐱` | typed or pasted Unicode, read as the equivalent LaTeX |

Function words only apply when `(` follows immediately: `abs` on its own is three letters.

Left to LaTeX because they're ambiguous: `n!=1` (n factorial equals 1, not "not equal"), `x<-1` (x less than −1), `a+-b` (a plus −b), and `inf` (infimum, not infinity).

## Output

| Function | For |
|---|---|
| `to_latex(&tree)` | Storing canonical LaTeX; interop with KaTeX, MathJax and LaTeX. Only standard commands, so plain-notation input comes out as normal LaTeX. |
| `to_mathml(&tree, display)` | Rendering in the browser with no JavaScript library. Targets [MathML Core](https://www.w3.org/TR/mathml-core/), which current Chrome, Edge, Firefox and Safari support. Unknown commands come out as `<merror>` so you can highlight them. |
| `to_unicode(&tree)` | Alt text, the plain-text clipboard, search indexing: `x² + √(y + 1)`, `ℝⁿ`, `∑ᵢ₌₁ⁿ`. |
| `to_svg(&tree, &options)` | Images: exports, thumbnails, PDFs, email, anywhere MathML isn't available. Self-contained, with baseline alignment for inline use and a text `<title>` for screen readers. |

For valid input, `parse_equation(&to_latex(&tree))` gives back the same tree.

SVG layout draws brackets, roots, fraction bars and big operators as paths, and places letters using built-in width tables measured from real fonts, so it needs no font files at render time. Pick the font the SVG will be displayed with:

| `SvgFont` | Font | Notes |
|---|---|---|
| `Times` (default) | Times New Roman, or Liberation Serif / Nimbus Roman | Available almost everywhere |
| `StixTwo` | STIX Two Text | Times-like, made for scientific publishing |
| `ComputerModern` | CMU Serif / Latin Modern Roman | The classic LaTeX look |
| `NotoSerif` | Noto Serif | Default serif on Android and many Linux systems |
| `DejaVuSerif` | DejaVu Serif | Common Linux default |
| `Libertinus` | Libertinus Serif | Popular in academic publishing |

With the matching font available (installed, or loaded as a web font), every glyph lands within 0.01 em of where the layout put it. `SvgOptions` also sets the size, display or inline style, colour (default `currentColor`), an optional CSS font-family override, and optional `data-span` attributes that map each glyph back to the input.

MathML renders best with an OpenType math font; browsers fall back to the system one (Cambria Math on Windows, STIX Two Math on macOS). For consistent results everywhere, load a math web font such as STIX Two Math or Latin Modern Math and set `math { font-family: ... }`.

## The tree

`parse_equation` returns a `Node`: its `kind` (an `EqNode`, below) and its `span` (byte offsets into the input). Children are `Node`s too. `node.children()` walks the tree in reading order, `node.node_at(offset)` finds the innermost node under a cursor, and `==` compares structure while ignoring spans.

`EqNode` has 28 variants. It's `#[non_exhaustive]`, so include a `_ =>` arm when matching.

```rust
EqNode::Text(String)                    // "x", "42", "α", "(" — and unary "-"
EqNode::Op { symbol, kind }             // "+" Binary, "=" Relation, "≤", "→", ...
EqNode::Space(f32)                      // explicit spacing only: \quad, \,, \hspace
EqNode::Seq(Vec<Node>)                  // horizontal sequence
EqNode::Sup(base, superscript)
EqNode::Sub(base, subscript)
EqNode::SupSub(base, sup, sub)
EqNode::Frac(numerator, denominator)
EqNode::Sqrt(contents)
EqNode::Root { index, content }         // \sqrt[3]{x}
EqNode::BigOp { symbol, lower, upper }
EqNode::Accent(contents, kind)
EqNode::Limit { name, lower }
EqNode::TextBlock(String)               // \text{...}
EqNode::MathFont { kind, content }
EqNode::Delimited { left, right, content }
EqNode::Middle(String)                  // \middle|
EqNode::SizedDelim { delim, size }      // \big( .. \Bigg)
EqNode::Matrix { kind, rows }
EqNode::Cases { rows }
EqNode::Align { rows }                  // align, aligned, split, ...
EqNode::Gather { rows }                 // gather, gathered
EqNode::Array { columns, rows, hlines } // array, \substack
EqNode::Binom(top, bottom)
EqNode::Brace { content, label, over }
EqNode::StackRel { base, annotation, over }
EqNode::Color { color, content }        // \textcolor, \color
EqNode::Enclose { kind, content }       // \boxed, \cancel, ...
```

The tree records meaning, not layout: whitespace in the input doesn't produce nodes, and operators are `Op` nodes with a class (`Binary` or `Relation`) so your renderer decides the spacing.

There's also `EqMetrics` (width/ascent/descent) if you need it for layout.

## Symbols

```rust
use rust_latex_parser::{latex_to_unicode, unicode_to_latex, symbols, op_kind};

latex_to_unicode("alpha");  // Some("α")
unicode_to_latex('≤');      // Some("leq")
op_kind("→");               // Some(OpKind::Relation)
symbols().count();          // every command name and its character — for autocomplete or a symbol palette
```

## Error handling

It doesn't error, and it doesn't crash: nesting deeper than any real equation (48 levels) is kept as plain text rather than recursed into, so pathological input can't overflow the stack. Bad input gives you a best-effort tree, and nothing you type is thrown away: unknown commands, unknown environments, and stray `}`, `&`, `\\` or `\end{...}` all come back as text nodes, and parsing carries on after them. An environment left open inside a group (e.g. `\frac{\begin{pmatrix} a}{b}`) closes at that group's `}`, and an unclosed `"quote` runs to the end. I needed this for a live editor where the input is always half-finished.

## Tests

```sh
cargo test
```

266 tests — parser correctness, plain-notation input and the LaTeX-wins rule, a randomized check over 5,000 generated malformed inputs that no content is ever dropped, source spans (in bounds, on character boundaries and nested inside their parents), LaTeX round-tripping (exact for valid input and random trees, a fixed point for random malformed input), well-formed MathML and SVG, and real-world expressions (quadratic formula, Gaussian integral, Schrodinger, Maxwell).

`tests/stress.rs` is the adversarial suite. It covers:
- random Unicode and every one- and two-character input
- every prefix of real expressions (what an editor sees as you type)
- bounded tree depth however many times any fragment, or pair of fragments, repeats
- a 1 MB paste
- timing checks that parsing and all four outputs stay linear over 30 input shapes
- determinism across threads
- the minimised inputs that fuzzing found

There is also a [`cargo fuzz`](https://github.com/rust-fuzz/cargo-fuzz) target. On arbitrary bytes, it checks span invariants, bounded depth, LaTeX round-trip stability, and well-formed MathML and SVG:

```sh
cargo +nightly fuzz run parse -- -dict=fuzz/latex.dict
```

## License

MIT
