use super::*;

#[test]
fn test_empty_rules() {
    let relations = Relations::new();
    let result = run_rules_source("", &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_cycle_detection_interpreted() {
    let mut relations = Relations::new();
    relations.add_import("a.py", "b.py", "*");
    relations.add_import("b.py", "a.py", "*");

    let rules = r#"
        relation reaches(String, String);
        relation cycle(String, String);

        reaches(from, to) <-- import(from, to, _);
        reaches(from, to) <-- import(from, mid, _), reaches(mid, to);
        cycle(a, b) <-- reaches(a, b), reaches(b, a), if a < b;

        warning("circular-deps", a) <-- cycle(a, _);
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].rule_id.as_str(), "circular-deps");
}

#[test]
fn test_no_cycles() {
    let mut relations = Relations::new();
    relations.add_import("a.py", "b.py", "*");
    relations.add_import("b.py", "c.py", "*");

    let rules = r#"
        relation reaches(String, String);
        relation cycle(String, String);

        reaches(from, to) <-- import(from, to, _);
        reaches(from, to) <-- import(from, mid, _), reaches(mid, to);
        cycle(a, b) <-- reaches(a, b), reaches(b, a), if a < b;

        warning("circular-deps", a) <-- cycle(a, _);
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_parse_rule_content_with_frontmatter() {
    let content = r#"
# ---
# id = "test-rule"
# message = "A test rule"
# ---

relation foo(String);
warning("test-rule", x) <-- foo(x);
"#;

    let rule = parse_rule_content(content, "fallback-id", false).unwrap();
    assert_eq!(rule.id, "test-rule");
    assert_eq!(rule.message, "A test rule");
    assert!(rule.enabled);
    assert!(!rule.builtin);
    assert!(rule.source.contains("relation foo"));
}

#[test]
fn test_parse_rule_content_without_frontmatter() {
    let content = "relation foo(String);\nwarning(\"x\", y) <-- foo(y);";

    let rule = parse_rule_content(content, "my-rule", false).unwrap();
    assert_eq!(rule.id, "my-rule");
    assert_eq!(rule.message, "Datalog rule");
    assert!(rule.source.contains("relation foo"));
}

#[test]
fn test_parse_rule_content_disabled() {
    let content = r#"
# ---
# id = "disabled-rule"
# enabled = false
# ---

warning("x", "y") <-- symbol(_, _, _, _);
"#;

    let rule = parse_rule_content(content, "x", false).unwrap();
    assert!(!rule.enabled);
}

#[test]
fn test_builtin_rules_parse() {
    for builtin in BUILTIN_RULES {
        let rule = parse_rule_content(builtin.content, builtin.id, true);
        assert!(rule.is_some(), "Failed to parse builtin: {}", builtin.id);
        let rule = rule.unwrap();
        assert!(rule.builtin);
        assert!(!rule.source.is_empty());
    }
}

#[test]
fn test_allow_patterns() {
    let content = r#"
# ---
# id = "test-allow"
# allow = ["**/tests/**", "**/*_test.py"]
# ---

warning("test-allow", file) <-- symbol(file, _, _, _);
"#;

    let mut relations = Relations::new();
    relations.add_symbol("src/main.py", "foo", "function", 1);
    relations.add_symbol("tests/test_foo.py", "test_foo", "function", 1);
    relations.add_symbol("src/foo_test.py", "bar", "function", 1);

    let rule = parse_rule_content(content, "test-allow", false).unwrap();
    assert_eq!(rule.allow.len(), 2);

    let result = run_rule(&rule, &relations).unwrap();
    // tests/test_foo.py matches **/tests/**, foo_test.py matches **/*_test.py
    // Only src/main.py should remain
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "src/main.py");
}

#[test]
fn test_negation() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "foo", "function", 1);
    relations.add_symbol("b.py", "bar", "function", 1);
    relations.add_call("a.py", "main", "foo", 5);
    // bar is never called

    let rules = r#"
        relation defined(String);
        relation called(String);
        relation uncalled(String);

        defined(name) <-- symbol(_, name, _, _);
        called(name) <-- call(_, _, name, _);
        uncalled(name) <-- defined(name), !called(name);

        warning("uncalled", name) <-- uncalled(name);
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "bar");
}

