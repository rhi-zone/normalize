# raise inside except without from — loses traceback chain

try:
    parse(data)
except ValueError:
    raise RuntimeError("parsing failed")

try:
    connect(host)
except ConnectionError:
    raise ServiceError("could not connect")
