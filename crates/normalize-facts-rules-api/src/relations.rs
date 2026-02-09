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

use abi_stable::{
    StableAbi,
    std_types::{RString, RVec},
};

/// A symbol fact: a named entity defined in a file.
///
/// Maps to Datalog: `symbol(file, name, kind, line)`
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct SymbolFact {
    /// File path relative to project root
    pub file: RString,
    /// Symbol name
    pub name: RString,
    /// Symbol kind (function, class, method, etc.)
    pub kind: RString,
    /// Line number where symbol is defined
    pub line: u32,
}

/// An import fact: a dependency from one file to another module.
///
/// Maps to Datalog: `import(from_file, to_module, name)`
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct ImportFact {
    /// File containing the import
    pub from_file: RString,
    /// Module/file being imported from
    pub to_module: RString,
    /// Name being imported (or "*" for wildcard)
    pub name: RString,
}

/// A call fact: a function call from one symbol to another.
///
/// Maps to Datalog: `call(caller_file, caller_name, callee_name, line)`
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct CallFact {
    /// File containing the call
    pub caller_file: RString,
    /// Name of the calling function/method
    pub caller_name: RString,
    /// Name of the called function/method
    pub callee_name: RString,
    /// Line number of the call
    pub line: u32,
}

/// A visibility fact: the visibility of a symbol.
///
/// Maps to Datalog: `visibility(file, name, vis)`
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct VisibilityFact {
    /// File path relative to project root
    pub file: RString,
    /// Symbol name
    pub name: RString,
    /// Visibility: "public", "private", "protected", "internal"
    pub visibility: RString,
}

/// An attribute fact: one attribute annotation on a symbol.
///
/// Maps to Datalog: `attribute(file, name, attr)`
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct AttributeFact {
    /// File path relative to project root
    pub file: RString,
    /// Symbol name
    pub name: RString,
    /// Attribute string (e.g. "#[derive(Debug)]", "@Override")
    pub attribute: RString,
}

/// A parent fact: symbol nesting hierarchy.
///
/// Maps to Datalog: `parent(file, child_name, parent_name)`
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct ParentFact {
    /// File path relative to project root
    pub file: RString,
    /// Child symbol name
    pub child_name: RString,
    /// Parent symbol name
    pub parent_name: RString,
}

/// A qualifier fact: call qualifier (receiver/module).
///
/// Maps to Datalog: `qualifier(caller_file, caller_name, callee_name, qual)`
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct QualifierFact {
    /// File containing the call
    pub caller_file: RString,
    /// Name of the calling function/method
    pub caller_name: RString,
    /// Name of the called function/method
    pub callee_name: RString,
    /// Qualifier ("self", module name, etc.)
    pub qualifier: RString,
}

/// A symbol range fact: start and end lines of a symbol.
///
/// Maps to Datalog: `symbol_range(file, name, start_line, end_line)`
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct SymbolRangeFact {
    /// File path relative to project root
    pub file: RString,
    /// Symbol name
    pub name: RString,
    /// Start line number
    pub start_line: u32,
    /// End line number
    pub end_line: u32,
}

/// An implements fact: a symbol implements an interface/trait.
///
/// Maps to Datalog: `implements(file, name, interface)`
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct ImplementsFact {
    /// File path relative to project root
    pub file: RString,
    /// Symbol name
    pub name: RString,
    /// Interface/trait name
    pub interface: RString,
}

/// An is_impl fact: symbol is a trait/interface implementation.
///
/// Maps to Datalog: `is_impl(file, name)`
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct IsImplFact {
    /// File path relative to project root
    pub file: RString,
    /// Symbol name
    pub name: RString,
}

/// A type method fact: a method signature on a type.
///
/// Maps to Datalog: `type_method(file, type_name, method_name)`
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct TypeMethodFact {
    /// File path relative to project root
    pub file: RString,
    /// Type (interface/class) name
    pub type_name: RString,
    /// Method name
    pub method_name: RString,
}

/// All relations (facts) available to rules.
///
/// This is the complete set of facts extracted from a codebase.
/// Rule packs receive this and apply Datalog rules over it.
#[repr(C)]
#[derive(Clone, Debug, Default, StableAbi)]
pub struct Relations {
    /// All symbols defined in the codebase
    pub symbols: RVec<SymbolFact>,
    /// All imports in the codebase
    pub imports: RVec<ImportFact>,
    /// All function calls in the codebase
    pub calls: RVec<CallFact>,
    /// Symbol visibility facts
    pub visibilities: RVec<VisibilityFact>,
    /// Symbol attribute facts (one per attribute per symbol)
    pub attributes: RVec<AttributeFact>,
    /// Symbol parent-child hierarchy
    pub parents: RVec<ParentFact>,
    /// Call qualifier facts (receiver/module on calls)
    pub qualifiers: RVec<QualifierFact>,
    /// Symbol range facts (start and end lines)
    pub symbol_ranges: RVec<SymbolRangeFact>,
    /// Implements facts (symbol implements interface/trait)
    pub implements: RVec<ImplementsFact>,
    /// Is-impl facts (symbol is a trait/interface implementation)
    pub is_impls: RVec<IsImplFact>,
    /// Type method facts (method signatures on types)
    pub type_methods: RVec<TypeMethodFact>,
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
            to_module: to_module.into(),
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