#[test]
fn test_string_comparison_in_if() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "MyClass", "class", 1);
    relations.add_symbol("a.py", "my_func", "function", 10);

    let rules = r#"
        relation func(String, String);
        func(file, name) <-- symbol(file, name, kind, _), if kind == "function";
        warning("func-found", name) <-- func(_, name);
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "my_func");
}

#[test]
fn test_aggregation_count() {
    let mut relations = Relations::new();
    relations.add_symbol("big.py", "a", "function", 1);
    relations.add_symbol("big.py", "b", "function", 2);
    relations.add_symbol("big.py", "c", "function", 3);
    relations.add_symbol("small.py", "x", "function", 1);

    let rules = r#"
        relation file_count(String, i32);
        file_count(file, c) <-- symbol(file, _, _, _), agg c = count() in symbol(file, _, _, _);
        warning("big-file", file) <-- file_count(file, c), if c > 2;
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "big.py");
}

#[test]
fn test_run_builtin_cycle_detection() {
    let mut relations = Relations::new();
    relations.add_import("a.py", "b.py", "*");
    relations.add_import("b.py", "a.py", "*");

    let rule = find_builtin("circular-deps");
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].rule_id.as_str(), "circular-deps");
}

#[test]
fn test_orphan_file() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "foo", "function", 1);
    relations.add_symbol("b.py", "bar", "function", 1);
    relations.add_symbol("c.py", "baz", "function", 1);
    relations.add_import("a.py", "b.py", "bar");
    // c.py is never imported

    // Orphan-file is disabled by default, force-enable for test
    let mut rule = find_builtin("orphan-file");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    let messages: Vec<&str> = result.iter().map(|d| d.message.as_str()).collect();
    assert!(messages.contains(&"a.py")); // a.py is also an orphan (not imported)
    assert!(messages.contains(&"c.py"));
    assert!(!messages.contains(&"b.py")); // b.py is imported
}

#[test]
fn test_self_import() {
    let mut relations = Relations::new();
    relations.add_import("a.py", "a.py", "foo");
    relations.add_import("b.py", "c.py", "bar");

    let rule = find_builtin("self-import");
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "a.py");
}

#[test]
fn test_god_file() {
    let mut relations = Relations::new();
    // Add 51 symbols to big.py
    for i in 0..51 {
        relations.add_symbol("big.py", &format!("sym_{}", i), "function", i);
    }
    // Add 5 symbols to small.py
    for i in 0..5 {
        relations.add_symbol("small.py", &format!("sym_{}", i), "function", i);
    }

    let rule = find_builtin("god-file");
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "big.py");
}

#[test]
fn test_fan_out() {
    let mut relations = Relations::new();
    // Add 51 calls from the same function (threshold is >50)
    for i in 0..51 {
        relations.add_call("a.py", "orchestrator", &format!("helper_{}", i), i);
    }
    // Add 3 calls from a simple function
    for i in 0..3 {
        relations.add_call("b.py", "simple", &format!("util_{}", i), i);
    }

    // Fan-out is disabled by default, force-enable for test
    let mut rule = find_builtin("fan-out");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "orchestrator");
}

#[test]
fn test_hub_file() {
    let mut relations = Relations::new();
    // 31 files import utils.py (threshold is >30)
    for i in 0..31 {
        relations.add_import(&format!("file_{}.py", i), "utils.py", "helper");
    }
    // Only 2 files import rare.py
    relations.add_import("a.py", "rare.py", "x");
    relations.add_import("b.py", "rare.py", "y");

    // Hub-file is disabled by default, force-enable for test
    let mut rule = find_builtin("hub-file");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "utils.py");
}

#[test]
fn test_duplicate_symbol_disabled_by_default() {
    // duplicate-symbol is disabled by default in frontmatter
    let builtin = BUILTIN_RULES
        .iter()
        .find(|b| b.id == "duplicate-symbol")
        .unwrap();
    let rule = parse_rule_content(builtin.content, builtin.id, true).unwrap();
    assert!(!rule.enabled);
}

#[test]
fn test_duplicate_symbol_when_enabled() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "process", "function", 1);
    relations.add_symbol("b.py", "process", "function", 5);
    relations.add_symbol("c.py", "unique", "function", 1);

    // Parse and force-enable
    let builtin = BUILTIN_RULES
        .iter()
        .find(|b| b.id == "duplicate-symbol")
        .unwrap();
    let mut rule = parse_rule_content(builtin.content, builtin.id, true).unwrap();
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "process");
}

