"""Workflow templates for 'moss workflow new'."""

MINIMAL_WORKFLOW = """# Minimal workflow
[workflow]
name = "{name}"
description = "A minimal workflow example"
version = "0.1.0"

[workflow.limits]
max_steps = 5
timeout_seconds = 60

[[workflow.steps]]
name = "check"
tool = "health.check"
"""

STANDARD_WORKFLOW = """# Standard analysis workflow
[workflow]
name = "{name}"
description = "Analyze and validate code"
version = "0.1.0"

[workflow.limits]
max_steps = 10
timeout_seconds = 300

[workflow.llm]
temperature = 0.0

[[workflow.steps]]
name = "analyze"
tool = "skeleton.extract"
# parameters = {{ file_path = "src/main.py" }}

[[workflow.steps]]
name = "validate"
tool = "validation.validate"
input_from = "analyze"
on_error = "continue"
"""

TEMPLATES = {
    "minimal": MINIMAL_WORKFLOW,
    "standard": STANDARD_WORKFLOW,
}
