use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Diagnostic, DocumentKind, ParseStatus, ReferenceKind};

/// Completeness of project-reference resolution, independent from parse coverage.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectScanStatus {
    Complete,
    Partial,
    Rejected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceResolutionStatus {
    Resolved,
    Missing,
    Ambiguous,
    NoStoredPath,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceResolutionBasis {
    DocumentRelative,
    DocumentBasename,
    ProjectRootRelative,
    ProjectRootBasename,
    WindowsPrefixMapping,
    SearchDirectoryRelative,
    SearchDirectoryBasename,
    HostAbsolute,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceTraversalStatus {
    Followed,
    Reused,
    Suppressed,
    Cycle,
    DepthLimited,
    Unresolved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectConfigurationIdentity {
    pub index: i64,
    pub name: Option<String>,
}

/// One physical document in a project graph.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectNode {
    pub id: String,
    /// Stable label relative to the project or an explicitly configured search root.
    pub path: String,
    pub is_root: bool,
    pub minimum_depth: u32,
    /// Absent when the target was resolved but intentionally not traversed.
    pub parse_status: Option<ParseStatus>,
    pub document_kind: Option<DocumentKind>,
    pub byte_len: Option<u64>,
    pub source_sha256: Option<String>,
    #[serde(default)]
    pub available_configurations: Vec<ProjectConfigurationIdentity>,
    #[serde(default)]
    pub selected_configuration_indices: Vec<i64>,
    #[serde(default)]
    pub requested_configurations: Vec<String>,
    #[serde(default)]
    pub diagnostic_codes: Vec<String>,
}

/// One source occurrence or view reference. Repeated occurrences remain separate edges.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectEdge {
    pub id: String,
    pub source_node_id: String,
    pub reference_index: u64,
    pub kind: ReferenceKind,
    pub source_configuration_index: Option<i64>,
    pub source_name: Option<String>,
    pub stored_path: Option<String>,
    pub referenced_configuration: Option<String>,
    pub expected_document_kind: Option<DocumentKind>,
    pub is_suppressed: Option<bool>,
    pub is_hidden: Option<bool>,
    pub exclude_from_bom: Option<bool>,
    pub resolution_status: ReferenceResolutionStatus,
    pub resolution_basis: Option<ReferenceResolutionBasis>,
    pub candidate_count: u64,
    #[serde(default)]
    pub candidate_paths: Vec<String>,
    pub resolved_path: Option<String>,
    pub target_node_id: Option<String>,
    pub traversal_status: ReferenceTraversalStatus,
}

/// Path-free aggregate suitable for sharing as a compatibility report.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectCompatibilityReport {
    pub schema_version: u32,
    pub status: ProjectScanStatus,
    pub node_count: u64,
    pub scanned_node_count: u64,
    pub edge_count: u64,
    #[serde(default)]
    pub document_kind_counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub parse_status_counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub resolution_status_counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub traversal_status_counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub diagnostic_code_counts: BTreeMap<String, u64>,
}

