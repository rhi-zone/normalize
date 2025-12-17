"""Codebase metrics and health dashboard.

This module provides tools to analyze codebase health and generate
HTML reports with visualizations of:
- Code complexity metrics
- File and module statistics
- Symbol counts and distribution
- Dependency relationships

Usage:
    from moss.metrics import collect_metrics, generate_dashboard

    # Collect metrics for a directory
    metrics = collect_metrics(Path("."), pattern="**/*.py")

    # Generate HTML dashboard
    html = generate_dashboard(metrics, title="My Project")
    Path("dashboard.html").write_text(html)
"""

from __future__ import annotations

import html
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any

from moss.skeleton import extract_python_skeleton


@dataclass
class FileMetrics:
    """Metrics for a single file."""

    path: Path
    lines: int = 0
    code_lines: int = 0
    comment_lines: int = 0
    blank_lines: int = 0
    classes: int = 0
    functions: int = 0
    methods: int = 0
    imports: int = 0
    complexity: int = 0  # Simplified cyclomatic complexity estimate


@dataclass
class ModuleMetrics:
    """Metrics aggregated by module/directory."""

    name: str
    file_count: int = 0
    total_lines: int = 0
    total_code_lines: int = 0
    total_classes: int = 0
    total_functions: int = 0
    total_methods: int = 0
    avg_complexity: float = 0.0


@dataclass
class CodebaseMetrics:
    """Complete codebase metrics."""

    # Basic counts
    total_files: int = 0
    total_lines: int = 0
    total_code_lines: int = 0
    total_comment_lines: int = 0
    total_blank_lines: int = 0

    # Symbol counts
    total_classes: int = 0
    total_functions: int = 0
    total_methods: int = 0
    total_imports: int = 0

    # Averages
    avg_file_lines: float = 0.0
    avg_complexity: float = 0.0

    # Per-file data
    files: list[FileMetrics] = field(default_factory=list)

    # Per-module data
    modules: list[ModuleMetrics] = field(default_factory=list)

    # Metadata
    timestamp: str = ""
    root_path: str = ""

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "total_files": self.total_files,
            "total_lines": self.total_lines,
            "total_code_lines": self.total_code_lines,
            "total_comment_lines": self.total_comment_lines,
            "total_blank_lines": self.total_blank_lines,
            "total_classes": self.total_classes,
            "total_functions": self.total_functions,
            "total_methods": self.total_methods,
            "total_imports": self.total_imports,
            "avg_file_lines": round(self.avg_file_lines, 1),
            "avg_complexity": round(self.avg_complexity, 2),
            "files": [
                {
                    "path": str(f.path),
                    "lines": f.lines,
                    "code_lines": f.code_lines,
                    "classes": f.classes,
                    "functions": f.functions,
                    "methods": f.methods,
                    "complexity": f.complexity,
                }
                for f in self.files
            ],
            "modules": [
                {
                    "name": m.name,
                    "file_count": m.file_count,
                    "total_lines": m.total_lines,
                    "total_classes": m.total_classes,
                    "total_functions": m.total_functions,
                }
                for m in self.modules
            ],
            "timestamp": self.timestamp,
            "root_path": self.root_path,
        }


