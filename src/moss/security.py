"""Security analysis via multi-tool orchestration.

Aggregates results from multiple security analysis tools:
- bandit: Python-specific security linting
- semgrep: SAST pattern matching with community rules
- (future) Snyk, CodeQL, etc.

Usage:
    from moss.security import SecurityAnalyzer

    analyzer = SecurityAnalyzer(project_root)
    results = analyzer.analyze()

    # Or via CLI:
    # moss security [directory] [--tools bandit,semgrep] [--severity medium]
"""

from __future__ import annotations

import json
import logging
import shutil
import subprocess
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from enum import IntEnum
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


class Severity(IntEnum):
    """Severity levels for security findings."""

    INFO = 0
    LOW = 1
    MEDIUM = 2
    HIGH = 3
    CRITICAL = 4

    @classmethod
    def from_string(cls, s: str) -> Severity:
        """Parse severity from string."""
        mapping = {
            "info": cls.INFO,
            "low": cls.LOW,
            "medium": cls.MEDIUM,
            "med": cls.MEDIUM,
            "high": cls.HIGH,
            "critical": cls.CRITICAL,
            "crit": cls.CRITICAL,
            "error": cls.HIGH,
            "warning": cls.MEDIUM,
            "warn": cls.MEDIUM,
        }
        return mapping.get(s.lower(), cls.MEDIUM)


@dataclass
class Finding:
    """A security finding from any tool."""

    tool: str
    rule_id: str
    message: str
    severity: Severity
    file_path: str
    line_start: int
    line_end: int | None = None
    cwe: str | None = None  # CWE ID if available
    owasp: str | None = None  # OWASP category if available
    fix_suggestion: str | None = None
    confidence: str | None = None  # low, medium, high

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "tool": self.tool,
            "rule_id": self.rule_id,
            "message": self.message,
            "severity": self.severity.name.lower(),
            "file": self.file_path,
            "line": self.line_start,
            "line_end": self.line_end,
            "cwe": self.cwe,
            "owasp": self.owasp,
            "fix": self.fix_suggestion,
            "confidence": self.confidence,
        }

    @property
    def location_key(self) -> str:
        """Key for deduplication by location."""
        return f"{self.file_path}:{self.line_start}"


@dataclass
class SecurityAnalysis:
    """Results from security analysis."""

    root: Path
    findings: list[Finding] = field(default_factory=list)
    tools_run: list[str] = field(default_factory=list)
    tools_skipped: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)

    @property
    def critical_count(self) -> int:
        return sum(1 for f in self.findings if f.severity == Severity.CRITICAL)

    @property
    def high_count(self) -> int:
        return sum(1 for f in self.findings if f.severity == Severity.HIGH)

    @property
    def medium_count(self) -> int:
        return sum(1 for f in self.findings if f.severity == Severity.MEDIUM)

    @property
    def low_count(self) -> int:
        return sum(1 for f in self.findings if f.severity == Severity.LOW)

    def filter_by_severity(self, min_severity: Severity) -> list[Finding]:
        """Get findings at or above a severity threshold."""
        return [f for f in self.findings if f.severity >= min_severity]

    def dedupe(self) -> SecurityAnalysis:
        """Remove duplicate findings at the same location."""
        seen: dict[str, Finding] = {}
        for finding in self.findings:
            key = finding.location_key
            if key not in seen or finding.severity > seen[key].severity:
                seen[key] = finding

        return SecurityAnalysis(
            root=self.root,
            findings=list(seen.values()),
            tools_run=self.tools_run,
            tools_skipped=self.tools_skipped,
            errors=self.errors,
        )

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "root": str(self.root),
            "summary": {
                "total": len(self.findings),
                "critical": self.critical_count,
                "high": self.high_count,
                "medium": self.medium_count,
                "low": self.low_count,
            },
            "tools_run": self.tools_run,
            "tools_skipped": self.tools_skipped,
            "errors": self.errors,
            "findings": [f.to_dict() for f in self.findings],
        }


class SecurityTool(ABC):
    """Base class for security analysis tools."""

    name: str = "unknown"

    @abstractmethod
    def is_available(self) -> bool:
        """Check if the tool is installed and available."""
        ...

    @abstractmethod
    def analyze(self, root: Path, **options: Any) -> list[Finding]:
        """Run the tool and return findings."""
        ...


