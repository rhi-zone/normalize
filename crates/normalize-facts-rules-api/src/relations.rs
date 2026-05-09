//! Relations (facts) that rules operate on.
//!
//! These are the inputs to Datalog rules, extracted from code by normalize-facts.
//! Each relation type maps to a Datalog predicate:
//!
//! - `symbol(file, name, kind, line)` - defined symbols
//! - `import(from_file, to_module, name)` - import statements
//! - `call(caller_file, caller_name, callee_name, line)` - function calls
//! - `visibility(file, name, vis)` - symbol visibility
//! - `attribute(file, name, attr)` - symbol attributes (one per attribute)
//! - `parent(file, child_name, parent_name)` - symbol nesting hierarchy
//! - `qualifier(caller_file, caller_name, callee_name, qual)` - call qualifier
//! - `symbol_range(file, name, start_line, end_line)` - symbol span
//! - `implements(file, name, interface)` - interface/trait implementation
//! - `is_impl(file, name)` - symbol is a trait/interface implementation
//! - `type_method(file, type_name, method_name)` - method signatures on types
//!
//! Cross-file resolution predicates (Phase 0):
//!
//! - `resolved_import(from_file, to_file, imported_name, local_alias, kind)` - resolved imports
//! - `module(file, canonical_module_path)` - canonical module identity of a file
//! - `export(file, name, kind)` - exported symbols
//! - `reexport(from_file, original_file, original_name, exported_as)` - re-export chains
//! - `symbol_use(file, name, line)` - symbol reference/use sites
//! - `resolved_reference(use_file, use_line, def_file, def_name, def_kind)` - resolved symbol refs
//! - `resolved_call(caller_file, caller_name, callee_file, callee_name, line)` - resolved calls
//! - `module_search_path(workspace_root, language, kind, path)` - module search paths

/// A symbol fact: a named entity defined in a file.
///
/// Maps to Datalog: `symbol(file, name, kind, line)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct SymbolFact {
    /// File path relative to project root
    pub file: String,
    /// Symbol name
    pub name: String,
    /// Symbol kind (function, class, method, etc.)
    pub kind: String,
    /// Line number where symbol is defined
    pub line: u32,
}

/// An import fact: a dependency from one file to another module.
///
/// Maps to Datalog: `import(from_file, to_module, name)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct ImportFact {
    /// File containing the import
    pub from_file: String,
    /// Raw module specifier as written in the source.
    ///
    /// The value depends on the language and import style:
    ///
    /// - **Relative or absolute file path** — e.g. `"../foo"`, `"./utils"` (JS/TS, Python
    ///   relative imports). The path is as written in source, not resolved to an absolute path.
    /// - **Module name** — e.g. `"os"` (Python stdlib), `"std::collections"` (Rust), `"fmt"`
    ///   (Go). These are not file paths and cannot be resolved without a module resolver.
    /// - **Empty string `""`** — when the grammar does not expose a module path for the
    ///   import, or for star imports that name no explicit module (e.g. some wildcard import
    ///   syntaxes). Callers should treat `""` as "module not known".
    ///
    /// Resolved file paths (when available) are stored separately in the index, not here.
    pub module_specifier: String,
    /// Name being imported (or "*" for wildcard)
    pub name: String,
}

/// A call fact: a function call from one symbol to another.
///
/// Maps to Datalog: `call(caller_file, caller_name, callee_name, line)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct CallFact {
    /// File containing the call
    pub caller_file: String,
    /// Name of the calling function/method
    pub caller_name: String,
    /// Name of the called function/method
    pub callee_name: String,
    /// Line number of the call
    pub line: u32,
}

/// A visibility fact: the visibility of a symbol.
///
/// Maps to Datalog: `visibility(file, name, vis)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct VisibilityFact {
    /// File path relative to project root
    pub file: String,
    /// Symbol name
    pub name: String,
    /// Visibility: "public", "private", "protected", "internal"
    pub visibility: String,
}

