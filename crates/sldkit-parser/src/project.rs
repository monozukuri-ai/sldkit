use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use sldkit_core::{
    AssemblyComponent, Diagnostic, DiagnosticKind, DiagnosticSeverity, DocumentKind,
    DocumentReference, ParseStatus, ProjectCompatibilityReport, ProjectConfigurationIdentity,
    ProjectEdge, ProjectNode, ProjectScanResult, ProjectScanStatus, ReferenceKind,
    ReferenceResolutionBasis, ReferenceResolutionStatus, ReferenceTraversalStatus, ResourceLimits,
    SourceDocument,
};

use crate::parse_path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsPrefixMapping {
    pub source_prefix: String,
    pub target_directory: PathBuf,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectScanOptions {
    pub project_root: Option<PathBuf>,
    pub root_configuration: Option<String>,
    pub search_directories: Vec<PathBuf>,
    pub windows_prefix_mappings: Vec<WindowsPrefixMapping>,
    pub follow_suppressed: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum IndexRootRole {
    Project,
    Mapping(usize),
    Search(usize),
}

struct IndexRoot {
    path: PathBuf,
    role: IndexRootRole,
}

struct IndexedFile {
    host_path: PathBuf,
    label: String,
    memberships: BTreeSet<usize>,
}

struct ProjectIndex {
    roots: Vec<IndexRoot>,
    files: Vec<IndexedFile>,
    project_root_id: usize,
    mapping_root_ids: Vec<Option<usize>>,
    search_root_ids: Vec<Option<usize>>,
    by_host_exact: BTreeMap<PathBuf, usize>,
    by_host_folded: BTreeMap<String, Vec<usize>>,
    by_root_basename: BTreeMap<(usize, String), Vec<usize>>,
}

struct IndexBuild {
    index: ProjectIndex,
    root_file_index: usize,
    diagnostics: Vec<Diagnostic>,
    partial: bool,
}

struct Resolution {
    status: ReferenceResolutionStatus,
    basis: Option<ReferenceResolutionBasis>,
    candidates: Vec<usize>,
}

#[derive(Clone)]
struct ReferenceSeed {
    reference_index: u64,
    kind: ReferenceKind,
    source_configuration_index: Option<i64>,
    source_name: Option<String>,
    stored_path: Option<String>,
    referenced_configuration: Option<String>,
    expected_document_kind: Option<DocumentKind>,
    is_suppressed: Option<bool>,
    is_hidden: Option<bool>,
    exclude_from_bom: Option<bool>,
}

#[derive(Clone)]
struct ParsedNode {
    document: Option<SourceDocument>,
}

struct Scanner<'a> {
    index: ProjectIndex,
    options: &'a ProjectScanOptions,
    limits: &'a ResourceLimits,
    nodes: BTreeMap<usize, ProjectNode>,
    parsed: BTreeMap<usize, ParsedNode>,
    processed_configurations: BTreeMap<usize, BTreeSet<i64>>,
    processed_document_references: BTreeSet<usize>,
    edges: Vec<ProjectEdge>,
    diagnostics: Vec<Diagnostic>,
    partial: bool,
    rejected: bool,
    edge_limit_reported: bool,
}

/// Scan a bounded directory index and resolve source-stored document references.
#[must_use]
pub fn scan_project_path(
    root_document: impl AsRef<Path>,
    options: &ProjectScanOptions,
    limits: &ResourceLimits,
) -> ProjectScanResult {
    let build = match build_index(root_document.as_ref(), options, limits) {
        Ok(build) => build,
        Err(diagnostic) => return rejected_result(vec![diagnostic]),
    };
    let root_index = build.root_file_index;
    let mut scanner = Scanner {
        index: build.index,
        options,
        limits,
        nodes: BTreeMap::new(),
        parsed: BTreeMap::new(),
        processed_configurations: BTreeMap::new(),
        processed_document_references: BTreeSet::new(),
        edges: Vec::new(),
        diagnostics: build.diagnostics,
        partial: build.partial,
        rejected: false,
        edge_limit_reported: false,
    };
    scanner.visit(
        root_index,
        0,
        options.root_configuration.as_deref(),
        &[root_index],
        true,
    );
    scanner.finish(root_index)
}

impl Scanner<'_> {
    fn finish(mut self, root_index: usize) -> ProjectScanResult {
        self.validate_expected_document_kinds();
        let status = if self.rejected {
            ProjectScanStatus::Rejected
        } else if self.partial {
            ProjectScanStatus::Partial
        } else {
            ProjectScanStatus::Complete
        };
        let root_node_id = self.nodes.get(&root_index).map(|node| node.id.clone());
        let mut nodes = self.nodes.into_values().collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.id.cmp(&right.id));
        self.edges.sort_by(|left, right| left.id.cmp(&right.id));
        let compatibility_report =
            ProjectCompatibilityReport::from_graph(status, &nodes, &self.edges, &self.diagnostics);
        ProjectScanResult {
            schema_version: 1,
            status,
            root_node_id,
            nodes,
            edges: self.edges,
            diagnostics: self.diagnostics,
            compatibility_report,
        }
    }

    fn ensure_node(&mut self, file_index: usize, depth: u32, is_root: bool) {
        let indexed = &self.index.files[file_index];
        let id = node_id(file_index);
        self.nodes
            .entry(file_index)
            .and_modify(|node| {
                node.minimum_depth = node.minimum_depth.min(depth);
                node.is_root |= is_root;
            })
            .or_insert_with(|| ProjectNode {
                id,
                path: indexed.label.clone(),
                is_root,
                minimum_depth: depth,
                parse_status: None,
                document_kind: None,
                byte_len: None,
                source_sha256: None,
                available_configurations: Vec::new(),
                selected_configuration_indices: Vec::new(),
                requested_configurations: Vec::new(),
                diagnostic_codes: Vec::new(),
            });
    }

    fn load_document(
        &mut self,
        file_index: usize,
        depth: u32,
        is_root: bool,
    ) -> Option<SourceDocument> {
        self.ensure_node(file_index, depth, is_root);
        if let Some(parsed) = self.parsed.get(&file_index) {
            return parsed.document.clone();
        }

        let result = parse_path(&self.index.files[file_index].host_path, self.limits);
        let document = result.document.clone();
        if let Some(node) = self.nodes.get_mut(&file_index) {
            node.parse_status = Some(result.status);
            node.diagnostic_codes = result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.clone())
                .collect();
            if let Some(source) = document.as_ref() {
                node.document_kind = Some(source.document_kind.value);
                node.byte_len = Some(source.source.byte_len);
                node.source_sha256 = Some(source.source.sha256.clone());
                node.available_configurations = source
                    .configurations
                    .iter()
                    .map(|configuration| ProjectConfigurationIdentity {
                        index: configuration.index.value,
                        name: configuration.name.as_ref().map(|name| name.value.clone()),
                    })
                    .collect();
            }
        }

        if !matches!(result.status, ParseStatus::Parsed | ParseStatus::Partial)
            || document.is_none()
        {
            self.partial = true;
            if is_root {
                self.rejected = true;
            }
            let (code, severity, kind, message) = if is_root {
                (
                    "project.root_parse_unavailable",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "the root document could not provide supported project-reference facts",
                )
            } else {
                (
                    "project.node_parse_incomplete",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Unsupported,
                    "a resolved project document could not provide supported reference facts",
                )
            };
            self.diagnostics.push(
                Diagnostic::new(code, severity, kind, message)
                    .with_detail("node_id", node_id(file_index))
                    .with_detail("parse_status", parse_status_name(result.status)),
            );
        }
        self.parsed.insert(
            file_index,
            ParsedNode {
                document: document.clone(),
            },
        );
        document
    }

    fn visit(
        &mut self,
        file_index: usize,
        depth: u32,
        requested_configuration: Option<&str>,
        ancestors: &[usize],
        is_root: bool,
    ) {
        let Some(document) = self.load_document(file_index, depth, is_root) else {
            return;
        };
        let selected = self.select_configurations(file_index, &document, requested_configuration);

        if self.processed_document_references.insert(file_index) {
            for (index, reference) in document.references.iter().enumerate() {
                if reference.kind == ReferenceKind::AssemblyComponent {
                    continue;
                }
                self.process_reference(
                    file_index,
                    depth,
                    reference_seed(index, reference),
                    ancestors,
                );
            }
        }

        for configuration_index in selected {
            let is_new = self
                .processed_configurations
                .entry(file_index)
                .or_default()
                .insert(configuration_index);
            if !is_new {
                continue;
            }
            let Some(configuration) = document
                .configurations
                .iter()
                .find(|configuration| configuration.index.value == configuration_index)
            else {
                continue;
            };
            for (index, component) in configuration.components.iter().enumerate() {
                self.process_reference(
                    file_index,
                    depth,
                    component_seed(index, component),
                    ancestors,
                );
            }
        }
    }

    fn select_configurations(
        &mut self,
        file_index: usize,
        document: &SourceDocument,
        requested: Option<&str>,
    ) -> Vec<i64> {
        if let Some(requested) = requested {
            if let Some(node) = self.nodes.get_mut(&file_index)
                && !node
                    .requested_configurations
                    .iter()
                    .any(|value| value == requested)
            {
                node.requested_configurations.push(requested.to_owned());
                node.requested_configurations.sort();
            }
            let mut matches = document
                .configurations
                .iter()
                .filter(|configuration| {
                    configuration
                        .name
                        .as_ref()
                        .is_some_and(|name| name.value == requested)
                        || configuration
                            .alternate_names
                            .iter()
                            .any(|name| name.value == requested)
                })
                .map(|configuration| configuration.index.value)
                .collect::<Vec<_>>();
            matches.sort_unstable();
            matches.dedup();
            if matches.len() != 1 {
                self.partial = true;
                let (code, message) = if matches.is_empty() {
                    (
                        "project.configuration_missing",
                        "a requested source configuration was not present in the resolved document",
                    )
                } else {
                    (
                        "project.configuration_ambiguous",
                        "a requested source configuration matched multiple configuration identities",
                    )
                };
                self.diagnostics.push(
                    Diagnostic::new(
                        code,
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Unresolved,
                        message,
                    )
                    .with_detail("node_id", node_id(file_index))
                    .with_detail("requested_configuration", requested)
                    .with_detail("candidate_count", matches.len().to_string()),
                );
                return Vec::new();
            }
            self.record_selected_configurations(file_index, &matches);
            matches
        } else {
            let mut selected = document
                .configurations
                .iter()
                .map(|configuration| configuration.index.value)
                .collect::<Vec<_>>();
            selected.sort_unstable();
            selected.dedup();
            self.record_selected_configurations(file_index, &selected);
            selected
        }
    }

    fn record_selected_configurations(&mut self, file_index: usize, indices: &[i64]) {
        if let Some(node) = self.nodes.get_mut(&file_index) {
            node.selected_configuration_indices
                .extend_from_slice(indices);
            node.selected_configuration_indices.sort_unstable();
            node.selected_configuration_indices.dedup();
        }
    }

    fn process_reference(
        &mut self,
        source_index: usize,
        depth: u32,
        seed: ReferenceSeed,
        ancestors: &[usize],
    ) {
        if self.project_edge_limit_reached() {
            return;
        }
        let resolution =
            self.index
                .resolve(source_index, seed.stored_path.as_deref(), self.options);
        let (candidate_count, candidate_paths) = self.reference_candidate_output(&resolution);
        let target_index = if resolution.status == ReferenceResolutionStatus::Resolved {
            resolution.candidates.first().copied()
        } else {
            None
        };
        let target_node_id = target_index.map(node_id);
        let resolved_path = target_index.map(|index| self.index.files[index].label.clone());
        let traversal_status = self.reference_traversal_status(
            source_index,
            target_index,
            depth,
            &seed,
            ancestors,
            &resolution,
        );

        let edge_id = format!("edge:{:08}", self.edges.len());
        self.edges.push(ProjectEdge {
            id: edge_id,
            source_node_id: node_id(source_index),
            reference_index: seed.reference_index,
            kind: seed.kind,
            source_configuration_index: seed.source_configuration_index,
            source_name: seed.source_name,
            stored_path: seed.stored_path,
            referenced_configuration: seed.referenced_configuration.clone(),
            expected_document_kind: seed.expected_document_kind,
            is_suppressed: seed.is_suppressed,
            is_hidden: seed.is_hidden,
            exclude_from_bom: seed.exclude_from_bom,
            resolution_status: resolution.status,
            resolution_basis: resolution.basis,
            candidate_count,
            candidate_paths,
            resolved_path,
            target_node_id,
            traversal_status,
        });

        if let Some(target_index) = target_index
            && matches!(
                traversal_status,
                ReferenceTraversalStatus::Followed | ReferenceTraversalStatus::Reused
            )
        {
            let mut next_ancestors = ancestors.to_vec();
            next_ancestors.push(target_index);
            self.visit(
                target_index,
                depth.saturating_add(1),
                seed.referenced_configuration.as_deref(),
                &next_ancestors,
                false,
            );
        }
    }

    fn project_edge_limit_reached(&mut self) -> bool {
        if u64::try_from(self.edges.len()).unwrap_or(u64::MAX) < self.limits.max_project_edges {
            return false;
        }
        self.rejected = true;
        if !self.edge_limit_reported {
            self.edge_limit_reported = true;
            self.diagnostics.push(
                Diagnostic::new(
                    "limit.project_edges",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "project graph exceeds the configured reference-edge limit",
                )
                .with_detail("limit", self.limits.max_project_edges.to_string()),
            );
        }
        true
    }

    fn reference_candidate_output(&mut self, resolution: &Resolution) -> (u64, Vec<String>) {
        let candidate_count = u64::try_from(resolution.candidates.len()).unwrap_or(u64::MAX);
        let candidate_paths = resolution
            .candidates
            .iter()
            .take(usize::try_from(self.limits.max_reference_candidates).unwrap_or(usize::MAX))
            .map(|index| self.index.files[*index].label.clone())
            .collect::<Vec<_>>();
        if candidate_count > self.limits.max_reference_candidates {
            self.diagnostics.push(
                Diagnostic::new(
                    "limit.reference_candidates",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Preserved,
                    "reference candidate output was truncated at the configured limit",
                )
                .with_detail("candidate_count", candidate_count.to_string())
                .with_detail("limit", self.limits.max_reference_candidates.to_string()),
            );
        }
        (candidate_count, candidate_paths)
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_traversal_status(
        &mut self,
        source_index: usize,
        target_index: Option<usize>,
        depth: u32,
        seed: &ReferenceSeed,
        ancestors: &[usize],
        resolution: &Resolution,
    ) -> ReferenceTraversalStatus {
        let Some(target_index) = target_index else {
            self.partial = true;
            self.reference_resolution_diagnostic(source_index, seed, resolution);
            return ReferenceTraversalStatus::Unresolved;
        };
        self.ensure_node(target_index, depth.saturating_add(1), false);
        if let Some(requested) = seed.referenced_configuration.as_deref()
            && let Some(node) = self.nodes.get_mut(&target_index)
            && !node
                .requested_configurations
                .iter()
                .any(|value| value == requested)
        {
            node.requested_configurations.push(requested.to_owned());
            node.requested_configurations.sort();
        }
        if seed.is_suppressed == Some(true) && !self.options.follow_suppressed {
            return ReferenceTraversalStatus::Suppressed;
        }
        if ancestors.contains(&target_index) {
            self.diagnostics.push(
                Diagnostic::new(
                    "project.reference_cycle",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Preserved,
                    "a resolved document-reference cycle was retained without recursion",
                )
                .with_detail("source_node_id", node_id(source_index))
                .with_detail("target_node_id", node_id(target_index)),
            );
            return ReferenceTraversalStatus::Cycle;
        }
        if depth >= self.limits.max_reference_depth {
            self.partial = true;
            self.diagnostics.push(
                Diagnostic::new(
                    "limit.reference_depth",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Unresolved,
                    "resolved reference was not traversed beyond the configured depth",
                )
                .with_detail("source_node_id", node_id(source_index))
                .with_detail("target_node_id", node_id(target_index))
                .with_detail("limit", self.limits.max_reference_depth.to_string()),
            );
            return ReferenceTraversalStatus::DepthLimited;
        }
        if self
            .nodes
            .get(&target_index)
            .is_some_and(|node| node.parse_status.is_some())
        {
            ReferenceTraversalStatus::Reused
        } else {
            ReferenceTraversalStatus::Followed
        }
    }

    fn reference_resolution_diagnostic(
        &mut self,
        source_index: usize,
        seed: &ReferenceSeed,
        resolution: &Resolution,
    ) {
        let (code, message) = match resolution.status {
            ReferenceResolutionStatus::Missing => (
                "project.reference_missing",
                "no indexed project file matched the stored reference path",
            ),
            ReferenceResolutionStatus::Ambiguous => (
                "project.reference_ambiguous",
                "multiple indexed project files matched the stored reference path",
            ),
            ReferenceResolutionStatus::NoStoredPath => (
                "project.reference_path_missing",
                "the source reference has no stored document path",
            ),
            ReferenceResolutionStatus::Resolved => return,
        };
        let mut diagnostic = Diagnostic::new(
            code,
            DiagnosticSeverity::Warning,
            DiagnosticKind::Unresolved,
            message,
        )
        .with_detail("source_node_id", node_id(source_index))
        .with_detail("candidate_count", resolution.candidates.len().to_string());
        if let Some(stored_path) = seed.stored_path.as_deref() {
            diagnostic = diagnostic.with_detail("stored_path", stored_path);
        }
        self.diagnostics.push(diagnostic);
    }

    fn validate_expected_document_kinds(&mut self) {
        let actual_kinds = self
            .nodes
            .values()
            .map(|node| (node.id.clone(), node.document_kind))
            .collect::<BTreeMap<_, _>>();
        for edge in &self.edges {
            let (Some(expected), Some(target_node_id)) =
                (edge.expected_document_kind, edge.target_node_id.as_deref())
            else {
                continue;
            };
            let Some(Some(actual)) = actual_kinds.get(target_node_id) else {
                continue;
            };
            if expected != *actual {
                self.partial = true;
                self.diagnostics.push(
                    Diagnostic::new(
                        "project.reference_document_kind_mismatch",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Unresolved,
                        "resolved document kind differs from source reference metadata",
                    )
                    .with_detail("edge_id", &edge.id)
                    .with_detail("expected", document_kind_name(expected))
                    .with_detail("actual", document_kind_name(*actual)),
                );
            }
        }
    }
}