def analyze_file(path: Path) -> FileMetrics:
    """Analyze a single Python file.

    Args:
        path: Path to Python file

    Returns:
        FileMetrics for the file
    """
    try:
        content = path.read_text()
    except Exception:
        return FileMetrics(path=path)

    lines = content.splitlines()
    metrics = FileMetrics(path=path, lines=len(lines))

    # Count line types
    in_docstring = False
    docstring_char = None

    for line in lines:
        stripped = line.strip()

        if not stripped:
            metrics.blank_lines += 1
            continue

        # Handle docstrings
        if not in_docstring:
            if stripped.startswith('"""') or stripped.startswith("'''"):
                docstring_char = stripped[:3]
                if stripped.count(docstring_char) >= 2:
                    # Single line docstring
                    metrics.comment_lines += 1
                else:
                    in_docstring = True
                    metrics.comment_lines += 1
                continue
        else:
            metrics.comment_lines += 1
            if docstring_char and docstring_char in stripped:
                in_docstring = False
                docstring_char = None
            continue

        # Check for comments
        if stripped.startswith("#"):
            metrics.comment_lines += 1
        else:
            metrics.code_lines += 1

        # Count imports
        if stripped.startswith("import ") or stripped.startswith("from "):
            metrics.imports += 1

    # Extract symbols using skeleton
    try:
        symbols = extract_python_skeleton(content)
        for sym in symbols:
            if sym.kind == "class":
                metrics.classes += 1
            elif sym.kind == "function":
                metrics.functions += 1
            # Count children
            for child in sym.children:
                if child.kind == "method":
                    metrics.methods += 1
                elif child.kind == "function":
                    metrics.functions += 1
    except SyntaxError:
        pass

    # Simple complexity estimate based on control flow keywords
    complexity_keywords = ["if", "elif", "for", "while", "except", "with", "and", "or"]
    for line in lines:
        for kw in complexity_keywords:
            if f" {kw} " in f" {line} " or line.strip().startswith(f"{kw} "):
                metrics.complexity += 1

    return metrics


def collect_metrics(
    root: Path,
    pattern: str = "**/*.py",
    exclude_patterns: list[str] | None = None,
) -> CodebaseMetrics:
    """Collect metrics for a directory.

    Args:
        root: Root directory to analyze
        pattern: Glob pattern for files
        exclude_patterns: Patterns to exclude (e.g., ["**/test_*", "**/__pycache__/*"])

    Returns:
        CodebaseMetrics for the codebase
    """
    root = Path(root).resolve()
    exclude_patterns = exclude_patterns or ["**/__pycache__/*", "**/.venv/*", "**/venv/*"]

    metrics = CodebaseMetrics(
        timestamp=datetime.now().isoformat(),
        root_path=str(root),
    )

    # Collect all matching files
    files = list(root.glob(pattern))

    # Filter excluded files
    for exclude in exclude_patterns:
        excluded = set(root.glob(exclude))
        files = [f for f in files if f not in excluded]

    # Analyze each file
    module_data: dict[str, list[FileMetrics]] = {}

    for file_path in sorted(files):
        file_metrics = analyze_file(file_path)
        metrics.files.append(file_metrics)

        # Aggregate totals
        metrics.total_lines += file_metrics.lines
        metrics.total_code_lines += file_metrics.code_lines
        metrics.total_comment_lines += file_metrics.comment_lines
        metrics.total_blank_lines += file_metrics.blank_lines
        metrics.total_classes += file_metrics.classes
        metrics.total_functions += file_metrics.functions
        metrics.total_methods += file_metrics.methods
        metrics.total_imports += file_metrics.imports

        # Group by module (first directory under root or src)
        try:
            rel_path = file_path.relative_to(root)
            parts = rel_path.parts
            if parts[0] == "src" and len(parts) > 1:
                module_name = parts[1]
            else:
                module_name = parts[0] if parts else "(root)"
        except ValueError:
            module_name = "(external)"

        if module_name not in module_data:
            module_data[module_name] = []
        module_data[module_name].append(file_metrics)

    # Calculate averages
    metrics.total_files = len(metrics.files)
    if metrics.total_files > 0:
        metrics.avg_file_lines = metrics.total_lines / metrics.total_files
        total_complexity = sum(f.complexity for f in metrics.files)
        metrics.avg_complexity = total_complexity / metrics.total_files

    # Build module metrics
    for name, module_files in sorted(module_data.items()):
        mod_metrics = ModuleMetrics(
            name=name,
            file_count=len(module_files),
            total_lines=sum(f.lines for f in module_files),
            total_code_lines=sum(f.code_lines for f in module_files),
            total_classes=sum(f.classes for f in module_files),
            total_functions=sum(f.functions for f in module_files),
            total_methods=sum(f.methods for f in module_files),
        )
        if module_files:
            mod_metrics.avg_complexity = sum(f.complexity for f in module_files) / len(module_files)
        metrics.modules.append(mod_metrics)

    return metrics


