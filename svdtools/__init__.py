"""
svdpatch.py
"""

import pathlib

from . import interrupts, makedeps, mmap, patch

__version__ = open(pathlib.Path(__file__).parent / "VERSION").read().strip()

del pathlib
