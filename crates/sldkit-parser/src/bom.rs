//! Occurrence expansion and quantity aggregation over a scanned project graph.
//!
//! The bill of materials is derived from `ProjectScanResult` edges only. It
//! never re-reads documents, never guesses a configuration, and keeps
//! suppressed, excluded, unresolved, and unknown states distinct.

use std::collections::BTreeMap;

use sldkit_core::{
    BillOfMaterials, BomChildrenState, BomItem, BomItemKey, BomOccurrence, BomOptions, BomStatus,
    BomSummary, Diagnostic, DiagnosticKind, DiagnosticSeverity, DocumentKind, ProjectEdge,
    ProjectNode, ProjectScanResult, ProjectScanStatus, ReferenceKind, ReferenceTraversalStatus,
};

struct Expansion<'a> {
    nodes: BTreeMap<&'a str, &'a ProjectNode>,
    edges_by_source: BTreeMap<(&'a str, i64), Vec<&'a ProjectEdge>>,
    options: &'a BomOptions,
    occurrences: Vec<BomOccurrence>,
    diagnostics: Vec<Diagnostic>,
    partial: bool,
    limit_reached: bool,
}

struct Pending {
    occurrence_index: usize,
    node_id: String,
    configuration_index: i64,
    depth: u32,
    suppressed_chain: bool,
    excluded_chain: bool,
}

/// Expand the root occurrence tree of a scanned graph and aggregate quantities.
#[must_use]
pub fn build_bill_of_materials(graph: &ProjectScanResult, options: &BomOptions) -> BillOfMaterials {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let Some(root) = graph
        .root_node_id
        .as_deref()
        .and_then(|id| nodes.get(id).copied())
    else {
        return rejected(
            options,
            vec![Diagnostic::new(
                "bom.root_missing",
                DiagnosticSeverity::Error,
                DiagnosticKind::Fatal,
                "the project graph has no root node to expand",
            )],
        );
    };
    let mut diagnostics = Vec::new();
    if graph.status == ProjectScanStatus::Rejected {
        diagnostics.push(Diagnostic::new(
            "bom.graph_rejected",
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            "the project graph was rejected; its edges cannot be expanded",
        ));
        return rejected(options, diagnostics);
    }
    let Some((root_configuration_index, root_configuration)) = select_root_configuration(
        root,
        options.root_configuration.as_deref(),
        &mut diagnostics,
    ) else {
        return rejected(options, diagnostics);
    };

    let mut expansion = Expansion {
        nodes,
        edges_by_source: component_edges(graph),
        options,
        occurrences: Vec::new(),
        diagnostics,
        partial: false,
        limit_reached: false,
    };
    expansion.expand_root(root, root_configuration_index, root_configuration.clone());
    expansion.finish(root, root_configuration_index, root_configuration)
}

fn component_edges(graph: &ProjectScanResult) -> BTreeMap<(&str, i64), Vec<&ProjectEdge>> {
    let mut edges_by_source: BTreeMap<(&str, i64), Vec<&ProjectEdge>> = BTreeMap::new();
    for edge in &graph.edges {
        if edge.kind != ReferenceKind::AssemblyComponent {
            continue;
        }
        let Some(configuration_index) = edge.source_configuration_index else {
            continue;
        };
        edges_by_source
            .entry((edge.source_node_id.as_str(), configuration_index))
            .or_default()
            .push(edge);
    }
    edges_by_source
}