/// An attribute fact: one attribute annotation on a symbol.
///
/// Maps to Datalog: `attribute(file, name, attr)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct AttributeFact {
    /// File path relative to project root
    pub file: String,
    /// Symbol name
    pub name: String,
    /// Attribute string (e.g. "#[derive(Debug)]", "@Override")
    pub attribute: String,
}

/// A parent fact: symbol nesting hierarchy.
///
/// Maps to Datalog: `parent(file, child_name, parent_name)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct ParentFact {
    /// File path relative to project root
    pub file: String,
    /// Child symbol name
    pub child_name: String,
    /// Parent symbol name
    pub parent_name: String,
}

/// A qualifier fact: call qualifier (receiver/module).
///
/// Maps to Datalog: `qualifier(caller_file, caller_name, callee_name, qual)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct QualifierFact {
    /// File containing the call
    pub caller_file: String,
    /// Name of the calling function/method
    pub caller_name: String,
    /// Name of the called function/method
    pub callee_name: String,
    /// Qualifier ("self", module name, etc.)
    pub qualifier: String,
}

/// A symbol range fact: start and end lines of a symbol.
///
/// Maps to Datalog: `symbol_range(file, name, start_line, end_line)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct SymbolRangeFact {
    /// File path relative to project root
    pub file: String,
    /// Symbol name
    pub name: String,
    /// Start line number
    pub start_line: u32,
    /// End line number
    pub end_line: u32,
}

/// An implements fact: a symbol implements an interface/trait.
///
/// Maps to Datalog: `implements(file, name, interface)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct ImplementsFact {
    /// File path relative to project root
    pub file: String,
    /// Symbol name
    pub name: String,
    /// Interface/trait name
    pub interface: String,
}

/// An is_impl fact: symbol is a trait/interface implementation.
///
/// Maps to Datalog: `is_impl(file, name)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct IsImplFact {
    /// File path relative to project root
    pub file: String,
    /// Symbol name
    pub name: String,
}

/// A type method fact: a method signature on a type.
///
/// Maps to Datalog: `type_method(file, type_name, method_name)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct TypeMethodFact {
    /// File path relative to project root
    pub file: String,
    /// Type (interface/class) name
    pub type_name: String,
    /// Method name
    pub method_name: String,
}

/// A resolved import fact: an import statement resolved to a specific file.
///
/// Maps to Datalog: `resolved_import(from_file, to_file, imported_name, local_alias, kind)`
/// kind ∈ {"direct", "glob", "reexport", "unresolved"}
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct ResolvedImportFact {
    /// File containing the import
    pub from_file: String,
    /// Resolved target file
    pub to_file: String,
    /// The imported name (e.g. "HashMap", "*" for glob)
    pub imported_name: String,
    /// Local alias (same as imported_name if no alias)
    pub local_alias: String,
    /// Resolution kind: "direct", "glob", "reexport", or "unresolved"
    pub kind: String,
}

/// A module fact: canonical module identity of a file.
///
/// Maps to Datalog: `module(file, canonical_module_path)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct ModuleFact {
    /// File path relative to project root
    pub file: String,
    /// Canonical module path (e.g. "mycrate::foo::bar")
    pub canonical_module_path: String,
}

/// An export fact: a symbol exported from a file.
///
/// Maps to Datalog: `export(file, name, kind)`
/// kind ∈ {"value", "type", "module", "reexport"}
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct ExportFact {
    /// File path relative to project root
    pub file: String,
    /// Exported name
    pub name: String,
    /// Export kind: "value", "type", "module", or "reexport"
    pub kind: String,
}

/// A reexport fact: a symbol re-exported through an intermediate file.
///
/// Maps to Datalog: `reexport(from_file, original_file, original_name, exported_as)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct ReexportFact {
    /// File doing the re-export
    pub from_file: String,
    /// File where the symbol is originally defined
    pub original_file: String,
    /// Original name of the symbol
    pub original_name: String,
    /// Name it is exported as (may differ with `as` alias)
    pub exported_as: String,
}

