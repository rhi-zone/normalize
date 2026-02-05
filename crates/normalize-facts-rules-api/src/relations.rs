//! Relations (facts) that rules operate on.
//!
//! These are the inputs to Datalog rules, extracted from code by normalize-facts.
//! Each relation type maps to a Datalog predicate:
//!
//! - `symbol(file, name, kind, line)` - defined symbols
//! - `import(from_file, to_module, name)` - import statements
//! - `call(caller_file, caller_name, callee_name, line)` - function calls

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
}