def generate_dashboard(
    metrics: CodebaseMetrics,
    title: str = "Codebase Metrics Dashboard",
) -> str:
    """Generate an HTML dashboard from metrics.

    Args:
        metrics: Collected codebase metrics
        title: Dashboard title

    Returns:
        HTML string
    """
    # Escape title for HTML
    title_escaped = html.escape(title)

    # Generate file size distribution for chart
    file_sizes = [f.lines for f in metrics.files]
    size_ranges = [
        ("0-50", len([s for s in file_sizes if s <= 50])),
        ("51-100", len([s for s in file_sizes if 50 < s <= 100])),
        ("101-200", len([s for s in file_sizes if 100 < s <= 200])),
        ("201-500", len([s for s in file_sizes if 200 < s <= 500])),
        ("500+", len([s for s in file_sizes if s > 500])),
    ]

    # Top files by size
    top_files = sorted(metrics.files, key=lambda f: f.lines, reverse=True)[:10]

    # Generate HTML
    dashboard_html = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title_escaped}</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, sans-serif;
            line-height: 1.6;
            color: #333;
            background: #f5f5f5;
            padding: 20px;
        }}
        .dashboard {{
            max-width: 1400px;
            margin: 0 auto;
        }}
        h1 {{
            color: #2c3e50;
            margin-bottom: 10px;
        }}
        .timestamp {{
            color: #666;
            font-size: 0.9em;
            margin-bottom: 20px;
        }}
        .grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }}
        .card {{
            background: white;
            border-radius: 8px;
            padding: 20px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }}
        .card h2 {{
            font-size: 1.1em;
            color: #666;
            margin-bottom: 10px;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }}
        .metric-value {{
            font-size: 2.5em;
            font-weight: bold;
            color: #2c3e50;
        }}
        .metric-unit {{
            font-size: 0.9em;
            color: #888;
        }}
        .wide-card {{
            grid-column: span 2;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
            margin-top: 10px;
        }}
        th, td {{
            padding: 10px;
            text-align: left;
            border-bottom: 1px solid #eee;
        }}
        th {{
            color: #666;
            font-weight: 600;
            font-size: 0.9em;
        }}
        .bar {{
            height: 20px;
            background: #3498db;
            border-radius: 3px;
            min-width: 2px;
        }}
        .bar-container {{
            background: #eee;
            border-radius: 3px;
            overflow: hidden;
        }}
        .chart {{
            display: flex;
            align-items: flex-end;
            gap: 10px;
            height: 150px;
            padding: 10px 0;
        }}
        .chart-bar {{
            flex: 1;
            background: #3498db;
            border-radius: 3px 3px 0 0;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: flex-end;
        }}
        .chart-label {{
            font-size: 0.8em;
            color: #666;
            margin-top: 5px;
            text-align: center;
        }}
        .chart-value {{
            font-size: 0.9em;
            font-weight: bold;
            color: white;
            padding: 2px 4px;
        }}
        @media (max-width: 768px) {{
            .wide-card {{
                grid-column: span 1;
            }}
        }}
    </style>
</head>
<body>
    <div class="dashboard">
        <h1>{title_escaped}</h1>
        <p class="timestamp">Generated: {metrics.timestamp} | Path: {
        html.escape(metrics.root_path)
    }</p>

        <div class="grid">
            <div class="card">
                <h2>Total Files</h2>
                <div class="metric-value">{metrics.total_files}</div>
                <div class="metric-unit">Python files analyzed</div>
            </div>

            <div class="card">
                <h2>Lines of Code</h2>
                <div class="metric-value">{metrics.total_code_lines:,}</div>
                <div class="metric-unit">excluding comments/blanks</div>
            </div>

            <div class="card">
                <h2>Total Lines</h2>
                <div class="metric-value">{metrics.total_lines:,}</div>
                <div class="metric-unit">all lines</div>
            </div>

            <div class="card">
                <h2>Avg File Size</h2>
                <div class="metric-value">{metrics.avg_file_lines:.0f}</div>
                <div class="metric-unit">lines per file</div>
            </div>
        </div>

        <div class="grid">
            <div class="card">
                <h2>Classes</h2>
                <div class="metric-value">{metrics.total_classes}</div>
            </div>

            <div class="card">
                <h2>Functions</h2>
                <div class="metric-value">{metrics.total_functions}</div>
            </div>

            <div class="card">
                <h2>Methods</h2>
                <div class="metric-value">{metrics.total_methods}</div>
            </div>

            <div class="card">
                <h2>Imports</h2>
                <div class="metric-value">{metrics.total_imports}</div>
            </div>
        </div>

        <div class="grid">
            <div class="card wide-card">
                <h2>File Size Distribution</h2>
                <div class="chart">
