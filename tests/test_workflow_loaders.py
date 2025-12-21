from pathlib import Path

import pytest

from moss.workflows import Workflow, load_workflow


class MockLoader:
    def can_load(self, path: Path) -> bool:
        return path.suffix == ".mock"

    def load(self, name, project_root, loading_stack) -> Workflow:
        return Workflow(name="mock-workflow", description="Mocked")


def test_toml_loader_basic(tmp_path):
    """Test that default TOML loader works."""
    workflow_dir = tmp_path / ".moss" / "workflows"
    workflow_dir.mkdir(parents=True)

    toml_content = """
[workflow]
name = "test-wf"
description = "Test Description"

[[workflow.steps]]
name = "step1"
tool = "test.tool"
"""
    (workflow_dir / "test-wf.toml").write_text(toml_content)

    wf = load_workflow("test-wf", project_root=tmp_path)
    assert wf.name == "test-wf"
    assert wf.description == "Test Description"
    assert len(wf.steps) == 1
    assert wf.steps[0].name == "step1"


def test_custom_loader(tmp_path):
    """Test that custom loader can be registered and used."""
    from moss.workflows import _LOADERS

    mock_loader = MockLoader()
    _LOADERS.append(mock_loader)

    try:
        # Should be able to load .mock files now
        mock_path = tmp_path / "test.mock"
        mock_path.touch()

        wf = load_workflow(mock_path, project_root=tmp_path)
        assert wf.name == "mock-workflow"
    finally:
        _LOADERS.remove(mock_loader)


def test_loader_not_found():
    """Test error when no loader handles the file."""
    with pytest.raises(FileNotFoundError):
        load_workflow("nonexistent-workflow-at-all")