class BanditTool(SecurityTool):
    """Bandit - Python security linter."""

    name = "bandit"

    def is_available(self) -> bool:
        return shutil.which("bandit") is not None

    def analyze(self, root: Path, **options: Any) -> list[Finding]:
        """Run bandit on Python files."""
        findings = []

        try:
            result = subprocess.run(
                [
                    "bandit",
                    "-r",
                    str(root),
                    "-f",
                    "json",
                    "-q",  # quiet, don't print to stderr
                    "--exclude",
                    ".venv,venv,node_modules,.git,__pycache__,dist,build",
                ],
                capture_output=True,
                text=True,
                timeout=300,
            )

            if result.stdout:
                data = json.loads(result.stdout)
                for item in data.get("results", []):
                    findings.append(
                        Finding(
                            tool=self.name,
                            rule_id=item.get("test_id", "unknown"),
                            message=item.get("issue_text", ""),
                            severity=Severity.from_string(item.get("issue_severity", "medium")),
                            file_path=item.get("filename", ""),
                            line_start=item.get("line_number", 0),
                            line_end=item.get("end_col_offset"),
                            cwe=self._get_cwe(item.get("test_id")),
                            confidence=item.get("issue_confidence", "").lower(),
                        )
                    )

        except subprocess.TimeoutExpired:
            logger.warning("Bandit timed out")
        except json.JSONDecodeError as e:
            logger.warning("Failed to parse bandit output: %s", e)
        except Exception as e:
            logger.warning("Bandit failed: %s", e)

        return findings

    def _get_cwe(self, test_id: str) -> str | None:
        """Map bandit test IDs to CWE numbers."""
        # Common mappings
        cwe_map = {
            "B101": "CWE-703",  # assert used
            "B102": "CWE-78",  # exec used
            "B103": "CWE-732",  # chmod
            "B104": "CWE-400",  # bind all interfaces
            "B105": "CWE-259",  # hardcoded password
            "B106": "CWE-259",  # hardcoded password
            "B107": "CWE-259",  # hardcoded password
            "B108": "CWE-377",  # insecure temp file
            "B110": "CWE-703",  # try/except pass
            "B112": "CWE-703",  # try/except continue
            "B201": "CWE-502",  # flask debug
            "B301": "CWE-502",  # pickle
            "B302": "CWE-502",  # marshal
            "B303": "CWE-327",  # md5/sha1
            "B304": "CWE-327",  # insecure cipher
            "B305": "CWE-327",  # insecure cipher mode
            "B306": "CWE-295",  # mktemp
            "B307": "CWE-78",  # eval
            "B308": "CWE-79",  # mark_safe
            "B309": "CWE-295",  # httpsconnection
            "B310": "CWE-330",  # urllib urlopen
            "B311": "CWE-330",  # random
            "B312": "CWE-295",  # telnetlib
            "B313": "CWE-611",  # xml parsing
            "B314": "CWE-611",  # xml parsing
            "B315": "CWE-611",  # xml parsing
            "B316": "CWE-611",  # xml parsing
            "B317": "CWE-611",  # xml parsing
            "B318": "CWE-611",  # xml parsing
            "B319": "CWE-611",  # xml parsing
            "B320": "CWE-611",  # xml parsing
            "B321": "CWE-295",  # ftplib
            "B323": "CWE-295",  # ssl unverified
            "B324": "CWE-327",  # hashlib insecure
            "B501": "CWE-295",  # requests no verify
            "B502": "CWE-295",  # ssl no verify
            "B503": "CWE-295",  # ssl bad version
            "B504": "CWE-295",  # ssl bad cipher
            "B505": "CWE-327",  # weak cryptographic key
            "B506": "CWE-94",  # yaml load
            "B507": "CWE-295",  # ssh no host key verify
            "B601": "CWE-78",  # paramiko exec
            "B602": "CWE-78",  # subprocess shell
            "B603": "CWE-78",  # subprocess no shell
            "B604": "CWE-78",  # any other function
            "B605": "CWE-78",  # os.system
            "B606": "CWE-78",  # os.popen
            "B607": "CWE-78",  # partial path
            "B608": "CWE-89",  # sql injection
            "B609": "CWE-78",  # wildcard injection
            "B610": "CWE-94",  # django extra
            "B611": "CWE-94",  # django raw
            "B701": "CWE-94",  # jinja2 autoescape
            "B702": "CWE-79",  # mako templates
            "B703": "CWE-79",  # django xss
        }
        return cwe_map.get(test_id)


class SemgrepTool(SecurityTool):
    """Semgrep - SAST pattern matching."""

    name = "semgrep"

    def is_available(self) -> bool:
        return shutil.which("semgrep") is not None

    def analyze(self, root: Path, **options: Any) -> list[Finding]:
        """Run semgrep with security rules."""
        findings = []

        try:
            result = subprocess.run(
                [
                    "semgrep",
                    "--config",
                    "auto",  # Auto-detect language and use default rules
                    "--json",
                    "--quiet",
                    "--exclude",
                    ".venv",
                    "--exclude",
                    "venv",
                    "--exclude",
                    "node_modules",
                    "--exclude",
                    ".git",
                    str(root),
                ],
                capture_output=True,
                text=True,
                timeout=600,
            )

            if result.stdout:
                data = json.loads(result.stdout)
                for item in data.get("results", []):
                    extra = item.get("extra", {})
                    metadata = extra.get("metadata", {})

                    findings.append(
                        Finding(
                            tool=self.name,
                            rule_id=item.get("check_id", "unknown"),
                            message=extra.get("message", ""),
                            severity=Severity.from_string(extra.get("severity", "medium")),
                            file_path=item.get("path", ""),
                            line_start=item.get("start", {}).get("line", 0),
                            line_end=item.get("end", {}).get("line"),
                            cwe=self._extract_cwe(metadata),
                            owasp=metadata.get("owasp"),
                            fix_suggestion=extra.get("fix"),
                            confidence=metadata.get("confidence", "").lower(),
                        )
                    )

        except subprocess.TimeoutExpired:
            logger.warning("Semgrep timed out")
        except json.JSONDecodeError as e:
            logger.warning("Failed to parse semgrep output: %s", e)
        except Exception as e:
            logger.warning("Semgrep failed: %s", e)

        return findings

    def _extract_cwe(self, metadata: dict) -> str | None:
        """Extract CWE from semgrep metadata."""
        cwe = metadata.get("cwe")
        if isinstance(cwe, list) and cwe:
            return cwe[0]
        if isinstance(cwe, str):
            return cwe
        return None


