"""Smart TOML navigation with jq-like filtering.

Provides exploration of TOML config files (pyproject.toml, Cargo.toml, etc.)
using jq-like path expressions.

Usage:
    from moss.toml_nav import parse_toml, query, to_json

    # Parse TOML file
    data = parse_toml(Path("pyproject.toml"))

    # Query with jq-like path
    result = query(data, ".project.name")
    result = query(data, ".dependencies.*")
    result = query(data, ".tool.ruff.lint.select")

CLI:
    moss toml pyproject.toml .project.name
    moss toml Cargo.toml ".dependencies | keys"
"""

from __future__ import annotations

import json
import re
import tomllib
from pathlib import Path
from typing import Any


def parse_toml(path: Path) -> dict[str, Any]:
    """Parse a TOML file to a Python dict.

    Args:
        path: Path to TOML file

    Returns:
        Parsed TOML data as dict

    Raises:
        FileNotFoundError: If file doesn't exist
        tomllib.TOMLDecodeError: If TOML is invalid
    """
    if not path.exists():
        raise FileNotFoundError(f"File not found: {path}")

    with open(path, "rb") as f:
        return tomllib.load(f)


def to_json(data: Any, indent: int = 2) -> str:
    """Convert data to JSON string.

    Args:
        data: Data to convert
        indent: Indentation level (default 2)

    Returns:
        JSON string
    """
    return json.dumps(data, indent=indent, default=str)


def query(data: Any, path: str) -> Any:
    """Query data with a jq-like path expression.

    Supported syntax:
        .key          - Access object key
        .key.subkey   - Nested access
        .[0]          - Array index
        .[-1]         - Negative index (last element)
        .*            - All values
        .key[]        - Iterate array
        .key | keys   - Get keys of object
        .key | length - Get length
        .key | type   - Get type name
        .key?         - Optional access (returns None if missing)

    Examples:
        query(data, ".project.name")
        query(data, ".dependencies.*")
        query(data, ".tool.ruff.lint.select")
        query(data, ".packages[0].name")
        query(data, ".dependencies | keys")

    Args:
        data: Data to query
        path: jq-like path expression

    Returns:
        Query result
    """
    if not path or path == ".":
        return data

    # Parse path into segments
    segments = _parse_path(path)
    return _execute_query(data, segments)


def _parse_path(path: str) -> list[dict[str, Any]]:
    """Parse a path expression into segments.

    Returns list of dicts with type and value:
        {"type": "key", "value": "name", "optional": False}
        {"type": "index", "value": 0}
        {"type": "wildcard"}
        {"type": "iterate"}
        {"type": "pipe", "value": "keys"}
    """
    segments: list[dict[str, Any]] = []

    # Handle leading dot
    if path.startswith("."):
        path = path[1:]

    # Tokenize
    i = 0
    while i < len(path):
        char = path[i]

        if char == ".":
            # Skip dots between segments
            i += 1
            continue

        if char == "[":
            # Array index or slice
            end = path.find("]", i)
            if end == -1:
                raise ValueError(f"Unclosed bracket at position {i}")
            content = path[i + 1 : end]
            if content == "":
                # Iterate: []
                segments.append({"type": "iterate"})
            else:
                # Index: [0], [-1]
                segments.append({"type": "index", "value": int(content)})
            i = end + 1
            continue

        if char == "*":
            # Wildcard
            segments.append({"type": "wildcard"})
            i += 1
            continue

        if char == "|":
            # Pipe operator
            i += 1
            # Skip whitespace
            while i < len(path) and path[i] == " ":
                i += 1
            # Read function name
            match = re.match(r"\w+", path[i:])
            if match:
                func = match.group(0)
                segments.append({"type": "pipe", "value": func})
                i += len(func)
            continue

        # Key access (supports hyphens in key names like "requires-python")
        match = re.match(r"([\w-]+)(\?)?", path[i:])
        if match:
            key = match.group(1)
            optional = match.group(2) == "?"
            segments.append({"type": "key", "value": key, "optional": optional})
            i += len(match.group(0))
            continue

        raise ValueError(f"Unexpected character '{char}' at position {i}")

    return segments