/// A symbol use fact: a reference/use site of a named symbol.
///
/// Maps to Datalog: `symbol_use(file, name, line)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct SymbolUseFact {
    /// File containing the use
    pub file: String,
    /// Name being used
    pub name: String,
    /// Line number of the use
    pub line: u32,
}

/// A resolved reference fact: a symbol use resolved to its definition.
///
/// Maps to Datalog: `resolved_reference(use_file, use_line, def_file, def_name, def_kind)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct ResolvedReferenceFact {
    /// File containing the use
    pub use_file: String,
    /// Line of the use
    pub use_line: u32,
    /// File containing the definition
    pub def_file: String,
    /// Name of the defined symbol
    pub def_name: String,
    /// Kind of the defined symbol
    pub def_kind: String,
}

/// A resolved call fact: a function call resolved to its definition file.
///
/// Maps to Datalog: `resolved_call(caller_file, caller_name, callee_file, callee_name, line)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct ResolvedCallFact {
    /// File containing the call
    pub caller_file: String,
    /// Name of the calling function
    pub caller_name: String,
    /// File containing the callee definition
    pub callee_file: String,
    /// Name of the called function
    pub callee_name: String,
    /// Line number of the call
    pub line: u32,
}

/// A module search path fact: a directory to search for modules.
///
/// Maps to Datalog: `module_search_path(workspace_root, language, kind, path)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct ModuleSearchPathFact {
    /// Workspace root this path belongs to
    pub workspace_root: String,
    /// Language this path applies to (e.g. "rust", "python")
    pub language: String,
    /// Kind of search path (e.g. "source", "stdlib", "third-party")
    pub kind: String,
    /// The search path
    pub path: String,
}

/// A CFG block fact.
///
/// Maps to Datalog: `cfg_block(file, func, func_line, block, kind)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct CfgBlockFact {
    /// Source file path
    pub file: String,
    /// Qualified function name
    pub func: String,
    /// Function start line (1-based)
    pub func_line: u32,
    /// Block ID (0-based, unique within function)
    pub block: u32,
    /// Block kind (e.g. "entry", "exit", "branch", "loophead")
    pub kind: String,
}

/// A CFG edge fact.
///
/// Maps to Datalog: `cfg_edge(file, func, func_line, from, to, kind, exception_type)`
///
/// `exception_type` is only meaningful for `kind == "exception"` edges:
/// - Empty string = conservative (type unknown, applies to any exception).
/// - Non-empty string = the exception type name (e.g. `"IOException"`).
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct CfgEdgeFact {
    /// Source file path
    pub file: String,
    /// Qualified function name
    pub func: String,
    /// Function start line (1-based)
    pub func_line: u32,
    /// Source block ID
    pub from: u32,
    /// Target block ID
    pub to: u32,
    /// Edge kind (e.g. "fallthrough", "conditionaltrue", "backedge", "exception")
    pub kind: String,
    /// Exception type for `kind == "exception"` edges. Empty string = conservative.
    pub exception_type: String,
}

/// A CFG variable definition fact.
///
/// Maps to Datalog: `cfg_def(file, func, func_line, block, name)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct CfgDefFact {
    /// Source file path
    pub file: String,
    /// Qualified function name
    pub func: String,
    /// Function start line (1-based)
    pub func_line: u32,
    /// Block ID this def occurs in
    pub block: u32,
    /// Name of the variable being defined
    pub name: String,
}

/// A CFG variable use fact.
///
/// Maps to Datalog: `cfg_use(file, func, func_line, block, name)`
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct CfgUseFact {
    /// Source file path
    pub file: String,
    /// Qualified function name
    pub func: String,
    /// Function start line (1-based)
    pub func_line: u32,
    /// Block ID this use occurs in
    pub block: u32,
    /// Name of the variable being used
    pub name: String,
}

