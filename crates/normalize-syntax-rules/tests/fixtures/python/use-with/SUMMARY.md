# python/use-with fixture

Fixture files for the `python/use-with` syntax rule test. `match.py` assigns the result of `open()` directly to a variable (bare open); `no_match.py` uses `with open(...)` context managers and calls to non-`open` functions which are not flagged.
