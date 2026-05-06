//! Statement types for the IR.

use super::{Expr, Span};
use serde::{Deserialize, Serialize};

/// A single name in an import or export specifier list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportName {
    /// The exported name being imported (e.g. `foo` in `import { foo } from '...'`).
    pub name: String,
    /// Local alias, if any (e.g. `bar` in `import { foo as bar } from '...'`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// True for namespace imports: `import * as ns from '...'`.
    pub is_namespace: bool,
}

impl ImportName {
    /// Plain named import: `import { name } from '...'`.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            alias: None,
            is_namespace: false,
        }
    }

    /// Aliased named import: `import { name as alias } from '...'`.
    pub fn aliased(name: impl Into<String>, alias: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            alias: Some(alias.into()),
            is_namespace: false,
        }
    }

    /// Default import: `import Name from '...'`.
    pub fn default(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            alias: None,
            is_namespace: false,
        }
    }

    /// Namespace import: `import * as ns from '...'`.
    pub fn namespace(alias: impl Into<String>) -> Self {
        Self {
            name: "*".into(),
            alias: Some(alias.into()),
            is_namespace: true,
        }
    }
}

/// A single name in an export specifier list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportName {
    /// The local name being exported (e.g. `foo` in `export { foo }`).
    pub name: String,
    /// Exported alias, if any (e.g. `bar` in `export { foo as bar }`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

impl ExportName {
    /// Plain export: `export { name }`.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            alias: None,
        }
    }

    /// Aliased export: `export { name as alias }`.
    pub fn aliased(name: impl Into<String>, alias: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            alias: Some(alias.into()),
        }
    }
}

/// A method in a class definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Method {
    /// Method name.
    pub name: String,
    /// Parameters.
    pub params: Vec<super::Param>,
    /// Method body.
    pub body: Vec<Stmt>,
    /// True for `static` methods.
    pub is_static: bool,
    /// Optional return type annotation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
}

impl Method {
    pub fn new(name: impl Into<String>, params: Vec<super::Param>, body: Vec<Stmt>) -> Self {
        Self {
            name: name.into(),
            params,
            body,
            is_static: false,
            return_type: None,
        }
    }
}