/// A CFG side-effect fact.
///
/// Maps to Datalog: `cfg_effect(file, func, func_line, block, kind, line, label)`
///
/// `kind` is one of: `"await"`, `"defer"`, `"yield"`, `"acquire"`, `"release"`, `"send"`, `"receive"`.
/// `label` is an optional text label (resource name, expression text, etc.; empty string if absent).
#[derive(Clone, Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct CfgEffectFact {
    /// Source file path
    pub file: String,
    /// Qualified function name
    pub func: String,
    /// Function start line (1-based)
    pub func_line: u32,
    /// Block ID this effect occurs in
    pub block: u32,
    /// Effect kind string (e.g. "await", "defer", "yield")
    pub kind: String,
    /// Source line of the effect (1-based)
    pub line: u32,
    /// Optional label (resource name, expression text). Empty string if absent.
    pub label: String,
}

/// All relations (facts) available to rules.
///
/// This is the complete set of facts extracted from a codebase.
/// Rule packs receive this and apply Datalog rules over it.
#[derive(Clone, Debug, Default, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]
pub struct Relations {
    /// All symbols defined in the codebase
    pub symbols: Vec<SymbolFact>,
    /// All imports in the codebase
    pub imports: Vec<ImportFact>,
    /// All function calls in the codebase
    pub calls: Vec<CallFact>,
    /// Symbol visibility facts
    pub visibilities: Vec<VisibilityFact>,
    /// Symbol attribute facts (one per attribute per symbol)
    pub attributes: Vec<AttributeFact>,
    /// Symbol parent-child hierarchy
    pub parents: Vec<ParentFact>,
    /// Call qualifier facts (receiver/module on calls)
    pub qualifiers: Vec<QualifierFact>,
    /// Symbol range facts (start and end lines)
    pub symbol_ranges: Vec<SymbolRangeFact>,
    /// Implements facts (symbol implements interface/trait)
    pub implements: Vec<ImplementsFact>,
    /// Is-impl facts (symbol is a trait/interface implementation)
    pub is_impls: Vec<IsImplFact>,
    /// Type method facts (method signatures on types)
    pub type_methods: Vec<TypeMethodFact>,
    /// Resolved import facts (import resolved to a specific file)
    pub resolved_imports: Vec<ResolvedImportFact>,
    /// Module identity facts (file → canonical module path)
    pub modules: Vec<ModuleFact>,
    /// Export facts (symbol exported from a file)
    pub exports: Vec<ExportFact>,
    /// Reexport facts (symbol re-exported through an intermediate)
    pub reexports: Vec<ReexportFact>,
    /// Symbol use facts (reference/use sites)
    pub symbol_uses: Vec<SymbolUseFact>,
    /// Resolved reference facts (use resolved to definition)
    pub resolved_references: Vec<ResolvedReferenceFact>,
    /// Resolved call facts (call resolved to definition file)
    pub resolved_calls: Vec<ResolvedCallFact>,
    /// Module search path facts (directories to search for modules)
    pub module_search_paths: Vec<ModuleSearchPathFact>,
    /// CFG block facts (one per basic block per function)
    pub cfg_blocks: Vec<CfgBlockFact>,
    /// CFG edge facts (one per control-flow edge per function)
    pub cfg_edges: Vec<CfgEdgeFact>,
    /// CFG variable definition facts (one per def site per block)
    pub cfg_defs: Vec<CfgDefFact>,
    /// CFG variable use facts (one per use site per block)
    pub cfg_uses: Vec<CfgUseFact>,
    /// CFG side-effect facts (await, defer, yield, acquire, release, send, receive)
    pub cfg_effects: Vec<CfgEffectFact>,
}

