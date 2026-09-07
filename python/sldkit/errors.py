from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .model import DrawingStructureResult, GeometryResult, ParseResult


class SldkitError(Exception):
    """Base exception for the public Python API."""


class ParseError(SldkitError):
    """Raised by strict parsing while retaining the structured result."""

    def __init__(self, result: ParseResult) -> None:
        self.result = result
        codes = ", ".join(item.code for item in result.diagnostics) or "none"
        message = f"strict parsing rejected status={result.status.value}; {codes}"
        super().__init__(message)


class GeometryError(SldkitError):
    """Raised by strict geometry decoding while retaining the structured result."""

    def __init__(self, result: GeometryResult) -> None:
        self.result = result
        codes = ", ".join(item.code for item in result.diagnostics) or "none"
        message = (
            f"strict geometry decoding rejected status={result.status.value}; {codes}"
        )
        super().__init__(message)


class DrawingStructureError(SldkitError):
    """Raised by strict Drawing inventory while retaining the structured result."""

    def __init__(self, result: DrawingStructureResult) -> None:
        self.result = result
        codes = ", ".join(item.code for item in result.diagnostics) or "none"
        message = (
            "strict Drawing structure inventory rejected "
            f"status={result.status.value}; {codes}"
        )
        super().__init__(message)
