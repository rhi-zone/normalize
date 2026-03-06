def safe():
    try:
        do_something()
    except ValueError:
        pass
    try:
        other()
    except Exception:
        pass
