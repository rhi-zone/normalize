from pathlib import Path

from moss.check_refs import RefChecker


def test_rust_refs(tmp_path):
    """Test that RefChecker finds Rust references."""
    root = tmp_path
    (root / "src").mkdir()
    (root / "docs").mkdir()

    # Rust file referencing doc
    rust_file = root / "src" / "main.rs"
    rust_file.write_text("// See: docs/architecture.md")

    # Doc referencing Rust file and Cargo.toml
    doc_file = root / "docs" / "architecture.md"
    doc_file.write_text("""
Detailed design in `src/main.rs`.
Project config in `Cargo.toml`.
""")

    (root / "Cargo.toml").touch()

    checker = RefChecker(root)
    result = checker.check()

    # Verify code -> doc
    assert len(result.code_to_docs) == 1
    assert result.code_to_docs[0].target_doc == Path("docs/architecture.md")

    # Verify doc -> code
    assert len(result.docs_to_code) == 2
    paths = [str(r.target_file) for r in result.docs_to_code]
    assert "src/main.rs" in paths
    assert "Cargo.toml" in paths


def test_crates_dir_refs(tmp_path):
    """Test references in crates/ directory."""
    root = tmp_path
    (root / "crates" / "moss-cli" / "src").mkdir(parents=True)
    (root / "docs").mkdir()

    rust_file = root / "crates" / "moss-cli" / "src" / "lib.rs"
    rust_file.write_text("// See: docs/cli.md")

    doc_file = root / "docs" / "cli.md"
    doc_file.write_text("Implemented in `crates/moss-cli/src/lib.rs`.")

    checker = RefChecker(root)
    result = checker.check()

    assert len(result.code_to_docs) == 1
    assert len(result.docs_to_code) == 1
    assert str(result.docs_to_code[0].target_file) == "crates/moss-cli/src/lib.rs"