#[test]
fn test_deny_promotes_warnings_to_errors() {
    let content = r#"
# ---
# id = "strict-rule"
# deny = true
# ---

warning("strict-rule", file) <-- symbol(file, _, _, _);
"#;

    let mut relations = Relations::new();
    relations.add_symbol("a.py", "foo", "function", 1);

    let rule = parse_rule_content(content, "strict-rule", false).unwrap();
    assert_eq!(rule.severity, Severity::Error);

    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].level, DiagnosticLevel::Error); // promoted from warning
}

#[test]
fn test_config_override_deny() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "foo", "function", 1);

    // self-import has severity=warning by default
    let rule = find_builtin("self-import");
    assert_eq!(rule.severity, Severity::Warning);

    // Apply config override with deny=true (legacy)
    let mut config = FactsRulesConfig::default();
    config.0.insert(
        "self-import".to_string(),
        FactsRuleOverride {
            deny: Some(true),
            ..Default::default()
        },
    );

    // Load with config
    let rules = load_all_rules(Path::new("/nonexistent"), &config);
    let self_import = rules.iter().find(|r| r.id == "self-import").unwrap();
    assert_eq!(self_import.severity, Severity::Error);
}

#[test]
fn test_config_override_allow() {
    let mut relations = Relations::new();
    for i in 0..51 {
        relations.add_symbol("big.py", &format!("sym_{}", i), "function", i);
    }
    for i in 0..51 {
        relations.add_symbol("generated/big.py", &format!("sym_{}", i), "function", i);
    }

    // Without config override, both files trigger god-file
    let mut rule = find_builtin("god-file");
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 2);

    // With config override, suppress generated/ files
    rule.allow.push(Pattern::new("generated/**").unwrap());
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "big.py");
}

#[test]
fn test_config_override_enable() {
    // fan-out is disabled by default
    let default_config = FactsRulesConfig::default();
    let rules = load_all_rules(Path::new("/nonexistent"), &default_config);
    let fan_out = rules.iter().find(|r| r.id == "fan-out").unwrap();
    assert!(!fan_out.enabled);

    // Enable via config
    let mut config = FactsRulesConfig::default();
    config.0.insert(
        "fan-out".to_string(),
        FactsRuleOverride {
            enabled: Some(true),
            ..Default::default()
        },
    );
    let rules = load_all_rules(Path::new("/nonexistent"), &config);
    let fan_out = rules.iter().find(|r| r.id == "fan-out").unwrap();
    assert!(fan_out.enabled);
}

#[test]
fn test_line_has_allow_comment() {
    assert!(line_has_allow_comment(
        "// normalize-facts-allow: god-file",
        "god-file"
    ));
    assert!(line_has_allow_comment(
        "# normalize-facts-allow: god-file",
        "god-file"
    ));
    assert!(line_has_allow_comment(
        "/* normalize-facts-allow: god-file */",
        "god-file"
    ));
    assert!(line_has_allow_comment(
        "// normalize-facts-allow: god-file - this file is intentionally large",
        "god-file"
    ));
    assert!(!line_has_allow_comment(
        "// normalize-facts-allow: god-file",
        "fan-out"
    ));
    assert!(!line_has_allow_comment(
        "// no suppression here",
        "god-file"
    ));
}

#[test]
fn test_filter_inline_allowed() {
    let dir = std::env::temp_dir().join("normalize_test_inline_allow");
    let _ = std::fs::create_dir_all(&dir);

    // File with suppression comment
    std::fs::write(
        dir.join("suppressed.py"),
        "# normalize-facts-allow: test-rule\ndef foo(): pass\n",
    )
    .unwrap();

    // File without suppression
    std::fs::write(dir.join("normal.py"), "def bar(): pass\n").unwrap();

    let mut diagnostics = vec![
        Diagnostic::warning("test-rule", "suppressed.py"),
        Diagnostic::warning("test-rule", "normal.py"),
        Diagnostic::warning("test-rule", "nonexistent.py"), // not a file, kept
    ];

    filter_inline_allowed(&mut diagnostics, &dir);

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].message.as_str(), "normal.py");
    assert_eq!(diagnostics[1].message.as_str(), "nonexistent.py");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_visibility_relation() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "foo", "function", 1);
    relations.add_symbol("a.py", "_bar", "function", 5);
    relations.add_visibility("a.py", "foo", "public");
    relations.add_visibility("a.py", "_bar", "private");

    let rules = r#"
        relation priv_func(String, String);
        priv_func(file, name) <-- visibility(file, name, vis), if vis == "private";
        warning("private-func", name) <-- priv_func(_, name);
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "_bar");
}