impl Expansion<'_> {
    fn expand_root(
        &mut self,
        root: &ProjectNode,
        root_configuration_index: i64,
        root_configuration: Option<String>,
    ) {
        self.occurrences.push(BomOccurrence {
            id: occurrence_id(0),
            parent_id: None,
            edge_id: None,
            node_id: Some(root.id.clone()),
            depth: 0,
            instance_name: None,
            instance_number: None,
            stored_path: None,
            document_kind: root.document_kind,
            referenced_configuration: root_configuration,
            configuration_index: Some(root_configuration_index),
            is_suppressed: None,
            is_hidden: None,
            exclude_from_bom: None,
            is_virtual: None,
            resolution_status: None,
            traversal_status: None,
            children_state: BomChildrenState::Expanded,
            counted: true,
        });
        self.expand(Pending {
            occurrence_index: 0,
            node_id: root.id.clone(),
            configuration_index: root_configuration_index,
            depth: 0,
            suppressed_chain: false,
            excluded_chain: false,
        });
    }

    fn finish(
        mut self,
        root: &ProjectNode,
        root_configuration_index: i64,
        root_configuration: Option<String>,
    ) -> BillOfMaterials {
        if self.limit_reached {
            self.diagnostics.push(
                Diagnostic::new(
                    "limit.bom_occurrences",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Unresolved,
                    "occurrence expansion stopped at the configured occurrence limit",
                )
                .with_detail("limit", self.options.max_occurrences.to_string()),
            );
        }
        let items = aggregate_items(&self.occurrences, &self.nodes);
        let summary = summarize(&self.occurrences, &items);
        let unresolved = self.occurrences.iter().any(|occurrence| {
            matches!(
                occurrence.children_state,
                BomChildrenState::Unresolved
                    | BomChildrenState::ConfigurationUnresolved
                    | BomChildrenState::NotTraversed
                    | BomChildrenState::LimitReached
            )
        });
        BillOfMaterials {
            schema_version: 1,
            status: if self.partial || unresolved || self.limit_reached {
                BomStatus::Partial
            } else {
                BomStatus::Complete
            },
            root_node_id: Some(root.id.clone()),
            root_configuration,
            root_configuration_index: Some(root_configuration_index),
            options: self.options.clone(),
            occurrences: self.occurrences,
            items,
            summary,
            diagnostics: self.diagnostics,
        }
    }

    fn expand(&mut self, pending: Pending) {
        let mut stack = vec![pending];
        while let Some(current) = stack.pop() {
            let edges = self
                .edges_by_source
                .get(&(current.node_id.as_str(), current.configuration_index))
                .cloned()
                .unwrap_or_default();
            let mut children = Vec::new();
            for edge in edges {
                if u64::try_from(self.occurrences.len()).unwrap_or(u64::MAX)
                    >= self.options.max_occurrences
                {
                    self.limit_reached = true;
                    self.partial = true;
                    self.occurrences[current.occurrence_index].children_state =
                        BomChildrenState::LimitReached;
                    return;
                }
                let child = self.occurrence_for_edge(edge, &current);
                let child_index = self.occurrences.len();
                if let Some(next) = child.1 {
                    children.push(Pending {
                        occurrence_index: child_index,
                        node_id: next.0,
                        configuration_index: next.1,
                        depth: current.depth.saturating_add(1),
                        suppressed_chain: current.suppressed_chain
                            || (child.0.is_suppressed == Some(true)
                                && !self.options.include_suppressed),
                        excluded_chain: current.excluded_chain
                            || (child.0.exclude_from_bom == Some(true)
                                && !self.options.include_excluded),
                    });
                }
                self.occurrences.push(child.0);
            }
            // Keep source order: push in reverse so the first child expands first.
            children.reverse();
            stack.extend(children);
        }
    }

    /// Build one occurrence and, when it can be expanded, the next pending state.
    fn occurrence_for_edge(
        &mut self,
        edge: &ProjectEdge,
        parent: &Pending,
    ) -> (BomOccurrence, Option<(String, i64)>) {
        let suppressed = edge.is_suppressed == Some(true) && !self.options.include_suppressed;
        let excluded = edge.exclude_from_bom == Some(true) && !self.options.include_excluded;
        let counted = !(parent.suppressed_chain || parent.excluded_chain || suppressed || excluded);
        let target = edge
            .target_node_id
            .as_deref()
            .and_then(|id| self.nodes.get(id).copied());
        let mut next = None;
        let children_state = match target {
            None => BomChildrenState::Unresolved,
            Some(target) if edge.traversal_status == ReferenceTraversalStatus::Suppressed => {
                if !self.options.include_suppressed {
                    BomChildrenState::Suppressed
                } else if target.document_kind != Some(DocumentKind::Assembly) {
                    BomChildrenState::Leaf
                } else {
                    self.diagnostics.push(
                        Diagnostic::new(
                            "bom.suppressed_not_traversed",
                            DiagnosticSeverity::Warning,
                            DiagnosticKind::Unresolved,
                            "a suppressed subassembly was not traversed by the scan; rerun the scan with follow_suppressed to expand it",
                        )
                        .with_detail("edge_id", &edge.id),
                    );
                    BomChildrenState::NotTraversed
                }
            }
            Some(target) if target.document_kind != Some(DocumentKind::Assembly) => {
                BomChildrenState::Leaf
            }
            Some(target)
                if matches!(
                    edge.traversal_status,
                    ReferenceTraversalStatus::Followed | ReferenceTraversalStatus::Reused
                ) =>
            {
                match self.target_configuration(edge, target) {
                    Some(configuration_index) => {
                        next = Some((target.id.clone(), configuration_index));
                        BomChildrenState::Expanded
                    }
                    None => BomChildrenState::ConfigurationUnresolved,
                }
            }
            Some(_) => BomChildrenState::NotTraversed,
        };
        let occurrence = BomOccurrence {
            id: occurrence_id(self.occurrences.len()),
            parent_id: Some(occurrence_id(parent.occurrence_index)),
            edge_id: Some(edge.id.clone()),
            node_id: edge.target_node_id.clone(),
            depth: parent.depth.saturating_add(1),
            instance_name: edge.source_name.clone(),
            instance_number: edge.instance_number,
            stored_path: edge.stored_path.clone(),
            document_kind: target.and_then(|node| node.document_kind),
            referenced_configuration: edge.referenced_configuration.clone(),
            configuration_index: next.as_ref().map(|(_, index)| *index),
            is_suppressed: edge.is_suppressed,
            is_hidden: edge.is_hidden,
            exclude_from_bom: edge.exclude_from_bom,
            is_virtual: edge.is_virtual,
            resolution_status: Some(edge.resolution_status),
            traversal_status: Some(edge.traversal_status),
            children_state,
            counted,
        };
        (occurrence, next)
    }

    /// Resolve the configuration an occurrence instantiates on its target node.
    fn target_configuration(&mut self, edge: &ProjectEdge, target: &ProjectNode) -> Option<i64> {
        let matches = match edge.referenced_configuration.as_deref() {
            Some(name) => target
                .available_configurations
                .iter()
                .filter(|configuration| configuration.name.as_deref() == Some(name))
                .map(|configuration| configuration.index)
                .collect::<Vec<_>>(),
            None => target
                .available_configurations
                .iter()
                .map(|configuration| configuration.index)
                .collect::<Vec<_>>(),
        };
        if matches.len() != 1 {
            self.partial = true;
            self.diagnostics.push(
                Diagnostic::new(
                    "bom.configuration_unresolved",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Unresolved,
                    "the referenced configuration did not match exactly one configuration of the target document",
                )
                .with_detail("edge_id", &edge.id)
                .with_detail("target_node_id", &target.id)
                .with_detail(
                    "referenced_configuration",
                    edge.referenced_configuration.clone().unwrap_or_default(),
                )
                .with_detail("candidate_count", matches.len().to_string()),
            );
            return None;
        }
        let index = matches[0];
        if edge.referenced_configuration.is_none() {
            self.diagnostics.push(
                Diagnostic::new(
                    "bom.configuration_defaulted",
                    DiagnosticSeverity::Info,
                    DiagnosticKind::Inferred,
                    "the occurrence stores no configuration name; the target's only configuration was used",
                )
                .with_detail("edge_id", &edge.id)
                .with_detail("target_node_id", &target.id),
            );
        }
        if !target.selected_configuration_indices.contains(&index) {
            self.partial = true;
            self.diagnostics.push(
                Diagnostic::new(
                    "bom.configuration_not_scanned",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Unresolved,
                    "the scan did not process the target configuration; its components are unavailable",
                )
                .with_detail("edge_id", &edge.id)
                .with_detail("target_node_id", &target.id)
                .with_detail("configuration_index", index.to_string()),
            );
            return None;
        }
        Some(index)
    }
}

