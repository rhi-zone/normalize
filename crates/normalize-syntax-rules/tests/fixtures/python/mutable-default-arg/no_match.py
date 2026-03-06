def append_item(item, lst=None):
    if lst is None:
        lst = []
    lst.append(item)
    return lst

def greet(name="world"):
    return f"hello {name}"