impl ProjectIndex {
    fn resolve(
        &self,
        source_index: usize,
        stored_path: Option<&str>,
        options: &ProjectScanOptions,
    ) -> Resolution {
        let Some(stored_path) = stored_path.map(str::trim).filter(|path| !path.is_empty()) else {
            return Resolution {
                status: ReferenceResolutionStatus::NoStoredPath,
                basis: None,
                candidates: Vec::new(),
            };
        };
        let normalized = normalize_source_path(stored_path);
        let basename = source_basename(&normalized);
        let windows_absolute = is_windows_absolute(stored_path);
        let host_absolute = !windows_absolute && Path::new(stored_path).is_absolute();
        let source_parent = self.files[source_index]
            .host_path
            .parent()
            .unwrap_or(self.roots[self.project_root_id].path.as_path());

        if !windows_absolute && !host_absolute {
            let candidates = self.lookup_host(&join_source_path(source_parent, &normalized));
            if !candidates.is_empty() {
                return classify(ReferenceResolutionBasis::DocumentRelative, candidates);
            }
        }
        if !windows_absolute && !host_absolute {
            let project_root = &self.roots[self.project_root_id].path;
            let candidates = self.lookup_host(&join_source_path(project_root, &normalized));
            if !candidates.is_empty() {
                return classify(ReferenceResolutionBasis::ProjectRootRelative, candidates);
            }
        }
        if host_absolute {
            let candidates = self.lookup_host(&lexical_normalize(Path::new(stored_path)));
            if !candidates.is_empty() {
                return classify(ReferenceResolutionBasis::HostAbsolute, candidates);
            }
        }

        if windows_absolute {
            let mapping_candidates = self.mapping_candidates(&normalized, options);
            if !mapping_candidates.is_empty() {
                return classify(
                    ReferenceResolutionBasis::WindowsPrefixMapping,
                    mapping_candidates,
                );
            }
        }

        let search_relative = self.search_relative_candidates(&normalized);
        if !search_relative.is_empty() {
            return classify(
                ReferenceResolutionBasis::SearchDirectoryRelative,
                search_relative,
            );
        }
        if let Some(basename) = basename {
            let candidates = self.lookup_host(&source_parent.join(basename));
            if !candidates.is_empty() {
                return classify(ReferenceResolutionBasis::DocumentBasename, candidates);
            }
        }
        if let Some(basename) = basename {
            let candidates = self.lookup_root_basename(self.project_root_id, basename);
            if !candidates.is_empty() {
                return classify(ReferenceResolutionBasis::ProjectRootBasename, candidates);
            }
        }
        if let Some(basename) = basename {
            let search_basename = self.search_basename_candidates(basename);
            if !search_basename.is_empty() {
                return classify(
                    ReferenceResolutionBasis::SearchDirectoryBasename,
                    search_basename,
                );
            }
        }

        Resolution {
            status: ReferenceResolutionStatus::Missing,
            basis: None,
            candidates: Vec::new(),
        }
    }