#[test]
fn test_attribute_relation() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "foo", "function", 1);
    relations.add_attribute("a.py", "foo", "@staticmethod");
    relations.add_attribute("a.py", "foo", "@override");

    let rules = r##"
        relation static_fn(String, String);
        static_fn(file, name) <-- attribute(file, name, attr), if attr == "@staticmethod";
        warning("static-fn", name) <-- static_fn(_, name);
    "##;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "foo");
}

#[test]
fn test_parent_relation() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "MyClass", "class", 1);
    relations.add_symbol("a.py", "method_a", "method", 2);
    relations.add_symbol("a.py", "method_b", "method", 5);
    relations.add_parent("a.py", "method_a", "MyClass");
    relations.add_parent("a.py", "method_b", "MyClass");

    let rules = r#"
        relation method_count(String, String, i32);
        method_count(file, cls, c) <--
            parent(file, _, cls),
            agg c = count() in parent(file, _, cls);
        warning("big-class", cls) <-- method_count(_, cls, c), if c > 1;
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "MyClass");
}

#[test]
fn test_qualifier_relation() {
    let mut relations = Relations::new();
    relations.add_call("a.py", "method_a", "method_b", 3);
    relations.add_qualifier("a.py", "method_a", "method_b", "self");
    relations.add_call("a.py", "main", "helper", 10);

    let rules = r#"
        relation self_call(String, String, String);
        self_call(file, caller, callee) <-- qualifier(file, caller, callee, q), if q == "self";
        warning("self-call", callee) <-- self_call(_, _, callee);
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "method_b");
}

#[test]
fn test_symbol_range_relation() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "big_func", "function", 1);
    relations.add_symbol("a.py", "small_func", "function", 50);
    relations.add_symbol_range("a.py", "big_func", 1, 100);
    relations.add_symbol_range("a.py", "small_func", 50, 55);

    let rules = r#"
        relation long_fn(String, String, u32);
        long_fn(file, name, len) <--
            symbol_range(file, name, start, end),
            let len = end - start,
            if len > 20u32;
        warning("long-fn", name) <-- long_fn(_, name, _);
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "big_func");
}

#[test]
fn test_implements_relation() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "MyClass", "class", 1);
    relations.add_implements("a.py", "MyClass", "Serializable");
    relations.add_implements("a.py", "MyClass", "Comparable");
    relations.add_symbol("b.py", "OtherClass", "class", 1);

    let rules = r#"
        relation impl_count(String, String, i32);
        impl_count(file, name, c) <--
            implements(file, name, _),
            agg c = count() in implements(file, name, _);
        warning("multi-impl", name) <-- impl_count(_, name, c), if c > 1;
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "MyClass");
}

#[test]
fn test_is_impl_relation() {
    let mut relations = Relations::new();
    relations.add_symbol("a.rs", "impl_method", "method", 5);
    relations.add_symbol("a.rs", "free_func", "function", 20);
    relations.add_is_impl("a.rs", "impl_method");

    let rules = r#"
        warning("is-impl", name) <-- is_impl(_, name);
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "impl_method");
}

#[test]
fn test_type_method_relation() {
    let mut relations = Relations::new();
    relations.add_type_method("a.py", "Animal", "speak");
    relations.add_type_method("a.py", "Animal", "move");
    relations.add_type_method("b.py", "Vehicle", "drive");

    let rules = r#"
        relation method_count(String, String, i32);
        method_count(file, t, c) <--
            type_method(file, t, _),
            agg c = count() in type_method(file, t, _);
        warning("rich-type", t) <-- method_count(_, t, c), if c > 1;
    "#;

    let result = run_rules_source(rules, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "Animal");
}

#[test]
fn test_god_class_fires() {
    let mut relations = Relations::new();
    // Add a class with 21 methods (threshold is >20)
    relations.add_symbol("a.py", "BigClass", "class", 1);
    for i in 0..21 {
        let method = format!("method_{}", i);
        relations.add_symbol("a.py", &method, "method", i + 2);
        relations.add_parent("a.py", &method, "BigClass");
    }
    // Add a small class (should not fire)
    relations.add_symbol("a.py", "SmallClass", "class", 100);
    relations.add_symbol("a.py", "do_thing", "method", 101);
    relations.add_parent("a.py", "do_thing", "SmallClass");

    let mut rule = find_builtin("god-class");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "BigClass");
}

