//! Container body analysis helpers.
//!
//! Free functions for analyzing container bodies in different delimiter styles.
//! Called from `Language::analyze_container_body` implementations.

use crate::ContainerBody;
use tree_sitter::Node;

/// Analyze a brace-delimited container body (`{ ... }`).
///
/// Expects `body_node` to span from `{` to `}` inclusive (as is standard in
/// tree-sitter grammars for C-family and similar languages).
pub(crate) fn analyze_brace_body(
    body_node: &Node,
    content: &str,
    inner_indent: &str,
) -> Option<ContainerBody> {
    let body_start = body_node.start_byte();
    let body_end = body_node.end_byte();

    let mut content_start = body_start;
    for (i, byte) in content[body_start..body_end].bytes().enumerate() {
        if byte == b'{' {
            content_start = body_start + i + 1;
            while content_start < body_end {
                let b = content.as_bytes()[content_start];
                if b == b'\n' {
                    content_start += 1;
                    break;
                } else if b.is_ascii_whitespace() {
                    content_start += 1;
                } else {
                    break;
                }
            }
            break;
        }
    }

    let mut content_end = body_end;
    for (i, byte) in content[body_start..body_end].bytes().rev().enumerate() {
        if byte == b'}' {
            content_end = body_end - i - 1;
            while content_end > content_start && content.as_bytes()[content_end - 1] == b' ' {
                content_end -= 1;
            }
            break;
        }
    }

    let is_empty = content[content_start..content_end].trim().is_empty();

    Some(ContainerBody {
        content_start,
        content_end,
        inner_indent: inner_indent.to_string(),
        is_empty,
    })
}

/// Analyze a container body with no surrounding delimiters.
///
/// Used for languages where the body node contains declarations directly,
/// without any enclosing keywords or braces (e.g., Ruby `body_statement`,
/// Julia module/struct bodies).
pub(crate) fn analyze_end_body(
    body_node: &Node,
    content: &str,
    inner_indent: &str,
) -> Option<ContainerBody> {
    let body_start = body_node.start_byte();
    let body_end = body_node.end_byte();

    // Skip leading newline if the body node begins immediately after the header line
    let content_start = if body_start < body_end && content.as_bytes()[body_start] == b'\n' {
        body_start + 1
    } else {
        body_start
    };

    let content_end = body_end;
    let is_empty = content[content_start..content_end].trim().is_empty();

    Some(ContainerBody {
        content_start,
        content_end,
        inner_indent: inner_indent.to_string(),
        is_empty,
    })
}

/// Analyze a parenthesis-delimited container body (`( ... )`).
///
/// Used for RON structs: `Foo(field: value, ...)`.
pub(crate) fn analyze_paren_body(
    body_node: &Node,
    content: &str,
    inner_indent: &str,
) -> Option<ContainerBody> {
    let body_start = body_node.start_byte();
    let body_end = body_node.end_byte();

    let mut content_start = body_start;
    for (i, byte) in content[body_start..body_end].bytes().enumerate() {
        if byte == b'(' {
            content_start = body_start + i + 1;
            while content_start < body_end {
                let b = content.as_bytes()[content_start];
                if b == b'\n' {
                    content_start += 1;
                    break;
                } else if b.is_ascii_whitespace() {
                    content_start += 1;
                } else {
                    break;
                }
            }
            break;
        }
    }

    let mut content_end = body_end;
    for (i, byte) in content[body_start..body_end].bytes().rev().enumerate() {
        if byte == b')' {
            content_end = body_end - i - 1;
            while content_end > content_start && content.as_bytes()[content_end - 1] == b' ' {
                content_end -= 1;
            }
            break;
        }
    }

    let is_empty = content[content_start..content_end].trim().is_empty();

    Some(ContainerBody {
        content_start,
        content_end,
        inner_indent: inner_indent.to_string(),
        is_empty,
    })
}

/// Analyze a body delimited by `is`/`begin` ... `end` (Ada, VHDL style).
///
/// Scans children for `is` and `begin` keywords; the last one found sets
/// `content_start`. Then finds the `end` keyword to set `content_end`.
/// This handles both `is ... end` (Ada packages, VHDL entities/packages)
/// and `is ... begin ... end` (VHDL architecture bodies).
pub(crate) fn analyze_is_begin_end_body(
    body_node: &Node,
    content: &str,
    inner_indent: &str,
) -> Option<ContainerBody> {
    let start = body_node.start_byte();
    let end = body_node.end_byte();
    let bytes = content.as_bytes();

    let mut content_start = start;
    let mut c = body_node.walk();
    for child in body_node.children(&mut c) {
        if matches!(child.kind(), "is" | "begin") {
            content_start = child.end_byte();
            if content_start < end && bytes[content_start] == b'\n' {
                content_start += 1;
            }
            // Don't break — "begin" may follow "is" and supersede it
        }
    }

    let mut content_end = end;
    let mut c2 = body_node.walk();
    for child in body_node.children(&mut c2) {
        if child.kind() == "end" {
            content_end = child.start_byte();
            while content_end > content_start && matches!(bytes[content_end - 1], b' ' | b'\t') {
                content_end -= 1;
            }
            break;
        }
    }

    let is_empty = content[content_start..content_end].trim().is_empty();

    Some(ContainerBody {
        content_start,
        content_end,
        inner_indent: inner_indent.to_string(),
        is_empty,
    })
}

/// Analyze an Elixir-style `do ... end` block.
///
/// Expects `body_node` to be a `do_block` spanning from `do` to `end`
/// (exclusive of the trailing newline). A thin wrapper around
/// [`analyze_keyword_end_body`].
pub(crate) fn analyze_do_end_body(
    body_node: &Node,
    content: &str,
    inner_indent: &str,
) -> Option<ContainerBody> {
    analyze_keyword_end_body(body_node, content, inner_indent)
}

/// Analyze a body node that opens with an arbitrary keyword on the first line
/// and closes with `end` (e.g., OCaml `struct...end`, `sig...end`; Elixir
/// `do...end`).
///
/// Skips the entire first line (the opening keyword line) for `content_start`,
/// and strips the trailing `end` for `content_end`.
pub(crate) fn analyze_keyword_end_body(
    body_node: &Node,
    content: &str,
    inner_indent: &str,
) -> Option<ContainerBody> {
    let body_start = body_node.start_byte();
    let body_end = body_node.end_byte();
    let bytes = content.as_bytes();

    // Skip past the first line (opening keyword: "do", "struct", "sig", etc.)
    let mut content_start = body_start;
    while content_start < body_end && bytes[content_start] != b'\n' {
        content_start += 1;
    }
    if content_start < body_end && bytes[content_start] == b'\n' {
        content_start += 1;
    }

    // Strip "end" from the tail: body_end - 3 should be the start of "end"
    let mut content_end = body_end;
    if body_end >= 3 && bytes.get(body_end - 3..body_end) == Some(b"end") {
        content_end = body_end - 3;
        // Strip indentation (spaces/tabs) before "end", but not newlines —
        // we want content[content_end..] to start with "\nend" or "end"
        while content_end > content_start && matches!(bytes[content_end - 1], b' ' | b'\t') {
            content_end -= 1;
        }
    }

    let is_empty = content[content_start..content_end].trim().is_empty();

    Some(ContainerBody {
        content_start,
        content_end,
        inner_indent: inner_indent.to_string(),
        is_empty,
    })
}