    fn lookup_host(&self, path: &Path) -> Vec<usize> {
        self.by_host_folded
            .get(&fold_host_path(path))
            .cloned()
            .unwrap_or_default()
    }

    fn lookup_root_basename(&self, root_id: usize, basename: &str) -> Vec<usize> {
        self.by_root_basename
            .get(&(root_id, fold_name(basename)))
            .cloned()
            .unwrap_or_default()
    }

    fn mapping_candidates(
        &self,
        normalized_stored: &str,
        options: &ProjectScanOptions,
    ) -> Vec<usize> {
        let stored_folded = normalized_stored.to_lowercase();
        let mut matches = Vec::new();
        let mut longest_prefix = 0_usize;
        for (index, mapping) in options.windows_prefix_mappings.iter().enumerate() {
            let Some(root_id) = self.mapping_root_ids.get(index).copied().flatten() else {
                continue;
            };
            let prefix = normalize_source_path(&mapping.source_prefix);
            let prefix_folded = prefix.to_lowercase();
            if !has_path_prefix(&stored_folded, &prefix_folded) {
                continue;
            }
            if prefix.len() < longest_prefix {
                continue;
            }
            if prefix.len() > longest_prefix {
                matches.clear();
                longest_prefix = prefix.len();
            }
            let suffix = normalized_stored
                .get(prefix.len()..)
                .unwrap_or_default()
                .trim_start_matches('/');
            matches.extend(self.lookup_host(&join_source_path(&self.roots[root_id].path, suffix)));
        }
        sorted_unique(matches, &self.files)
    }

