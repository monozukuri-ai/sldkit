use serde::{Deserialize, Serialize};

/// Named resource-limit profile exposed by Rust and Python APIs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitProfile {
    Desktop,
    Service,
}

impl LimitProfile {
    #[must_use]
    pub const fn limits(self) -> ResourceLimits {
        match self {
            Self::Desktop => ResourceLimits::desktop(),
            Self::Service => ResourceLimits::service(),
        }
    }
}

/// Hard bounds applied before or during parsing untrusted input.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceLimits {
    pub max_file_size: u64,
    pub max_total_uncompressed_bytes: u64,
    pub max_compression_ratio: u64,
    pub max_stream_count: u64,
    pub max_string_bytes: u64,
    pub max_xml_stream_bytes: u64,
    pub max_xml_nodes: u32,
    pub max_nesting_depth: u32,
    pub max_reference_depth: u32,
    pub max_project_roots: u64,
    pub max_project_entries: u64,
    pub max_project_files: u64,
    pub max_project_edges: u64,
    pub max_reference_candidates: u64,
}

impl ResourceLimits {
    #[must_use]
    pub const fn desktop() -> Self {
        Self {
            max_file_size: 2 * 1024 * 1024 * 1024,
            max_total_uncompressed_bytes: 8 * 1024 * 1024 * 1024,
            max_compression_ratio: 1_000,
            max_stream_count: 1_000_000,
            max_string_bytes: 64 * 1024 * 1024,
            max_xml_stream_bytes: 64 * 1024 * 1024,
            max_xml_nodes: 2_000_000,
            max_nesting_depth: 256,
            max_reference_depth: 256,
            max_project_roots: 256,
            max_project_entries: 1_000_000,
            max_project_files: 100_000,
            max_project_edges: 1_000_000,
            max_reference_candidates: 1_024,
        }
    }

    #[must_use]
    pub const fn service() -> Self {
        Self {
            max_file_size: 256 * 1024 * 1024,
            max_total_uncompressed_bytes: 1024 * 1024 * 1024,
            max_compression_ratio: 200,
            max_stream_count: 100_000,
            max_string_bytes: 8 * 1024 * 1024,
            max_xml_stream_bytes: 8 * 1024 * 1024,
            max_xml_nodes: 250_000,
            max_nesting_depth: 64,
            max_reference_depth: 64,
            max_project_roots: 32,
            max_project_entries: 100_000,
            max_project_files: 10_000,
            max_project_edges: 100_000,
            max_reference_candidates: 128,
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        LimitProfile::Desktop.limits()
    }
}

#[cfg(test)]
mod tests {
    use super::{LimitProfile, ResourceLimits};

    #[test]
    fn service_profile_is_tighter_than_desktop() {
        let desktop = ResourceLimits::desktop();
        let service = LimitProfile::Service.limits();

        assert!(service.max_file_size < desktop.max_file_size);
        assert!(service.max_stream_count < desktop.max_stream_count);
        assert!(service.max_nesting_depth < desktop.max_nesting_depth);
        assert!(service.max_project_roots < desktop.max_project_roots);
        assert!(service.max_project_files < desktop.max_project_files);
        assert!(service.max_project_edges < desktop.max_project_edges);
    }
}
