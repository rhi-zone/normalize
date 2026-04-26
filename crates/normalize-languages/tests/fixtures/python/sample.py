import os
import sys
from collections import defaultdict
from typing import Optional, List

class DataProcessor:
    """Process data items."""

    def __init__(self, name: str):
        self.name = name
        self.items: List[str] = []

    def add(self, item: str) -> None:
        self.items.append(item)

    # Process all items
    @property
    def process(self) -> List[str]:
        result = []
        for item in self.items:
            if item.startswith("_"):
                continue
            result.append(item.upper())
        return result


def load_file(path: str) -> Optional[str]:
    if not os.path.exists(path):
        return None
    with open(path) as f:
        return f.read()


def count_words(text: str) -> dict:
    counts = defaultdict(int)
    for word in text.split():
        counts[word] += 1
    return dict(counts)
