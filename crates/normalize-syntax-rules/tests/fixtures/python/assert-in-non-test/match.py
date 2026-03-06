def validate_input(value):
    assert value is not None, "value must not be None"
    assert isinstance(value, int), f"expected int, got {type(value)}"
    return value * 2


def process(data):
    assert len(data) > 0
    return data[0]
