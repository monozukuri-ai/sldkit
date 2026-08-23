use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Container family inferred from file bytes, not from the filename.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Envelope {
    ModernChunk,
    Ole2Cfb,
    ZipOpc,
    Unknown,
}

/// `SolidWorks` document kind, retaining whether it came from content or a hint.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Part,
    Assembly,
    Drawing,
    Unknown,
}

/// Provenance of a value in the source-faithful model.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueOrigin {
    Source,
    Derived,
    Inferred,
    Hint,
    Preserved,
}

/// A value coupled to evidence about where it came from.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceValue<T> {
    pub value: T,
    pub origin: ValueOrigin,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
}

impl<T> SourceValue<T> {
    #[must_use]
    pub fn new(value: T, origin: ValueOrigin, evidence: Vec<String>) -> Self {
        Self {
            value,
            origin,
            evidence,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceInputKind {
    Path,
    Bytes,
}

/// Identity of the exact bytes supplied to the parser.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceInfo {
    pub input_kind: SourceInputKind,
    pub label: Option<String>,
    pub byte_len: u64,
    pub sha256: String,
}

/// Classification of a property without normalizing it into a downstream CAD IR.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyKind {
    Custom,
    Core,
    System,
}

/// Presence and decoding state of the value element in one source property.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyValueState {
    Present,
    Empty,
    Missing,
    UnsupportedType,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyScope {
    Global,
    Configuration,
}

/// A validated binary resource remains retrievable through its container entry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryResourceKind {
    PreviewPng,
    PreviewDib,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BinaryResource {
    pub kind: BinaryResourceKind,
    pub entry_id: String,
    pub stream_path: String,
    /// Byte offset within the decoded stream, not the compressed source frame.
    pub decoded_offset: u64,
    pub byte_len: u64,
    pub sha256: String,
    pub media_type: String,
}

/// Exact decimal tokens from `SolidWorks`' cached mass-property value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MassProperties {
    pub raw_value: SourceValue<String>,
    pub center_of_gravity: [String; 3],
    pub volume: String,
    pub surface_area: String,
    pub mass: String,
    pub moments_of_inertia: [String; 3],
    pub products_of_inertia: [String; 3],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_values: Vec<String>,
}

/// One source component occurrence in an assembly configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AssemblyComponent {
    pub configuration_index: i64,
    pub instance_name: Option<SourceValue<String>>,
    pub stored_path: Option<SourceValue<String>>,
    pub document_kind: Option<SourceValue<DocumentKind>>,
    pub referenced_configuration: Option<SourceValue<String>>,
    pub component_reference: Option<SourceValue<String>>,
    pub is_suppressed: Option<SourceValue<bool>>,
    pub is_hidden: Option<SourceValue<bool>>,
    pub exclude_from_bom: Option<SourceValue<bool>>,
    pub source_model_ref: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_attributes: BTreeMap<String, String>,
}

/// Configuration identity is index-based; unresolved names and parents stay absent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Configuration {
    pub index: SourceValue<i64>,
    pub name: Option<SourceValue<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternate_names: Vec<SourceValue<String>>,
    pub parent_name: Option<SourceValue<String>>,
    pub parent_index: Option<SourceValue<i64>>,
    pub preview: Option<BinaryResource>,
    pub mass_properties: Option<MassProperties>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<AssemblyComponent>,
}

/// Property values stay source-specific and do not map to a common CAD IR here.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CustomProperty {
    pub name: SourceValue<String>,
    pub raw_value: Option<SourceValue<String>>,
    pub value_type: Option<String>,
    pub value_state: PropertyValueState,
    pub kind: PropertyKind,
    pub scope: PropertyScope,
    pub configuration: Option<String>,
    pub configuration_index: Option<i64>,
    pub stream_path: String,
    pub pid: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    AssemblyComponent,
    DrawingView,
    ExternalFeature,
    Unknown,
}

/// Stored reference and host resolution are intentionally separate fields.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DocumentReference {
    pub kind: ReferenceKind,
    pub source_name: Option<SourceValue<String>>,
    pub stored_path: Option<SourceValue<String>>,
    pub resolved_path: Option<String>,
    pub document_kind: Option<SourceValue<DocumentKind>>,
    pub configuration: Option<SourceValue<String>>,
    pub configuration_index: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingView {
    pub source_id: Option<String>,
    pub name: Option<SourceValue<String>>,
    pub referenced_document: Option<SourceValue<String>>,
    pub referenced_configuration: Option<SourceValue<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingSheet {
    pub source_id: Option<String>,
    pub name: Option<SourceValue<String>>,
    pub preview: Option<BinaryResource>,
    #[serde(default)]
    pub views: Vec<DrawingView>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordOffsetBasis {
    SourceFile,
    DecodedStream,
}

/// An uninterpreted record remains traceable to the original source.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UnknownRecord {
    pub entry_id: Option<String>,
    pub stream_path: Option<String>,
    pub record_kind: Option<u64>,
    pub offset_basis: RecordOffsetBasis,
    pub offset: u64,
    pub length: u64,
    pub sha256: String,
    pub reason_code: String,
}

/// `SolidWorks`-specific source model. Downstream IR mapping stays out of this type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceDocument {
    pub source: SourceInfo,
    pub envelope: SourceValue<Envelope>,
    pub document_kind: SourceValue<DocumentKind>,
    pub internal_version: Option<SourceValue<u64>>,
    #[serde(default)]
    pub configurations: Vec<Configuration>,
    #[serde(default)]
    pub properties: Vec<CustomProperty>,
    #[serde(default)]
    pub references: Vec<DocumentReference>,
    pub preview: Option<BinaryResource>,
    #[serde(default)]
    pub sheets: Vec<DrawingSheet>,
    #[serde(default)]
    pub unknown_records: Vec<UnknownRecord>,
}
