"""Language Server Protocol (LSP) server for Moss.

This module provides an LSP server that integrates Moss's code analysis
capabilities with IDEs and editors via the Language Server Protocol.

Features:
- Diagnostics: Report complexity warnings, code smells
- Hover: Show function metrics (CFG complexity, node count)
- Document Symbols: Show code structure from skeleton view
- Code Actions: Suggest fixes from autofix system
- Go to Definition: Navigate via anchor resolution

Usage:
    # Start server with stdio transport
    moss lsp

    # Or programmatically
    from moss.lsp_server import create_server
    server = create_server()
    server.start_io()
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from typing import TYPE_CHECKING

try:
    from lsprotocol import types as lsp
    from pygls.lsp.server import LanguageServer
    from pygls.workspace import TextDocument

    HAS_LSP = True
except ImportError:
    HAS_LSP = False

if TYPE_CHECKING:
    from lsprotocol import types as lsp
    from pygls.lsp.server import LanguageServer

logger = logging.getLogger(__name__)


# =============================================================================
# Configuration
# =============================================================================


@dataclass
class MossLSPConfig:
    """Configuration for Moss LSP server."""

    # Diagnostics settings
    enable_diagnostics: bool = True
    complexity_warning_threshold: int = 10
    complexity_error_threshold: int = 20

    # Feature toggles
    enable_hover: bool = True
    enable_document_symbols: bool = True
    enable_code_actions: bool = True
    enable_goto_definition: bool = True

    # Analysis settings
    analyze_on_save: bool = True
    analyze_on_change: bool = False  # Can be expensive


# =============================================================================
# Document Analysis Cache
# =============================================================================


@dataclass
class DocumentAnalysis:
    """Cached analysis results for a document."""

    uri: str
    version: int
    cfgs: list[dict] = field(default_factory=list)
    skeleton: dict | None = None
    symbols: list[dict] = field(default_factory=list)
    diagnostics: list = field(default_factory=list)


class AnalysisCache:
    """Cache for document analysis results."""

    def __init__(self) -> None:
        self._cache: dict[str, DocumentAnalysis] = {}

    def get(self, uri: str, version: int) -> DocumentAnalysis | None:
        """Get cached analysis if version matches."""
        cached = self._cache.get(uri)
        if cached and cached.version == version:
            return cached
        return None

    def set(self, analysis: DocumentAnalysis) -> None:
        """Store analysis in cache."""
        self._cache[analysis.uri] = analysis

    def invalidate(self, uri: str) -> None:
        """Remove document from cache."""
        self._cache.pop(uri, None)

    def clear(self) -> None:
        """Clear all cached analysis."""
        self._cache.clear()


# =============================================================================
# Moss LSP Server
# =============================================================================


class MossLanguageServer(LanguageServer if HAS_LSP else object):
    """LSP server providing Moss analysis capabilities."""

    def __init__(self) -> None:
        if not HAS_LSP:
            raise RuntimeError(
                "LSP dependencies not installed. Install with: pip install moss[lsp]"
            )
        super().__init__("moss-lsp", "0.1.0")
        self.config = MossLSPConfig()
        self.cache = AnalysisCache()

    def analyze_document(self, doc: TextDocument) -> DocumentAnalysis:
        """Analyze a document and cache results."""
        # Check cache first
        cached = self.cache.get(doc.uri, doc.version)
        if cached:
            return cached

        analysis = DocumentAnalysis(uri=doc.uri, version=doc.version)

        # Only analyze Python files
        if not doc.uri.endswith(".py"):
            self.cache.set(analysis)
            return analysis

        try:
            source = doc.source
            if source:
                analysis = self._analyze_python(doc.uri, doc.version, source)
        except (OSError, SyntaxError, ValueError) as e:
            logger.warning(f"Analysis failed for {doc.uri}: {e}")

        self.cache.set(analysis)
        return analysis

    def _analyze_python(self, uri: str, version: int, source: str) -> DocumentAnalysis:
        """Perform Python-specific analysis."""
        from moss_intelligence.cfg import CFGBuilder
        from moss_intelligence.skeleton import extract_python_skeleton

        analysis = DocumentAnalysis(uri=uri, version=version)

        # Extract CFGs
        try:
            builder = CFGBuilder()
            cfgs = builder.build_from_source(source)
            analysis.cfgs = []
            for cfg in cfgs:
                # Get line range from entry node
                start_line = 1
                end_line = None
                for node in cfg.nodes.values():
                    if node.line_start is not None:
                        if start_line is None or node.line_start < start_line:
                            start_line = node.line_start
                        if end_line is None or (node.line_end and node.line_end > end_line):
                            end_line = node.line_end or node.line_start

                analysis.cfgs.append(
                    {
                        "name": cfg.name,
                        "node_count": cfg.node_count,
                        "edge_count": cfg.edge_count,
                        "complexity": cfg.cyclomatic_complexity,
                        "start_line": start_line or 1,
                        "end_line": end_line or start_line or 1,
                    }
                )
        except (SyntaxError, ValueError) as e:
            logger.debug(f"CFG extraction failed: {e}")

        # Extract skeleton
        try:
            symbols = extract_python_skeleton(source)
            analysis.symbols = self._extract_symbols(symbols)
        except (SyntaxError, ValueError) as e:
            logger.debug(f"Skeleton extraction failed: {e}")

        # Generate diagnostics
        analysis.diagnostics = self._generate_diagnostics(analysis)

        return analysis

    def _extract_symbols(self, symbols: list) -> list[dict]:
        """Extract document symbols from skeleton symbols."""
        result = []

        for sym in symbols:
            symbol = {
                "name": sym.name,
                "kind": sym.kind,
                "start_line": sym.lineno,
                "end_line": sym.end_lineno or sym.lineno,
                "children": [],
            }

            # Extract nested items (methods, nested classes)
            for child in sym.children:
                symbol["children"].append(
                    {
                        "name": child.name,
                        "kind": child.kind,
                        "start_line": child.lineno,
                        "end_line": child.end_lineno or child.lineno,
                    }
                )

            result.append(symbol)

        return result

    def _generate_diagnostics(self, analysis: DocumentAnalysis) -> list:
        """Generate LSP diagnostics from analysis."""
        if not HAS_LSP:
            return []

        diagnostics = []

        for cfg in analysis.cfgs:
            complexity = cfg.get("complexity", 0)
            start_line = cfg.get("start_line", 1) - 1  # LSP uses 0-based lines
            name = cfg.get("name", "function")

            if complexity >= self.config.complexity_error_threshold:
                diagnostics.append(
                    lsp.Diagnostic(
                        range=lsp.Range(
                            start=lsp.Position(line=start_line, character=0),
                            end=lsp.Position(line=start_line, character=1000),
                        ),
                        message=f"High complexity ({complexity}) in '{name}'. "
                        f"Consider refactoring into smaller functions.",
                        severity=lsp.DiagnosticSeverity.Error,
                        source="moss",
                        code="high-complexity",
                    )
                )
            elif complexity >= self.config.complexity_warning_threshold:
                diagnostics.append(
                    lsp.Diagnostic(
                        range=lsp.Range(
                            start=lsp.Position(line=start_line, character=0),
                            end=lsp.Position(line=start_line, character=1000),
                        ),
                        message=f"Moderate complexity ({complexity}) in '{name}'. "
                        f"Consider simplifying.",
                        severity=lsp.DiagnosticSeverity.Warning,
                        source="moss",
                        code="moderate-complexity",
                    )
                )

        return diagnostics


# =============================================================================
# Server Factory and Setup
# =============================================================================


def create_server() -> MossLanguageServer:
    """Create and configure the Moss LSP server."""
    if not HAS_LSP:
        raise RuntimeError("LSP dependencies not installed. Install with: pip install moss[lsp]")

    server = MossLanguageServer()

    @server.feature(lsp.TEXT_DOCUMENT_DID_OPEN)
    def did_open(params: lsp.DidOpenTextDocumentParams) -> None:
        """Handle document open."""
        doc = server.workspace.get_text_document(params.text_document.uri)
        analysis = server.analyze_document(doc)
        if server.config.enable_diagnostics:
            server.publish_diagnostics(doc.uri, analysis.diagnostics)

    @server.feature(lsp.TEXT_DOCUMENT_DID_SAVE)
    def did_save(params: lsp.DidSaveTextDocumentParams) -> None:
        """Handle document save."""
        if server.config.analyze_on_save:
            doc = server.workspace.get_text_document(params.text_document.uri)
            server.cache.invalidate(doc.uri)
            analysis = server.analyze_document(doc)
            if server.config.enable_diagnostics:
                server.publish_diagnostics(doc.uri, analysis.diagnostics)

    @server.feature(lsp.TEXT_DOCUMENT_DID_CHANGE)
    def did_change(params: lsp.DidChangeTextDocumentParams) -> None:
        """Handle document change."""
        if server.config.analyze_on_change:
            doc = server.workspace.get_text_document(params.text_document.uri)
            server.cache.invalidate(doc.uri)
            analysis = server.analyze_document(doc)
            if server.config.enable_diagnostics:
                server.publish_diagnostics(doc.uri, analysis.diagnostics)

    @server.feature(lsp.TEXT_DOCUMENT_DID_CLOSE)
    def did_close(params: lsp.DidCloseTextDocumentParams) -> None:
        """Handle document close."""
        server.cache.invalidate(params.text_document.uri)
        server.publish_diagnostics(params.text_document.uri, [])

    @server.feature(lsp.TEXT_DOCUMENT_HOVER)
    def hover(params: lsp.HoverParams) -> lsp.Hover | None:
        """Provide hover information."""
        if not server.config.enable_hover:
            return None

        doc = server.workspace.get_text_document(params.text_document.uri)
        analysis = server.analyze_document(doc)

        line = params.position.line + 1  # Convert to 1-based

        # Find CFG containing this line
        for cfg in analysis.cfgs:
            start = cfg.get("start_line", 0)
            end = cfg.get("end_line", 0)
            if start <= line <= end:
                return lsp.Hover(
                    contents=lsp.MarkupContent(
                        kind=lsp.MarkupKind.Markdown,
                        value=f"**{cfg['name']}**\n\n"
                        f"- Nodes: {cfg['node_count']}\n"
                        f"- Edges: {cfg['edge_count']}\n"
                        f"- Cyclomatic Complexity: {cfg['complexity']}\n",
                    )
                )

        return None

    @server.feature(lsp.TEXT_DOCUMENT_DOCUMENT_SYMBOL)
    def document_symbols(
        params: lsp.DocumentSymbolParams,
    ) -> list[lsp.DocumentSymbol] | None:
        """Provide document symbols."""
        if not server.config.enable_document_symbols:
            return None

        doc = server.workspace.get_text_document(params.text_document.uri)
        analysis = server.analyze_document(doc)

        symbols = []
        for sym in analysis.symbols:
            kind = _symbol_kind(sym.get("kind", ""))
            children = []

            for child in sym.get("children", []):
                child_kind = _symbol_kind(child.get("kind", ""))
                children.append(
                    lsp.DocumentSymbol(
                        name=child["name"],
                        kind=child_kind,
                        range=lsp.Range(
                            start=lsp.Position(line=child.get("start_line", 1) - 1, character=0),
                            end=lsp.Position(line=child.get("end_line", 1) - 1, character=1000),
                        ),
                        selection_range=lsp.Range(
                            start=lsp.Position(line=child.get("start_line", 1) - 1, character=0),
                            end=lsp.Position(line=child.get("start_line", 1) - 1, character=1000),
                        ),
                    )
                )

            symbols.append(
                lsp.DocumentSymbol(
                    name=sym["name"],
                    kind=kind,
                    range=lsp.Range(
                        start=lsp.Position(line=sym.get("start_line", 1) - 1, character=0),
                        end=lsp.Position(line=sym.get("end_line", 1) - 1, character=1000),
                    ),
                    selection_range=lsp.Range(
                        start=lsp.Position(line=sym.get("start_line", 1) - 1, character=0),
                        end=lsp.Position(line=sym.get("start_line", 1) - 1, character=1000),
                    ),
                    children=children if children else None,
                )
            )

        return symbols if symbols else None

    @server.feature(lsp.TEXT_DOCUMENT_DEFINITION)
    def goto_definition(
        params: lsp.DefinitionParams,
    ) -> lsp.Location | list[lsp.Location] | None:
        """Go to definition via anchor resolution."""
        if not server.config.enable_goto_definition:
            return None

        doc = server.workspace.get_text_document(params.text_document.uri)
        if not doc.source:
            return None

        # Get word at position
        word = _get_word_at_position(doc.source, params.position)
        if not word:
            return None

        # Try to resolve as anchor
        try:
            from moss_intelligence.anchors import AnchorResolver

            resolver = AnchorResolver(doc.source)
            match = resolver.resolve(word)

            if match and match.span:
                return lsp.Location(
                    uri=doc.uri,
                    range=lsp.Range(
                        start=lsp.Position(line=match.span.start_line - 1, character=0),
                        end=lsp.Position(line=match.span.end_line - 1, character=1000),
                    ),
                )
        except (ValueError, KeyError) as e:
            logger.debug(f"Anchor resolution failed: {e}")

        return None

    @server.feature(lsp.TEXT_DOCUMENT_CODE_ACTION)
    def code_actions(
        params: lsp.CodeActionParams,
    ) -> list[lsp.CodeAction] | None:
        """Provide code actions from autofix system."""
        if not server.config.enable_code_actions:
            return None

        actions = []

        # Check for complexity-related diagnostics
        for diagnostic in params.context.diagnostics:
            if diagnostic.source == "moss" and diagnostic.code == "high-complexity":
                actions.append(
                    lsp.CodeAction(
                        title="Extract complex logic into helper functions",
                        kind=lsp.CodeActionKind.RefactorExtract,
                        diagnostics=[diagnostic],
                        disabled=lsp.CodeActionDisabledType(
                            reason="Automatic extraction not yet implemented"
                        ),
                    )
                )

        return actions if actions else None

    return server


def _symbol_kind(kind: str) -> lsp.SymbolKind:
    """Convert skeleton kind to LSP SymbolKind."""
    if not HAS_LSP:
        return 0  # type: ignore

    kind_map = {
        "class": lsp.SymbolKind.Class,
        "function": lsp.SymbolKind.Function,
        "method": lsp.SymbolKind.Method,
        "variable": lsp.SymbolKind.Variable,
        "constant": lsp.SymbolKind.Constant,
        "import": lsp.SymbolKind.Module,
    }
    return kind_map.get(kind.lower(), lsp.SymbolKind.Variable)


def _get_word_at_position(source: str, position: lsp.Position) -> str | None:
    """Extract the word at the given position."""
    if not HAS_LSP:
        return None

    lines = source.splitlines()
    if position.line >= len(lines):
        return None

    line = lines[position.line]
    if position.character >= len(line):
        return None

    # Find word boundaries
    start = position.character
    end = position.character

    # Move start backward to find word start
    while start > 0 and (line[start - 1].isalnum() or line[start - 1] == "_"):
        start -= 1

    # Move end forward to find word end
    while end < len(line) and (line[end].isalnum() or line[end] == "_"):
        end += 1

    word = line[start:end]
    return word if word else None


# =============================================================================
# CLI Integration
# =============================================================================


def start_server(transport: str = "stdio") -> None:
    """Start the Moss LSP server.

    Args:
        transport: "stdio" or "tcp:host:port"
    """
    if not HAS_LSP:
        raise RuntimeError("LSP dependencies not installed. Install with: pip install moss[lsp]")

    server = create_server()

    if transport == "stdio":
        server.start_io()
    elif transport.startswith("tcp:"):
        parts = transport.split(":")
        host = parts[1] if len(parts) > 1 else "127.0.0.1"
        port = int(parts[2]) if len(parts) > 2 else 2087
        server.start_tcp(host, port)
    else:
        raise ValueError(f"Unknown transport: {transport}")
