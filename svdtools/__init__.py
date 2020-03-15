"""
svdpatch.py
"""

import pathlib

from . import makedeps, patch, interrupts

__version__ = open(pathlib.Path(__file__).parent / "VERSION").read().strip()

del pathlib
