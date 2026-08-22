# Project scanning

Project scanning builds a dependency graph from references decoded from
SolidWorks Part, Assembly, and Drawing documents.

## Paths and resolution

An edge preserves the path stored in the source document separately from the
resolved project-relative path. Resolution favors explicit structure over
basename matching:

1. Document-relative and project-root-relative paths
2. Explicit Windows prefix mappings and search roots
3. Document-directory, project-root, and search-root basename candidates

If multiple candidates remain at the same priority, the edge is reported as
ambiguous and no file is selected. Windows drive and UNC paths are not treated
as native absolute paths on Linux or macOS.

## Configurations and suppression

Configuration names are matched exactly. When no configuration is selected,
the graph includes the union of references decoded from all source
configurations.

Suppressed references are retained as edges and resolved when possible. They
are not traversed by default. Missing suppression or visibility values remain
unknown rather than becoming `false`.

## Traversal

Repeated occurrences remain separate edges while a physical document maps to
one node. Cycles, reused nodes, suppressed traversal, depth limits, and graph
limits have distinct traversal states and diagnostics.

The scanner indexes only recognized CAD filename suffixes and does not recurse
through directory symlinks. Root count, indexed entry count, candidate count,
edge count, node count, and depth are bounded by the selected resource profile.

## Sharing results

The full graph contains filenames, stored paths, resolved paths, configuration
names, and source hashes. Treat it as source-sensitive data.

The `--summary` output contains aggregate counts and diagnostic codes without
paths, filenames, configuration names, or hashes. It is intended for sharing
compatibility results when those aggregate counts are acceptable to disclose.
