//! Interfaces for evidence-producing Podman and systemd consumer probes.
//!
//! This crate is in the repovec nursery. It models source artifacts, generated
//! units, raw external-tool evidence, and the ports implemented by concrete
//! Podman-generator and `systemd-analyze verify` adapters.

use std::fmt;

use repovec_unit_contract::{ArtifactId, Diagnostic, Severity};

/// Identifies the consumer that owns a source artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceKind {
    /// A native systemd unit.
    NativeUnit,
    /// A Podman Quadlet source document.
    Quadlet,
}

/// A source artifact supplied to an external consumer probe.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct SourceArtifact<'source> {
    id: &'source ArtifactId,
    kind: SourceKind,
    contents: &'source str,
}

impl<'source> SourceArtifact<'source> {
    /// Creates a source artifact.
    #[must_use]
    pub const fn new(id: &'source ArtifactId, kind: SourceKind, contents: &'source str) -> Self {
        Self { id, kind, contents }
    }

    /// Returns the artifact identity.
    #[must_use]
    pub const fn id(&self) -> &ArtifactId { self.id }

    /// Returns the source kind.
    #[must_use]
    pub const fn kind(&self) -> SourceKind { self.kind }

    /// Explicitly exposes the source contents to a probe adapter.
    #[must_use]
    pub const fn expose_contents(&self) -> &str { self.contents }
}

impl fmt::Debug for SourceArtifact<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceArtifact")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("contents_len", &self.contents.len())
            .finish()
    }
}

/// A native unit emitted by a generator.
#[derive(Clone, Eq, PartialEq)]
pub struct GeneratedUnit {
    name: String,
    contents: String,
}

impl GeneratedUnit {
    /// Creates a generated unit.
    #[must_use]
    pub fn new(name: impl Into<String>, contents: impl Into<String>) -> Self {
        Self { name: name.into(), contents: contents.into() }
    }

    /// Returns the generated unit name.
    #[must_use]
    pub fn name(&self) -> &str { &self.name }

    /// Explicitly exposes generated unit contents for verification.
    #[must_use]
    pub fn expose_contents(&self) -> &str { &self.contents }
}

impl fmt::Debug for GeneratedUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedUnit")
            .field("name", &self.name)
            .field("contents_len", &self.contents.len())
            .finish()
    }
}

/// Identifies the external-consumer stage that produced evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeStage {
    /// Podman converted Quadlet documents into native units.
    QuadletGeneration,
    /// Systemd verified native unit files.
    SystemdVerification,
}

/// Describes one external consumer-probe invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeInvocation {
    stage: ProbeStage,
    program: String,
    arguments: Vec<String>,
    version: Option<String>,
}

impl ProbeInvocation {
    /// Creates probe invocation metadata.
    #[must_use]
    pub fn new(
        stage: ProbeStage,
        program: impl Into<String>,
        arguments: Vec<String>,
        version: Option<String>,
    ) -> Self {
        Self { stage, program: program.into(), arguments, version }
    }

    /// Returns the probe stage.
    #[must_use]
    pub const fn stage(&self) -> ProbeStage { self.stage }

    /// Returns the invoked program.
    #[must_use]
    pub fn program(&self) -> &str { &self.program }

    /// Returns the exact argument vector.
    #[must_use]
    pub fn arguments(&self) -> &[String] { &self.arguments }

    /// Returns the recorded tool version when available.
    #[must_use]
    pub fn version(&self) -> Option<&str> { self.version.as_deref() }
}

/// Captures process status and raw output buffers.
#[derive(Clone, Eq, PartialEq)]
pub struct ProbeOutput {
    status_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl ProbeOutput {
    /// Creates process output evidence.
    #[must_use]
    pub const fn new(status_code: Option<i32>, stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self { status_code, stdout, stderr }
    }

    /// Returns the process status code, or `None` after signal termination.
    #[must_use]
    pub const fn status_code(&self) -> Option<i32> { self.status_code }

    /// Explicitly exposes standard output bytes.
    #[must_use]
    pub fn expose_stdout(&self) -> &[u8] { &self.stdout }

    /// Explicitly exposes standard error bytes.
    #[must_use]
    pub fn expose_stderr(&self) -> &[u8] { &self.stderr }

    /// Returns whether the invocation exited successfully.
    #[must_use]
    pub fn succeeded(&self) -> bool { self.status_code == Some(0) }
}

impl fmt::Debug for ProbeOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProbeOutput")
            .field("status_code", &self.status_code)
            .field("stdout_len", &self.stdout.len())
            .field("stderr_len", &self.stderr.len())
            .finish()
    }
}

/// Raw invocation evidence retained for audit and diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolEvidence {
    invocation: ProbeInvocation,
    output: ProbeOutput,
}

impl ToolEvidence {
    /// Creates an evidence record from one completed tool invocation.
    #[must_use]
    pub const fn new(invocation: ProbeInvocation, output: ProbeOutput) -> Self {
        Self { invocation, output }
    }

