# Rust-LaTeX-Parser

LaTeX math parser for Rust. Takes a string, gives you an AST. No dependencies.


## Install

```toml
[dependencies]
rust-latex-parser = "0.2"
```

Upgrading from 0.1? See [CHANGELOG.md](CHANGELOG.md) — the short version is to add a `_ =>` arm wherever you `match` on `EqNode`.

## Usage

```rust
use rust_latex_parser::{parse_equation, EqNode};

let tree = parse_equation("\\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}");

match tree {
    EqNode::Frac(numerator, denominator) => {
        // do whatever you want with it
    }
    _ => {}
}
```

You get back an `EqNode` tree. Render it, convert it, inspect it — that part's on you.

## What it handles

- **Fractions and roots:** `\frac{a}{b}`, `\dfrac`, `\tfrac`, `\cfrac`, `a/b`, `{a \over b}`, `\sqrt{x}`, `\sqrt[3]{x}`
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

You can also skip backslashes for common stuff — `pi` works, `sqrt(x)` works, `int_0^1` works. Parens after barewords act as grouping, so `sqrt(x+1)` grabs the whole `x+1`.

## The tree

26 variants. `EqNode` is `#[non_exhaustive]`, so include a `_ =>` arm when matching; it derives `PartialEq` for easy comparisons in tests.

```rust
EqNode::Text(String)                    // "x", "42", "α"
EqNode::Space(f32)                      // inserted around operators
EqNode::Seq(Vec<EqNode>)               // horizontal sequence
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

There's also `EqMetrics` (width/ascent/descent) if you need it for layout.

## Symbol lookup

`latex_to_unicode` works standalone if you just need that:

```rust
use rust_latex_parser::latex_to_unicode;

latex_to_unicode("alpha");      // Some("α")
latex_to_unicode("rightarrow"); // Some("→")
latex_to_unicode("garbage");    // None
```

## Error handling

It doesn't error. Bad input gives you a best-effort tree, and nothing you type is thrown away: unknown commands, unknown environments, and stray `}`, `&`, `\\` or `\end{...}` all come back as text nodes, and parsing carries on after them. An environment left open inside a group (e.g. `\frac{\begin{pmatrix} a}{b}`) closes at that group's `}`. I needed this for a live editor where the input is always half-finished.

## Tests

```sh
cargo test
```

174 tests — parser correctness, adversarial input (deep nesting, emoji, 10k tokens), a randomized check over 5,000 generated malformed inputs that no content is ever dropped, real-world expressions (quadratic formula, Gaussian integral, Schrodinger, Maxwell), and coverage of the extended syntax.

## License

MIT
