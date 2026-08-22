use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Severity used to decide whether processing may continue.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// Stable semantic class for a diagnostic.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKind {
    Fatal,
    Unsupported,
    Malformed,
    Unresolved,
    Inferred,
    Preserved,
}

/// Machine-readable parser observation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    /// Stable identifier. Human-readable wording may change independently.
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub kind: DiagnosticKind,
    pub message: String,
    pub offset: Option<u64>,
    pub stream_path: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

impl Diagnostic {
    #[must_use]
    pub fn new(
        code: impl Into<String>,
        severity: DiagnosticSeverity,
        kind: DiagnosticKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity,
            kind,
            message: message.into(),
            offset: None,
            stream_path: None,
            details: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn at_offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    #[must_use]
    pub fn in_stream(mut self, path: impl Into<String>) -> Self {
        self.stream_path = Some(path.into());
        self
    }

    #[must_use]
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, DiagnosticKind, DiagnosticSeverity};

    #[test]
    fn diagnostic_builder_preserves_machine_readable_context() {
        let diagnostic = Diagnostic::new(
            "input.truncated_signature",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "truncated signature",
        )
        .at_offset(0)
        .with_detail("expected", "ole2_cfb");

        assert_eq!(diagnostic.offset, Some(0));
        assert_eq!(
            diagnostic.details.get("expected").map(String::as_str),
            Some("ole2_cfb")
        );
    }
}
