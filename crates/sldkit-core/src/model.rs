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
    /// `swIsVirtualComponent`: the referenced document is saved inside this assembly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_virtual: Option<SourceValue<bool>>,
    /// `swFlexible`: a flexible subassembly occurrence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_flexible: Option<SourceValue<bool>>,
    /// `swReferenceNumber`: the instance number shown after the component name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_number: Option<SourceValue<i64>>,
    /// `swID`: the assembly-local component identity used by display states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_id: Option<SourceValue<i64>>,
    /// The 16 exact decimal `swTransform` tokens in source order. Matrix layout,
    /// length unit, direction, and parent/root basis remain unverified; see
    /// the assembly validation guide before interpreting them as a placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_transform: Option<SourceValue<Vec<String>>>,
    pub source_model_ref: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_attributes: BTreeMap<String, String>,
}

/// Exact decimal tokens of a saved model bounding box.
///
/// The six tokens are `min x, min y, min z, max x, max y, max z` in source
/// order. The metre length unit is an inference cross-checked against decoded
/// Part geometry extents, not a declared source field.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModelBoundingBox {
    pub raw_value: SourceValue<String>,
    pub min: [String; 3],
    pub max: [String; 3],
    pub length_unit: SourceValue<String>,
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
    /// Localized material label saved with the configuration, for example a
    /// "not specified" placeholder. It is not resolved against a material library.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<SourceValue<String>>,
    /// Saved model bounding box of the configuration root model, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounding_box: Option<ModelBoundingBox>,
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

/// One `@`-separated record of the `SW-All Sheet Format Data` system property.
///
/// Field roles are inferred from controlled samples; unassigned fields stay in
/// `fields` in source order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingSheetFormatData {
    pub raw_value: SourceValue<String>,
    #[serde(default)]
    pub fields: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingSheet {
    pub source_id: Option<String>,
    pub name: Option<SourceValue<String>>,
    pub preview: Option<BinaryResource>,
    /// Source `PaperSize` text, for example `USER_SIZE`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper_size: Option<SourceValue<String>>,
    /// Source `PaperSize@Description` code; see `paper_size_name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper_size_code: Option<SourceValue<i64>>,
    /// Inferred `swDwgPaperSizes_e` name for `paper_size_code`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper_size_name: Option<SourceValue<String>>,
    /// Exact decimal scale numerator and denominator tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_numerator: Option<SourceValue<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_denominator: Option<SourceValue<String>>,
    /// Stored sheet-format template path (`TemplateName`), unresolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet_format_path: Option<SourceValue<String>>,
    /// View whose model supplies custom-property values for the sheet format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_property_view: Option<SourceValue<String>>,
    /// Exact decimal sheet width and height tokens; metres are an inference
    /// cross-checked against standard ISO sheet sizes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<SourceValue<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<SourceValue<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_data: Option<DrawingSheetFormatData>,
    #[serde(default)]
    pub views: Vec<DrawingView>,
}

/// One integer unit-setting code with its inferred API enumeration name.
///
/// The code is the exact source value. The name applies the `SolidWorks` API
/// enumeration named in the evidence to that code and is therefore inferred.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UnitSetting {
    pub property_name: String,
    pub code: SourceValue<i64>,
    pub name: Option<SourceValue<String>>,
}

/// Document unit settings decoded from `SW_Unit*` system properties.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DocumentUnits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_system: Option<UnitSetting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<UnitSetting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_decimal_places: Option<SourceValue<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dual_length: Option<UnitSetting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dual_length_decimal_places: Option<SourceValue<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<UnitSetting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle_decimal_places: Option<SourceValue<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_property_length: Option<UnitSetting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_property_decimal_places: Option<SourceValue<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass: Option<UnitSetting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<UnitSetting>,
}

/// One `Dimension` child of a `KeyWords` feature record; values are display text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeatureDimension {
    pub name: Option<String>,
    pub raw_value: String,
}

/// One `FeatureManager` item recorded in the saved `KeyWords` XML.
///
/// This is an inventory of saved names, localized type labels, and display
/// dimension text. It is not a feature-history reconstruction and carries no
/// geometry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeatureRecord {
    /// XML local element name such as `Feature`, `Extrusion`, `HoleWizard`, or `Sketch`.
    pub source_tag: String,
    pub source_id: Option<String>,
    pub name: Option<SourceValue<String>>,
    pub source_type: Option<String>,
    pub dissectable: Option<bool>,
    pub dissectable_root: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dissectable_children: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<FeatureDimension>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_attributes: BTreeMap<String, String>,
    pub stream_path: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddedDocumentRole {
    VirtualComponent,
}

/// A complete document stored inside another document's container.
///
/// The entry can be extracted with the container entry ID and parsed as its
/// own input. Only the container envelope is probed here.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EmbeddedDocument {
    pub role: EmbeddedDocumentRole,
    /// Name derived from the stream path, for example `Part2` for `VirtualComp/Part2`.
    pub name: String,
    pub entry_id: String,
    pub stream_path: String,
    pub byte_len: Option<u64>,
    pub sha256: Option<String>,
    pub envelope: Option<Envelope>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<DocumentUnits>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feature_records: Vec<FeatureRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embedded_documents: Vec<EmbeddedDocument>,
    #[serde(default)]
    pub unknown_records: Vec<UnknownRecord>,
}