    fn search_relative_candidates(&self, normalized_stored: &str) -> Vec<usize> {
        let relative = trim_source_root(normalized_stored);
        let mut matches = Vec::new();
        for root_id in self.search_root_ids.iter().flatten() {
            matches
                .extend(self.lookup_host(&join_source_path(&self.roots[*root_id].path, relative)));
        }
        sorted_unique(matches, &self.files)
    }

    fn search_basename_candidates(&self, basename: &str) -> Vec<usize> {
        let mut matches = Vec::new();
        for root_id in self.search_root_ids.iter().flatten() {
            matches.extend(self.lookup_root_basename(*root_id, basename));
        }
        sorted_unique(matches, &self.files)
    }
}

#[allow(clippy::too_many_lines)]
fn build_index(
    root_document: &Path,
    options: &ProjectScanOptions,
    limits: &ResourceLimits,
) -> Result<IndexBuild, Diagnostic> {
    let requested_root_count = 1_u64
        .saturating_add(u64::try_from(options.windows_prefix_mappings.len()).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(options.search_directories.len()).unwrap_or(u64::MAX));
    if requested_root_count > limits.max_project_roots {
        return Err(Diagnostic::new(
            "limit.project_roots",
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            "project scan exceeds the configured root-directory limit",
        )
        .with_detail("root_count", requested_root_count.to_string())
        .with_detail("limit", limits.max_project_roots.to_string()));
    }
    let root_file = fs::canonicalize(root_document).map_err(|error| {
        fatal_io_diagnostic(
            "project.root_document_unreadable",
            "root project document could not be resolved",
            root_document,
            &error,
        )
    })?;
    if !root_file.is_file() {
        return Err(Diagnostic::new(
            "project.root_document_not_file",
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            "root project document is not a regular file",
        )
        .with_detail("path", display_path(&root_file)));
    }
    let project_root_input = options
        .project_root
        .as_deref()
        .or_else(|| root_file.parent())
        .unwrap_or_else(|| Path::new("."));
    let project_root = fs::canonicalize(project_root_input).map_err(|error| {
        fatal_io_diagnostic(
            "project.scan_root_unreadable",
            "project scan root could not be resolved",
            project_root_input,
            &error,
        )
    })?;
    if !project_root.is_dir() {
        return Err(Diagnostic::new(
            "project.scan_root_not_directory",
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            "project scan root is not a directory",
        )
        .with_detail("path", display_path(&project_root)));
    }
    if !root_file.starts_with(&project_root) {
        return Err(Diagnostic::new(
            "project.root_outside_scan_root",
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            "root project document must be contained by the project scan root",
        )
        .with_detail("root_document", display_path(&root_file))
        .with_detail("project_root", display_path(&project_root)));
    }

    let mut diagnostics = Vec::new();
    let mut partial = false;
    let mut roots = vec![IndexRoot {
        path: project_root,
        role: IndexRootRole::Project,
    }];
    let project_root_id = 0;
    let mut mapping_root_ids = Vec::new();
    for (index, mapping) in options.windows_prefix_mappings.iter().enumerate() {
        if mapping.source_prefix.trim().is_empty() {
            partial = true;
            diagnostics.push(Diagnostic::new(
                "project.windows_prefix_empty",
                DiagnosticSeverity::Warning,
                DiagnosticKind::Unresolved,
                "an empty Windows prefix mapping was ignored",
            ));
            mapping_root_ids.push(None);
            continue;
        }
        match fs::canonicalize(&mapping.target_directory) {
            Ok(path) if path.is_dir() => {
                let root_id = if let Some(root_id) = roots.iter().position(|root| root.path == path)
                {
                    root_id
                } else {
                    let root_id = roots.len();
                    roots.push(IndexRoot {
                        path,
                        role: IndexRootRole::Mapping(index),
                    });
                    root_id
                };
                mapping_root_ids.push(Some(root_id));
            }
            Ok(path) => {
                partial = true;
                diagnostics.push(
                    Diagnostic::new(
                        "project.mapping_target_not_directory",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Unresolved,
                        "a Windows prefix mapping target is not a directory",
                    )
                    .with_detail("path", display_path(&path)),
                );
                mapping_root_ids.push(None);
            }
            Err(error) => {
                partial = true;
                diagnostics.push(
                    Diagnostic::new(
                        "project.mapping_target_unreadable",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Unresolved,
                        "a Windows prefix mapping target could not be resolved",
                    )
                    .with_detail("path", display_path(&mapping.target_directory))
                    .with_detail("error_kind", format!("{:?}", error.kind())),
                );
                mapping_root_ids.push(None);
            }
        }
    }
    let mut search_root_ids = Vec::new();
    for (index, directory) in options.search_directories.iter().enumerate() {
        match fs::canonicalize(directory) {
            Ok(path) if path.is_dir() => {
                let root_id = if let Some(root_id) = roots.iter().position(|root| root.path == path)
                {
                    root_id
                } else {
                    let root_id = roots.len();
                    roots.push(IndexRoot {
                        path,
                        role: IndexRootRole::Search(index),
                    });
                    root_id
                };
                search_root_ids.push(Some(root_id));
            }
            Ok(path) => {
                partial = true;
                diagnostics.push(
                    Diagnostic::new(
                        "project.search_path_not_directory",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Unresolved,
                        "a project search path is not a directory",
                    )
                    .with_detail("path", display_path(&path)),
                );
                search_root_ids.push(None);
            }
            Err(error) => {
                partial = true;
                diagnostics.push(
                    Diagnostic::new(
                        "project.search_path_unreadable",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Unresolved,
                        "a project search path could not be resolved",
                    )
                    .with_detail("path", display_path(directory))
                    .with_detail("error_kind", format!("{:?}", error.kind())),
                );
                search_root_ids.push(None);
            }
        }
    }

    let mut index = ProjectIndex {
        roots,
        files: Vec::new(),
        project_root_id,
        mapping_root_ids,
        search_root_ids,
        by_host_exact: BTreeMap::new(),
        by_host_folded: BTreeMap::new(),
        by_root_basename: BTreeMap::new(),
    };
    let mut entries_seen = 0_u64;
    for root_id in 0..index.roots.len() {
        let (root_diagnostics, root_partial) =
            scan_index_root(&mut index, root_id, &mut entries_seen, limits)?;
        diagnostics.extend(root_diagnostics);
        partial |= root_partial;
    }
    if !index.by_host_exact.contains_key(&root_file) {
        add_indexed_file(&mut index, project_root_id, root_file.clone(), limits)?;
    }
    finalize_lookup_maps(&mut index);
    let Some(root_file_index) = index.by_host_exact.get(&root_file).copied() else {
        return Err(Diagnostic::new(
            "project.root_document_not_indexed",
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            "root project document was not present in the bounded directory index",
        ));
    };

    Ok(IndexBuild {
        index,
        root_file_index,
        diagnostics,
        partial,
    })
}

