def append_item(item, lst=[]):
    lst.append(item)
    return lst

def merge(data, cache={}):
    cache.update(data)
    return cache
