"""
svdpatch.py
"""

import pathlib

from . import interrupts, makedeps, patch

__version__ = open(pathlib.Path(__file__).parent / "VERSION").read().strip()

del pathlib
