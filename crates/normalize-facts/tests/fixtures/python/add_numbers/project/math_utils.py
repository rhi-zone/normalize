"""Simple math utilities."""


def add(a: int, b: int) -> int:
    """Return the sum of a and b."""
    return a + b


def multiply(a: int, b: int) -> int:
    """Return the product of a and b."""
    return a * b


class Calculator:
    """Stateful calculator that records history."""

    def __init__(self) -> None:
        self.history: list[int] = []

    def compute(self, op: str, a: int, b: int) -> int:
        """Apply op ('add' or 'mul') to a and b."""
        if op == "add":
            result = add(a, b)
        else:
            result = multiply(a, b)
        self.history.append(result)
        return result
