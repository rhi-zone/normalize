def loop_(items):
    result = 0
    for item in items:
        if item == 0:
            break
        result += item
    return result
