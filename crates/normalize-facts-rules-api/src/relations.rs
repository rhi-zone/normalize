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

/// A symbol fact: a named entity defined in a file.
///
/// Maps to Datalog: `symbol(file, name, kind, line)`
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
pub struct IsImplFact {
    /// File path relative to project root
    pub file: String,
    /// Symbol name
    pub name: String,
}

/// A type method fact: a method signature on a type.
///
/// Maps to Datalog: `type_method(file, type_name, method_name)`
#[derive(Clone, Debug)]
pub struct TypeMethodFact {
    /// File path relative to project root
    pub file: String,
    /// Type (interface/class) name
    pub type_name: String,
    /// Method name
    pub method_name: String,
}

/// All relations (facts) available to rules.
///
/// This is the complete set of facts extracted from a codebase.
/// Rule packs receive this and apply Datalog rules over it.
#[derive(Clone, Debug, Default)]
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
}
