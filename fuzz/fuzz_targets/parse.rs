//! Fuzz the parser and every output format.
//!
//! Run with `cargo +nightly fuzz run parse -- -dict=fuzz/latex.dict`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rust_latex_parser::{
    parse_equation, to_latex, to_mathml, to_svg, to_unicode, MathDisplay, Node, SvgOptions,
};

/// Spans in bounds, on char boundaries, children inside parents; returns
/// the tree depth. Iterative so a deep tree can't overflow the fuzzer.
fn check_tree(input: &str, root: &Node) -> usize {
    let mut max = 0;
    let mut stack = vec![(root, 1usize)];
    while let Some((node, depth)) = stack.pop() {
        max = max.max(depth);
        let s = node.span;
        assert!(s.start <= s.end && s.end <= input.len());
        assert!(input.is_char_boundary(s.start) && input.is_char_boundary(s.end));
        for child in node.children() {
            if !child.span.is_empty() && !s.is_empty() {
                assert!(s.start <= child.span.start && child.span.end <= s.end);
            }
            stack.push((child, depth + 1));
        }
    }
    max
}

/// Every character is legal in XML 1.0 and tags balance.
fn check_xml(xml: &str) {
    assert!(xml.chars().all(|c| matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}')));
    let mut depth: i64 = 0;
    let mut rest = xml;
    while let Some(i) = rest.find('<') {
        let end = rest[i..].find('>').expect("unclosed tag") + i;
        let tag = &rest[i + 1..end];
        if tag.starts_with('/') {
            depth -= 1;
        } else if !tag.ends_with('/') {
            depth += 1;
        }
        assert!(depth >= 0);
        rest = &rest[end + 1..];
    }
    assert_eq!(depth, 0);
}

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let tree = parse_equation(&input);
    let depth = check_tree(&input, &tree);
    assert!(depth < 200, "tree depth {}", depth);

    let latex = to_latex(&tree);
    if depth < 20 {
        // Canonical LaTeX settles after at most one more round.
        let again = to_latex(&parse_equation(&latex));
        assert_eq!(again, to_latex(&parse_equation(&again)));
    }
    check_xml(&to_mathml(&tree, MathDisplay::Block));
    check_xml(&to_svg(&tree, &SvgOptions::default()));
    let _ = to_unicode(&tree);
});
