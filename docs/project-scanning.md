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

## Virtual components

A component whose `is_virtual` flag is set is saved inside its assembly rather
than as a project file; its stored path points at a temporary directory. The
scanner resolves such a reference to the `VirtualComp/<name>` stream of the
referencing assembly whose name matches the stored basename stem, before or
after the `^` owner suffix, and reports the basis
`embedded_virtual_component`. The embedded document becomes its own node with
the label `<assembly label>#VirtualComp/<name>` and an `embedded_source` that
names the container node and entry. It is parsed from the extracted stream and
traversed like any other node, including nested virtual components.

Several matching streams are `ambiguous`; none fall back to the ordinary path
rules and, when those fail too, add `project.virtual_component_unresolved`.
Components without the flag are never matched against embedded streams.

## Project index

`index_project` parses every CAD document under a project root and resolves all
of their references into one graph in which each indexed document is a root.
`ProjectScanResult.edges_to` and `referencing_nodes` then answer reverse
questions, for example which Drawings or Assemblies reference a Part:

```python
graph = sldkit.index_project("project")
part = graph.node_by_path("parts/bracket.SLDPRT")
drawings = graph.referencing_nodes(part.id, kind=sldkit.ReferenceKind.DRAWING_VIEW)
```

All source configurations contribute edges, and the same limits apply as for a
scan. The index has no `root_node_id`.

## Bill of materials

`build_bom` expands the root occurrence tree of a scanned graph and aggregates
quantities per document and referenced configuration:

```python
graph = sldkit.scan_project("project/top.SLDASM", project_root="project",
                            configuration="Default")
bom = sldkit.build_bom(graph)
for item in bom.part_items():
    print(item.display_name, item.key.configuration, item.quantity)
```

The expansion follows only edges of the instantiated configuration of each
node; a referenced configuration must match exactly one configuration of the
target, otherwise the occurrence is `configuration_unresolved`. The root
configuration must be unambiguous: pass `root_configuration`, or scan with a
selected configuration. Suppressed and BOM-excluded occurrences are counted
separately unless `include_suppressed` or `include_excluded` is set; a
suppressed subassembly that the scan did not traverse cannot be expanded.
Unknown suppression or exclusion flags are counted and reported in
`unknown_state_quantity`. Unresolved references remain items keyed by their
folded stored basename. Every occurrence keeps its edge identity, so the
indented tree and the aggregated items describe the same expansion.

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
