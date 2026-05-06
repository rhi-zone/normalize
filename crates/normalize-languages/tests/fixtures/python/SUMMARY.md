# fixtures/python

Python fixture files for `.scm` query tests and extraction fixtures.

- `sample.py` — defines a `DataProcessor` class with methods, and `load_file`/`count_words` top-level functions; imports `os`, `sys`, `collections.defaultdict`, and `typing` symbols.
- `imports/` — extraction fixture: a Python file with `import os/sys` and `from X import Y` statements with `input.py` + `expected.json`; verifies import extraction.
