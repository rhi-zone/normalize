//! Control flow graph (CFG) builder and renderer for normalize.
//!
//! Builds a structured CFG from a tree-sitter parse tree using `.cfg.scm` queries.
//! Supports rendering to Mermaid flowcharts for visualization.
//!
//! # Architecture
//!
//! - [`Cfg`] is the core data model: a set of [`BasicBlock`]s connected by [`Edge`]s.
//! - [`builder`] constructs a `Cfg` from a tree-sitter `Tree` and a `.cfg.scm` query string.
//! - [`mermaid`] renders a `Cfg` to a Mermaid flowchart string.
//! - [`service`] (behind the `cli` feature) exposes the CLI `normalize cfg` subcommand.

use std::ops::Range;

pub mod builder;
pub mod mermaid;
#[cfg(feature = "cli")]
pub mod service;

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Identifies a function by file path, qualified name, and start line.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct FunctionId {
    /// Source file path (relative to workspace root).
    pub file: String,
    /// Qualified function name (e.g. `module::func` in Rust, `Class.method` in Python).
    pub qualified_name: String,
    /// 1-based line number where the function definition starts.
    pub start_line: u32,
}

/// Identifies a basic block within a [`Cfg`].
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct BlockId(pub u32);

// ---------------------------------------------------------------------------
// Block kinds
// ---------------------------------------------------------------------------

/// The structural role of a basic block in the CFG.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    /// Synthetic entry block (before the first statement).
    Entry,
    /// Synthetic exit block (after the last statement / all returns).
    Exit,
    /// Sequential statements with no control flow.
    Statement,
    /// A conditional branch (if/else head).
    Branch,
    /// The head of a loop (where the loop condition is evaluated).
    LoopHead,
    /// The body of a loop.
    LoopBody,
    /// The block following a loop (after exit condition).
    LoopExit,
    /// A catch/except block.
    Catch,
    /// Code after an unconditional exit (return/break/continue/throw).
    Unreachable,
}

// ---------------------------------------------------------------------------
// Edge kinds
// ---------------------------------------------------------------------------

/// The type of control flow represented by a CFG edge.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Normal sequential flow.
    Fallthrough,
    /// True branch of a conditional.
    ConditionalTrue,
    /// False branch of a conditional.
    ConditionalFalse,
    /// Back-edge to a loop head (for `continue` or loop repetition).
    BackEdge,
    /// Jump to the block after the enclosing loop (`break`).
    Break,
    /// Jump to the loop head (`continue`).
    Continue,
    /// Function return.
    Return,
    /// Exception thrown.
    Exception,
}

// ---------------------------------------------------------------------------
// Basic block
// ---------------------------------------------------------------------------

/// A basic block in the CFG: a maximal linear sequence of statements.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct BasicBlock {
    /// Unique identifier for this block within the CFG.
    pub id: BlockId,
    /// Byte range in the source file covered by this block (0-indexed).
    pub byte_range: Range<usize>,
    /// First source line covered by this block (1-indexed).
    pub start_line: u32,
    /// Last source line covered by this block (1-indexed).
    pub end_line: u32,
    /// Structural role of this block.
    pub kind: BlockKind,
}

// ---------------------------------------------------------------------------
// Edge
// ---------------------------------------------------------------------------

/// A directed edge between two basic blocks in a CFG.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Edge {
    /// Source block.
    pub from: BlockId,
    /// Target block.
    pub to: BlockId,
    /// Kind of control flow this edge represents.
    pub kind: EdgeKind,
}

// ---------------------------------------------------------------------------
// CFG
// ---------------------------------------------------------------------------

/// A control flow graph for a single function.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Cfg {
    /// The function this CFG was built from.
    pub function: FunctionId,
    /// All basic blocks in the CFG (in order of creation).
    pub blocks: Vec<BasicBlock>,
    /// All edges connecting basic blocks.
    pub edges: Vec<Edge>,
    /// ID of the synthetic entry block.
    pub entry: BlockId,
    /// ID of the synthetic exit block.
    pub exit: BlockId,
}

impl Cfg {
    /// Look up a block by ID. Panics if the ID is not found (internal consistency error).
    pub fn block(&self, id: BlockId) -> &BasicBlock {
        self.blocks
            .iter()
            .find(|b| b.id == id)
            .unwrap_or_else(|| panic!("CFG internal error: block {:?} not found", id))
    }

    /// Render this CFG as a Mermaid flowchart string.
    pub fn to_mermaid(&self) -> String {
        mermaid::render(self)
    }
}
