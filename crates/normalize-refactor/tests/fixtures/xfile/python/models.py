from .utils import format_name

class Person:
    def __init__(self, name: str) -> None:
        self.name = format_name(name)