    /// Returns the invocation metadata.
    #[must_use]
    pub const fn invocation(&self) -> &ProbeInvocation { &self.invocation }

    /// Returns the raw process output container.
    #[must_use]
    pub const fn output(&self) -> &ProbeOutput { &self.output }

    /// Returns the probe stage.
    #[must_use]
    pub const fn stage(&self) -> ProbeStage { self.invocation.stage() }

    /// Returns the invoked program.
    #[must_use]
    pub fn program(&self) -> &str { self.invocation.program() }

    /// Returns the exact argument vector.
    #[must_use]
    pub fn arguments(&self) -> &[String] { self.invocation.arguments() }

    /// Returns the recorded tool version when available.
    #[must_use]
    pub fn version(&self) -> Option<&str> { self.invocation.version() }

    /// Returns the process status code, or `None` after signal termination.
    #[must_use]
    pub const fn status_code(&self) -> Option<i32> { self.output.status_code() }

    /// Explicitly exposes standard output bytes.
    #[must_use]
    pub fn expose_stdout(&self) -> &[u8] { self.output.expose_stdout() }

    /// Explicitly exposes standard error bytes.
    #[must_use]
    pub fn expose_stderr(&self) -> &[u8] { self.output.expose_stderr() }

    /// Returns whether the invocation exited successfully.
    #[must_use]
    pub fn succeeded(&self) -> bool { self.output.succeeded() }
}

/// Result of passing Quadlet sources through a generator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationReport {
    units: Vec<GeneratedUnit>,
    evidence: ToolEvidence,
    diagnostics: Vec<Diagnostic>,
}

impl GenerationReport {
    /// Creates a Quadlet generation report.
    #[must_use]
    pub const fn new(
        units: Vec<GeneratedUnit>,
        evidence: ToolEvidence,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self { units, evidence, diagnostics }
    }

    /// Returns generated native units.
    #[must_use]
    pub fn units(&self) -> &[GeneratedUnit] { &self.units }

    /// Returns invocation evidence.
    #[must_use]
    pub const fn evidence(&self) -> &ToolEvidence { &self.evidence }

    /// Returns diagnostics derived from the invocation.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] { &self.diagnostics }

    /// Returns whether generation succeeded without error diagnostics.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.evidence.succeeded() && diagnostics_are_valid(&self.diagnostics)
    }
}

/// Result of passing native units through systemd verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationReport {
    evidence: ToolEvidence,
    diagnostics: Vec<Diagnostic>,
}

impl VerificationReport {
    /// Creates a systemd verification report.
    #[must_use]
    pub const fn new(evidence: ToolEvidence, diagnostics: Vec<Diagnostic>) -> Self {
        Self { evidence, diagnostics }
    }

    /// Returns invocation evidence.
    #[must_use]
    pub const fn evidence(&self) -> &ToolEvidence { &self.evidence }

    /// Returns diagnostics derived from the invocation.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] { &self.diagnostics }

    /// Returns whether verification succeeded without error diagnostics.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.evidence.succeeded() && diagnostics_are_valid(&self.diagnostics)
    }
}

fn diagnostics_are_valid(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().all(|diagnostic| diagnostic.severity() != Severity::Error)
}

/// Generates native units from caller-supplied Quadlet sources.
pub trait QuadletGenerator {
    /// Error returned before a generation report can be produced.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Runs the generator without performing ambient source discovery.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when the generator cannot be invoked or its
    /// evidence cannot be collected.
    fn generate(&self, sources: &[SourceArtifact<'_>]) -> Result<GenerationReport, Self::Error>;
}

/// Verifies caller-supplied native units with a systemd consumer.
pub trait SystemdVerifier {
    /// Error returned before a verification report can be produced.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Runs verification without performing ambient source discovery.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when the verifier cannot be invoked or its
    /// evidence cannot be collected.
    fn verify(&self, units: &[SourceArtifact<'_>]) -> Result<VerificationReport, Self::Error>;
}

#[cfg(test)]
mod tests {
    use repovec_unit_contract::ArtifactId;

    use super::{
        ProbeInvocation, ProbeOutput, ProbeStage, SourceArtifact, SourceKind, ToolEvidence,
    };

    #[test]
    fn debug_output_redacts_source_and_tool_buffers() {
        let artifact = ArtifactId::new("secret.container");
        let source =
            SourceArtifact::new(&artifact, SourceKind::Quadlet, "Environment=TOKEN=do-not-print");
        let evidence = ToolEvidence::new(
            ProbeInvocation::new(
                ProbeStage::QuadletGeneration,
                "podman-system-generator",
                Vec::new(),
                Some(String::from("6.1.0")),
            ),
            ProbeOutput::new(Some(1), b"generated secret".to_vec(), b"diagnostic secret".to_vec()),
        );

        let source_debug = format!("{source:?}");
        let evidence_debug = format!("{evidence:?}");

        assert!(!source_debug.contains("do-not-print"));
        assert!(!evidence_debug.contains("generated secret"));
        assert!(!evidence_debug.contains("diagnostic secret"));
    }
}