impl Relations {
    /// Create empty relations
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a symbol fact
    pub fn add_symbol(&mut self, file: &str, name: &str, kind: &str, line: u32) {
        self.symbols.push(SymbolFact {
            file: file.into(),
            name: name.into(),
            kind: kind.into(),
            line,
        });
    }

    /// Add an import fact
    pub fn add_import(&mut self, from_file: &str, to_module: &str, name: &str) {
        self.imports.push(ImportFact {
            from_file: from_file.into(),
            module_specifier: to_module.into(),
            name: name.into(),
        });
    }

    /// Add a call fact
    pub fn add_call(&mut self, caller_file: &str, caller_name: &str, callee_name: &str, line: u32) {
        self.calls.push(CallFact {
            caller_file: caller_file.into(),
            caller_name: caller_name.into(),
            callee_name: callee_name.into(),
            line,
        });
    }

    /// Add a visibility fact
    pub fn add_visibility(&mut self, file: &str, name: &str, visibility: &str) {
        self.visibilities.push(VisibilityFact {
            file: file.into(),
            name: name.into(),
            visibility: visibility.into(),
        });
    }

    /// Add an attribute fact
    pub fn add_attribute(&mut self, file: &str, name: &str, attribute: &str) {
        self.attributes.push(AttributeFact {
            file: file.into(),
            name: name.into(),
            attribute: attribute.into(),
        });
    }

    /// Add a parent fact
    pub fn add_parent(&mut self, file: &str, child_name: &str, parent_name: &str) {
        self.parents.push(ParentFact {
            file: file.into(),
            child_name: child_name.into(),
            parent_name: parent_name.into(),
        });
    }

    /// Add a qualifier fact
    pub fn add_qualifier(
        &mut self,
        caller_file: &str,
        caller_name: &str,
        callee_name: &str,
        qualifier: &str,
    ) {
        self.qualifiers.push(QualifierFact {
            caller_file: caller_file.into(),
            caller_name: caller_name.into(),
            callee_name: callee_name.into(),
            qualifier: qualifier.into(),
        });
    }

    /// Add a symbol range fact
    pub fn add_symbol_range(&mut self, file: &str, name: &str, start_line: u32, end_line: u32) {
        self.symbol_ranges.push(SymbolRangeFact {
            file: file.into(),
            name: name.into(),
            start_line,
            end_line,
        });
    }

    /// Add an implements fact
    pub fn add_implements(&mut self, file: &str, name: &str, interface: &str) {
        self.implements.push(ImplementsFact {
            file: file.into(),
            name: name.into(),
            interface: interface.into(),
        });
    }

    /// Add an is_impl fact
    pub fn add_is_impl(&mut self, file: &str, name: &str) {
        self.is_impls.push(IsImplFact {
            file: file.into(),
            name: name.into(),
        });
    }

    /// Add a type method fact
    pub fn add_type_method(&mut self, file: &str, type_name: &str, method_name: &str) {
        self.type_methods.push(TypeMethodFact {
            file: file.into(),
            type_name: type_name.into(),
            method_name: method_name.into(),
        });
    }

    /// Add a resolved import fact
    pub fn add_resolved_import(
        &mut self,
        from_file: &str,
        to_file: &str,
        imported_name: &str,
        local_alias: &str,
        kind: &str,
    ) {
        self.resolved_imports.push(ResolvedImportFact {
            from_file: from_file.into(),
            to_file: to_file.into(),
            imported_name: imported_name.into(),
            local_alias: local_alias.into(),
            kind: kind.into(),
        });
    }

    /// Add a module fact
    pub fn add_module(&mut self, file: &str, canonical_module_path: &str) {
        self.modules.push(ModuleFact {
            file: file.into(),
            canonical_module_path: canonical_module_path.into(),
        });
    }

    /// Add an export fact
    pub fn add_export(&mut self, file: &str, name: &str, kind: &str) {
        self.exports.push(ExportFact {
            file: file.into(),
            name: name.into(),
            kind: kind.into(),
        });
    }

