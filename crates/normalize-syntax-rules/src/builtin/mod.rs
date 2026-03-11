//! Embedded builtin rules for syntax-based linting.
//!
//! Rules are embedded at compile time and loaded as the lowest-priority source.
//! Users can override or disable them via ~/.config/normalize/rules/ or .normalize/rules/.

use crate::BuiltinRule;

/// All embedded builtin rules.
pub const BUILTIN_RULES: &[BuiltinRule] = &[
    // Rust rules
    BuiltinRule {
        id: "rust/todo-macro",
        content: include_str!("rust_todo_macro.scm"),
    },
    BuiltinRule {
        id: "rust/println-debug",
        content: include_str!("rust_println_debug.scm"),
    },
    BuiltinRule {
        id: "rust/dbg-macro",
        content: include_str!("rust_dbg_macro.scm"),
    },
    BuiltinRule {
        id: "rust/expect-empty",
        content: include_str!("rust_expect_empty.scm"),
    },
    BuiltinRule {
        id: "rust/unwrap-in-impl",
        content: include_str!("rust_unwrap_in_impl.scm"),
    },
    BuiltinRule {
        id: "rust/unnecessary-let",
        content: include_str!("rust_unnecessary_let.scm"),
    },
    BuiltinRule {
        id: "rust/unnecessary-type-alias",
        content: include_str!("rust_unnecessary_type_alias.scm"),
    },
    BuiltinRule {
        id: "rust/chained-if-let",
        content: include_str!("rust_chained_if_let.scm"),
    },
    BuiltinRule {
        id: "rust/numeric-type-annotation",
        content: include_str!("rust_numeric_type_annotation.scm"),
    },
    BuiltinRule {
        id: "rust/tuple-return",
        content: include_str!("rust_tuple_return.scm"),
    },
    BuiltinRule {
        id: "rust/static-mut",
        content: include_str!("rust_static_mut.scm"),
    },
    BuiltinRule {
        id: "hardcoded-secret",
        content: include_str!("hardcoded_secret.scm"),
    },
    // JavaScript/TypeScript rules
    BuiltinRule {
        id: "js/console-log",
        content: include_str!("js_console_log.scm"),
    },
    BuiltinRule {
        id: "js/unnecessary-const",
        content: include_str!("js_unnecessary_const.scm"),
    },
    BuiltinRule {
        id: "js/module-let",
        content: include_str!("js_module_let.scm"),
    },
    BuiltinRule {
        id: "js/var-declaration",
        content: include_str!("js_var_declaration.scm"),
    },
    BuiltinRule {
        id: "js/typeof-string",
        content: include_str!("js_typeof_string.scm"),
    },
    BuiltinRule {
        id: "js/eq-null",
        content: include_str!("js_eq_null.scm"),
    },
    BuiltinRule {
        id: "js/no-await-in-loop",
        content: include_str!("js_no_await_in_loop.scm"),
    },
    BuiltinRule {
        id: "js/no-prototype-builtins",
        content: include_str!("js_no_prototype_builtins.scm"),
    },
    BuiltinRule {
        id: "js/prefer-optional-chain",
        content: include_str!("js_prefer_optional_chain.scm"),
    },
    BuiltinRule {
        id: "typescript/tuple-return",
        content: include_str!("typescript_tuple_return.scm"),
    },
    BuiltinRule {
        id: "typescript/no-any",
        content: include_str!("typescript_no_any.scm"),
    },
    BuiltinRule {
        id: "typescript/no-non-null-assertion",
        content: include_str!("typescript_no_non_null_assertion.scm"),
    },
    BuiltinRule {
        id: "typescript/no-empty-interface",
        content: include_str!("typescript_no_empty_interface.scm"),
    },
    BuiltinRule {
        id: "typescript/no-inferrable-types",
        content: include_str!("typescript_no_inferrable_types.scm"),
    },
    // Python rules
    BuiltinRule {
        id: "python/print-debug",
        content: include_str!("python_print_debug.scm"),
    },
    BuiltinRule {
        id: "python/breakpoint",
        content: include_str!("python_breakpoint.scm"),
    },
    BuiltinRule {
        id: "python/tuple-return",
        content: include_str!("python_tuple_return.scm"),
    },
    BuiltinRule {
        id: "python/module-assign",
        content: include_str!("python_module_assign.scm"),
    },
    BuiltinRule {
        id: "python/bare-except",
        content: include_str!("python_bare_except.scm"),
    },
    BuiltinRule {
        id: "python/mutable-default-arg",
        content: include_str!("python_mutable_default_arg.scm"),
    },
    BuiltinRule {
        id: "python/assert-in-non-test",
        content: include_str!("python_assert.scm"),
    },
    BuiltinRule {
        id: "python/use-enumerate",
        content: include_str!("python_use_enumerate.scm"),
    },
    BuiltinRule {
        id: "python/raise-without-from",
        content: include_str!("python_raise_without_from.scm"),
    },
    // Go rules
    BuiltinRule {
        id: "go/fmt-print",
        content: include_str!("go_fmt_print.scm"),
    },
    BuiltinRule {
        id: "go/many-returns",
        content: include_str!("go_many_returns.scm"),
    },
    BuiltinRule {
        id: "go/package-var",
        content: include_str!("go_package_var.scm"),
    },
    BuiltinRule {
        id: "go/error-ignored",
        content: include_str!("go_error_ignored.scm"),
    },
    BuiltinRule {
        id: "go/empty-return",
        content: include_str!("go_empty_return.scm"),
    },
    BuiltinRule {
        id: "go/defer-in-loop",
        content: include_str!("go_defer_in_loop.scm"),
    },
    BuiltinRule {
        id: "go/context-todo",
        content: include_str!("go_context_todo.scm"),
    },
    BuiltinRule {
        id: "go/sync-mutex-copied",
        content: include_str!("go_sync_mutex_copied.scm"),
    },
    // Ruby rules
    BuiltinRule {
        id: "ruby/binding-pry",
        content: include_str!("ruby_binding_pry.scm"),
    },
    BuiltinRule {
        id: "ruby/global-var",
        content: include_str!("ruby_global_var.scm"),
    },
    BuiltinRule {
        id: "ruby/rescue-exception",
        content: include_str!("ruby_rescue_exception.scm"),
    },
    BuiltinRule {
        id: "ruby/puts-in-lib",
        content: include_str!("ruby_puts_in_lib.scm"),
    },
    BuiltinRule {
        id: "ruby/string-concat",
        content: include_str!("ruby_string_concat.scm"),
    },
    BuiltinRule {
        id: "ruby/double-negation",
        content: include_str!("ruby_double_negation.scm"),
    },
    BuiltinRule {
        id: "ruby/open-struct",
        content: include_str!("ruby_open_struct.scm"),
    },
    // Cross-language rules
    BuiltinRule {
        id: "no-todo-comment",
        content: include_str!("no_todo_comment.scm"),
    },
    BuiltinRule {
        id: "no-fixme-comment",
        content: include_str!("no_fixme_comment.scm"),
    },
];
