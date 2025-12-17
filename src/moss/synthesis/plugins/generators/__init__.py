"""Built-in code generator plugins.

Generators:
- PlaceholderGenerator: Returns TODO placeholders (current behavior)
- TemplateGenerator: User-configurable code templates
"""

from .placeholder import PlaceholderGenerator
from .template import TemplateGenerator

__all__ = [
    "PlaceholderGenerator",
    "TemplateGenerator",
]
