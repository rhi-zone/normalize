import os
import sys
from pathlib import Path
from collections import defaultdict, OrderedDict

def read_file(path: str) -> str:
    p = Path(path)
    return p.read_text()

def list_dir(directory: str):
    return os.listdir(directory)
