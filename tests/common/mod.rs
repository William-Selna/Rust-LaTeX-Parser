//! Shared helpers for integration tests.
#![allow(dead_code)]

use rust_latex_parser::{parse_equation, EqNode};

/// Render a tree as a compact S-expression, ignoring spacing so tests can
/// focus on structure. Whitespace-only text and `Space` nodes are dropped
/// and single-element sequences are unwrapped.
pub fn sx(node: &EqNode) -> String {
    fn opt(n: &Option<Box<EqNode>>) -> String {
        n.as_ref().map(|n| sx(n)).unwrap_or_else(|| "_".to_string())
    }
    fn rows(rows: &[Vec<EqNode>]) -> String {
        rows.iter()
            .map(|r| format!("[{}]", r.iter().map(sx).collect::<Vec<_>>().join(" ")))
            .collect::<Vec<_>>()
            .join(" ")
    }
    match node {
        EqNode::Text(s) => s.clone(),
        EqNode::Op { symbol, .. } => symbol.clone(),
        EqNode::Space(_) => String::new(),
        EqNode::Seq(nodes) => {
            let parts: Vec<String> = nodes
                .iter()
                .map(sx)
                .filter(|s| !s.trim().is_empty())
                .collect();
            if parts.len() == 1 {
                parts[0].clone()
            } else {
                format!("(seq {})", parts.join(" "))
            }
        }
        EqNode::Sup(b, s) => format!("(sup {} {})", sx(b), sx(s)),
        EqNode::Sub(b, s) => format!("(sub {} {})", sx(b), sx(s)),
        EqNode::SupSub(b, p, s) => format!("(supsub {} {} {})", sx(b), sx(p), sx(s)),
        EqNode::Frac(n, d) => format!("(frac {} {})", sx(n), sx(d)),
        EqNode::Sqrt(c) => format!("(sqrt {})", sx(c)),
        EqNode::Root { index, content } => format!("(root {} {})", sx(index), sx(content)),
        EqNode::BigOp { symbol, lower, upper } => format!("(bigop {} {} {})", symbol, opt(lower), opt(upper)),
        EqNode::Accent(c, k) => format!("(accent {:?} {})", k, sx(c)),
        EqNode::Limit { name, lower } => format!("(limit {} {})", name, opt(lower)),
        EqNode::TextBlock(s) => format!("(text {:?})", s),
        EqNode::MathFont { kind, content } => format!("(font {:?} {})", kind, sx(content)),
        EqNode::Delimited { left, right, content } => format!("(delim {:?} {:?} {})", left, right, sx(content)),
        EqNode::SizedDelim { delim, size } => format!("(sized {} {:?})", size, delim),
        EqNode::Matrix { kind, rows: r } => format!("(matrix {:?} {})", kind, rows(r)),
        EqNode::Cases { rows: r } => format!(
            "(cases {})",
            r.iter()
                .map(|(v, c)| format!("[{} {}]", sx(v), c.as_ref().map(sx).unwrap_or_else(|| "_".into())))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        EqNode::Align { rows: r } => format!("(align {})", rows(r)),
        EqNode::Gather { rows: r } => format!("(gather {})", r.iter().map(sx).collect::<Vec<_>>().join(" ")),
        EqNode::Array { columns, rows: r, hlines } => format!("(array {:?} {:?} {})", columns, hlines, rows(r)),
        EqNode::Binom(a, b) => format!("(binom {} {})", sx(a), sx(b)),
        EqNode::Brace { content, label, over } => format!("(brace {} {} {})", over, sx(content), opt(label)),
        EqNode::StackRel { base, annotation, over } => format!("(stack {} {} {})", over, sx(base), sx(annotation)),
        EqNode::Color { color, content } => format!("(color {} {})", color, sx(content)),
        EqNode::Enclose { kind, content } => format!("(enclose {:?} {})", kind, sx(content)),
        other => format!("{:?}", other),
    }
}

/// Parse and render as an S-expression.
pub fn p(input: &str) -> String {
    sx(&parse_equation(input))
}

/// Concatenate every piece of visible text in the tree (text, symbols,
/// operator names, delimiters, colors...), in no particular order.
pub fn all_text(node: &EqNode) -> String {
    let mut out = String::new();
    collect(node, &mut out);
    out
}

fn collect(node: &EqNode, out: &mut String) {
    let mut each = |n: &EqNode| collect(n, out);
    match node {
        EqNode::Text(s) | EqNode::TextBlock(s) | EqNode::Op { symbol: s, .. } => out.push_str(s),
        EqNode::Space(_) => {}
        EqNode::Seq(v) => v.iter().for_each(each),
        EqNode::Sup(a, b) | EqNode::Sub(a, b) | EqNode::Frac(a, b) | EqNode::Binom(a, b) => {
            each(a);
            each(b);
        }
        EqNode::SupSub(a, b, c) => {
            each(a);
            each(b);
            each(c);
        }
        EqNode::Sqrt(c) | EqNode::Accent(c, _) | EqNode::MathFont { content: c, .. } | EqNode::Enclose { content: c, .. } => each(c),
        EqNode::Root { index, content } => {
            each(index);
            each(content);
        }
        EqNode::BigOp { symbol, lower, upper } => {
            out.push_str(symbol);
            lower.iter().chain(upper.iter()).for_each(|n| collect(n, out));
        }
        EqNode::Limit { name, lower } => {
            out.push_str(name);
            lower.iter().for_each(|n| collect(n, out));
        }
        EqNode::Delimited { left, right, content } => {
            out.push_str(left);
            out.push_str(right);
            collect(content, out);
        }
        EqNode::SizedDelim { delim, .. } => out.push_str(delim),
        EqNode::Matrix { rows, .. } | EqNode::Align { rows } | EqNode::Array { rows, .. } => {
            rows.iter().flatten().for_each(|n| collect(n, out))
        }
        EqNode::Gather { rows } => rows.iter().for_each(|n| collect(n, out)),
        EqNode::Cases { rows } => {
            for (v, c) in rows {
                collect(v, out);
                c.iter().for_each(|n| collect(n, out));
            }
        }
        EqNode::Brace { content, label, .. } => {
            collect(content, out);
            label.iter().for_each(|n| collect(n, out));
        }
        EqNode::StackRel { base, annotation, .. } => {
            collect(base, out);
            collect(annotation, out);
        }
        EqNode::Color { color, content } => {
            out.push_str(color);
            collect(content, out);
        }
        other => panic!("all_text: unhandled node {:?}", other),
    }
}