/// A statement (doesn't produce a value directly).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    /// Expression statement: `expr;`.
    Expr(Expr),

    /// Variable declaration: `let name = init` or `const name = init`.
    Let {
        name: String,
        init: Option<Expr>,
        mutable: bool,
        /// Optional type annotation (e.g. `string` for `let x: string = ...`).
        #[serde(skip_serializing_if = "Option::is_none")]
        type_annotation: Option<String>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// Block: `{ stmts... }`.
    Block(Vec<Stmt>),

    /// If statement: `if (test) consequent else alternate`.
    If {
        test: Expr,
        consequent: Box<Stmt>,
        alternate: Option<Box<Stmt>>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// While loop: `while (test) body`.
    While {
        test: Expr,
        body: Box<Stmt>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// For loop: `for (init; test; update) body`.
    For {
        init: Option<Box<Stmt>>,
        test: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// For-in/for-of loop: `for (variable in/of iterable) body`.
    ForIn {
        variable: String,
        iterable: Expr,
        body: Box<Stmt>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// Return statement: `return expr`.
    Return(Option<Expr>),

    /// Break statement.
    Break,

    /// Continue statement.
    Continue,

    /// Try/catch/finally statement.
    TryCatch {
        body: Box<Stmt>,
        catch_param: Option<String>,
        catch_body: Option<Box<Stmt>>,
        finally_body: Option<Box<Stmt>>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// Function declaration.
    Function(crate::Function),

    /// Import statement: `import { X, Y } from 'source'` or `import * as ns from 'source'`.
    ///
    /// `names` is empty for side-effect-only imports: `import './side-effect'`.
    Import {
        /// The module specifier string (e.g. `"./module"`, `"react"`).
        source: String,
        /// Named/namespace/default specifiers. Empty means side-effect import.
        names: Vec<ImportName>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// Export statement: `export { X, Y }` or re-export: `export { X } from 'source'`.
    Export {
        /// Names being exported.
        names: Vec<ExportName>,
        /// Source module for re-exports (e.g. `"./other"` in `export { X } from './other'`).
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// Class definition: `class Foo extends Bar { method() { ... } }`.
    Class {
        /// Class name.
        name: String,
        /// Superclass name (e.g. `"Bar"` in `class Foo extends Bar`).
        #[serde(skip_serializing_if = "Option::is_none")]
        extends: Option<String>,
        /// Methods (including constructor).
        methods: Vec<Method>,
        /// Source location (populated by readers; ignored by writers).
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },

    /// Comment (line or block). Used to preserve documentation comments during translation.
    ///
    /// `text` is the raw comment text including delimiters (e.g. `// foo`, `/* bar */`,
    /// `/** JSDoc */`, `-- Lua`, `--[[ block ]]`). Writers emit the text verbatim so the
    /// correct delimiter style for the target language must be supplied by the writer.
    ///
    /// For cross-language translation, use `Stmt::comment_line(text)` / `Stmt::comment_block(text)`
    /// which store only the content; writers format it according to the target language.
    Comment {
        /// Comment content (without delimiters).
        text: String,
        /// Whether this was originally a block comment (`/* */`, `--[[ ]]`).
        block: bool,
        /// Source location.
        #[serde(skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },
}

// Builder methods for statements
impl Stmt {
    pub fn expr(e: Expr) -> Self {
        Stmt::Expr(e)
    }

    pub fn let_decl(name: impl Into<String>, init: Option<Expr>) -> Self {
        Stmt::Let {
            name: name.into(),
            init,
            mutable: true,
            type_annotation: None,
            span: None,
        }
    }

    pub fn const_decl(name: impl Into<String>, init: Expr) -> Self {
        Stmt::Let {
            name: name.into(),
            init: Some(init),
            mutable: false,
            type_annotation: None,
            span: None,
        }
    }

    pub fn block(stmts: Vec<Stmt>) -> Self {
        Stmt::Block(stmts)
    }

    pub fn if_stmt(test: Expr, consequent: Stmt, alternate: Option<Stmt>) -> Self {
        Stmt::If {
            test,
            consequent: Box::new(consequent),
            alternate: alternate.map(Box::new),
            span: None,
        }
    }

    pub fn while_loop(test: Expr, body: Stmt) -> Self {
        Stmt::While {
            test,
            body: Box::new(body),
            span: None,
        }
    }

    pub fn for_loop(
        init: Option<Stmt>,
        test: Option<Expr>,
        update: Option<Expr>,
        body: Stmt,
    ) -> Self {
        Stmt::For {
            init: init.map(Box::new),
            test,
            update,
            body: Box::new(body),
            span: None,
        }
    }

    pub fn for_in(variable: impl Into<String>, iterable: Expr, body: Stmt) -> Self {
        Stmt::ForIn {
            variable: variable.into(),
            iterable,
            body: Box::new(body),
            span: None,
        }
    }

    pub fn return_stmt(expr: Option<Expr>) -> Self {
        Stmt::Return(expr)
    }

    pub fn break_stmt() -> Self {
        Stmt::Break
    }

    pub fn continue_stmt() -> Self {
        Stmt::Continue
    }

    pub fn try_catch(
        body: Stmt,
        catch_param: Option<String>,
        catch_body: Option<Stmt>,
        finally_body: Option<Stmt>,
    ) -> Self {
        Stmt::TryCatch {
            body: Box::new(body),
            catch_param,
            catch_body: catch_body.map(Box::new),
            finally_body: finally_body.map(Box::new),
            span: None,
        }
    }

    pub fn function(f: crate::Function) -> Self {
        Stmt::Function(f)
    }

    /// Create an import statement.
    pub fn import(source: impl Into<String>, names: Vec<ImportName>) -> Self {
        Stmt::Import {
            source: source.into(),
            names,
            span: None,
        }
    }

    /// Create an export statement.
    pub fn export(names: Vec<ExportName>, source: Option<String>) -> Self {
        Stmt::Export {
            names,
            source,
            span: None,
        }
    }

    /// Create a class definition.
    pub fn class(name: impl Into<String>, extends: Option<String>, methods: Vec<Method>) -> Self {
        Stmt::Class {
            name: name.into(),
            extends,
            methods,
            span: None,
        }
    }

    /// Create a line comment from raw content (without `//` or `--` delimiter).
    pub fn comment_line(text: impl Into<String>) -> Self {
        Stmt::Comment {
            text: text.into(),
            block: false,
            span: None,
        }
    }

    /// Create a block comment from raw content (without `/* */` or `--[[ ]]` delimiters).
    pub fn comment_block(text: impl Into<String>) -> Self {
        Stmt::Comment {
            text: text.into(),
            block: true,
            span: None,
        }
    }

    /// Attach a source location span to this statement.
    pub fn with_span(self, span: Span) -> Self {
        match self {
            Stmt::Let {
                name,
                init,
                mutable,
                type_annotation,
                ..
            } => Stmt::Let {
                name,
                init,
                mutable,
                type_annotation,
                span: Some(span),
            },
            Stmt::If {
                test,
                consequent,
                alternate,
                ..
            } => Stmt::If {
                test,
                consequent,
                alternate,
                span: Some(span),
            },
            Stmt::While { test, body, .. } => Stmt::While {
                test,
                body,
                span: Some(span),
            },
            Stmt::For {
                init,
                test,
                update,
                body,
                ..
            } => Stmt::For {
                init,
                test,
                update,
                body,
                span: Some(span),
            },
            Stmt::ForIn {
                variable,
                iterable,
                body,
                ..
            } => Stmt::ForIn {
                variable,
                iterable,
                body,
                span: Some(span),
            },
            Stmt::TryCatch {
                body,
                catch_param,
                catch_body,
                finally_body,
                ..
            } => Stmt::TryCatch {
                body,
                catch_param,
                catch_body,
                finally_body,
                span: Some(span),
            },
            Stmt::Comment { text, block, .. } => Stmt::Comment {
                text,
                block,
                span: Some(span),
            },
            Stmt::Import { source, names, .. } => Stmt::Import {
                source,
                names,
                span: Some(span),
            },
            Stmt::Export { names, source, .. } => Stmt::Export {
                names,
                source,
                span: Some(span),
            },
            Stmt::Class {
                name,
                extends,
                methods,
                ..
            } => Stmt::Class {
                name,
                extends,
                methods,
                span: Some(span),
            },
            other => other,
        }
    }
}
