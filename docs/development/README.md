# Developer documentation

These guides are for contributors collecting parser evidence and maintainers
checking source distributions and releases. The [user documentation](../README.md)
describes the supported API, file profiles, and limits.

- [Geometry validation](geometry-validation.md): STEP reference values, NURBS
  comparisons, and boundary diagnostics.
- [Part validation](part-validation.md): independent SolidWorks API captures
  for bounded orientation and boundary checks.
- [Assembly validation](assembly-validation.md): placement capture and acceptance
  criteria for future native transform support.
- [Drawing validation](drawing-validation.md): controlled file pairs and
  independent sheet/view observations.
- [Release checks](releasing.md): licensing, source builds, installed-package
  checks, and publication.
- [Script catalog](../../scripts/README.md): tool purposes, dependencies, and
  source-distribution availability.

The reusable validation contracts live in [schemas](../schemas/).
Run commands from the repository root or the extracted source-distribution root
as specified by each guide. Private CAD files and completed capture records are
not distributed; provide your own inputs and record their hashes.
