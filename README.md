# Rust-LaTeX-Parser

LaTeX math parser for Rust. Takes a string, gives you an AST. No dependencies.


## Install

```toml
[dependencies]
rust-latex-parser = "0.1"
```

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

Fractions (`\frac{a}{b}`, `\dfrac`, `\tfrac`, `a/b`, `{a \over b}`), roots (`\sqrt{x}`), super/subscripts, matrices (including `smallmatrix`), cases (and `dcases`), big operators with limits (`\sum`, `\int`, `\prod`, `\bigcup`, `\bigoplus`, ..., with `\limits`/`\nolimits`), trig and log functions, custom operators (`\operatorname{rank}`), `\bmod`/`\pmod`, accents (`\hat`, `\vec`, `\bar`, `\widehat`, `\overrightarrow`), `\left...\middle...\right` with any delimiter (`\{`, `\langle`, `\lfloor`, `\|`, ...), math fonts (`\mathbb`, `\mathcal`, `\mathscr`, etc.), styled text (`\textbf`, `\mbox`), binomials, over/underbraces, stacked relations, extensible arrows (`\xrightarrow[below]{above}`), negation (`\not\in` → `∉`), spacing (`\hspace{1em}`, `\mkern3mu`), and 270+ symbols (Greek, arrows, set theory, relations and their negations, the usual).

Purely presentational commands with no output of their own (`\displaystyle`, `\nonumber`, `\label{...}`) are accepted and skipped.

You can also skip backslashes for common stuff — `pi` works, `sqrt(x)` works, `int_0^1` works. Parens after barewords act as grouping, so `sqrt(x+1)` grabs the whole `x+1`.

## The tree

19 variants:

```rust
EqNode::Text(String)                    // "x", "42", "α"
EqNode::Space(f32)                      // inserted around operators
EqNode::Seq(Vec<EqNode>)               // horizontal sequence
EqNode::Sup(base, superscript)
EqNode::Sub(base, subscript)
EqNode::SupSub(base, sup, sub)
EqNode::Frac(numerator, denominator)
EqNode::Sqrt(contents)
EqNode::BigOp { symbol, lower, upper }
EqNode::Accent(contents, kind)
EqNode::Limit { name, lower }
EqNode::TextBlock(String)               // \text{...}
EqNode::MathFont { kind, content }
EqNode::Delimited { left, right, content }
EqNode::Matrix { kind, rows }
EqNode::Cases { rows }
EqNode::Binom(top, bottom)
EqNode::Brace { content, label, over }
EqNode::StackRel { base, annotation, over }
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

148 tests — parser correctness, adversarial input (deep nesting, emoji, 10k tokens), a randomized check over 5,000 generated malformed inputs that no content is ever dropped, real-world expressions (quadratic formula, Gaussian integral, Schrodinger, Maxwell), Greek alphabet coverage, and the newly supported commands.

## License

MIT