#[test]
fn test_god_class_no_fire() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "NormalClass", "class", 1);
    for i in 0..5 {
        let method = format!("method_{}", i);
        relations.add_symbol("a.py", &method, "method", i + 2);
        relations.add_parent("a.py", &method, "NormalClass");
    }

    let mut rule = find_builtin("god-class");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_long_function_fires() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "huge_func", "function", 1);
    relations.add_symbol_range("a.py", "huge_func", 1, 150);
    relations.add_symbol("a.py", "tiny_func", "function", 200);
    relations.add_symbol_range("a.py", "tiny_func", 200, 210);

    let mut rule = find_builtin("long-function");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "huge_func");
}

#[test]
fn test_long_function_no_fire() {
    let mut relations = Relations::new();
    relations.add_symbol("a.py", "short_func", "function", 1);
    relations.add_symbol_range("a.py", "short_func", 1, 50);

    let mut rule = find_builtin("long-function");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_dead_api_fires() {
    let mut relations = Relations::new();
    // Public function defined in a.py, never called from another file
    relations.add_symbol("a.py", "unused_pub", "function", 1);
    relations.add_visibility("a.py", "unused_pub", "public");
    // Public function defined in b.py, called from a.py (not dead)
    relations.add_symbol("b.py", "used_pub", "function", 1);
    relations.add_visibility("b.py", "used_pub", "public");
    relations.add_call("a.py", "main", "used_pub", 5);

    let mut rule = find_builtin("dead-api");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "unused_pub");
}

#[test]
fn test_dead_api_no_fire() {
    let mut relations = Relations::new();
    // Public function called from another file
    relations.add_symbol("a.py", "helper", "function", 1);
    relations.add_visibility("a.py", "helper", "public");
    relations.add_call("b.py", "main", "helper", 5);

    let mut rule = find_builtin("dead-api");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_missing_impl_fires() {
    let mut relations = Relations::new();
    // Interface with 2 methods
    relations.add_type_method("iface.ts", "Serializable", "serialize");
    relations.add_type_method("iface.ts", "Serializable", "deserialize");
    // Class implements Serializable but only has serialize
    relations.add_symbol("impl.ts", "MyClass", "class", 1);
    relations.add_implements("impl.ts", "MyClass", "Serializable");
    relations.add_parent("impl.ts", "serialize", "MyClass");
    // Missing: deserialize

    let mut rule = find_builtin("missing-impl");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "MyClass");
}

#[test]
fn test_missing_impl_no_fire() {
    let mut relations = Relations::new();
    // Interface with 1 method
    relations.add_type_method("iface.ts", "Runnable", "run");
    // Class implements Runnable and has the method
    relations.add_symbol("impl.ts", "Worker", "class", 1);
    relations.add_implements("impl.ts", "Worker", "Runnable");
    relations.add_parent("impl.ts", "run", "Worker");

    let mut rule = find_builtin("missing-impl");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_unused_import_fires() {
    let mut relations = Relations::new();
    relations.add_import("a.py", "b.py", "helper");
    // "helper" is never called in a.py

    let mut rule = find_builtin("unused-import");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "helper");
}