fn scan_index_root(
    index: &mut ProjectIndex,
    root_id: usize,
    entries_seen: &mut u64,
    limits: &ResourceLimits,
) -> Result<(Vec<Diagnostic>, bool), Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut partial = false;
    let mut pending = vec![index.roots[root_id].path.clone()];
    while let Some(directory) = pending.pop() {
        let read_dir = match fs::read_dir(&directory) {
            Ok(read_dir) => read_dir,
            Err(error) => {
                partial = true;
                diagnostics.push(
                    Diagnostic::new(
                        "project.directory_unreadable",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Unresolved,
                        "an indexed project directory could not be read",
                    )
                    .with_detail("path", display_path(&directory))
                    .with_detail("error_kind", format!("{:?}", error.kind())),
                );
                continue;
            }
        };
        let mut entries = Vec::new();
        for entry in read_dir {
            match entry {
                Ok(entry) => entries.push(entry),
                Err(error) => {
                    partial = true;
                    diagnostics.push(
                        Diagnostic::new(
                            "project.directory_entry_unreadable",
                            DiagnosticSeverity::Warning,
                            DiagnosticKind::Unresolved,
                            "an indexed directory entry could not be read",
                        )
                        .with_detail("path", display_path(&directory))
                        .with_detail("error_kind", format!("{:?}", error.kind())),
                    );
                }
            }
        }
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            *entries_seen = entries_seen.saturating_add(1);
            if *entries_seen > limits.max_project_entries {
                return Err(Diagnostic::new(
                    "limit.project_entries",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "directory index exceeds the configured entry limit",
                )
                .with_detail("limit", limits.max_project_entries.to_string()));
            }
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    partial = true;
                    diagnostics.push(
                        Diagnostic::new(
                            "project.directory_entry_unreadable",
                            DiagnosticSeverity::Warning,
                            DiagnosticKind::Unresolved,
                            "an indexed directory entry type could not be read",
                        )
                        .with_detail("path", display_path(&entry.path()))
                        .with_detail("error_kind", format!("{:?}", error.kind())),
                    );
                    continue;
                }
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() && is_cad_path(&entry.path()) {
                add_indexed_file(index, root_id, entry.path(), limits)?;
            }
        }
        pending.sort_by(|left, right| right.cmp(left));
    }
    Ok((diagnostics, partial))
}

fn add_indexed_file(
    index: &mut ProjectIndex,
    root_id: usize,
    host_path: PathBuf,
    limits: &ResourceLimits,
) -> Result<(), Diagnostic> {
    if let Some(existing) = index.by_host_exact.get(&host_path).copied() {
        index.files[existing].memberships.insert(root_id);
        return Ok(());
    }
    if u64::try_from(index.files.len()).unwrap_or(u64::MAX) >= limits.max_project_files {
        return Err(Diagnostic::new(
            "limit.project_files",
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            "project index exceeds the configured CAD file limit",
        )
        .with_detail("limit", limits.max_project_files.to_string()));
    }
    let relative = host_path
        .strip_prefix(&index.roots[root_id].path)
        .unwrap_or(host_path.as_path());
    let relative_label = display_relative_path(relative);
    let label = match index.roots[root_id].role {
        IndexRootRole::Project => relative_label,
        IndexRootRole::Mapping(mapping) => format!("@mapping/{mapping}/{relative_label}"),
        IndexRootRole::Search(search) => format!("@search/{search}/{relative_label}"),
    };
    let file_index = index.files.len();
    index.files.push(IndexedFile {
        host_path: host_path.clone(),
        label,
        memberships: BTreeSet::from([root_id]),
    });
    index.by_host_exact.insert(host_path, file_index);
    Ok(())
}

fn finalize_lookup_maps(index: &mut ProjectIndex) {
    index.by_host_folded.clear();
    index.by_root_basename.clear();
    for (file_index, file) in index.files.iter().enumerate() {
        index
            .by_host_folded
            .entry(fold_host_path(&file.host_path))
            .or_default()
            .push(file_index);
        let Some(basename) = file.host_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        for root_id in &file.memberships {
            index
                .by_root_basename
                .entry((*root_id, fold_name(basename)))
                .or_default()
                .push(file_index);
        }
    }
    let files = &index.files;
    for candidates in index.by_host_folded.values_mut() {
        *candidates = sorted_unique(std::mem::take(candidates), files);
    }
    for candidates in index.by_root_basename.values_mut() {
        *candidates = sorted_unique(std::mem::take(candidates), files);
    }
}

fn classify(basis: ReferenceResolutionBasis, candidates: Vec<usize>) -> Resolution {
    Resolution {
        status: if candidates.len() == 1 {
            ReferenceResolutionStatus::Resolved
        } else {
            ReferenceResolutionStatus::Ambiguous
        },
        basis: Some(basis),
        candidates,
    }
}

fn component_seed(index: usize, component: &AssemblyComponent) -> ReferenceSeed {
    ReferenceSeed {
        reference_index: u64::try_from(index).unwrap_or(u64::MAX),
        kind: ReferenceKind::AssemblyComponent,
        source_configuration_index: Some(component.configuration_index),
        source_name: component
            .instance_name
            .as_ref()
            .map(|value| value.value.clone()),
        stored_path: component
            .stored_path
            .as_ref()
            .map(|value| value.value.clone()),
        referenced_configuration: component
            .referenced_configuration
            .as_ref()
            .map(|value| value.value.clone()),
        expected_document_kind: component.document_kind.as_ref().map(|value| value.value),
        is_suppressed: component.is_suppressed.as_ref().map(|value| value.value),
        is_hidden: component.is_hidden.as_ref().map(|value| value.value),
        exclude_from_bom: component.exclude_from_bom.as_ref().map(|value| value.value),
    }
}

