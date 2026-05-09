//! CFG builder: constructs a [`Cfg`] from a tree-sitter parse tree and a `.cfg.scm` query.
//!
//! # Algorithm
//!
//! The builder does a structured walk of the function body using CFG query captures:
//! - `@cfg.branch` nodes become branch blocks with conditional edges to then/else sub-CFGs.
//! - `@cfg.loop` nodes become loop-head/body/exit triples with back-edges.
//! - `@cfg.match` nodes fan out one block per arm, all arms merge to a join block.
//! - `@cfg.exit.*` nodes terminate the current block and open an `Unreachable` continuation.
//! - All other statement nodes are appended to the current sequential block.

use crate::{
    BasicBlock, BlockId, BlockKind, Cfg, DefSite, Edge, EdgeKind, Effect, EffectKind, FunctionId,
    UseSite,
};
use std::ops::Range;
use streaming_iterator::StreamingIterator;

/// Error returned by the CFG builder.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// The provided query string could not be compiled by tree-sitter.
    #[error("failed to compile CFG query: {0}")]
    QueryCompile(String),
    /// No function body was found at the specified byte range.
    #[error("no function body found at byte range {0}..{1}")]
    NoBody(usize, usize),
}

// ---------------------------------------------------------------------------
// Internal builder state
// ---------------------------------------------------------------------------

struct Builder {
    blocks: Vec<BasicBlock>,
    edges: Vec<Edge>,
    next_id: u32,
    /// Stack of (loop_head, loop_exit) pairs for break/continue resolution.
    loop_stack: Vec<(BlockId, BlockId)>,
}

impl Builder {
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            edges: Vec::new(),
            next_id: 0,
            loop_stack: Vec::new(),
        }
    }

    fn alloc_block(&mut self, kind: BlockKind, byte_range: Range<usize>, line: u32) -> BlockId {
        let id = BlockId(self.next_id);
        self.next_id += 1;
        self.blocks.push(BasicBlock {
            id,
            byte_range,
            start_line: line,
            end_line: line,
            kind,
            defs: Vec::new(),
            uses: Vec::new(),
            effects: Vec::new(),
        });
        id
    }

    fn add_edge(&mut self, from: BlockId, to: BlockId, kind: EdgeKind) {
        self.edges.push(Edge { from, to, kind });
    }

    fn block_mut(&mut self, id: BlockId) -> &mut BasicBlock {
        self.blocks
            .iter_mut()
            .find(|b| b.id == id)
            .unwrap_or_else(|| panic!("CFG internal error: block {:?} not found", id))
    }
}

// ---------------------------------------------------------------------------
// Capture classification
// ---------------------------------------------------------------------------

/// Parsed classification of a node based on CFG query capture names.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CaptureKind {
    Branch,
    BranchCondition,
    BranchThen,
    BranchElse,
    Loop,
    LoopCondition,
    LoopBody,
    Match,
    MatchScrutinee,
    MatchArm,
    Try,
    TryBody,
    TryCatch,
    TryFinally,
    ExitReturn,
    ExitBreak,
    ExitContinue,
    ExitThrow,
    /// Variable/binding definition site (`@cfg.def`).
    Def,
    /// The identifier name node for a def (`@cfg.def.name`).
    DefName,
    /// Variable use site (`@cfg.use`).
    Use,
    /// The identifier name node for a use (`@cfg.use.name`).
    UseName,
    // Effect captures — @cfg.effect.*
    EffectAwait,
    EffectDefer,
    EffectYield,
    EffectAcquire,
    EffectRelease,
    EffectSend,
    EffectReceive,
}