    /// Add a reexport fact
    pub fn add_reexport(
        &mut self,
        from_file: &str,
        original_file: &str,
        original_name: &str,
        exported_as: &str,
    ) {
        self.reexports.push(ReexportFact {
            from_file: from_file.into(),
            original_file: original_file.into(),
            original_name: original_name.into(),
            exported_as: exported_as.into(),
        });
    }

    /// Add a symbol use fact
    pub fn add_symbol_use(&mut self, file: &str, name: &str, line: u32) {
        self.symbol_uses.push(SymbolUseFact {
            file: file.into(),
            name: name.into(),
            line,
        });
    }

    /// Add a resolved reference fact
    pub fn add_resolved_reference(
        &mut self,
        use_file: &str,
        use_line: u32,
        def_file: &str,
        def_name: &str,
        def_kind: &str,
    ) {
        self.resolved_references.push(ResolvedReferenceFact {
            use_file: use_file.into(),
            use_line,
            def_file: def_file.into(),
            def_name: def_name.into(),
            def_kind: def_kind.into(),
        });
    }

    /// Add a resolved call fact
    pub fn add_resolved_call(
        &mut self,
        caller_file: &str,
        caller_name: &str,
        callee_file: &str,
        callee_name: &str,
        line: u32,
    ) {
        self.resolved_calls.push(ResolvedCallFact {
            caller_file: caller_file.into(),
            caller_name: caller_name.into(),
            callee_file: callee_file.into(),
            callee_name: callee_name.into(),
            line,
        });
    }

    /// Add a module search path fact
    pub fn add_module_search_path(
        &mut self,
        workspace_root: &str,
        language: &str,
        kind: &str,
        path: &str,
    ) {
        self.module_search_paths.push(ModuleSearchPathFact {
            workspace_root: workspace_root.into(),
            language: language.into(),
            kind: kind.into(),
            path: path.into(),
        });
    }

    /// Add a CFG block fact
    pub fn add_cfg_block(
        &mut self,
        file: &str,
        func: &str,
        func_line: u32,
        block: u32,
        kind: &str,
    ) {
        self.cfg_blocks.push(CfgBlockFact {
            file: file.into(),
            func: func.into(),
            func_line,
            block,
            kind: kind.into(),
        });
    }

    /// Add a CFG edge fact
    #[allow(clippy::too_many_arguments)]
    pub fn add_cfg_edge(
        &mut self,
        file: &str,
        func: &str,
        func_line: u32,
        from: u32,
        to: u32,
        kind: &str,
        exception_type: &str,
    ) {
        self.cfg_edges.push(CfgEdgeFact {
            file: file.into(),
            func: func.into(),
            func_line,
            from,
            to,
            kind: kind.into(),
            exception_type: exception_type.into(),
        });
    }

    /// Add a CFG variable definition fact
    pub fn add_cfg_def(&mut self, file: &str, func: &str, func_line: u32, block: u32, name: &str) {
        self.cfg_defs.push(CfgDefFact {
            file: file.into(),
            func: func.into(),
            func_line,
            block,
            name: name.into(),
        });
    }

    /// Add a CFG variable use fact
    pub fn add_cfg_use(&mut self, file: &str, func: &str, func_line: u32, block: u32, name: &str) {
        self.cfg_uses.push(CfgUseFact {
            file: file.into(),
            func: func.into(),
            func_line,
            block,
            name: name.into(),
        });
    }

    /// Add a CFG side-effect fact
    #[allow(clippy::too_many_arguments)]
    pub fn add_cfg_effect(
        &mut self,
        file: &str,
        func: &str,
        func_line: u32,
        block: u32,
        kind: &str,
        line: u32,
        label: &str,
    ) {
        self.cfg_effects.push(CfgEffectFact {
            file: file.into(),
            func: func.into(),
            func_line,
            block,
            kind: kind.into(),
            line,
            label: label.into(),
        });
    }
}
