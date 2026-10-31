# Changelog

## 0.2.0

Math input for equation editors whose users may not know LaTeX: plain
notation alongside LaTeX, a tree that records meaning and source
positions rather than layout, and LaTeX, MathML, SVG and Unicode output.

### Breaking changes

- **Nodes carry source spans.** `parse_equation` returns a
  `Node { kind: EqNode, span: Span }`, and all children are `Node`s.
  `Span` holds byte offsets into the input. `Node` equality ignores spans.

- **Operators are `Op` nodes.** `EqNode::Op { symbol, kind }` with
  `OpKind::{Binary, Relation}` replaces the `Space(4.0), Text(op),
  Space(4.0)` padding 0.1 inserted around operators. Spacing is now the
  renderer's job. A sign with no left operand (`-x`, `a = -b`) is a unary
  `Text("-")`.
- **Whitespace produces no nodes.** 0.1 emitted a `Text(" ")` for every
  space. `Space` is now only produced by explicit spacing commands (`\quad`,
  `\,`, `\hspace{..}`).
- **New variants:** `Op`, `Middle`, `Root`, `SizedDelim`, `Align`,
  `Gather`, `Array`, `Color`, `Enclose`. `AccentKind` and `MathFontKind`
  also have new variants, and there are new enums `OpKind`, `EncloseKind`
  and `ArrayColumn`.
- `EqNode` and the kind enums are now `#[non_exhaustive]`, so later releases
  can add syntax without breaking your code.
- `\bmod` is `Op("mod")`. `\xrightarrow` and friends are a `StackRel`
  around an arrow `Op`, without `Space` padding.

**To upgrade:**
- Match on `node.kind` instead of `node`. Children are `Node`s, so
  recursive code keeps the same shape. To build a tree by hand, convert
  each `EqNode` with `.into()` (e.g. `EqNode::Text("x".into()).into()`).
- Add a `_ =>` arm to each `match` on these enums.
- Handle `Op` where you used to see `Space` + `Text` + `Space` around
  operators.
- Stop filtering out `Text(" ")` nodes; they no longer appear.

### Output changes

These change the tree for input that 0.1.x already accepted:

- `\sqrt[n]{x}` produces `Root`. In 0.1.x it produced a broken `Sqrt`.
- Environments that 0.1.x kept as literal text now produce real nodes:
  `align`, `aligned`, `split`, `alignat` and `eqnarray` produce `Align`.
  `gather` produces `Gather`. `array` produces `Array`. `equation` is
  unwrapped.
- `\middle|` produces `Middle` instead of plain text, so it can stretch.
- Primes become superscripts: `f'` is `Sup(f, "′")`.
- `%` starts a comment that runs to the end of the line. Use `\%` for a
  literal percent sign.
- **`/` takes whole operands.** In 0.1, only the single nodes either side of
  the slash were used, so `ab/cd` was `a`, `b/c`, `d`. Now `/` takes:
  - a parenthesized group, with the parentheses dropped: `(a+b)/(c+d)`
  - a function application: `sin(x)/x`
  - otherwise the run of adjacent operands: `dy/dx`, `n!/k!`

  Whitespace ends a run.
- `x^(n+1)` takes the whole parenthesized group as the superscript. In 0.1
  the superscript was just `(`.
- `\ldots` / `\dots` produce baseline `…` (U+2026) instead of `⋯`.
  `\cdots` is unchanged.
- `\varsigma`, `\vartheta`, `\varpi` and `\varrho` map to their own glyphs
  (ς ϑ ϖ ϱ) instead of the plain letters.
- `\|` produces `‖` instead of `|`.
- All whitespace after a command name is skipped, as in TeX.

### Added

**Plain notation** (where a shortcut clashes with a plausible LaTeX
reading, LaTeX wins):
- ASCII shortcuts: `<=` `>=` `->` `=>` `<=>` `<->` `+/-`. These are left to
  LaTeX: `n!=1`, `x<-1` and `a+-b`.
- Parentheses group scripts and big-operator limits: `x^(n+1)`,
  `sum_(i=1)^n`.
- Function-style shortcuts when `(` follows immediately:
  - `abs`, `norm`, `floor`, `ceil`
  - `sqrt`, `cbrt`, `root(n, x)`, `binom(n, k)`
  - `hat`, `bar`, `vec`, `dot`, `ddot`, `tilde`, `ul`
  - `text(...)`
- `"quoted text"` is upright text. An unclosed quote runs to the end of the
  input.
- Typed or pasted Unicode:
  - superscripts and subscripts (`x²`, `a₁`) and primes (`f′`)
  - `√` `∛` `∜`, vulgar fractions (`½`)
  - big operators with Unicode limits (`∫₀¹`)
  - math letters (`ℝ` → `\mathbb{R}`)
  - operator symbols, which get the same class as their commands
- More words without a backslash: every distinct Greek letter, `infinity`,
  `neq`, `leq`, `geq`, `subset(eq)`, `supset(eq)`, `notin`, `cdots`,
  `ldots`.

**Editor support:**
- `Node::span` on every node, `Node::children()` for generic traversal,
  and `Node::node_at(offset)` to find the node under a cursor.
- Parsing is about 15× faster on long input.
- Nesting deeper than 48 levels is kept as plain text instead of being
  recursed into, so no input can overflow the stack. 5,000 levels of
  every construct parse and render on a 256 KB stack in release builds.

