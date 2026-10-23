//! Public contracts shared by the `SolidWorks` parser layers.

mod bom;
mod diagnostic;
mod display;
mod drawing;
mod geometry;
mod geometry_evaluation;
mod inventory;
mod limits;
mod model;
mod project;
mod result;

pub use bom::{
    BillOfMaterials, BomChildrenState, BomItem, BomItemKey, BomOccurrence, BomOptions, BomStatus,
    BomSummary,
};
pub use diagnostic::{Diagnostic, DiagnosticKind, DiagnosticSeverity};
pub use display::{
    DisplayColorSource, DisplayMeshBounds, DisplayMeshBuffers, DisplayMeshDocument,
    DisplayMeshPart, DisplayMeshResult, DisplayMeshStatus, DisplayMeshStream,
    DisplaySurfaceReference,
};
pub use drawing::{
    DrawingBytePartitionStatus, DrawingCarrier, DrawingCarrierRole, DrawingRecord,
    DrawingRecordClass, DrawingRecordSource, DrawingStructureCoverage, DrawingStructureDocument,
    DrawingStructureResult, DrawingStructureSheet, DrawingStructureStatus, DrawingStructureView,
};
pub use geometry::{
    GeometryBody, GeometryByteClassification, GeometryByteCoverage, GeometryByteDomain,
    GeometryByteOffsetBasis, GeometryBytePartitionStatus, GeometryByteRange, GeometryByteStorage,
    GeometryCarrier, GeometryCarrierDomain, GeometryCoedge, GeometryConfigurationState,
    GeometryConstruction, GeometryConstructionDomain, GeometryDecodedSpan, GeometryDerivedInterval,
    GeometryDerivedLoopRole, GeometryDocument, GeometryEdge, GeometryEntityProvenance,
    GeometryExactness, GeometryFace, GeometryFidelityReport, GeometryFinding,
    GeometryIntervalMethod, GeometryLoop, GeometryLoopRoleMethod, GeometryLoss, GeometryModel,
    GeometryPcurveState, GeometryPcurveUse, GeometryPoint, GeometryRawRecord, GeometryRegion,
    GeometryResult, GeometryShell, GeometrySourceObject, GeometryStatus, GeometryStreamCandidate,
    GeometryStreamRole, GeometryStreamSelection, GeometryTessellation, GeometryTessellationChannel,
    GeometryTessellationTextureAssignment, GeometryTessellationTriangleGroup,
    GeometryTopologyMetrics, GeometryVertex, GeometryVertexUse,
};
pub use geometry_evaluation::{
    BoundingBoxMethod, EdgeIntervalSource, EdgeSampleStatus, EdgeSmoothness, EvaluatedBodyBounds,
    EvaluatedBoundingBox, EvaluatedBounds, EvaluatedConfigurationBounds, EvaluatedEdge,
    EvaluatedMesh, GeometryEvaluationDocument, GeometryEvaluationOptions, GeometryEvaluationResult,
    GeometryEvaluationStatus, GeometryEvaluationSummary,
};
pub use inventory::{
    ByteRange, ChecksumStatus, CompressionMethod, ContainerInventory, ExtractionMode,
    ExtractionResult, ExtractionStatus, InventoryEntry, InventoryEntryKind, InventoryEntryState,
    InventoryResult, InventoryStatus, StreamExtraction,
};
pub use limits::{LimitProfile, ResourceLimits};
pub use model::{
    AssemblyComponent, BinaryResource, BinaryResourceKind, Configuration, CustomProperty,
    DocumentKind, DocumentReference, DocumentUnits, DrawingSheet, DrawingSheetFormatData,
    DrawingView, EmbeddedDocument, EmbeddedDocumentRole, Envelope, FeatureDimension, FeatureRecord,
    MassProperties, ModelBoundingBox, PropertyKind, PropertyScope, PropertyValueState,
    RecordOffsetBasis, ReferenceKind, SourceDocument, SourceInfo, SourceInputKind, SourceValue,
    UnitSetting, UnknownRecord, ValueOrigin,
};
pub use project::{
    EmbeddedNodeSource, ProjectCompatibilityReport, ProjectConfigurationIdentity, ProjectEdge,
    ProjectNode, ProjectScanResult, ProjectScanStatus, ReferenceResolutionBasis,
    ReferenceResolutionStatus, ReferenceTraversalStatus,
};
pub use result::{
    CoverageReport, ParseResult, ParseStatus, ProbeConfidence, ProbeEvidence, ProbeResult,
    ProbeStatus, SemanticCoverage,
};
