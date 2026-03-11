# python/raise-without-from fixture

Fixture files for the `python/raise-without-from` syntax rule test. `match.py` raises new exceptions inside `except` blocks without a `from` clause; `no_match.py` uses `from e`, `from None`, bare `raise`, and raises outside except blocks which are not flagged.