fn select_root_configuration(
    root: &ProjectNode,
    requested: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(i64, Option<String>)> {
    let candidates = match requested {
        Some(name) => root
            .available_configurations
            .iter()
            .filter(|configuration| configuration.name.as_deref() == Some(name))
            .collect::<Vec<_>>(),
        None if root.selected_configuration_indices.len() == 1 => root
            .available_configurations
            .iter()
            .filter(|configuration| configuration.index == root.selected_configuration_indices[0])
            .collect::<Vec<_>>(),
        None => root.available_configurations.iter().collect::<Vec<_>>(),
    };
    if candidates.len() == 1 {
        return Some((candidates[0].index, candidates[0].name.clone()));
    }
    let (code, message) = match (requested, candidates.is_empty()) {
        (Some(_), true) => (
            "bom.root_configuration_missing",
            "the requested root configuration is not present on the root node",
        ),
        (Some(_), false) => (
            "bom.root_configuration_ambiguous",
            "the requested root configuration matched multiple configuration identities",
        ),
        (None, true) => (
            "bom.root_configuration_unavailable",
            "the root node exposes no configuration identity",
        ),
        (None, false) => (
            "bom.root_configuration_required",
            "the root has several configurations; pass an exact root configuration name",
        ),
    };
    diagnostics.push(
        Diagnostic::new(
            code,
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            message,
        )
        .with_detail("root_node_id", &root.id)
        .with_detail("requested_configuration", requested.unwrap_or_default())
        .with_detail("candidate_count", candidates.len().to_string()),
    );
    None
}

/// `(node id, folded stored basename, configuration name)`.
type ItemKey = (Option<String>, Option<String>, Option<String>);

fn aggregate_items(
    occurrences: &[BomOccurrence],
    nodes: &BTreeMap<&str, &ProjectNode>,
) -> Vec<BomItem> {
    let mut items: BTreeMap<ItemKey, BomItem> = BTreeMap::new();
    for occurrence in occurrences.iter().filter(|item| item.parent_id.is_some()) {
        let node = occurrence
            .node_id
            .as_deref()
            .and_then(|id| nodes.get(id).copied());
        let stored_basename = if node.is_none() {
            occurrence
                .stored_path
                .as_deref()
                .map(basename_of)
                .map(str::to_lowercase)
        } else {
            None
        };
        let key = (
            node.map(|node| node.id.clone()),
            stored_basename.clone(),
            occurrence.referenced_configuration.clone(),
        );
        let item = items.entry(key).or_insert_with(|| BomItem {
            key: BomItemKey {
                node_id: node.map(|node| node.id.clone()),
                stored_basename,
                configuration: occurrence.referenced_configuration.clone(),
            },
            document_path: node.map(|node| node.path.clone()),
            document_kind: node.and_then(|node| node.document_kind),
            display_name: node.map_or_else(
                || {
                    occurrence
                        .stored_path
                        .as_deref()
                        .map(|path| basename_of(path).to_owned())
                },
                |node| Some(basename_of(&node.path).to_owned()),
            ),
            quantity: 0,
            suppressed_quantity: 0,
            excluded_quantity: 0,
            unknown_state_quantity: 0,
            minimum_depth: occurrence.depth,
            is_virtual: occurrence.is_virtual,
            occurrence_ids: Vec::new(),
        });
        if occurrence.counted {
            item.quantity = item.quantity.saturating_add(1);
        } else if occurrence.is_suppressed == Some(true)
            || ancestor_suppressed(occurrences, occurrence)
        {
            item.suppressed_quantity = item.suppressed_quantity.saturating_add(1);
        } else {
            item.excluded_quantity = item.excluded_quantity.saturating_add(1);
        }
        if occurrence.is_suppressed.is_none() || occurrence.exclude_from_bom.is_none() {
            item.unknown_state_quantity = item.unknown_state_quantity.saturating_add(1);
        }
        item.minimum_depth = item.minimum_depth.min(occurrence.depth);
        if item.is_virtual != occurrence.is_virtual {
            item.is_virtual = None;
        }
        item.occurrence_ids.push(occurrence.id.clone());
    }
    let mut output = items.into_values().collect::<Vec<_>>();
    output.sort_by(|left, right| {
        (
            left.document_path.as_deref(),
            left.key.stored_basename.as_deref(),
            left.key.configuration.as_deref(),
        )
            .cmp(&(
                right.document_path.as_deref(),
                right.key.stored_basename.as_deref(),
                right.key.configuration.as_deref(),
            ))
    });
    output
}

fn ancestor_suppressed(occurrences: &[BomOccurrence], occurrence: &BomOccurrence) -> bool {
    let mut parent_id = occurrence.parent_id.as_deref();
    while let Some(id) = parent_id {
        let Some(parent) = occurrences.iter().find(|item| item.id == id) else {
            return false;
        };
        if parent.is_suppressed == Some(true) {
            return true;
        }
        parent_id = parent.parent_id.as_deref();
    }
    false
}

fn summarize(occurrences: &[BomOccurrence], items: &[BomItem]) -> BomSummary {
    let mut children_state_counts = BTreeMap::new();
    for occurrence in occurrences {
        let count = children_state_counts
            .entry(children_state_name(occurrence.children_state).to_owned())
            .or_insert(0_u64);
        *count = count.saturating_add(1);
    }
    BomSummary {
        occurrence_count: u64::try_from(occurrences.len()).unwrap_or(u64::MAX),
        item_count: u64::try_from(items.len()).unwrap_or(u64::MAX),
        part_item_count: u64::try_from(
            items
                .iter()
                .filter(|item| item.document_kind == Some(DocumentKind::Part))
                .count(),
        )
        .unwrap_or(u64::MAX),
        assembly_item_count: u64::try_from(
            items
                .iter()
                .filter(|item| item.document_kind == Some(DocumentKind::Assembly))
                .count(),
        )
        .unwrap_or(u64::MAX),
        unresolved_item_count: u64::try_from(
            items
                .iter()
                .filter(|item| item.key.node_id.is_none())
                .count(),
        )
        .unwrap_or(u64::MAX),
        children_state_counts,
    }
}

fn rejected(options: &BomOptions, diagnostics: Vec<Diagnostic>) -> BillOfMaterials {
    BillOfMaterials {
        schema_version: 1,
        status: BomStatus::Rejected,
        root_node_id: None,
        root_configuration: options.root_configuration.clone(),
        root_configuration_index: None,
        options: options.clone(),
        occurrences: Vec::new(),
        items: Vec::new(),
        summary: BomSummary::default(),
        diagnostics,
    }
}

fn occurrence_id(index: usize) -> String {
    format!("occ:{index:08}")
}

fn basename_of(path: &str) -> &str {
    path.rsplit(['\\', '/'])
        .find(|component| !component.is_empty())
        .unwrap_or(path)
}

const fn children_state_name(state: BomChildrenState) -> &'static str {
    match state {
        BomChildrenState::Expanded => "expanded",
        BomChildrenState::Leaf => "leaf",
        BomChildrenState::Suppressed => "suppressed",
        BomChildrenState::Unresolved => "unresolved",
        BomChildrenState::ConfigurationUnresolved => "configuration_unresolved",
        BomChildrenState::NotTraversed => "not_traversed",
        BomChildrenState::LimitReached => "limit_reached",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use sldkit_core::{BomChildrenState, BomOptions, BomStatus, DocumentKind, ResourceLimits};
    use tempfile::TempDir;

    use super::build_bill_of_materials;
    use crate::{ProjectScanOptions, scan_project_path, tests::modern_file};

    const ROOT_XML: &str = r#"<root><swHeader><swFile id="0" swDocType="ASSEMBLY"/>
        <swFile id="1" swDocType="ASSEMBLY" swPath="sub.SLDASM"/>
        <swFile id="2" swDocType="PART" swPath="leaf.SLDPRT"/>
        <swFile id="3" swDocType="PART" swPath="missing.SLDPRT"/>
        <swFile id="4" swDocType="PART" swPath="excluded.SLDPRT"/>
        </swHeader><swModelList>
        <swModel id="self" swFileRef="0" swConfigurationId="0">
        <swConfiguration swID="0" swName="Default"/>
        <swReference swModelRef="m1" swName="sub-1" swReferenceNumber="1" swSuppressed="NO" swExcludeFromBOM="NO"/>
        <swReference swModelRef="m1" swName="sub-2" swReferenceNumber="2" swSuppressed="NO" swExcludeFromBOM="NO"/>
        <swReference swModelRef="m2" swName="leaf-1" swReferenceNumber="1" swSuppressed="YES" swExcludeFromBOM="NO"/>
        <swReference swModelRef="m3" swName="missing-1" swReferenceNumber="1" swSuppressed="NO" swExcludeFromBOM="NO"/>
        <swReference swModelRef="m4" swName="excluded-1" swReferenceNumber="1" swSuppressed="NO" swExcludeFromBOM="YES"/>
        </swModel>
        <swModel id="alt" swFileRef="0" swConfigurationId="1">
        <swConfiguration swID="1" swName="Alternate"/>
        <swReference swModelRef="m2" swName="leaf-1" swReferenceNumber="1" swSuppressed="NO"/>
        </swModel>
        <swModel id="m1" swFileRef="1" swConfigurationName="Default"/>
        <swModel id="m2" swFileRef="2" swConfigurationName="Default"/>
        <swModel id="m3" swFileRef="3" swConfigurationName="Default"/>
        <swModel id="m4" swFileRef="4" swConfigurationName="Default"/>
        </swModelList></root>"#;

    const SUB_XML: &str = r#"<root><swHeader><swFile id="0" swDocType="ASSEMBLY"/>
        <swFile id="1" swDocType="PART" swPath="leaf.SLDPRT"/>
        <swFile id="2" swDocType="PART" swPath="bolt.SLDPRT"/>
        </swHeader><swModelList>
        <swModel id="self" swFileRef="0" swConfigurationId="0">
        <swConfiguration swID="0" swName="Default"/>
        <swReference swModelRef="m1" swName="leaf-1" swReferenceNumber="1" swSuppressed="NO" swExcludeFromBOM="NO"/>
        <swReference swModelRef="m2" swName="bolt-1" swReferenceNumber="1" swSuppressed="NO" swExcludeFromBOM="NO"/>
        <swReference swModelRef="m2" swName="bolt-2" swReferenceNumber="2" swSuppressed="NO" swExcludeFromBOM="NO"/>
        </swModel>
        <swModel id="m1" swFileRef="1" swConfigurationName="Default"/>
        <swModel id="m2" swFileRef="2" swConfigurationName="Default"/>
        </swModelList></root>"#;

    const PART_XML: &str = r#"<root><swHeader><swFile id="0" swDocType="PART"/></swHeader><swModelList><swModel id="self" swFileRef="0" swConfigurationId="0"><swConfiguration swID="0" swName="Default"/></swModel></swModelList></root>"#;

    fn write_project(project: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(
            project.join("root.SLDASM"),
            modern_file(&[("swXmlContents/COMPINSTANCETREE", ROOT_XML.as_bytes())])?,
        )?;
        fs::write(
            project.join("sub.SLDASM"),
            modern_file(&[("swXmlContents/COMPINSTANCETREE", SUB_XML.as_bytes())])?,
        )?;
        for part in ["leaf.SLDPRT", "bolt.SLDPRT", "excluded.SLDPRT"] {
            fs::write(
                project.join(part),
                modern_file(&[("swXmlContents/Features", PART_XML.as_bytes())])?,
            )?;
        }
        Ok(())
    }

    #[test]
    fn quantities_multiply_through_subassemblies_and_keep_states_distinct()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        write_project(temp.path())?;
        let graph = scan_project_path(
            temp.path().join("root.SLDASM"),
            &ProjectScanOptions {
                project_root: Some(temp.path().to_path_buf()),
                root_configuration: Some("Default".to_owned()),
                ..ProjectScanOptions::default()
            },
            &ResourceLimits::service(),
        );
        let bom = build_bill_of_materials(&graph, &BomOptions::default());

        assert_eq!(bom.status, BomStatus::Partial);
        assert_eq!(bom.root_configuration.as_deref(), Some("Default"));
        assert_eq!(bom.summary.occurrence_count, 12);
        assert_eq!(bom.summary.unresolved_item_count, 1);
        let item = |name: &str| {
            bom.items
                .iter()
                .find(|item| item.display_name.as_deref() == Some(name))
                .ok_or_else(|| format!("item {name} missing"))
        };
        let sub = item("sub.SLDASM")?;
        assert_eq!(
            (sub.quantity, sub.document_kind),
            (2, Some(DocumentKind::Assembly))
        );
        let bolt = item("bolt.SLDPRT")?;
        assert_eq!((bolt.quantity, bolt.minimum_depth), (4, 2));
        let leaf = item("leaf.SLDPRT")?;
        assert_eq!((leaf.quantity, leaf.suppressed_quantity), (2, 1));
        let excluded = item("excluded.SLDPRT")?;
        assert_eq!((excluded.quantity, excluded.excluded_quantity), (0, 1));
        let missing = item("missing.SLDPRT")?;
        assert_eq!(missing.key.node_id, None);
        assert_eq!(
            missing.key.stored_basename.as_deref(),
            Some("missing.sldprt")
        );
        assert_eq!(missing.quantity, 1);
        assert!(
            bom.occurrences
                .iter()
                .any(|occurrence| occurrence.children_state == BomChildrenState::Unresolved)
        );
        assert!(
            bom.occurrences
                .iter()
                .any(
                    |occurrence| occurrence.children_state == BomChildrenState::Suppressed
                        || (occurrence.is_suppressed == Some(true) && !occurrence.counted)
                )
        );
        let occurrence_ids = bom
            .occurrences
            .iter()
            .map(|occurrence| occurrence.id.as_str())
            .collect::<Vec<_>>();
        let mut sorted = occurrence_ids.clone();
        sorted.sort_unstable();
        assert_eq!(
            occurrence_ids, sorted,
            "occurrence ids are emitted in order"
        );

        let inclusive = build_bill_of_materials(
            &graph,
            &BomOptions {
                include_suppressed: true,
                include_excluded: true,
                ..BomOptions::default()
            },
        );
        let leaf = inclusive
            .items
            .iter()
            .find(|item| item.display_name.as_deref() == Some("leaf.SLDPRT"))
            .ok_or("leaf missing")?;
        assert_eq!((leaf.quantity, leaf.suppressed_quantity), (3, 0));
        Ok(())
    }

    #[test]
    fn root_configuration_is_never_guessed_and_limits_are_explicit()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        write_project(temp.path())?;
        let union = scan_project_path(
            temp.path().join("root.SLDASM"),
            &ProjectScanOptions {
                project_root: Some(temp.path().to_path_buf()),
                ..ProjectScanOptions::default()
            },
            &ResourceLimits::service(),
        );
        let rejected = build_bill_of_materials(&union, &BomOptions::default());
        assert_eq!(rejected.status, BomStatus::Rejected);
        assert!(
            rejected
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "bom.root_configuration_required")
        );

        let alternate = build_bill_of_materials(
            &union,
            &BomOptions {
                root_configuration: Some("Alternate".to_owned()),
                ..BomOptions::default()
            },
        );
        assert_eq!(alternate.status, BomStatus::Complete);
        assert_eq!(alternate.summary.occurrence_count, 2);
        assert_eq!(
            alternate.items[0].display_name.as_deref(),
            Some("leaf.SLDPRT")
        );

        let unknown = build_bill_of_materials(
            &union,
            &BomOptions {
                root_configuration: Some("Nope".to_owned()),
                ..BomOptions::default()
            },
        );
        assert_eq!(unknown.status, BomStatus::Rejected);

        let limited = build_bill_of_materials(
            &union,
            &BomOptions {
                root_configuration: Some("Default".to_owned()),
                max_occurrences: 3,
                ..BomOptions::default()
            },
        );
        assert_eq!(limited.status, BomStatus::Partial);
        assert_eq!(limited.summary.occurrence_count, 3);
        assert!(
            limited
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "limit.bom_occurrences")
        );
        Ok(())
    }
}