def _execute_query(data: Any, segments: list[dict[str, Any]]) -> Any:
    """Execute a parsed query on data."""
    result = data

    for seg in segments:
        seg_type = seg["type"]

        if seg_type == "key":
            key = seg["value"]
            optional = seg.get("optional", False)
            if isinstance(result, dict):
                if key in result:
                    result = result[key]
                elif optional:
                    result = None
                else:
                    raise KeyError(f"Key not found: {key}")
            else:
                if optional:
                    result = None
                else:
                    raise TypeError(f"Cannot access key '{key}' on {type(result).__name__}")

        elif seg_type == "index":
            idx = seg["value"]
            if isinstance(result, (list, tuple)):
                try:
                    result = result[idx]
                except IndexError as e:
                    raise IndexError(f"Index {idx} out of range") from e
            else:
                raise TypeError(f"Cannot index {type(result).__name__}")

        elif seg_type == "wildcard":
            if isinstance(result, dict):
                result = list(result.values())
            elif isinstance(result, (list, tuple)):
                result = list(result)
            else:
                result = [result]

        elif seg_type == "iterate":
            if isinstance(result, (list, tuple)):
                # Keep as list for further processing
                result = list(result)
            elif isinstance(result, dict):
                result = list(result.values())
            else:
                result = [result]

        elif seg_type == "pipe":
            func = seg["value"]
            if func == "keys":
                if isinstance(result, dict):
                    result = list(result.keys())
                else:
                    raise TypeError(f"Cannot get keys of {type(result).__name__}")
            elif func == "values":
                if isinstance(result, dict):
                    result = list(result.values())
                else:
                    raise TypeError(f"Cannot get values of {type(result).__name__}")
            elif func == "length":
                if isinstance(result, (list, tuple, dict, str)):
                    result = len(result)
                else:
                    raise TypeError(f"Cannot get length of {type(result).__name__}")
            elif func == "type":
                result = type(result).__name__
            elif func == "first":
                if isinstance(result, (list, tuple)) and len(result) > 0:
                    result = result[0]
                elif isinstance(result, dict):
                    result = next(iter(result.values()), None)
                else:
                    result = None
            elif func == "last":
                if isinstance(result, (list, tuple)) and len(result) > 0:
                    result = result[-1]
                else:
                    result = None
            elif func == "sort":
                if isinstance(result, list):
                    result = sorted(result, key=lambda x: str(x))
                else:
                    raise TypeError(f"Cannot sort {type(result).__name__}")
            elif func == "reverse":
                if isinstance(result, list):
                    result = list(reversed(result))
                else:
                    raise TypeError(f"Cannot reverse {type(result).__name__}")
            elif func == "flatten":
                if isinstance(result, list):
                    flat = []
                    for item in result:
                        if isinstance(item, list):
                            flat.extend(item)
                        else:
                            flat.append(item)
                    result = flat
                else:
                    raise TypeError(f"Cannot flatten {type(result).__name__}")
            else:
                raise ValueError(f"Unknown function: {func}")

    return result


def list_keys(data: dict[str, Any], prefix: str = "") -> list[str]:
    """List all keys in a nested dict structure.

    Returns flattened key paths like:
        project.name
        project.version
        dependencies.requests
        tool.ruff.lint.select

    Args:
        data: Dict to list keys from
        prefix: Prefix for nested keys

    Returns:
        List of key paths
    """
    keys: list[str] = []

    for key, value in data.items():
        full_key = f"{prefix}.{key}" if prefix else key
        keys.append(full_key)

        if isinstance(value, dict):
            keys.extend(list_keys(value, full_key))

    return keys


def summarize_toml(data: dict[str, Any]) -> dict[str, Any]:
    """Generate a summary of a TOML file's structure.

    Returns a dict with:
        - sections: list of top-level sections
        - key_count: total number of keys
        - nested_depth: maximum nesting depth
        - types: count of value types

    Args:
        data: Parsed TOML data

    Returns:
        Summary dict
    """
    sections = list(data.keys())
    all_keys = list_keys(data)

    # Calculate max depth
    max_depth = 0
    for key in all_keys:
        depth = key.count(".") + 1
        if depth > max_depth:
            max_depth = depth

    # Count value types
    types: dict[str, int] = {}

    def count_types(d: Any) -> None:
        if isinstance(d, dict):
            types["object"] = types.get("object", 0) + 1
            for v in d.values():
                count_types(v)
        elif isinstance(d, list):
            types["array"] = types.get("array", 0) + 1
            for v in d:
                count_types(v)
        elif isinstance(d, str):
            types["string"] = types.get("string", 0) + 1
        elif isinstance(d, bool):
            types["boolean"] = types.get("boolean", 0) + 1
        elif isinstance(d, int):
            types["integer"] = types.get("integer", 0) + 1
        elif isinstance(d, float):
            types["float"] = types.get("float", 0) + 1
        else:
            types["other"] = types.get("other", 0) + 1

    count_types(data)

    return {
        "sections": sections,
        "key_count": len(all_keys),
        "nested_depth": max_depth,
        "types": types,
    }


def format_result(result: Any, output_format: str = "auto") -> str:
    """Format query result for display.

    Args:
        result: Query result
        output_format: "json", "text", or "auto"

    Returns:
        Formatted string
    """
    if output_format == "json" or (output_format == "auto" and isinstance(result, (dict, list))):
        return to_json(result)

    if isinstance(result, str):
        return result
    if isinstance(result, bool):
        return str(result).lower()
    if result is None:
        return "null"
    return str(result)


__all__ = [
    "format_result",
    "list_keys",
    "parse_toml",
    "query",
    "summarize_toml",
    "to_json",
]
