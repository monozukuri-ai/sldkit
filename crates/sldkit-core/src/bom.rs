use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Diagnostic, DocumentKind, ReferenceResolutionStatus, ReferenceTraversalStatus};

/// Completeness of an occurrence expansion over a scanned project graph.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BomStatus {
    Complete,
    Partial,
    Rejected,
}

/// Why an occurrence has no expanded children.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BomChildrenState {
    /// Children were expanded from the target document's selected configuration.
    Expanded,
    /// The target is not an assembly or has no component references.
    Leaf,
    /// The occurrence is suppressed in its parent configuration.
    Suppressed,
    /// The stored reference did not resolve to a scanned node.
    Unresolved,
    /// The referenced configuration name did not match exactly one configuration.
    ConfigurationUnresolved,
    /// The scan did not traverse the target (depth limit, cycle, or not followed).
    NotTraversed,
    /// The occurrence limit stopped the expansion.
    LimitReached,
}

/// Options controlling occurrence expansion and quantity aggregation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BomOptions {
    /// Exact root configuration name. Required when the root has several.
    pub root_configuration: Option<String>,
    /// Count suppressed occurrences in `quantity` instead of `suppressed_quantity`.
    pub include_suppressed: bool,
    /// Count occurrences excluded from the BOM in `quantity` instead of `excluded_quantity`.
    pub include_excluded: bool,
    /// Upper bound on expanded occurrences; exceeding it yields a partial result.
    pub max_occurrences: u64,
}

impl Default for BomOptions {
    fn default() -> Self {
        Self {
            root_configuration: None,
            include_suppressed: false,
            include_excluded: false,
            max_occurrences: 1_000_000,
        }
    }
}

/// One expanded occurrence path through the assembly tree.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BomOccurrence {
    pub id: String,
    pub parent_id: Option<String>,
    /// Graph edge that produced this occurrence; absent for the root.
    pub edge_id: Option<String>,
    pub node_id: Option<String>,
    pub depth: u32,
    pub instance_name: Option<String>,
    pub instance_number: Option<i64>,
    pub stored_path: Option<String>,
    pub document_kind: Option<DocumentKind>,
    pub referenced_configuration: Option<String>,
    /// Configuration index resolved on the target node, when exactly one matched.
    pub configuration_index: Option<i64>,
    pub is_suppressed: Option<bool>,
    pub is_hidden: Option<bool>,
    pub exclude_from_bom: Option<bool>,
    pub is_virtual: Option<bool>,
    pub resolution_status: Option<ReferenceResolutionStatus>,
    pub traversal_status: Option<ReferenceTraversalStatus>,
    pub children_state: BomChildrenState,
    /// `true` when this occurrence or an ancestor is suppressed or excluded.
    pub counted: bool,
}

/// Identity of one aggregated line item.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BomItemKey {
    /// Resolved node identity, or `None` for unresolved references.
    pub node_id: Option<String>,
    /// Case-folded stored basename used when no node resolved.
    pub stored_basename: Option<String>,
    pub configuration: Option<String>,
}

/// Aggregated quantities for one document and configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BomItem {
    pub key: BomItemKey,
    pub document_path: Option<String>,
    pub document_kind: Option<DocumentKind>,
    pub display_name: Option<String>,
    /// Occurrences counted under the selected inclusion rules.
    pub quantity: u64,
    pub suppressed_quantity: u64,
    pub excluded_quantity: u64,
    /// Occurrences whose suppression or BOM-exclusion state is unknown.
    pub unknown_state_quantity: u64,
    pub minimum_depth: u32,
    pub is_virtual: Option<bool>,
    #[serde(default)]
    pub occurrence_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct BomSummary {
    pub occurrence_count: u64,
    pub item_count: u64,
    pub part_item_count: u64,
    pub assembly_item_count: u64,
    pub unresolved_item_count: u64,
    #[serde(default)]
    pub children_state_counts: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BillOfMaterials {
    pub schema_version: u32,
    pub status: BomStatus,
    pub root_node_id: Option<String>,
    pub root_configuration: Option<String>,
    pub root_configuration_index: Option<i64>,
    pub options: BomOptions,
    #[serde(default)]
    pub occurrences: Vec<BomOccurrence>,
    #[serde(default)]
    pub items: Vec<BomItem>,
    pub summary: BomSummary,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}