fn reference_seed(index: usize, reference: &DocumentReference) -> ReferenceSeed {
    ReferenceSeed {
        reference_index: u64::try_from(index).unwrap_or(u64::MAX),
        kind: reference.kind,
        source_configuration_index: reference.configuration_index,
        source_name: reference
            .source_name
            .as_ref()
            .map(|value| value.value.clone()),
        stored_path: reference
            .stored_path
            .as_ref()
            .map(|value| value.value.clone()),
        referenced_configuration: reference
            .configuration
            .as_ref()
            .map(|value| value.value.clone()),
        expected_document_kind: reference.document_kind.as_ref().map(|value| value.value),
        is_suppressed: None,
        is_hidden: None,
        exclude_from_bom: None,
    }
}

fn rejected_result(diagnostics: Vec<Diagnostic>) -> ProjectScanResult {
    let status = ProjectScanStatus::Rejected;
    let compatibility_report =
        ProjectCompatibilityReport::from_graph(status, &[], &[], &diagnostics);
    ProjectScanResult {
        schema_version: 1,
        status,
        root_node_id: None,
        nodes: Vec::new(),
        edges: Vec::new(),
        diagnostics,
        compatibility_report,
    }
}

fn fatal_io_diagnostic(
    code: &str,
    message: &str,
    path: &Path,
    error: &std::io::Error,
) -> Diagnostic {
    Diagnostic::new(
        code,
        DiagnosticSeverity::Error,
        DiagnosticKind::Fatal,
        message,
    )
    .with_detail("path", display_path(path))
    .with_detail("error_kind", format!("{:?}", error.kind()))
}

fn is_cad_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "sldprt" | "sldasm" | "slddrw"
            )
        })
}

fn normalize_source_path(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|component| !component.is_empty() && *component != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn trim_source_root(path: &str) -> &str {
    let without_drive = path
        .get(2..)
        .filter(|_| path.as_bytes().get(1) == Some(&b':'))
        .unwrap_or(path);
    without_drive.trim_start_matches('/')
}

fn source_basename(path: &str) -> Option<&str> {
    path.rsplit('/')
        .find(|component| !component.is_empty() && *component != "." && *component != "..")
}

fn join_source_path(base: &Path, source_path: &str) -> PathBuf {
    let mut output = base.to_path_buf();
    for component in trim_source_root(source_path).split('/') {
        match component {
            "" | "." => {}
            ".." => {
                let _ = output.pop();
            }
            value => output.push(value),
        }
    }
    lexical_normalize(&output)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = output.pop();
            }
            Component::Prefix(prefix) => output.push(prefix.as_os_str()),
            Component::RootDir => output.push(component.as_os_str()),
            Component::Normal(value) => output.push(value),
        }
    }
    output
}

fn is_windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    (bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/'))
        || path.starts_with(r"\\")
        || path.starts_with("//")
}

fn has_path_prefix(path_folded: &str, prefix_folded: &str) -> bool {
    if prefix_folded.is_empty() || !path_folded.starts_with(prefix_folded) {
        return false;
    }
    path_folded.len() == prefix_folded.len()
        || path_folded
            .as_bytes()
            .get(prefix_folded.len())
            .is_some_and(|byte| *byte == b'/')
}

fn fold_host_path(path: &Path) -> String {
    display_path(&lexical_normalize(path)).to_lowercase()
}

fn fold_name(name: &str) -> String {
    name.to_lowercase()
}

fn sorted_unique(mut candidates: Vec<usize>, files: &[IndexedFile]) -> Vec<usize> {
    candidates.sort_by(|left, right| files[*left].label.cmp(&files[*right].label));
    candidates.dedup();
    candidates
}

fn node_id(file_index: usize) -> String {
    format!("node:{file_index:08}")
}

fn display_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

const fn parse_status_name(status: ParseStatus) -> &'static str {
    match status {
        ParseStatus::Parsed => "parsed",
        ParseStatus::Partial => "partial",
        ParseStatus::Unsupported => "unsupported",
        ParseStatus::Malformed => "malformed",
        ParseStatus::Rejected => "rejected",
    }
}