fn parse_capture_name(name: &str) -> Option<CaptureKind> {
    match name {
        "cfg.branch" => Some(CaptureKind::Branch),
        "cfg.branch.condition" => Some(CaptureKind::BranchCondition),
        "cfg.branch.then" => Some(CaptureKind::BranchThen),
        "cfg.branch.else" => Some(CaptureKind::BranchElse),
        "cfg.loop" => Some(CaptureKind::Loop),
        "cfg.loop.condition" => Some(CaptureKind::LoopCondition),
        "cfg.loop.body" => Some(CaptureKind::LoopBody),
        "cfg.match" => Some(CaptureKind::Match),
        "cfg.match.scrutinee" => Some(CaptureKind::MatchScrutinee),
        "cfg.match.arm" => Some(CaptureKind::MatchArm),
        "cfg.try" => Some(CaptureKind::Try),
        "cfg.try.body" => Some(CaptureKind::TryBody),
        "cfg.try.catch" => Some(CaptureKind::TryCatch),
        "cfg.try.finally" => Some(CaptureKind::TryFinally),
        "cfg.exit.return" => Some(CaptureKind::ExitReturn),
        "cfg.exit.break" => Some(CaptureKind::ExitBreak),
        "cfg.exit.continue" => Some(CaptureKind::ExitContinue),
        "cfg.exit.throw" => Some(CaptureKind::ExitThrow),
        "cfg.def" => Some(CaptureKind::Def),
        "cfg.def.name" => Some(CaptureKind::DefName),
        "cfg.use" => Some(CaptureKind::Use),
        "cfg.use.name" => Some(CaptureKind::UseName),
        "cfg.effect.await" => Some(CaptureKind::EffectAwait),
        "cfg.effect.defer" => Some(CaptureKind::EffectDefer),
        "cfg.effect.yield" => Some(CaptureKind::EffectYield),
        "cfg.effect.acquire" => Some(CaptureKind::EffectAcquire),
        "cfg.effect.release" => Some(CaptureKind::EffectRelease),
        "cfg.effect.send" => Some(CaptureKind::EffectSend),
        "cfg.effect.receive" => Some(CaptureKind::EffectReceive),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Classified node
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct ClassifiedNode {
    kind: CaptureKind,
    byte_range: Range<usize>,
    start_line: u32,
    end_line: u32,
    /// For Branch: optional condition/then/else child byte ranges (from sub-captures).
    branch_condition: Option<Range<usize>>,
    branch_then: Option<Range<usize>>,
    branch_else: Option<Range<usize>>,
    /// For Loop: optional condition and body byte ranges.
    loop_condition: Option<Range<usize>>,
    loop_body: Option<Range<usize>>,
    /// For Match: list of arm byte ranges.
    match_arms: Vec<Range<usize>>,
    /// For Try: body/catch/finally byte ranges.
    try_body: Option<Range<usize>>,
    try_catches: Vec<Range<usize>>,
    try_finally: Option<Range<usize>>,
}

// ---------------------------------------------------------------------------
// Sequence result
// ---------------------------------------------------------------------------

/// Result of building a sub-sequence of blocks.
struct SequenceResult {
    /// The tail block after the sequence.
    tail: BlockId,
    /// Whether the sequence terminated unconditionally (return/break/continue/throw).
    terminated: bool,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build a CFG from a tree-sitter tree, a query string, source bytes, and function metadata.
///
/// `body_range` is the byte range of the function body within the source. The builder
/// only processes nodes within this range.
pub fn build(
    tree: &tree_sitter::Tree,
    query_source: &str,
    source: &[u8],
    function_id: FunctionId,
    body_range: Range<usize>,
) -> Result<Cfg, BuildError> {
    let language = tree.language();
    let query = tree_sitter::Query::new(&language, query_source)
        .map_err(|e| BuildError::QueryCompile(e.to_string()))?;

    // Collect all captures inside the body range, grouped by their top-level node.
    let mut cursor = tree_sitter::QueryCursor::new();
    cursor.set_byte_range(body_range.start..body_range.end);

    // Build a list of top-level CFG control-flow nodes (branches, loops, exits).
    // We need to identify the top-level structure nodes and their children.
    let capture_names = query.capture_names().to_vec();

    // First pass: collect all "structural" nodes (branch/loop/match/try) and their sub-captures.
    // We'll build a flat list of events sorted by byte offset.
    let mut structural_nodes: Vec<ClassifiedNode> = Vec::new();

    // Collect match data: (primary_capture_name_idx, primary_node_range, sub_captures)
    // We collect into owned structures so we can release the borrow from the streaming iterator.
    struct MatchData {
        primary_name_idx: usize,
        primary_start_byte: usize,
        primary_end_byte: usize,
        primary_start_row: usize,
        primary_end_row: usize,
        sub_captures: Vec<(usize, usize, usize)>, // (name_idx, start_byte, end_byte)
    }

    let mut raw_matches: Vec<MatchData> = Vec::new();
    let mut matches_iter = cursor.matches(&query, tree.root_node(), source);
    while let Some(m) = matches_iter.next() {
        let primary = m.captures.iter().find(|c| {
            let name = capture_names[c.index as usize];
            matches!(
                name,
                "cfg.branch"
                    | "cfg.loop"
                    | "cfg.match"
                    | "cfg.try"
                    | "cfg.exit.return"
                    | "cfg.exit.break"
                    | "cfg.exit.continue"
                    | "cfg.exit.throw"
            )
        });
        let Some(primary_cap) = primary else {
            continue;
        };
        let pnode = primary_cap.node;
        let subs: Vec<_> = m
            .captures
            .iter()
            .map(|c| (c.index as usize, c.node.start_byte(), c.node.end_byte()))
            .collect();
        raw_matches.push(MatchData {
            primary_name_idx: primary_cap.index as usize,
            primary_start_byte: pnode.start_byte(),
            primary_end_byte: pnode.end_byte(),
            primary_start_row: pnode.start_position().row,
            primary_end_row: pnode.end_position().row,
            sub_captures: subs,
        });
    }
    drop(matches_iter);

    for mat in &raw_matches {
        let primary_name = capture_names[mat.primary_name_idx];
        let Some(kind) = parse_capture_name(primary_name) else {
            continue;
        };

        let start = mat.primary_start_byte;
        let end = mat.primary_end_byte;

        // Skip nodes outside body range
        if start < body_range.start || end > body_range.end {
            continue;
        }

        let start_line = mat.primary_start_row as u32 + 1;
        let end_line = mat.primary_end_row as u32 + 1;

        let mut cn = ClassifiedNode {
            kind,
            byte_range: start..end,
            start_line,
            end_line,
            branch_condition: None,
            branch_then: None,
            branch_else: None,
            loop_condition: None,
            loop_body: None,
            match_arms: Vec::new(),
            try_body: None,
            try_catches: Vec::new(),
            try_finally: None,
        };

        // Collect sub-captures
        for &(name_idx, sub_start, sub_end) in &mat.sub_captures {
            let name = capture_names[name_idx];
            let cr = sub_start..sub_end;
            match name {
                "cfg.branch.condition" => cn.branch_condition = Some(cr),
                "cfg.branch.then" => cn.branch_then = Some(cr),
                "cfg.branch.else" => cn.branch_else = Some(cr),
                "cfg.loop.condition" => cn.loop_condition = Some(cr),
                "cfg.loop.body" => cn.loop_body = Some(cr),
                "cfg.match.arm" => cn.match_arms.push(cr),
                "cfg.try.body" => cn.try_body = Some(cr),
                "cfg.try.catch" => cn.try_catches.push(cr),
                "cfg.try.finally" => cn.try_finally = Some(cr),
                _ => {}
            }
        }

        structural_nodes.push(cn);
    }

    // Sort by start byte and deduplicate (tree-sitter can produce duplicate matches).
    structural_nodes.sort_by_key(|n| n.byte_range.start);
    structural_nodes.dedup_by_key(|n| n.byte_range.start);

    // Filter out nested structural nodes — keep only top-level ones within the body.
    // A node is top-level if no other structural node's range fully contains it.
    let top_level = filter_top_level(structural_nodes);

    // --- Second pass: collect def/use sites ---
    // Raw (byte_offset, name, line, is_def) tuples, collected before building blocks.
    struct RawDefUse {
        byte_offset: usize,
        name: String,
        line: u32,
        is_def: bool,
    }
    let mut raw_def_use: Vec<RawDefUse> = Vec::new();

    // We need to check if the query has any def/use captures at all.
    let has_def_use = capture_names
        .iter()
        .any(|n| matches!(*n, "cfg.def" | "cfg.def.name" | "cfg.use" | "cfg.use.name"));

    if has_def_use {
        let mut cursor2 = tree_sitter::QueryCursor::new();
        cursor2.set_byte_range(body_range.start..body_range.end);

        // Collect raw matches for def/use
        struct DefUseMatchData {
            // Primary capture: cfg.def or cfg.use
            primary_start: usize,
            primary_end: usize,
            primary_start_row: usize,
            is_def: bool,
            // Optional name sub-capture
            name_start: usize,
            name_end: usize,
            name_start_row: usize,
            has_name: bool,
        }
        let mut du_matches: Vec<DefUseMatchData> = Vec::new();

        let mut du_iter = cursor2.matches(&query, tree.root_node(), source);
        while let Some(m) = du_iter.next() {
            let primary = m.captures.iter().find(|c| {
                let name = capture_names[c.index as usize];
                matches!(name, "cfg.def" | "cfg.use")
            });
            let Some(primary_cap) = primary else {
                continue;
            };
            let is_def = capture_names[primary_cap.index as usize] == "cfg.def";
            let pnode = primary_cap.node;

            // Look for the .name sub-capture
            let name_cap = m.captures.iter().find(|c| {
                let n = capture_names[c.index as usize];
                (is_def && n == "cfg.def.name") || (!is_def && n == "cfg.use.name")
            });

            let (name_start, name_end, name_start_row, has_name) = if let Some(nc) = name_cap {
                (
                    nc.node.start_byte(),
                    nc.node.end_byte(),
                    nc.node.start_position().row,
                    true,
                )
            } else {
                (
                    pnode.start_byte(),
                    pnode.end_byte(),
                    pnode.start_position().row,
                    false,
                )
            };

            du_matches.push(DefUseMatchData {
                primary_start: pnode.start_byte(),
                primary_end: pnode.end_byte(),
                primary_start_row: pnode.start_position().row,
                is_def,
                name_start,
                name_end,
                name_start_row,
                has_name,
            });
        }
        drop(du_iter);

        for mat in &du_matches {
            // Skip nodes outside body range
            if mat.primary_start < body_range.start || mat.primary_end > body_range.end {
                continue;
            }
            let byte_offset = if mat.has_name {
                mat.name_start
            } else {
                mat.primary_start
            };
            let line = if mat.has_name {
                row_to_line(mat.name_start_row)
            } else {
                row_to_line(mat.primary_start_row)
            };
            let name_bytes = &source[mat.name_start..mat.name_end];
            let name = String::from_utf8_lossy(name_bytes).into_owned();
            raw_def_use.push(RawDefUse {
                byte_offset,
                name,
                line,
                is_def: mat.is_def,
            });
        }
    }

    // Build CFG from the top-level structural nodes.
    let mut b = Builder::new();

    let entry_line = row_to_line(tree.root_node().start_position().row);
    let entry = b.alloc_block(BlockKind::Entry, 0..0, entry_line);
    let exit_line = entry_line;
    let exit = b.alloc_block(BlockKind::Exit, 0..0, exit_line);

    let seq = build_sequence(&mut b, &top_level, entry, &body_range, source, exit);

    if !seq.terminated {
        b.add_edge(seq.tail, exit, EdgeKind::Fallthrough);
    }

    // Assign def/use sites to blocks: each site goes to the block whose byte_range
    // contains the site's byte_offset. Falls back to entry block if none found.
    for du in raw_def_use {
        let block_id = b
            .blocks
            .iter()
            .find(|blk| {
                !blk.byte_range.is_empty()
                    && blk.byte_range.start <= du.byte_offset
                    && du.byte_offset < blk.byte_range.end
            })
            .map(|blk| blk.id)
            .unwrap_or(entry);
        let blk = b.block_mut(block_id);
        if du.is_def {
            blk.defs.push(DefSite {
                name: du.name,
                byte_offset: du.byte_offset,
                line: du.line,
            });
        } else {
            blk.uses.push(UseSite {
                name: du.name,
                byte_offset: du.byte_offset,
                line: du.line,
            });
        }
    }

    // --- Effects pass ---
    // Collect effect captures and assign them to blocks by byte range.
    let has_effects = capture_names.iter().any(|n| n.starts_with("cfg.effect."));

    if has_effects {
        struct RawEffect {
            kind: EffectKind,
            byte_offset: usize,
            line: u32,
            label: Option<String>,
        }
        let mut raw_effects: Vec<RawEffect> = Vec::new();

        let mut eff_cursor = tree_sitter::QueryCursor::new();
        eff_cursor.set_byte_range(body_range.start..body_range.end);

        // Collect all effect captures as owned data.
        struct EffMatchData {
            name_idx: usize,
            start_byte: usize,
            end_byte: usize,
            start_row: usize,
        }
        let mut eff_matches: Vec<EffMatchData> = Vec::new();
        let mut eff_iter = eff_cursor.matches(&query, tree.root_node(), source);
        while let Some(m) = eff_iter.next() {
            for cap in m.captures {
                let cap_name = capture_names[cap.index as usize];
                if cap_name.starts_with("cfg.effect.") {
                    eff_matches.push(EffMatchData {
                        name_idx: cap.index as usize,
                        start_byte: cap.node.start_byte(),
                        end_byte: cap.node.end_byte(),
                        start_row: cap.node.start_position().row,
                    });
                }
            }
        }
        drop(eff_iter);

        for mat in &eff_matches {
            if mat.start_byte < body_range.start || mat.end_byte > body_range.end {
                continue;
            }
            let cap_name = capture_names[mat.name_idx];
            let kind = match cap_name {
                "cfg.effect.await" => EffectKind::Await,
                "cfg.effect.defer" => EffectKind::Defer,
                "cfg.effect.yield" => EffectKind::Yield,
                "cfg.effect.acquire" => EffectKind::Acquire,
                "cfg.effect.release" => EffectKind::Release,
                "cfg.effect.send" => EffectKind::Send,
                "cfg.effect.receive" => EffectKind::Receive,
                _ => continue,
            };
            let text = String::from_utf8_lossy(&source[mat.start_byte..mat.end_byte]).into_owned();
            // Truncate label to 120 chars so it stays useful without bloating the DB.
            let label = if text.len() > 120 {
                Some(format!("{}…", &text[..120]))
            } else {
                Some(text)
            };
            raw_effects.push(RawEffect {
                kind,
                byte_offset: mat.start_byte,
                line: row_to_line(mat.start_row),
                label,
            });
        }

        // Assign effects to blocks.
        for eff in raw_effects {
            let block_id = b
                .blocks
                .iter()
                .find(|blk| {
                    !blk.byte_range.is_empty()
                        && blk.byte_range.start <= eff.byte_offset
                        && eff.byte_offset < blk.byte_range.end
                })
                .map(|blk| blk.id)
                .unwrap_or(entry);
            b.block_mut(block_id).effects.push(Effect {
                kind: eff.kind,
                byte_offset: eff.byte_offset,
                line: eff.line,
                label: eff.label,
            });
        }
    }

    Ok(Cfg {
        function: function_id,
        blocks: b.blocks,
        edges: b.edges,
        entry,
        exit,
    })
}

/// Filter to top-level nodes: remove any node that is fully contained within another.
fn filter_top_level(nodes: Vec<ClassifiedNode>) -> Vec<ClassifiedNode> {
    let mut result: Vec<ClassifiedNode> = Vec::new();
    'outer: for node in nodes {
        for other in &result {
            if other.byte_range.start <= node.byte_range.start
                && other.byte_range.end >= node.byte_range.end
                && other.byte_range != node.byte_range
            {
                continue 'outer;
            }
        }
        result.push(node);
    }
    result
}

fn row_to_line(row: usize) -> u32 {
    row as u32 + 1
}

/// Build a sequence of blocks from a list of top-level structural nodes.
///
/// Returns a [`SequenceResult`] with the tail block and a terminated flag.
fn build_sequence(
    b: &mut Builder,
    nodes: &[ClassifiedNode],
    entry: BlockId,
    body_range: &Range<usize>,
    source: &[u8],
    exit: BlockId,
) -> SequenceResult {
    let mut current = entry;
    let mut terminated = false;
    let mut prev_end = body_range.start;

    for node in nodes {
        // If there's a gap between prev_end and this node's start, those bytes
        // are sequential statements — extend the current block to cover them.
        if node.byte_range.start > prev_end && !terminated {
            let gap_start = prev_end;
            let gap_end = node.byte_range.start;
            // Find the line from source bytes.
            let line = byte_offset_to_line(source, gap_start);
            let end_line = byte_offset_to_line(source, gap_end.saturating_sub(1));
            let cur_block = b.block_mut(current);
            if cur_block.byte_range.is_empty() {
                cur_block.byte_range = gap_start..gap_end;
                cur_block.start_line = line;
                cur_block.end_line = end_line;
            } else {
                cur_block.byte_range.end = gap_end;
                cur_block.end_line = end_line;
            }
        }

        if terminated {
            // After an unconditional exit, subsequent code is unreachable.
            // We'll still process structural nodes to maintain graph completeness,
            // but we start a new unreachable block.
            let unreach_line = node.start_line;
            let unreach = b.alloc_block(
                BlockKind::Unreachable,
                node.byte_range.clone(),
                unreach_line,
            );
            current = unreach;
            terminated = false;
        }

        match &node.kind {
            CaptureKind::Branch => {
                let r = build_branch(b, node, current, body_range, source, exit);
                current = r.tail;
                terminated = r.terminated;
            }
            CaptureKind::Loop => {
                let r = build_loop(b, node, current, body_range, source, exit);
                current = r.tail;
                terminated = r.terminated;
            }
            CaptureKind::Match => {
                let r = build_match(b, node, current, body_range, source, exit);
                current = r.tail;
                terminated = r.terminated;
            }
            CaptureKind::Try => {
                let r = build_try(b, node, current, body_range, source, exit);
                current = r.tail;
                terminated = r.terminated;
            }
            CaptureKind::ExitReturn => {
                let cur_block = b.block_mut(current);
                if cur_block.byte_range.is_empty() {
                    cur_block.byte_range = node.byte_range.clone();
                    cur_block.start_line = node.start_line;
                    cur_block.end_line = node.end_line;
                } else {
                    cur_block.byte_range.end = node.byte_range.end;
                    cur_block.end_line = node.end_line;
                }
                b.add_edge(current, exit, EdgeKind::Return);
                terminated = true;
            }
            CaptureKind::ExitBreak => {
                let cur_block = b.block_mut(current);
                if cur_block.byte_range.is_empty() {
                    cur_block.byte_range = node.byte_range.clone();
                    cur_block.start_line = node.start_line;
                    cur_block.end_line = node.end_line;
                } else {
                    cur_block.byte_range.end = node.byte_range.end;
                    cur_block.end_line = node.end_line;
                }
                if let Some(&(_, loop_exit)) = b.loop_stack.last() {
                    b.add_edge(current, loop_exit, EdgeKind::Break);
                } else {
                    // Break outside loop (e.g., in match arm) — treat as fallthrough to exit.
                    b.add_edge(current, exit, EdgeKind::Break);
                }
                terminated = true;
            }
            CaptureKind::ExitContinue => {
                let cur_block = b.block_mut(current);
                if cur_block.byte_range.is_empty() {
                    cur_block.byte_range = node.byte_range.clone();
                    cur_block.start_line = node.start_line;
                    cur_block.end_line = node.end_line;
                } else {
                    cur_block.byte_range.end = node.byte_range.end;
                    cur_block.end_line = node.end_line;
                }
                if let Some(&(loop_head, _)) = b.loop_stack.last() {
                    b.add_edge(current, loop_head, EdgeKind::Continue);
                } else {
                    b.add_edge(current, exit, EdgeKind::Continue);
                }
                terminated = true;
            }
            CaptureKind::ExitThrow => {
                let cur_block = b.block_mut(current);
                if cur_block.byte_range.is_empty() {
                    cur_block.byte_range = node.byte_range.clone();
                    cur_block.start_line = node.start_line;
                    cur_block.end_line = node.end_line;
                } else {
                    cur_block.byte_range.end = node.byte_range.end;
                    cur_block.end_line = node.end_line;
                }
                b.add_edge(current, exit, EdgeKind::Exception);
                terminated = true;
            }
            _ => {}
        }

        prev_end = node.byte_range.end;
    }

    // Extend current block to end of body if there are trailing bytes.
    if !terminated && body_range.end > prev_end {
        let gap_start = prev_end;
        let gap_end = body_range.end;
        let line = byte_offset_to_line(source, gap_start);
        let end_line = byte_offset_to_line(source, gap_end.saturating_sub(1));
        let cur_block = b.block_mut(current);
        if cur_block.byte_range.is_empty() {
            cur_block.byte_range = gap_start..gap_end;
            cur_block.start_line = line;
            cur_block.end_line = end_line;
        } else {
            cur_block.byte_range.end = gap_end;
            cur_block.end_line = end_line;
        }
    }

    SequenceResult {
        tail: current,
        terminated,
    }
}

fn build_branch(
    b: &mut Builder,
    node: &ClassifiedNode,
    pred: BlockId,
    body_range: &Range<usize>,
    source: &[u8],
    exit: BlockId,
) -> SequenceResult {
    // Create branch block (extends pred or is new).
    let branch_block = pred;
    {
        let bb = b.block_mut(branch_block);
        if bb.byte_range.is_empty() {
            bb.byte_range = node.byte_range.clone();
            bb.start_line = node.start_line;
            bb.end_line = node.start_line; // just the header line
        }
        bb.kind = BlockKind::Branch;
    }

    // Then arm.
    let then_range = node
        .branch_then
        .clone()
        .unwrap_or_else(|| node.byte_range.clone());
    let then_line = byte_offset_to_line(source, then_range.start);
    let then_block = b.alloc_block(BlockKind::Statement, 0..0, then_line);
    b.add_edge(branch_block, then_block, EdgeKind::ConditionalTrue);

    // Recurse into then arm.
    let then_top_level = extract_top_level_in_range(b, node, &then_range, body_range);
    let then_seq = build_sequence(b, &then_top_level, then_block, &then_range, source, exit);

    // Else arm (optional).
    let join = b.alloc_block(BlockKind::Statement, 0..0, node.end_line);

    if let Some(else_range) = &node.branch_else {
        let else_line = byte_offset_to_line(source, else_range.start);
        let else_block = b.alloc_block(BlockKind::Statement, 0..0, else_line);
        b.add_edge(branch_block, else_block, EdgeKind::ConditionalFalse);

        let else_top_level = extract_top_level_in_range(b, node, else_range, body_range);
        let else_seq = build_sequence(b, &else_top_level, else_block, else_range, source, exit);

        if !then_seq.terminated {
            b.add_edge(then_seq.tail, join, EdgeKind::Fallthrough);
        }
        if !else_seq.terminated {
            b.add_edge(else_seq.tail, join, EdgeKind::Fallthrough);
        }

        SequenceResult {
            tail: join,
            terminated: then_seq.terminated && else_seq.terminated,
        }
    } else {
        // No else: ConditionalFalse goes directly to join.
        b.add_edge(branch_block, join, EdgeKind::ConditionalFalse);
        if !then_seq.terminated {
            b.add_edge(then_seq.tail, join, EdgeKind::Fallthrough);
        }
        SequenceResult {
            tail: join,
            terminated: false,
        }
    }
}

fn build_loop(
    b: &mut Builder,
    node: &ClassifiedNode,
    pred: BlockId,
    body_range: &Range<usize>,
    source: &[u8],
    exit: BlockId,
) -> SequenceResult {
    let loop_head = b.alloc_block(
        BlockKind::LoopHead,
        node.byte_range.clone(),
        node.start_line,
    );
    let loop_exit = b.alloc_block(BlockKind::LoopExit, 0..0, node.end_line);

    b.add_edge(pred, loop_head, EdgeKind::Fallthrough);

    // Push loop stack so break/continue inside body resolve correctly.
    b.loop_stack.push((loop_head, loop_exit));

    let body_sub_range = node
        .loop_body
        .clone()
        .unwrap_or_else(|| node.byte_range.clone());
    let body_line = byte_offset_to_line(source, body_sub_range.start);
    let loop_body = b.alloc_block(BlockKind::LoopBody, 0..0, body_line);

    // For loops with a condition (while, for): loop_head branches to body or exit.
    // For unconditional loops (Rust `loop`): loop_head → body; exit via break.
    if node.loop_condition.is_some() {
        b.add_edge(loop_head, loop_body, EdgeKind::ConditionalTrue);
        b.add_edge(loop_head, loop_exit, EdgeKind::ConditionalFalse);
    } else {
        // Unconditional loop: head always enters body; exit is via break.
        b.add_edge(loop_head, loop_body, EdgeKind::Fallthrough);
    }

    let body_top_level = extract_top_level_in_range(b, node, &body_sub_range, body_range);
    let body_seq = build_sequence(b, &body_top_level, loop_body, &body_sub_range, source, exit);

    if !body_seq.terminated {
        b.add_edge(body_seq.tail, loop_head, EdgeKind::BackEdge);
    }

    b.loop_stack.pop();

    SequenceResult {
        tail: loop_exit,
        terminated: false,
    }
}

fn build_match(
    b: &mut Builder,
    node: &ClassifiedNode,
    pred: BlockId,
    body_range: &Range<usize>,
    source: &[u8],
    exit: BlockId,
) -> SequenceResult {
    let match_block = pred;
    {
        let mb = b.block_mut(match_block);
        if mb.byte_range.is_empty() {
            mb.byte_range = node.byte_range.clone();
            mb.start_line = node.start_line;
            mb.end_line = node.start_line;
        }
        mb.kind = BlockKind::Branch;
    }

    let join = b.alloc_block(BlockKind::Statement, 0..0, node.end_line);
    let mut all_arms_terminated = !node.match_arms.is_empty();

    if node.match_arms.is_empty() {
        // No arms captured — just connect match to join.
        b.add_edge(match_block, join, EdgeKind::Fallthrough);
        return SequenceResult {
            tail: join,
            terminated: false,
        };
    }

    for arm_range in &node.match_arms {
        let arm_line = byte_offset_to_line(source, arm_range.start);
        let arm_block = b.alloc_block(BlockKind::Statement, 0..0, arm_line);
        b.add_edge(match_block, arm_block, EdgeKind::ConditionalTrue);

        let arm_top_level = extract_top_level_in_range_raw(arm_range, body_range);
        let _ = arm_top_level; // Sub-nodes within arms not recursed for now.
        let arm_seq = build_sequence(b, &[], arm_block, arm_range, source, exit);

        if !arm_seq.terminated {
            b.add_edge(arm_seq.tail, join, EdgeKind::Fallthrough);
            all_arms_terminated = false;
        }
    }

    SequenceResult {
        tail: join,
        terminated: all_arms_terminated,
    }
}

fn build_try(
    b: &mut Builder,
    node: &ClassifiedNode,
    pred: BlockId,
    body_range: &Range<usize>,
    source: &[u8],
    exit: BlockId,
) -> SequenceResult {
    let try_block = pred;
    {
        let tb = b.block_mut(try_block);
        if tb.byte_range.is_empty() {
            tb.byte_range = node.byte_range.clone();
            tb.start_line = node.start_line;
            tb.end_line = node.start_line;
        }
        tb.kind = BlockKind::Statement;
    }

    let join = b.alloc_block(BlockKind::Statement, 0..0, node.end_line);

    // Try body.
    let try_body_range = node
        .try_body
        .clone()
        .unwrap_or_else(|| node.byte_range.clone());
    let try_body_line = byte_offset_to_line(source, try_body_range.start);
    let try_body_block = b.alloc_block(BlockKind::Statement, 0..0, try_body_line);
    b.add_edge(try_block, try_body_block, EdgeKind::Fallthrough);

    let body_top_level = extract_top_level_in_range(b, node, &try_body_range, body_range);
    let body_seq = build_sequence(
        b,
        &body_top_level,
        try_body_block,
        &try_body_range,
        source,
        exit,
    );

    if !body_seq.terminated {
        b.add_edge(body_seq.tail, join, EdgeKind::Fallthrough);
    }

    // Catch blocks.
    for catch_range in &node.try_catches {
        let catch_line = byte_offset_to_line(source, catch_range.start);
        let catch_block = b.alloc_block(BlockKind::Catch, 0..0, catch_line);
        b.add_edge(try_block, catch_block, EdgeKind::Exception);
        let catch_seq = build_sequence(b, &[], catch_block, catch_range, source, exit);
        if !catch_seq.terminated {
            b.add_edge(catch_seq.tail, join, EdgeKind::Fallthrough);
        }
    }

    // Finally block (if present) — connects to join after both paths.
    if let Some(finally_range) = &node.try_finally {
        let finally_line = byte_offset_to_line(source, finally_range.start);
        let finally_block = b.alloc_block(BlockKind::Statement, 0..0, finally_line);
        b.add_edge(join, finally_block, EdgeKind::Fallthrough);
        let join2 = b.alloc_block(BlockKind::Statement, 0..0, node.end_line);
        let finally_seq = build_sequence(b, &[], finally_block, finally_range, source, exit);
        if !finally_seq.terminated {
            b.add_edge(finally_seq.tail, join2, EdgeKind::Fallthrough);
        }
        return SequenceResult {
            tail: join2,
            terminated: false,
        };
    }

    SequenceResult {
        tail: join,
        terminated: false,
    }
}

/// Extract top-level structural nodes that fall within `range` from a node's nested captures.
/// Since we only have a flat list of top-level nodes (from the outer builder), this function
/// returns an empty slice — recursive sub-nodes are handled by the query capturing them at the
/// outer level. In this simple model, nested constructs within arms/branches appear as
/// top-level nodes in the outer structural list already filtered to the sub-range.
fn extract_top_level_in_range(
    _b: &Builder,
    _parent: &ClassifiedNode,
    _range: &Range<usize>,
    _body_range: &Range<usize>,
) -> Vec<ClassifiedNode> {
    // Nodes nested within branches/loops/arms are NOT in our flat top-level list
    // (they were filtered out as non-top-level by filter_top_level). The recursive
    // build_sequence calls here receive an empty node list, which means nested control
    // flow within arms is represented as a single Statement block covering the arm body.
    // Full recursive CFG support requires re-running the query within each sub-range,
    // which is deferred to a follow-up — the outer query already handles flat structures.
    Vec::new()
}

fn extract_top_level_in_range_raw(
    _range: &Range<usize>,
    _body_range: &Range<usize>,
) -> Vec<ClassifiedNode> {
    Vec::new()
}

/// Convert a byte offset to a 1-based line number by counting newlines in source.
fn byte_offset_to_line(source: &[u8], offset: usize) -> u32 {
    let clamped = offset.min(source.len());
    let newlines = source[..clamped].iter().filter(|&&b| b == b'\n').count();
    newlines as u32 + 1
}