impl ProjectCompatibilityReport {
    #[must_use]
    pub fn from_graph(
        status: ProjectScanStatus,
        nodes: &[ProjectNode],
        edges: &[ProjectEdge],
        diagnostics: &[Diagnostic],
    ) -> Self {
        let mut document_kind_counts = BTreeMap::new();
        let mut parse_status_counts = BTreeMap::new();
        let mut resolution_status_counts = BTreeMap::new();
        let mut traversal_status_counts = BTreeMap::new();
        let mut diagnostic_code_counts = BTreeMap::new();

        for node in nodes {
            if let Some(kind) = node.document_kind {
                increment(&mut document_kind_counts, document_kind_name(kind));
            }
            if let Some(parse_status) = node.parse_status {
                increment(&mut parse_status_counts, parse_status_name(parse_status));
            }
            for code in &node.diagnostic_codes {
                increment(&mut diagnostic_code_counts, code);
            }
        }
        for edge in edges {
            increment(
                &mut resolution_status_counts,
                resolution_status_name(edge.resolution_status),
            );
            increment(
                &mut traversal_status_counts,
                traversal_status_name(edge.traversal_status),
            );
        }
        for diagnostic in diagnostics {
            increment(&mut diagnostic_code_counts, &diagnostic.code);
        }

        Self {
            schema_version: 1,
            status,
            node_count: u64::try_from(nodes.len()).unwrap_or(u64::MAX),
            scanned_node_count: u64::try_from(
                nodes
                    .iter()
                    .filter(|node| node.parse_status.is_some())
                    .count(),
            )
            .unwrap_or(u64::MAX),
            edge_count: u64::try_from(edges.len()).unwrap_or(u64::MAX),
            document_kind_counts,
            parse_status_counts,
            resolution_status_counts,
            traversal_status_counts,
            diagnostic_code_counts,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectScanResult {
    pub schema_version: u32,
    pub status: ProjectScanStatus,
    pub root_node_id: Option<String>,
    #[serde(default)]
    pub nodes: Vec<ProjectNode>,
    #[serde(default)]
    pub edges: Vec<ProjectEdge>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    pub compatibility_report: ProjectCompatibilityReport,
}

fn increment(counts: &mut BTreeMap<String, u64>, key: &str) {
    let count = counts.entry(key.to_owned()).or_default();
    *count = count.saturating_add(1);
}

const fn document_kind_name(value: DocumentKind) -> &'static str {
    match value {
        DocumentKind::Part => "part",
        DocumentKind::Assembly => "assembly",
        DocumentKind::Drawing => "drawing",
        DocumentKind::Unknown => "unknown",
    }
}

const fn parse_status_name(value: ParseStatus) -> &'static str {
    match value {
        ParseStatus::Parsed => "parsed",
        ParseStatus::Partial => "partial",
        ParseStatus::Unsupported => "unsupported",
        ParseStatus::Malformed => "malformed",
        ParseStatus::Rejected => "rejected",
    }
}

const fn resolution_status_name(value: ReferenceResolutionStatus) -> &'static str {
    match value {
        ReferenceResolutionStatus::Resolved => "resolved",
        ReferenceResolutionStatus::Missing => "missing",
        ReferenceResolutionStatus::Ambiguous => "ambiguous",
        ReferenceResolutionStatus::NoStoredPath => "no_stored_path",
    }
}

const fn traversal_status_name(value: ReferenceTraversalStatus) -> &'static str {
    match value {
        ReferenceTraversalStatus::Followed => "followed",
        ReferenceTraversalStatus::Reused => "reused",
        ReferenceTraversalStatus::Suppressed => "suppressed",
        ReferenceTraversalStatus::Cycle => "cycle",
        ReferenceTraversalStatus::DepthLimited => "depth_limited",
        ReferenceTraversalStatus::Unresolved => "unresolved",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectCompatibilityReport, ProjectEdge, ProjectNode, ProjectScanStatus,
        ReferenceResolutionStatus, ReferenceTraversalStatus,
    };

    #[test]
    fn compatibility_report_contains_no_paths() -> Result<(), serde_json::Error> {
        let nodes = vec![ProjectNode {
            id: "node:root.SLDASM".to_owned(),
            path: "root.SLDASM".to_owned(),
            is_root: true,
            minimum_depth: 0,
            parse_status: None,
            document_kind: None,
            byte_len: None,
            source_sha256: None,
            available_configurations: Vec::new(),
            selected_configuration_indices: Vec::new(),
            requested_configurations: Vec::new(),
            diagnostic_codes: Vec::new(),
        }];
        let edges = vec![ProjectEdge {
            id: "edge:00000000".to_owned(),
            source_node_id: "node:root.SLDASM".to_owned(),
            reference_index: 0,
            kind: crate::ReferenceKind::AssemblyComponent,
            source_configuration_index: None,
            source_name: None,
            stored_path: Some(r"C:\private\child.SLDPRT".to_owned()),
            referenced_configuration: None,
            expected_document_kind: None,
            is_suppressed: None,
            is_hidden: None,
            exclude_from_bom: None,
            resolution_status: ReferenceResolutionStatus::Missing,
            resolution_basis: None,
            candidate_count: 0,
            candidate_paths: Vec::new(),
            resolved_path: None,
            target_node_id: None,
            traversal_status: ReferenceTraversalStatus::Unresolved,
        }];
        let report =
            ProjectCompatibilityReport::from_graph(ProjectScanStatus::Partial, &nodes, &edges, &[]);
        let json = serde_json::to_string(&report)?;
        assert!(!json.contains("private"));
        assert_eq!(report.resolution_status_counts.get("missing"), Some(&1));
        Ok(())
    }
}