const fn document_kind_name(kind: DocumentKind) -> &'static str {
    match kind {
        DocumentKind::Part => "part",
        DocumentKind::Assembly => "assembly",
        DocumentKind::Drawing => "drawing",
        DocumentKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fmt::{self, Write as _},
        fs,
        io::Write,
        path::Path,
    };

    use flate2::{Compression, write::DeflateEncoder};
    use sldkit_core::{
        ProjectScanStatus, ReferenceResolutionBasis, ReferenceResolutionStatus,
        ReferenceTraversalStatus, ResourceLimits,
    };
    use tempfile::TempDir;

    use super::{ProjectScanOptions, WindowsPrefixMapping, scan_project_path};

    struct ComponentSpec<'a> {
        path: &'a str,
        kind: &'a str,
        name: &'a str,
        suppressed: bool,
    }

    fn modern_file(stream_name: &str, payload: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(payload)?;
        let compressed = encoder.finish()?;
        let encoded_name = stream_name
            .as_bytes()
            .iter()
            .map(|value| value.rotate_right(4))
            .collect::<Vec<_>>();
        let mut output = b"SLDK\x00\x00\x00\x04\x14\x00\x06\x00\x08\x00".to_vec();
        output.extend_from_slice(&7_u32.to_le_bytes());
        output.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
        output.extend_from_slice(
            &u32::try_from(compressed.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        output.extend_from_slice(
            &u32::try_from(payload.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        output.extend_from_slice(
            &u32::try_from(encoded_name.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        output.extend_from_slice(&encoded_name);
        output.extend_from_slice(&compressed);
        Ok(output)
    }

    fn assembly_xml(components: &[ComponentSpec<'_>]) -> Result<String, fmt::Error> {
        let mut files = String::new();
        let mut references = String::new();
        let mut models = String::new();
        for (index, component) in components.iter().enumerate() {
            write!(
                files,
                r#"<swFile id="{}" swDocType="{}" swPath="{}"/>"#,
                index + 1,
                component.kind,
                component.path
            )?;
            write!(
                references,
                r#"<swReference swModelRef="m{}" swName="{}" swSuppressed="{}"/>"#,
                index + 1,
                component.name,
                if component.suppressed { "YES" } else { "NO" }
            )?;
            write!(
                models,
                r#"<swModel id="m{}" swFileRef="{}" swConfigurationName="Default"/>"#,
                index + 1,
                index + 1
            )?;
        }
        Ok(format!(
            r#"<root><swHeader><swFile id="0" swDocType="ASSEMBLY"/>{files}</swHeader><swModelList><swModel id="self" swFileRef="0" swConfigurationId="0"><swConfiguration swID="0" swName="Default"/>{references}</swModel>{models}</swModelList></root>"#
        ))
    }

    fn part_xml() -> &'static str {
        r#"<root><swHeader><swFile id="0" swDocType="PART"/></swHeader><swModelList><swModel id="self" swFileRef="0" swConfigurationId="0"><swConfiguration swID="0" swName="Default"/></swModel></swModelList></root>"#
    }

    fn write_assembly(
        path: &Path,
        components: &[ComponentSpec<'_>],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let xml = assembly_xml(components)?;
        fs::write(
            path,
            modern_file("swXmlContents/COMPINSTANCETREE", xml.as_bytes())?,
        )?;
        Ok(())
    }

    fn write_part(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(
            path,
            modern_file("swXmlContents/Features", part_xml().as_bytes())?,
        )?;
        Ok(())
    }

    fn write_configured_assembly(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let xml = r#"<root><swHeader>
            <swFile id="0" swDocType="ASSEMBLY"/>
            <swFile id="1" swDocType="PART" swPath="default.SLDPRT"/>
            <swFile id="2" swDocType="PART" swPath="alternate.SLDPRT"/>
            </swHeader><swModelList>
            <swModel id="self-default" swFileRef="0" swConfigurationId="0">
            <swConfiguration swID="0" swName="Default"/>
            <swReference swModelRef="default" swName="default-1" swSuppressed="NO"/>
            </swModel>
            <swModel id="self-alt" swFileRef="0" swConfigurationId="1">
            <swConfiguration swID="1" swName="Alternate"/>
            <swReference swModelRef="alternate" swName="alternate-1" swSuppressed="NO"/>
            </swModel>
            <swModel id="default" swFileRef="1" swConfigurationName="Default"/>
            <swModel id="alternate" swFileRef="2" swConfigurationName="Default"/>
            </swModelList></root>"#;
        fs::write(
            path,
            modern_file("swXmlContents/COMPINSTANCETREE", xml.as_bytes())?,
        )?;
        Ok(())
    }

    fn write_relocated_derived_assembly(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let xml = r#"<root><swHeader>
            <swFile id="old-self" swDocType="ASSEMBLY" swPath="OLD.SLDASM"/>
            <swFile id="part-1" swDocType="PART" swPath="TEST1.SLDPRT"/>
            <swFile id="part-2" swDocType="PART" swPath="TEST2.SLDPRT"/>
            <swFile id="new-self" swDocType="ASSEMBLY" swPath="TEST2.SLDASM"/>
            </swHeader><swModelList>
            <swModel id="part-model-1" swFileRef="part-1" swConfigurationName="Default"/>
            <swModel id="part-model-2" swFileRef="part-2" swConfigurationName="Default"/>
            <swModel id="root-base" swFileRef="old-self" swConfigurationId="1">
              <swReference swModelRef="part-model-1" swName="TEST1" swSuppressed="NO"/>
              <swReference swModelRef="part-model-2" swName="TEST2" swSuppressed="NO"/>
            </swModel>
            <swModel id="root-derived" swFileRef="new-self" swConfigurationId="2">
              <swReference swModelRef="part-model-1" swName="TEST1" swSuppressed="NO"/>
              <swReference swModelRef="part-model-2" swName="TEST2" swSuppressed="YES"/>
            </swModel>
            </swModelList><swConfigurationList>
            <swConfiguration swID="1" swName="Base" swModelRef="root-base"/>
            <swConfiguration swID="2" swName="Derived-Suppressed" swModelRef="root-derived"/>
            </swConfigurationList></root>"#;
        fs::write(
            path,
            modern_file("swXmlContents/COMPINSTANCETREE", xml.as_bytes())?,
        )?;
        Ok(())
    }

    #[test]
    fn nested_and_repeated_references_reuse_one_target_node()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let project = temp.path();
        fs::create_dir(project.join("sub"))?;
        let root = project.join("root.SLDASM");
        write_assembly(
            &root,
            &[
                ComponentSpec {
                    path: "sub/child.SLDASM",
                    kind: "ASSEMBLY",
                    name: "child-1",
                    suppressed: false,
                },
                ComponentSpec {
                    path: "sub/child.SLDASM",
                    kind: "ASSEMBLY",
                    name: "child-2",
                    suppressed: false,
                },
            ],
        )?;
        write_assembly(
            &project.join("sub/child.SLDASM"),
            &[ComponentSpec {
                path: "../leaf.SLDPRT",
                kind: "PART",
                name: "leaf-1",
                suppressed: false,
            }],
        )?;
        write_part(&project.join("leaf.SLDPRT"))?;

        let result = scan_project_path(
            &root,
            &ProjectScanOptions {
                project_root: Some(project.to_path_buf()),
                root_configuration: Some("Default".to_owned()),
                ..ProjectScanOptions::default()
            },
            &ResourceLimits::service(),
        );

        assert_eq!(result.status, ProjectScanStatus::Complete);
        assert_eq!(result.nodes.len(), 3);
        assert_eq!(result.edges.len(), 3);
        assert_eq!(
            result
                .edges
                .iter()
                .filter(|edge| edge.resolved_path.as_deref() == Some("sub/child.SLDASM"))
                .count(),
            2
        );
        assert!(
            result
                .edges
                .iter()
                .any(|edge| { edge.traversal_status == ReferenceTraversalStatus::Reused })
        );
        assert_eq!(result.compatibility_report.scanned_node_count, 3);
        Ok(())
    }

    #[test]
    fn ambiguous_basename_and_windows_literal_are_not_guessed()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let project = temp.path();
        fs::create_dir(project.join("a"))?;
        fs::create_dir(project.join("b"))?;
        let root = project.join("root.SLDASM");
        write_assembly(
            &root,
            &[
                ComponentSpec {
                    path: "child.SLDPRT",
                    kind: "PART",
                    name: "ambiguous-1",
                    suppressed: false,
                },
                ComponentSpec {
                    path: r"C:\vendor\mapped.SLDPRT",
                    kind: "PART",
                    name: "windows-1",
                    suppressed: false,
                },
            ],
        )?;
        write_part(&project.join("a/child.SLDPRT"))?;
        write_part(&project.join("b/child.SLDPRT"))?;
        write_part(&project.join(r"C:\vendor\mapped.SLDPRT"))?;

        let result = scan_project_path(
            &root,
            &ProjectScanOptions {
                project_root: Some(project.to_path_buf()),
                ..ProjectScanOptions::default()
            },
            &ResourceLimits::service(),
        );

        assert_eq!(result.status, ProjectScanStatus::Partial);
        assert_eq!(
            result.edges[0].resolution_status,
            ReferenceResolutionStatus::Ambiguous
        );
        assert_eq!(result.edges[0].candidate_count, 2);
        assert_eq!(
            result.edges[1].resolution_status,
            ReferenceResolutionStatus::Missing
        );
        assert!(result.edges[1].resolved_path.is_none());
        Ok(())
    }

    #[test]
    fn explicit_windows_mapping_can_resolve_outside_project_root()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let project = temp.path().join("project");
        let mapped = temp.path().join("mapped");
        fs::create_dir(&project)?;
        fs::create_dir(&mapped)?;
        let root = project.join("root.SLDASM");
        write_assembly(
            &root,
            &[ComponentSpec {
                path: r"Z:\vendor\child.SLDPRT",
                kind: "PART",
                name: "child-1",
                suppressed: false,
            }],
        )?;
        write_part(&project.join("child.SLDPRT"))?;
        write_part(&mapped.join("child.SLDPRT"))?;

        let result = scan_project_path(
            &root,
            &ProjectScanOptions {
                project_root: Some(project),
                windows_prefix_mappings: vec![WindowsPrefixMapping {
                    source_prefix: r"Z:\vendor".to_owned(),
                    target_directory: mapped,
                }],
                ..ProjectScanOptions::default()
            },
            &ResourceLimits::service(),
        );

        assert_eq!(result.status, ProjectScanStatus::Complete);
        assert_eq!(
            result.edges[0].resolution_basis,
            Some(ReferenceResolutionBasis::WindowsPrefixMapping)
        );
        assert_eq!(
            result.edges[0].resolved_path.as_deref(),
            Some("@mapping/0/child.SLDPRT")
        );
        Ok(())
    }

    #[test]
    fn cycles_suppression_and_depth_limits_are_explicit() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = TempDir::new()?;
        let project = temp.path();
        let first = project.join("first.SLDASM");
        let second = project.join("second.SLDASM");
        write_assembly(
            &first,
            &[ComponentSpec {
                path: "second.SLDASM",
                kind: "ASSEMBLY",
                name: "second-1",
                suppressed: false,
            }],
        )?;
        write_assembly(
            &second,
            &[ComponentSpec {
                path: "first.SLDASM",
                kind: "ASSEMBLY",
                name: "first-1",
                suppressed: false,
            }],
        )?;

        let cycle = scan_project_path(
            &first,
            &ProjectScanOptions {
                project_root: Some(project.to_path_buf()),
                ..ProjectScanOptions::default()
            },
            &ResourceLimits::service(),
        );
        assert_eq!(cycle.status, ProjectScanStatus::Complete);
        assert!(
            cycle
                .edges
                .iter()
                .any(|edge| { edge.traversal_status == ReferenceTraversalStatus::Cycle })
        );
        assert!(
            cycle
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "project.reference_cycle" })
        );

        let mut limited_limits = ResourceLimits::service();
        limited_limits.max_reference_depth = 0;
        let limited = scan_project_path(
            &first,
            &ProjectScanOptions {
                project_root: Some(project.to_path_buf()),
                ..ProjectScanOptions::default()
            },
            &limited_limits,
        );
        assert_eq!(limited.status, ProjectScanStatus::Partial);
        assert_eq!(
            limited.edges[0].traversal_status,
            ReferenceTraversalStatus::DepthLimited
        );
        assert_eq!(limited.compatibility_report.scanned_node_count, 1);

        let suppressed_root = project.join("suppressed.SLDASM");
        write_assembly(
            &suppressed_root,
            &[ComponentSpec {
                path: "second.SLDASM",
                kind: "ASSEMBLY",
                name: "second-1",
                suppressed: true,
            }],
        )?;
        let suppressed = scan_project_path(
            &suppressed_root,
            &ProjectScanOptions {
                project_root: Some(project.to_path_buf()),
                ..ProjectScanOptions::default()
            },
            &ResourceLimits::service(),
        );
        assert_eq!(suppressed.status, ProjectScanStatus::Complete);
        assert_eq!(
            suppressed.edges[0].traversal_status,
            ReferenceTraversalStatus::Suppressed
        );
        assert_eq!(suppressed.compatibility_report.scanned_node_count, 1);
        Ok(())
    }

    #[test]
    fn configuration_selection_is_exact_and_limits_the_graph()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let project = temp.path();
        let root = project.join("root.SLDASM");
        write_configured_assembly(&root)?;
        write_part(&project.join("default.SLDPRT"))?;
        write_part(&project.join("alternate.SLDPRT"))?;

        let selected = scan_project_path(
            &root,
            &ProjectScanOptions {
                project_root: Some(project.to_path_buf()),
                root_configuration: Some("Alternate".to_owned()),
                ..ProjectScanOptions::default()
            },
            &ResourceLimits::service(),
        );
        assert_eq!(selected.status, ProjectScanStatus::Complete);
        assert_eq!(selected.edges.len(), 1);
        assert_eq!(
            selected.edges[0].resolved_path.as_deref(),
            Some("alternate.SLDPRT")
        );
        let root_node = selected
            .nodes
            .iter()
            .find(|node| node.is_root)
            .ok_or("root node missing")?;
        assert_eq!(root_node.selected_configuration_indices, vec![1]);

        let mismatched_case = scan_project_path(
            &root,
            &ProjectScanOptions {
                project_root: Some(project.to_path_buf()),
                root_configuration: Some("alternate".to_owned()),
                ..ProjectScanOptions::default()
            },
            &ResourceLimits::service(),
        );
        assert_eq!(mismatched_case.status, ProjectScanStatus::Partial);
        assert!(mismatched_case.edges.is_empty());
        assert!(
            mismatched_case
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "project.configuration_missing")
        );
        Ok(())
    }

    #[test]
    fn relocated_derived_root_keeps_resolved_and_suppressed_component_edges()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let project = temp.path();
        let root = project.join("TEST2.SLDASM");
        write_relocated_derived_assembly(&root)?;
        write_part(&project.join("TEST1.SLDPRT"))?;
        write_part(&project.join("TEST2.SLDPRT"))?;

        let result = scan_project_path(
            &root,
            &ProjectScanOptions {
                project_root: Some(project.to_path_buf()),
                root_configuration: Some("Derived-Suppressed".to_owned()),
                ..ProjectScanOptions::default()
            },
            &ResourceLimits::service(),
        );

        assert_eq!(result.status, ProjectScanStatus::Complete);
        assert_eq!(result.nodes.len(), 3);
        assert_eq!(result.edges.len(), 2);
        assert_eq!(result.compatibility_report.scanned_node_count, 2);
        assert!(result.edges.iter().any(|edge| {
            edge.resolved_path.as_deref() == Some("TEST1.SLDPRT")
                && edge.traversal_status == ReferenceTraversalStatus::Followed
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.resolved_path.as_deref() == Some("TEST2.SLDPRT")
                && edge.is_suppressed == Some(true)
                && edge.traversal_status == ReferenceTraversalStatus::Suppressed
        }));
        Ok(())
    }

    #[test]
    fn drawing_views_are_graph_edges_and_graph_size_is_bounded()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let project = temp.path();
        let drawing = project.join("drawing.SLDDRW");
        let keywords = r#"<root><Sheet Type="Sheet" Name="Sheet1" id="s1">
            <View Name="Front" Description="Default" id="v1">part.SLDPRT</View>
            </Sheet></root>"#;
        fs::write(
            &drawing,
            modern_file("swXmlContents/KeyWords", keywords.as_bytes())?,
        )?;
        write_part(&project.join("part.SLDPRT"))?;

        let scanned = scan_project_path(
            &drawing,
            &ProjectScanOptions {
                project_root: Some(project.to_path_buf()),
                ..ProjectScanOptions::default()
            },
            &ResourceLimits::service(),
        );
        assert_eq!(scanned.status, ProjectScanStatus::Complete);
        assert_eq!(scanned.edges.len(), 1);
        assert_eq!(
            scanned.edges[0].kind,
            sldkit_core::ReferenceKind::DrawingView
        );
        assert_eq!(
            scanned.edges[0].resolved_path.as_deref(),
            Some("part.SLDPRT")
        );

        let mut limits = ResourceLimits::service();
        limits.max_project_edges = 0;
        let rejected = scan_project_path(
            &drawing,
            &ProjectScanOptions {
                project_root: Some(project.to_path_buf()),
                ..ProjectScanOptions::default()
            },
            &limits,
        );
        assert_eq!(rejected.status, ProjectScanStatus::Rejected);
        assert!(rejected.edges.is_empty());
        assert!(
            rejected
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "limit.project_edges")
        );
        Ok(())
    }
}
