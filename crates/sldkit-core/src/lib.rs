//! Public contracts shared by the `SolidWorks` parser layers.

mod diagnostic;
mod drawing;
mod geometry;
mod inventory;
mod limits;
mod model;
mod project;
mod result;

pub use diagnostic::{Diagnostic, DiagnosticKind, DiagnosticSeverity};
pub use drawing::{
    DrawingBytePartitionStatus, DrawingCarrier, DrawingCarrierRole, DrawingRecord,
    DrawingRecordClass, DrawingRecordSource, DrawingStructureCoverage, DrawingStructureDocument,
    DrawingStructureResult, DrawingStructureSheet, DrawingStructureStatus, DrawingStructureView,
};
pub use geometry::{
    GeometryBody, GeometryByteCoverage, GeometryByteDomain, GeometryByteOffsetBasis,
    GeometryBytePartitionStatus, GeometryByteStorage, GeometryCarrier, GeometryCarrierDomain,
    GeometryCoedge, GeometryConfigurationState, GeometryConstruction, GeometryConstructionDomain,
    GeometryDocument, GeometryEdge, GeometryEntityProvenance, GeometryExactness, GeometryFace,
    GeometryFidelityReport, GeometryFinding, GeometryLoop, GeometryLoss, GeometryModel,
    GeometryPcurveState, GeometryPcurveUse, GeometryPoint, GeometryRawRecord, GeometryRegion,
    GeometryResult, GeometryShell, GeometrySourceObject, GeometryStatus, GeometryStreamCandidate,
    GeometryStreamRole, GeometryStreamSelection, GeometryTessellation, GeometryTessellationChannel,
    GeometryTessellationTextureAssignment, GeometryTessellationTriangleGroup,
    GeometryTopologyMetrics, GeometryVertex, GeometryVertexUse,
};
pub use inventory::{
    ByteRange, ChecksumStatus, CompressionMethod, ContainerInventory, ExtractionMode,
    ExtractionResult, ExtractionStatus, InventoryEntry, InventoryEntryKind, InventoryEntryState,
    InventoryResult, InventoryStatus, StreamExtraction,
};
pub use limits::{LimitProfile, ResourceLimits};
pub use model::{
    AssemblyComponent, BinaryResource, BinaryResourceKind, Configuration, CustomProperty,
    DocumentKind, DocumentReference, DrawingSheet, DrawingView, Envelope, MassProperties,
    PropertyKind, PropertyScope, PropertyValueState, RecordOffsetBasis, ReferenceKind,
    SourceDocument, SourceInfo, SourceInputKind, SourceValue, UnknownRecord, ValueOrigin,
};
pub use project::{
    ProjectCompatibilityReport, ProjectConfigurationIdentity, ProjectEdge, ProjectNode,
    ProjectScanResult, ProjectScanStatus, ReferenceResolutionBasis, ReferenceResolutionStatus,
    ReferenceTraversalStatus,
};
pub use result::{
    CoverageReport, ParseResult, ParseStatus, ProbeConfidence, ProbeEvidence, ProbeResult,
    ProbeStatus, SemanticCoverage,
};
