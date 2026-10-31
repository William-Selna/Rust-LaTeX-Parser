# Changelog

## 0.2.0

### Breaking changes

- `EqNode` has new variants: `Root`, `SizedDelim`, `Align`, `Gather`, `Array`,
  `Color` and `Enclose`. `AccentKind` and `MathFontKind` also have new
  variants, and there are two new enums, `EncloseKind` and `ArrayColumn`.
- `EqNode`, `AccentKind`, `MathFontKind`, `MatrixKind`, `EncloseKind` and
  `ArrayColumn` are now `#[non_exhaustive]`. Future releases can add syntax
  without breaking your code.

**To upgrade:** add a `_ =>` arm to each `match` on these enums. Handle the
new variants explicitly if your renderer should support them.

### Output changes

These change the tree for input that 0.1.x already accepted:

- `\sqrt[n]{x}` produces `Root`. In 0.1.x it produced a broken `Sqrt`.
- Environments that 0.1.x kept as literal text now produce real nodes:
  `align`, `align*`, `aligned`, `split`, `alignat`, `alignedat`, `flalign`
  and `eqnarray` produce `Align`. `gather` and `gathered` produce `Gather`.
  `array` and `subarray` produce `Array`. `equation` is unwrapped.
- Primes become superscripts: `f'` is `Sup(f, "′")` instead of `f` followed
  by a literal `'`.
- `%` starts a comment that runs to the end of the line. Use `\%` for a
  literal percent sign.
- `\ldots` / `\dots` produce baseline `…` (U+2026) instead of `⋯`.
  `\cdots` is unchanged. `\dotsc`, `\dotsb` and related commands are added.
- `\varsigma`, `\vartheta`, `\varpi` and `\varrho` map to their own glyphs
  (ς ϑ ϖ ϱ) instead of the plain letters.
- `\|` produces `‖` instead of `|`.
- `\notin`, `\ni`, `\parallel`, `\simeq`, `\mapsto`, `\hookrightarrow` and
  `\hookleftarrow` get operator spacing like the other relations.
- A run of whitespace (including newlines) becomes a single `Text(" ")`
  node. Previously each character became its own node, and newlines were
  kept as `Text("\n")`.
- All whitespace after a command name is skipped, as in TeX. Previously
  only one space was skipped.

### Added

- `\sqrt[n]{x}` (and the bareword `sqrt[n](x)`) as `Root`.
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
