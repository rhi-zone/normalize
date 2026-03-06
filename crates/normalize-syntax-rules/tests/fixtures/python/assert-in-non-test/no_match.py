def validate_input(value):
    if value is None:
        raise ValueError("value must not be None")
    if not isinstance(value, int):
        raise TypeError(f"expected int, got {type(value)!r}")
    return value * 2


def process(data):
    if not data:
        raise ValueError("data must not be empty")
    return data[0]
