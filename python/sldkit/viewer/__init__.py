"""Offline HTML visualization of public sldkit results.

This module is opt-in; importing sldkit does not import the viewer.
"""

from ._html import view_file, write_html

__all__ = ["view_file", "write_html"]
