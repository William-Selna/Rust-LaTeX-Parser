# Changelog

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
