"""Tests for SARIF output module."""

import json
from pathlib import Path

from moss.rules import Rule, RuleResult, Violation
from moss.sarif import (
    SARIF_VERSION,
    SARIFConfig,
    generate_sarif,
    sarif_from_rules_result,
    write_sarif,
)


class TestSARIFConfig:
    """Tests for SARIFConfig dataclass."""

    def test_default_values(self):
        config = SARIFConfig()

        assert config.tool_name == "moss"
        assert config.tool_version == "0.1.0"
        assert config.include_snippets is True
        assert config.include_fingerprints is True

    def test_custom_values(self):
        config = SARIFConfig(
            tool_name="custom-tool",
            tool_version="2.0.0",
            include_snippets=False,
        )

        assert config.tool_name == "custom-tool"
        assert config.include_snippets is False


class TestGenerateSarif:
    """Tests for generate_sarif function."""

    def test_empty_result(self):
        result = RuleResult()

        sarif = generate_sarif(result)

        assert sarif["version"] == SARIF_VERSION
        assert "$schema" in sarif
        assert len(sarif["runs"]) == 1
        assert sarif["runs"][0]["results"] == []

    def test_single_violation(self):
        rule = Rule(
            name="test-rule",
            pattern="test",
            message="Test message",
            severity="warning",
            category="test-category",
        )
        result = RuleResult(
            violations=[
                Violation(
                    rule=rule,
                    file_path=Path("test.py"),
                    line=10,
                    column=5,
                    match_text="test",
                    context="line with test",
                )
            ]
        )

        sarif = generate_sarif(result)

        assert len(sarif["runs"][0]["results"]) == 1
        sarif_result = sarif["runs"][0]["results"][0]
        assert sarif_result["ruleId"] == "test-rule"
        assert sarif_result["level"] == "warning"
        assert sarif_result["message"]["text"] == "Test message"

    def test_location_info(self):
        rule = Rule(name="test", pattern="x", message="msg")
        result = RuleResult(
            violations=[
                Violation(
                    rule=rule,
                    file_path=Path("src/file.py"),
                    line=42,
                    column=13,
                    match_text="x",
                )
            ]
        )

        sarif = generate_sarif(result)

        location = sarif["runs"][0]["results"][0]["locations"][0]
        physical = location["physicalLocation"]
        assert "src/file.py" in physical["artifactLocation"]["uri"]
        assert physical["region"]["startLine"] == 42
        assert physical["region"]["startColumn"] == 13

    def test_includes_snippet(self):
        rule = Rule(name="test", pattern="x", message="msg")
        result = RuleResult(
            violations=[
                Violation(
                    rule=rule,
                    file_path=Path("test.py"),
                    line=1,
                    column=1,
                    match_text="x",
                    context="full line with x in it",
                )
            ]
        )

        sarif = generate_sarif(result)

        location = sarif["runs"][0]["results"][0]["locations"][0]
        snippet = location["physicalLocation"]["region"]["snippet"]
        assert "x" in snippet["text"]

    def test_excludes_snippet_when_disabled(self):
        rule = Rule(name="test", pattern="x", message="msg")
        result = RuleResult(
            violations=[
                Violation(
                    rule=rule,
                    file_path=Path("test.py"),
                    line=1,
                    column=1,
                    match_text="x",
                    context="context text",
                )
            ]
        )

        config = SARIFConfig(include_snippets=False)
        sarif = generate_sarif(result, config)

        location = sarif["runs"][0]["results"][0]["locations"][0]
        assert "snippet" not in location["physicalLocation"]["region"]

    def test_includes_fingerprints(self):
        rule = Rule(name="test", pattern="x", message="msg")
        result = RuleResult(
            violations=[
                Violation(
                    rule=rule,
                    file_path=Path("test.py"),
                    line=1,
                    column=1,
                    match_text="x",
                )
            ]
        )

        sarif = generate_sarif(result)

        sarif_result = sarif["runs"][0]["results"][0]
        assert "fingerprints" in sarif_result
        assert "primaryLocationLineHash" in sarif_result["fingerprints"]

    def test_severity_mapping(self):
        # Test error
        error_rule = Rule(name="err", pattern="x", message="m", severity="error")
        error_result = RuleResult(
            violations=[
                Violation(rule=error_rule, file_path=Path("t.py"), line=1, column=1, match_text="x")
            ]
        )
        sarif = generate_sarif(error_result)
        assert sarif["runs"][0]["results"][0]["level"] == "error"

        # Test warning
        warn_rule = Rule(name="warn", pattern="x", message="m", severity="warning")
        warn_result = RuleResult(
            violations=[
                Violation(rule=warn_rule, file_path=Path("t.py"), line=1, column=1, match_text="x")
            ]
        )
        sarif = generate_sarif(warn_result)
        assert sarif["runs"][0]["results"][0]["level"] == "warning"

        # Test info -> note
        info_rule = Rule(name="info", pattern="x", message="m", severity="info")
        info_result = RuleResult(
            violations=[
                Violation(rule=info_rule, file_path=Path("t.py"), line=1, column=1, match_text="x")
            ]
        )
        sarif = generate_sarif(info_result)
        assert sarif["runs"][0]["results"][0]["level"] == "note"

    def test_multiple_violations(self):
        rule1 = Rule(name="rule1", pattern="a", message="msg1")
        rule2 = Rule(name="rule2", pattern="b", message="msg2")
        result = RuleResult(
            violations=[
                Violation(rule=rule1, file_path=Path("a.py"), line=1, column=1, match_text="a"),
                Violation(rule=rule1, file_path=Path("b.py"), line=2, column=2, match_text="a"),
                Violation(rule=rule2, file_path=Path("c.py"), line=3, column=3, match_text="b"),
            ]
        )

        sarif = generate_sarif(result)

        assert len(sarif["runs"][0]["results"]) == 3
        # Should have 2 unique rules
        rules = sarif["runs"][0]["tool"]["driver"]["rules"]
        rule_ids = {r["id"] for r in rules}
        assert rule_ids == {"rule1", "rule2"}

    def test_tool_info(self):
        result = RuleResult()
        config = SARIFConfig(
            tool_name="my-tool",
            tool_version="1.2.3",
            tool_information_uri="https://example.com",
        )

        sarif = generate_sarif(result, config)

        driver = sarif["runs"][0]["tool"]["driver"]
        assert driver["name"] == "my-tool"
        assert driver["version"] == "1.2.3"
        assert driver["informationUri"] == "https://example.com"

    def test_rule_descriptors(self):
        rule = Rule(
            name="my-rule",
            pattern="x",
            message="My message",
            severity="error",
            category="security",
            documentation="https://docs.example.com",
            fix="Remove the issue",
        )
        result = RuleResult(
            violations=[
                Violation(rule=rule, file_path=Path("t.py"), line=1, column=1, match_text="x")
            ]
        )

        sarif = generate_sarif(result)

        rules = sarif["runs"][0]["tool"]["driver"]["rules"]
        assert len(rules) == 1
        rule_desc = rules[0]
        assert rule_desc["id"] == "my-rule"
        assert rule_desc["shortDescription"]["text"] == "My message"
        assert rule_desc["defaultConfiguration"]["level"] == "error"
        assert rule_desc["properties"]["category"] == "security"
        assert rule_desc["helpUri"] == "https://docs.example.com"
        assert "Remove the issue" in rule_desc["help"]["text"]

    def test_relative_paths(self):
        rule = Rule(name="test", pattern="x", message="msg")
        result = RuleResult(
            violations=[
                Violation(
                    rule=rule,
                    file_path=Path("/home/user/project/src/file.py"),
                    line=1,
                    column=1,
                    match_text="x",
                )
            ]
        )

        config = SARIFConfig(base_path=Path("/home/user/project"))
        sarif = generate_sarif(result, config)

        location = sarif["runs"][0]["results"][0]["locations"][0]
        uri = location["physicalLocation"]["artifactLocation"]["uri"]
        assert "src/file.py" in uri
        assert "/home/user" not in uri

    def test_invocation_info(self):
        result = RuleResult()

        sarif = generate_sarif(result)

        invocations = sarif["runs"][0]["invocations"]
        assert len(invocations) == 1
        assert invocations[0]["executionSuccessful"] is True
        assert "endTimeUtc" in invocations[0]


class TestWriteSarif:
    """Tests for write_sarif function."""

    def test_writes_json_file(self, tmp_path: Path):
        sarif = {"version": SARIF_VERSION, "runs": []}
        output_path = tmp_path / "results.sarif"

        write_sarif(sarif, output_path)

        assert output_path.exists()
        content = output_path.read_text()
        parsed = json.loads(content)
        assert parsed["version"] == SARIF_VERSION


class TestSarifFromRulesResult:
    """Tests for sarif_from_rules_result convenience function."""

    def test_returns_json_string(self):
        result = RuleResult()

        json_str = sarif_from_rules_result(result)

        # Should be valid JSON
        parsed = json.loads(json_str)
        assert parsed["version"] == SARIF_VERSION

    def test_custom_tool_info(self):
        result = RuleResult()

        json_str = sarif_from_rules_result(
            result,
            tool_name="custom",
            version="3.0.0",
        )

        parsed = json.loads(json_str)
        assert parsed["runs"][0]["tool"]["driver"]["name"] == "custom"
        assert parsed["runs"][0]["tool"]["driver"]["version"] == "3.0.0"