# Registry of available tools
SECURITY_TOOLS: list[SecurityTool] = [
    BanditTool(),
    SemgrepTool(),
]


class SecurityAnalyzer:
    """Orchestrates multiple security analysis tools."""

    def __init__(
        self,
        root: Path,
        tools: list[str] | None = None,
        min_severity: Severity = Severity.LOW,
    ):
        """Initialize the analyzer.

        Args:
            root: Project root directory
            tools: List of tool names to use (None = all available)
            min_severity: Minimum severity to report
        """
        self.root = Path(root).resolve()
        self.requested_tools = tools
        self.min_severity = min_severity

    def analyze(self, dedupe: bool = True) -> SecurityAnalysis:
        """Run all available security tools.

        Args:
            dedupe: Remove duplicate findings at same location

        Returns:
            SecurityAnalysis with aggregated findings
        """
        result = SecurityAnalysis(root=self.root)

        for tool in SECURITY_TOOLS:
            # Skip if not requested
            if self.requested_tools and tool.name not in self.requested_tools:
                continue

            if not tool.is_available():
                result.tools_skipped.append(tool.name)
                logger.debug("Skipping %s (not installed)", tool.name)
                continue

            logger.info("Running %s...", tool.name)
            try:
                findings = tool.analyze(self.root)
                result.findings.extend(findings)
                result.tools_run.append(tool.name)
                logger.info("%s found %d issues", tool.name, len(findings))
            except Exception as e:
                result.errors.append(f"{tool.name}: {e}")
                logger.error("%s failed: %s", tool.name, e)

        # Filter by severity
        result.findings = [f for f in result.findings if f.severity >= self.min_severity]

        # Sort by severity (critical first), then file, then line
        result.findings.sort(key=lambda f: (-f.severity, f.file_path, f.line_start))

        if dedupe:
            result = result.dedupe()

        return result


def format_security_analysis(analysis: SecurityAnalysis) -> str:
    """Format security analysis as markdown."""
    lines = ["## Security Analysis", ""]

    # Summary
    total = len(analysis.findings)
    if total == 0 and not analysis.tools_run:
        lines.append("No security tools available. Install bandit or semgrep:")
        lines.append("  pip install bandit")
        lines.append("  pip install semgrep")
        return "\n".join(lines)

    lines.append(f"**Tools run:** {', '.join(analysis.tools_run) or 'none'}")
    if analysis.tools_skipped:
        lines.append(f"**Tools skipped:** {', '.join(analysis.tools_skipped)} (not installed)")
    lines.append("")

    if analysis.errors:
        lines.append("**Errors:**")
        for err in analysis.errors:
            lines.append(f"  - {err}")
        lines.append("")

    # Counts
    if total == 0:
        lines.append("No security issues found.")
        return "\n".join(lines)

    lines.append("### Summary")
    lines.append(f"- Critical: {analysis.critical_count}")
    lines.append(f"- High: {analysis.high_count}")
    lines.append(f"- Medium: {analysis.medium_count}")
    lines.append(f"- Low: {analysis.low_count}")
    lines.append(f"- **Total: {total}**")
    lines.append("")

    # Group by severity
    lines.append("### Findings")
    lines.append("")

    current_severity = None
    for finding in analysis.findings:
        if finding.severity != current_severity:
            current_severity = finding.severity
            lines.append(f"#### {current_severity.name}")
            lines.append("")

        cwe_info = f" ({finding.cwe})" if finding.cwe else ""
        lines.append(f"**{finding.rule_id}**{cwe_info} - {finding.tool}")
        lines.append(f"  `{finding.file_path}:{finding.line_start}`")
        lines.append(f"  {finding.message}")
        if finding.fix_suggestion:
            lines.append(f"  *Fix:* {finding.fix_suggestion}")
        lines.append("")

    return "\n".join(lines)


def analyze_security(
    root: Path | str,
    tools: list[str] | None = None,
    min_severity: str = "low",
) -> SecurityAnalysis:
    """Convenience function to run security analysis.

    Args:
        root: Project root directory
        tools: List of tool names (None = all available)
        min_severity: Minimum severity ("low", "medium", "high", "critical")

    Returns:
        SecurityAnalysis with findings
    """
    analyzer = SecurityAnalyzer(
        Path(root),
        tools=tools,
        min_severity=Severity.from_string(min_severity),
    )
    return analyzer.analyze()