#[test]
fn test_unused_import_no_fire() {
    let mut relations = Relations::new();
    relations.add_import("a.py", "b.py", "helper");
    relations.add_call("a.py", "main", "helper", 5);

    let mut rule = find_builtin("unused-import");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_unused_import_wildcard_ignored() {
    let mut relations = Relations::new();
    relations.add_import("a.py", "b.py", "*");

    let mut rule = find_builtin("unused-import");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_barrel_file_fires() {
    let mut relations = Relations::new();
    // File with only imports, no own symbols
    relations.add_import("index.ts", "a.ts", "foo");
    relations.add_import("index.ts", "b.ts", "bar");

    let mut rule = find_builtin("barrel-file");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "index.ts");
}

#[test]
fn test_barrel_file_no_fire() {
    let mut relations = Relations::new();
    relations.add_import("app.ts", "utils.ts", "helper");
    relations.add_symbol("app.ts", "main", "function", 1);

    let mut rule = find_builtin("barrel-file");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_bidirectional_deps_fires() {
    let mut relations = Relations::new();
    relations.add_import("a.py", "b.py", "foo");
    relations.add_import("b.py", "a.py", "bar");

    let mut rule = find_builtin("bidirectional-deps");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_bidirectional_deps_no_fire() {
    let mut relations = Relations::new();
    relations.add_import("a.py", "b.py", "foo");
    relations.add_import("a.py", "c.py", "bar");

    let mut rule = find_builtin("bidirectional-deps");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_deep_nesting_fires() {
    let mut relations = Relations::new();
    // 5 levels: Top -> A -> B -> C -> TooDeep (4 parent hops = >3)
    relations.add_symbol("a.py", "Top", "class", 1);
    relations.add_symbol("a.py", "A", "class", 5);
    relations.add_parent("a.py", "A", "Top");
    relations.add_symbol("a.py", "B", "class", 10);
    relations.add_parent("a.py", "B", "A");
    relations.add_symbol("a.py", "C", "class", 15);
    relations.add_parent("a.py", "C", "B");
    relations.add_symbol("a.py", "TooDeep", "function", 20);
    relations.add_parent("a.py", "TooDeep", "C");

    let mut rule = find_builtin("deep-nesting");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    let messages: Vec<&str> = result.iter().map(|d| d.message.as_str()).collect();
    assert!(messages.contains(&"TooDeep"));
}

#[test]
fn test_deep_nesting_no_fire() {
    let mut relations = Relations::new();
    // 2 levels: MyClass -> method (only 1 parent hop)
    relations.add_symbol("a.py", "MyClass", "class", 1);
    relations.add_symbol("a.py", "method", "method", 5);
    relations.add_parent("a.py", "method", "MyClass");

    let mut rule = find_builtin("deep-nesting");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_layering_violation_fires() {
    let mut relations = Relations::new();
    // Both files have test attributes
    relations.add_symbol("test_a.py", "test_foo", "function", 1);
    relations.add_attribute("test_a.py", "test_foo", "#[test]");
    relations.add_symbol("test_b.py", "test_bar", "function", 1);
    relations.add_attribute("test_b.py", "test_bar", "#[test]");
    relations.add_import("test_a.py", "test_b.py", "helper");

    let mut rule = find_builtin("layering-violation");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "test_a.py");
}

#[test]
fn test_layering_violation_no_fire() {
    let mut relations = Relations::new();
    // test_a has test attribute, utils does not
    relations.add_symbol("test_a.py", "test_foo", "function", 1);
    relations.add_attribute("test_a.py", "test_foo", "#[test]");
    relations.add_symbol("utils.py", "helper", "function", 1);
    relations.add_import("test_a.py", "utils.py", "helper");

    let mut rule = find_builtin("layering-violation");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_missing_export_fires() {
    let mut relations = Relations::new();
    relations.add_symbol("utils.py", "helper", "function", 1);
    relations.add_visibility("utils.py", "helper", "public");
    // No file imports utils.py

    let mut rule = find_builtin("missing-export");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].message.as_str(), "utils.py");
}

#[test]
fn test_missing_export_no_fire() {
    let mut relations = Relations::new();
    relations.add_symbol("utils.py", "helper", "function", 1);
    relations.add_visibility("utils.py", "helper", "public");
    relations.add_import("app.py", "utils.py", "helper");

    let mut rule = find_builtin("missing-export");
    rule.enabled = true;
    let result = run_rule(&rule, &relations).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_new_builtins_parse() {
    // These rules should be disabled by default
    for id in &["barrel-file", "layering-violation", "missing-export"] {
        let rule = find_builtin(id);
        assert!(!rule.enabled, "{} should be disabled by default", id);
        assert!(rule.builtin, "{} should be builtin", id);
    }

    // These rules should be enabled by default
    for id in &[
        "unused-import",
        "bidirectional-deps",
        "deep-nesting",
        "god-class",
        "long-function",
    ] {
        let rule = find_builtin(id);
        assert!(rule.enabled, "{} should be enabled by default", id);
        assert!(rule.builtin, "{} should be builtin", id);
    }
}

/// Helper to find and parse a builtin rule by ID.
fn find_builtin(id: &str) -> FactsRule {
    let builtin = BUILTIN_RULES.iter().find(|b| b.id == id).unwrap();
    parse_rule_content(builtin.content, builtin.id, true).unwrap()
}