"""

    # Add chart bars
    max_count = max(count for _, count in size_ranges) if size_ranges else 1
    for _label, count in size_ranges:
        height_pct = (count / max_count * 100) if max_count > 0 else 0
        bar_height = f"{max(height_pct, 5):.0f}%"
        bar_style = f"height: {bar_height}"
        dashboard_html += f"""                    <div class="chart-bar" style="{bar_style}">
                        <span class="chart-value">{count}</span>
                    </div>
"""

    dashboard_html += """                </div>
                <div style="display: flex; gap: 10px; justify-content: space-around;">
"""
    for label, _ in size_ranges:
        dashboard_html += f'                    <span class="chart-label">{label}</span>\n'

    dashboard_html += """                </div>
            </div>

            <div class="card wide-card">
                <h2>Line Composition</h2>
                <table>
                    <tr>
                        <td>Code Lines</td>
                        <td style="width: 60%;">
                            <div class="bar-container">
"""

    total = metrics.total_lines or 1
    code_pct = metrics.total_code_lines / total * 100
    comment_pct = metrics.total_comment_lines / total * 100
    blank_pct = metrics.total_blank_lines / total * 100

    code_bar = f'<div class="bar" style="width: {code_pct:.1f}%;"></div>'
    comment_style = f"width: {comment_pct:.1f}%; background: #27ae60;"
    comment_bar = f'<div class="bar" style="{comment_style}"></div>'
    blank_style = f"width: {blank_pct:.1f}%; background: #95a5a6;"
    blank_bar = f'<div class="bar" style="{blank_style}"></div>'

    dashboard_html += f"""                                {code_bar}
                            </div>
                        </td>
                        <td>{metrics.total_code_lines:,} ({code_pct:.1f}%)</td>
                    </tr>
                    <tr>
                        <td>Comments</td>
                        <td>
                            <div class="bar-container">
                                {comment_bar}
                            </div>
                        </td>
                        <td>{metrics.total_comment_lines:,} ({comment_pct:.1f}%)</td>
                    </tr>
                    <tr>
                        <td>Blank Lines</td>
                        <td>
                            <div class="bar-container">
                                {blank_bar}
                            </div>
                        </td>
                        <td>{metrics.total_blank_lines:,} ({blank_pct:.1f}%)</td>
                    </tr>
                </table>
            </div>
        </div>

        <div class="grid">
            <div class="card wide-card">
                <h2>Modules</h2>
                <table>
                    <tr>
                        <th>Module</th>
                        <th>Files</th>
                        <th>Lines</th>
                        <th>Classes</th>
                        <th>Functions</th>
                    </tr>
"""

    for mod in metrics.modules:
        dashboard_html += f"""                    <tr>
                        <td>{html.escape(mod.name)}</td>
                        <td>{mod.file_count}</td>
                        <td>{mod.total_lines:,}</td>
                        <td>{mod.total_classes}</td>
                        <td>{mod.total_functions}</td>
                    </tr>
"""

    dashboard_html += """                </table>
            </div>

            <div class="card wide-card">
                <h2>Largest Files</h2>
                <table>
                    <tr>
                        <th>File</th>
                        <th>Lines</th>
                        <th>Classes</th>
                        <th>Functions</th>
                    </tr>
"""

    for f in top_files:
        try:
            rel_path = f.path.relative_to(Path(metrics.root_path))
        except ValueError:
            rel_path = f.path
        dashboard_html += f"""                    <tr>
                        <td>{html.escape(str(rel_path))}</td>
                        <td>{f.lines:,}</td>
                        <td>{f.classes}</td>
                        <td>{f.functions}</td>
                    </tr>
"""

    dashboard_html += """                </table>
            </div>
        </div>
    </div>
</body>
</html>"""

    return dashboard_html