**Output formats** (in the new `output` module):
- `to_latex`: canonical LaTeX using standard commands only. For valid
  input it parses back to the same tree.
- `to_mathml` with `MathDisplay::{Inline, Block}`: MathML Core for native
  browser rendering. Unknown commands are wrapped in `<merror>`.
- `to_unicode`: readable plain text (`x² + √(y + 1)`).
- `to_svg` with `SvgOptions`: standalone SVG from a built-in layout
  engine.
  - Uses Times metrics, so it needs no font files.
  - Brackets, roots, big operators and accents are drawn as vector paths.
  - Includes baseline alignment for inline use and a `<title>` for screen
    readers.
  - Optionally adds `data-span` attributes that map glyphs back to the
    source.

**Syntax:**
- `align`, `gather` and `array` environments as above. `array` supports
  column specs (`c|c`, `*{3}{c}`, `p{..}`, `@{..}`) and `\hline`. Also
  `\substack{...}`.
- `\big`, `\Big`, `\bigg` and `\Bigg`, plus their `l`/`r`/`m` forms, as
  `SizedDelim`.
- `\boxed`, `\fbox`, `\cancel`, `\bcancel` and `\xcancel` as `Enclose`.
- `\textcolor{c}{x}` and the `\color{c}` switch as `Color`.
- Accents: `\underline`, `\overleftarrow`, `\overleftrightarrow`, `\acute`,
  `\grave`, `\breve`, `\check`, `\widecheck`, `\mathring` and `\dddot`.
- Fonts: `\mathit`, `\mathnormal`, `\textit`, `\emph` and `\textsl`
  (`Italic`); `\boldsymbol`, `\bm` and `\pmb` (`BoldItalic`).
- `\perp` is a relation. `\not` negates `⊢ ⊨ ⊑ ⊒ ≍` too.

**API:**
- `unicode_to_latex`, the reverse symbol lookup.
- `symbols()`: every command and its character, for autocomplete or
  palettes.
- `op_kind()`: the operator class of a symbol.
- `PartialEq` on `EqNode` and related types. `Eq` and `Hash` on the
  fieldless kind enums.

## 0.1.1

This is a bug-fix release with no API changes. Output only differs from
0.1.0 for input that 0.1.0 handled incorrectly. Valid input that already
parsed correctly gives the same tree as before.

### Fixed

- **Input is no longer silently dropped.** The parser used to stop at a
  stray `}`, `&`, `\\` or `\end{...}` and throw away everything after it.
  For example, `a & b` lost the `b`, and any unknown environment cut off the
  rest of the input. Stray tokens are now kept as text and parsing
  continues.
  - Unknown environments are kept as text.
  - A mismatched `\end{...}` is preserved.
  - An environment left open inside a group (e.g. half-typed
    `\frac{\begin{pmatrix} a}{b}`) closes at that group's `}`.
  - `&` and `\\` inside braces or `\left...\right` no longer split a
    matrix cell.
- `\left\{`, `\left\langle`, `\left\|`, `\left\lvert` and other command
  delimiters now parse correctly. `\left<` / `\right>` map to ⟨ ⟩.
- Whitespace before an argument or script no longer becomes the argument.
  For example, `\frac{a} {b}`, `sqrt x`, `a/ b` and `x^ 2` now parse as
  intended.
- `cases` rows with more than two `&` cells keep the extra cells instead of
  dropping them.
- A trailing `\\` followed by a newline before `\end{...}` no longer adds an
  extra empty row.
- Brace arguments such as environment names are now read with balanced
  braces.

### Added

These commands previously came out as literal `\command` text. They now map
onto the existing node types:

- `\dfrac`, `\tfrac`, `\cfrac`, `\dbinom` and `\tbinom`, and infix `\over` /
  `\choose`.
- `\operatorname{...}` / `\operatorname*{...}` (as `Limit`), plus `\Pr`,
  `\lg`, `\coth`, `\bmod`, `\pmod{..}` and `\mod{..}`.
- Big operators: `\bigcup`, `\bigcap`, `\bigoplus`, `\bigotimes`, `\bigodot`,
  `\biguplus`, `\bigsqcup`, `\bigvee`, `\bigwedge`, `\iiiint`, `\oiint` and
  `\oiiint`.
- `\limits`, `\nolimits`, `\displaystyle`, `\textstyle`, `\nonumber`,
  `\notag` and `\label{..}` are accepted and ignored.
- `smallmatrix` and `dcases` environments, plus `\\[2pt]` row spacing.
- `\middle` inside `\left...\right`.
- `\widehat`, `\widetilde` and `\overrightarrow`.
- `\mathscr`, `\Bbb`, `\mathup`, `\textbf`, `\textsf`, `\texttt`, `\textrm`
  and `\mbox`.
- `\not` negation (`\not\in` → `∉`, `\not=` → `≠`, ...).
- Extensible arrows: `\xrightarrow[below]{above}` and related commands (as
  `StackRel`).
- Spacing: `\hspace{..}`, `\kern`, `\mkern`, `\enspace`, `\thinspace` and
  related commands.
- About 140 new symbols (276 in total), including `\mid`, `\leqslant`,
  `\subsetneq`, `\longrightarrow`, `\models`, `\therefore` and `\checkmark`.

## 0.1.0

Initial release.
