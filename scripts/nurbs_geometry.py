"""Independent, bounded NURBS evaluator used only by validation tooling."""

from __future__ import annotations

import bisect
import math
from collections import Counter
from dataclasses import dataclass
from itertools import product
from typing import Any


class NurbsValidationError(ValueError):
    """Invalid input or a definition outside the numerical comparison profile."""


def finite(value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise NurbsValidationError("expected a finite number")
    number = float(value)
    if not math.isfinite(number):
        raise NurbsValidationError("expected a finite number")
    return number


def integer(value: Any, lower: int, upper: int) -> int:
    if type(value) is not int or not lower <= value <= upper:
        raise NurbsValidationError(f"expected integer in [{lower}, {upper}]")
    return value


@dataclass(frozen=True)
class Axis:
    degree: int
    count: int
    knots: tuple[float, ...]

    @classmethod
    def read(cls, degree: Any, count: Any, knots: Any, periodic: Any) -> Axis:
        degree = integer(degree, 1, 32)
        count = integer(count, degree + 1, 100_000)
        if periodic is not False:
            raise NurbsValidationError(
                "only explicitly nonperiodic NURBS are supported"
            )
        values = tuple(map(finite, knots))
        if len(values) != count + degree + 1 or any(
            a > b for a, b in zip(values, values[1:], strict=False)
        ):
            raise NurbsValidationError("invalid knot count or ordering")
        a, b = values[degree], values[count]
        mult = Counter(values)
        if a >= b or mult[a] != degree + 1 or mult[b] != degree + 1:
            raise NurbsValidationError("expected a nonempty clamped knot domain")
        if any(v != a for v in values[: degree + 1]) or any(
            v != b for v in values[count:]
        ):
            raise NurbsValidationError("nonclamped knot domain")
        if any(m > degree - 1 for k, m in mult.items() if a < k < b):
            raise NurbsValidationError("internal knots must have C1 continuity")
        return cls(degree, count, values)

    def samples(self, subdivisions: int) -> list[float]:
        subdivisions = integer(subdivisions, 2, 64)
        distinct = sorted(set(self.knots))
        if (len(distinct) - 1) * subdivisions + 1 > 100_000:
            raise NurbsValidationError("sample budget exceeded")
        return sorted(
            {
                a + (b - a) * i / subdivisions
                for a, b in zip(distinct, distinct[1:], strict=False)
                for i in range(subdivisions)
            }
            | {distinct[-1]}
        )


def de_boor(
    points: list[tuple[float, ...]],
    knots: tuple[float, ...],
    degree: int,
    parameter: float,
) -> tuple[float, ...]:
    """Evaluate a polynomial B-spline in homogeneous coordinates."""
    count = len(points)
    if not knots[degree] <= parameter <= knots[count]:
        raise NurbsValidationError("parameter outside support domain")
    span = min(count - 1, bisect.bisect_right(knots, parameter) - 1)
    work = list(points[span - degree : span + 1])
    for level in range(1, degree + 1):
        for j in range(degree, level - 1, -1):
            i = span - degree + j
            denominator = knots[i + degree + 1 - level] - knots[i]
            if denominator <= 0:
                raise NurbsValidationError("degenerate de Boor denominator")
            alpha = (parameter - knots[i]) / denominator
            work[j] = tuple(
                (1 - alpha) * a + alpha * b
                for a, b in zip(work[j - 1], work[j], strict=True)
            )
    return work[degree]


def polynomial(
    points: list[tuple[float, ...]], axis: Axis, parameter: float
) -> tuple[tuple[float, ...], tuple[float, ...]]:
    value = de_boor(points, axis.knots, axis.degree, parameter)
    derivatives = []
    for i, (a, b) in enumerate(zip(points, points[1:], strict=False)):
        denominator = axis.knots[i + axis.degree + 1] - axis.knots[i + 1]
        if denominator <= 0:
            raise NurbsValidationError("degenerate derivative support")
        derivatives.append(
            tuple(
                axis.degree * (y - x) / denominator for x, y in zip(a, b, strict=True)
            )
        )
    return value, de_boor(derivatives, axis.knots[1:-1], axis.degree - 1, parameter)


@dataclass(frozen=True)
class Nurbs:
    domain: str
    axes: tuple[Axis, ...]
    points: tuple[tuple[float, float, float], ...]
    weights: tuple[float, ...]

    @classmethod
    def read(cls, domain: str, definition: dict) -> Nurbs:
        if definition.get("kind") != "nurbs":
            raise NurbsValidationError("expected a NURBS definition")
        raw_points = definition["control_points"]
        if not 2 <= len(raw_points) <= 100_000:
            raise NurbsValidationError("unsupported pole count")
        points = tuple(tuple(finite(p[k]) for k in ("x", "y", "z")) for p in raw_points)
        if domain == "curve":
            axes = (
                Axis.read(
                    definition["degree"],
                    len(points),
                    definition["knots"],
                    definition["periodic"],
                ),
            )
        elif domain == "surface":
            axes = tuple(
                Axis.read(
                    definition[f"{k}_degree"],
                    definition[f"{k}_count"],
                    definition[f"{k}_knots"],
                    definition[f"{k}_periodic"],
                )
                for k in ("u", "v")
            )
        else:
            raise NurbsValidationError("expected curve or surface")
        if math.prod(axis.count for axis in axes) != len(points):
            raise NurbsValidationError("pole count does not match tensor dimensions")
        weights = tuple(map(finite, definition.get("weights", [1.0] * len(points))))
        if len(weights) != len(points) or any(w <= 0 for w in weights):
            raise NurbsValidationError("expected one positive weight per pole")
        scale = max(weights)
        weights = tuple(w / scale for w in weights)
        if any(not math.isfinite(w) or w <= 0 for w in weights):
            raise NurbsValidationError("weight normalization is not representable")
        return cls(domain, axes, points, weights)

    def evaluate(
        self, parameters: tuple[float, ...]
    ) -> tuple[tuple[float, ...], tuple[tuple[float, ...], ...]]:
        if len(parameters) != len(self.axes):
            raise NurbsValidationError("parameter dimension mismatch")
        parameters = tuple(map(finite, parameters))
        poles = [
            tuple(c * w for c in point) + (w,)
            for point, w in zip(self.points, self.weights, strict=True)
        ]
        if self.domain == "curve":
            value, derivative = polynomial(poles, self.axes[0], parameters[0])
            derivatives = (derivative,)
        else:
            u, v = self.axes
            rows = [
                polynomial(poles[i * v.count : (i + 1) * v.count], v, parameters[1])
                for i in range(u.count)
            ]
            value, du = polynomial([r[0] for r in rows], u, parameters[0])
            dv = de_boor([r[1] for r in rows], u.knots, u.degree, parameters[0])
            derivatives = (du, dv)
        if value[3] <= 0 or not all(math.isfinite(x) for x in value):
            raise NurbsValidationError("invalid homogeneous evaluation")
        point = tuple(c / value[3] for c in value[:3])
        tangents = tuple(
            tuple((d[i] - point[i] * d[3]) / value[3] for i in range(3))
            for d in derivatives
        )
        if not all(math.isfinite(x) for vector in (point, *tangents) for x in vector):
            raise NurbsValidationError("nonfinite evaluation result")
        return point, tangents

    def parameters(self, subdivisions: int) -> list[tuple[float, ...]]:
        axes = [axis.samples(subdivisions) for axis in self.axes]
        if math.prod(map(len, axes)) > 100_000:
            raise NurbsValidationError("sample budget exceeded")
        return list(product(*axes))
