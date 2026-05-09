//! Mermaid flowchart renderer for [`Cfg`].
//!
//! Renders a CFG as a Mermaid `flowchart TD` diagram.
//! Block shapes follow Mermaid conventions:
//! - `([label])` — entry/exit (stadium shape)
//! - `[label]`   — statement / loop-body / unreachable
//! - `{label}`   — branch / loop-head
//! - `((label))` — (not used currently)
//! - `[/label\]` — catch block

use crate::{BasicBlock, BlockId, BlockKind, Cfg, EdgeKind};

/// Render a [`Cfg`] as a Mermaid `flowchart TD` string.
pub fn render(cfg: &Cfg) -> String {
    let mut out = String::from("flowchart TD\n");

    // Emit block declarations.
    for block in &cfg.blocks {
        let label = block_label(block);
        let node_str = match block.kind {
            BlockKind::Entry | BlockKind::Exit => {
                format!("    {}([\"{}\"])\n", block_node_id(block.id), label)
            }
            BlockKind::Branch | BlockKind::LoopHead => {
                format!("    {}{{\"{}\"}}\n", block_node_id(block.id), label)
            }
            BlockKind::Catch => {
                format!("    {}[\"/{}\\\\\"]\n", block_node_id(block.id), label)
            }
            BlockKind::Statement
            | BlockKind::LoopBody
            | BlockKind::LoopExit
            | BlockKind::Unreachable
            | BlockKind::Deferred
            | BlockKind::Acquire
            | BlockKind::Release => {
                format!("    {}[\"{}\"]\n", block_node_id(block.id), label)
            }
        };
        out.push_str(&node_str);
    }

    out.push('\n');

    // Emit edges.
    for edge in &cfg.edges {
        let from = block_node_id(edge.from);
        let to = block_node_id(edge.to);
        let label_str = edge_label(&edge.kind);
        if label_str.is_empty() {
            out.push_str(&format!("    {} --> {}\n", from, to));
        } else {
            out.push_str(&format!("    {} -->|\"{}\"| {}\n", from, label_str, to));
        }
    }

    out
}

fn block_node_id(id: BlockId) -> String {
    format!("b{}", id.0)
}

fn block_label(block: &BasicBlock) -> String {
    let kind_str = match block.kind {
        BlockKind::Entry => return "entry".to_string(),
        BlockKind::Exit => return "exit".to_string(),
        BlockKind::Statement => "Statement",
        BlockKind::Branch => "Branch",
        BlockKind::LoopHead => "LoopHead",
        BlockKind::LoopBody => "LoopBody",
        BlockKind::LoopExit => "LoopExit",
        BlockKind::Catch => "Catch",
        BlockKind::Unreachable => "Unreachable",
        BlockKind::Deferred => "Deferred",
        BlockKind::Acquire => "Acquire",
        BlockKind::Release => "Release",
    };

    if block.start_line == block.end_line {
        format!("{}<br/>line {}", kind_str, block.start_line)
    } else {
        format!(
            "{}<br/>lines {}-{}",
            kind_str, block.start_line, block.end_line
        )
    }
}

fn edge_label(kind: &EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Fallthrough => "",
        EdgeKind::ConditionalTrue => "true",
        EdgeKind::ConditionalFalse => "false",
        EdgeKind::BackEdge => "back",
        EdgeKind::Break => "break",
        EdgeKind::Continue => "continue",
        EdgeKind::Return => "return",
        EdgeKind::Exception => "exception",
        EdgeKind::Suspend => "suspend",
        EdgeKind::Resume => "resume",
    }
}
