//! Parser-neutral interfaces for validating systemd and Quadlet contracts.
//!
//! This crate is in the repovec nursery. It defines source-aware unit views,
//! validation rules, and structured diagnostics without choosing a parser,
//! renderer, or product policy.

use std::{borrow::Cow, fmt};

/// Identifies one source or generated artifact under validation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactId(String);

impl ArtifactId {
    /// Creates an artifact identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }

    /// Returns the identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Identifies a half-open byte range in an artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    start: usize,
    end: usize,
}

impl SourceSpan {
    /// Creates a span when `start` does not follow `end`.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Option<Self> {
        if start <= end { Some(Self { start, end }) } else { None }
    }

    /// Returns the inclusive start byte offset.
    #[must_use]
    pub const fn start(self) -> usize { self.start }

    /// Returns the exclusive end byte offset.
    #[must_use]
    pub const fn end(self) -> usize { self.end }

    /// Returns the span length in bytes.
    #[must_use]
    pub const fn len(self) -> usize { self.end - self.start }

    /// Returns whether the span contains no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool { self.start == self.end }
}

/// Describes where a directive occurrence originated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceOrigin {
    /// A package or administrator supplied base unit.
    BaseUnit,
    /// A unit drop-in applied after a base unit.
    DropIn,
    /// Output produced by a unit generator.
    Generated,
    /// A synthetic source used by tests or tooling.
    Synthetic,
}

/// Raw and decoded forms of one directive value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectiveValue<'source> {
    raw: Cow<'source, str>,
    decoded: Cow<'source, str>,
}

impl<'source> DirectiveValue<'source> {
    /// Creates a directive value.
    #[must_use]
    pub fn new(raw: impl Into<Cow<'source, str>>, decoded: impl Into<Cow<'source, str>>) -> Self {
        Self { raw: raw.into(), decoded: decoded.into() }
    }

    /// Returns the value as written after the directive separator.
    #[must_use]
    pub fn raw(&self) -> &str { &self.raw }

    /// Returns the parser-decoded value.
    #[must_use]
    pub fn decoded(&self) -> &str { &self.decoded }
}

/// Source context attached to one directive occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceSource {
    span: Option<SourceSpan>,
    origin: SourceOrigin,
}

impl OccurrenceSource {
    /// Creates source context without a source span.
    #[must_use]
    pub const fn new(origin: SourceOrigin) -> Self { Self { span: None, origin } }

    /// Attaches a source span.
    #[must_use]
    pub const fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// Returns the source span when the adapter can provide one.
    #[must_use]
    pub const fn span(self) -> Option<SourceSpan> { self.span }

    /// Returns the source origin.
    #[must_use]
    pub const fn origin(self) -> SourceOrigin { self.origin }
}

/// One ordered occurrence of a directive in a unit view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectiveOccurrence<'source> {
    section: Cow<'source, str>,
    directive: Cow<'source, str>,
    value: DirectiveValue<'source>,
    source: OccurrenceSource,
}

impl<'source> DirectiveOccurrence<'source> {
    /// Creates a directive occurrence.
    #[must_use]
    pub fn new(
        section: impl Into<Cow<'source, str>>,
        directive: impl Into<Cow<'source, str>>,
        value: DirectiveValue<'source>,
        source: OccurrenceSource,
    ) -> Self {
        Self { section: section.into(), directive: directive.into(), value, source }
    }

    /// Returns the section name without brackets.
    #[must_use]
    pub fn section(&self) -> &str { &self.section }

    /// Returns the directive name.
    #[must_use]
    pub fn directive(&self) -> &str { &self.directive }

    /// Returns the value as written after the directive separator.
    #[must_use]
    pub fn raw_value(&self) -> &str { self.value.raw() }

    /// Returns the parser-decoded value.
    #[must_use]
    pub fn decoded_value(&self) -> &str { self.value.decoded() }

    /// Returns the source span when the adapter can provide one.
    #[must_use]
    pub const fn span(&self) -> Option<SourceSpan> { self.source.span() }

    /// Returns the source origin.
    #[must_use]
    pub const fn origin(&self) -> SourceOrigin { self.source.origin() }
}

/// Presents ordered directive occurrences without exposing a parser type.
pub trait UnitView {
    /// Returns matching occurrences in effective source order.
    fn occurrences<'view>(
        &'view self,
        section: &str,
        directive: &str,
    ) -> Box<dyn Iterator<Item = DirectiveOccurrence<'view>> + 'view>;
}

/// Describes the effect of one diagnostic on validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    /// Informative context that does not invalidate the artifact.
    Note,
    /// A concern that does not invalidate the artifact.
    Warning,
    /// A contract violation that invalidates the artifact.
    Error,
}

/// Classifies whether diagnostic context may contain secret-derived material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sensitivity {
    /// The diagnostic may be rendered through ordinary output channels.
    Public,
    /// Renderers must preserve redaction and restricted handling.
    Secret,
}

/// A structured finding produced while validating an artifact.
#[derive(Clone, Eq, PartialEq)]
pub struct Diagnostic {
    code: &'static str,
    severity: Severity,
    message: String,
    artifact: ArtifactId,
    span: Option<SourceSpan>,
    sensitivity: Sensitivity,
}

impl fmt::Debug for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.sensitivity {
            Sensitivity::Public => self.message.as_str(),
            Sensitivity::Secret => "REDACTED",
        };
        formatter
            .debug_struct("Diagnostic")
            .field("code", &self.code)
            .field("severity", &self.severity)
            .field("message", &message)
            .field("artifact", &self.artifact)
            .field("span", &self.span)
            .field("sensitivity", &self.sensitivity)
            .finish()
    }
}

impl Diagnostic {
    /// Creates a public diagnostic without a source span.
    #[must_use]
    pub fn new(
        code: &'static str,
        severity: Severity,
        message: impl Into<String>,
        artifact: ArtifactId,
    ) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            artifact,
            span: None,
            sensitivity: Sensitivity::Public,
        }
    }

    /// Attaches a source span.
    #[must_use]
    pub const fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// Marks the diagnostic as secret-derived.
    #[must_use]
    pub const fn secret(mut self) -> Self {
        self.sensitivity = Sensitivity::Secret;
        self
    }

    /// Returns the stable diagnostic code.
    #[must_use]
    pub const fn code(&self) -> &'static str { self.code }

    /// Returns the diagnostic severity.
    #[must_use]
    pub const fn severity(&self) -> Severity { self.severity }

    /// Returns the human-readable diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str { &self.message }

    /// Returns the artifact identity.
    #[must_use]
    pub const fn artifact(&self) -> &ArtifactId { &self.artifact }

    /// Returns the source span when one is available.
    #[must_use]
    pub const fn span(&self) -> Option<SourceSpan> { self.span }

    /// Returns the diagnostic sensitivity.
    #[must_use]
    pub const fn sensitivity(&self) -> Sensitivity { self.sensitivity }
}

/// Receives structured diagnostics from validation rules.
pub trait DiagnosticSink {
    /// Records one diagnostic.
    fn emit(&mut self, diagnostic: Diagnostic);
}

/// Checks one aspect of an artifact contract.
pub trait Rule {
    /// Evaluates the rule and emits zero or more diagnostics.
    fn check(
        &self,
        artifact: &ArtifactId,
        unit: &dyn UnitView,
        diagnostics: &mut dyn DiagnosticSink,
    );
}

/// Accumulated diagnostics for one validation pass.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ValidationReport {
    diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    /// Creates an empty validation report.
    #[must_use]
    pub const fn new() -> Self { Self { diagnostics: Vec::new() } }

    /// Returns all diagnostics in emission order.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] { &self.diagnostics }

    /// Consumes the report and returns its diagnostics.
    #[must_use]
    pub fn into_diagnostics(self) -> Vec<Diagnostic> { self.diagnostics }

    /// Returns whether the report contains no error-severity diagnostics.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.diagnostics.iter().all(|diagnostic| diagnostic.severity() != Severity::Error)
    }
}

impl DiagnosticSink for ValidationReport {
    fn emit(&mut self, diagnostic: Diagnostic) { self.diagnostics.push(diagnostic); }
}

/// Evaluates rules in order and returns all emitted diagnostics.
#[must_use]
pub fn validate(
    artifact: &ArtifactId,
    unit: &dyn UnitView,
    rules: &[&dyn Rule],
) -> ValidationReport {
    let mut report = ValidationReport::new();
    for rule in rules {
        rule.check(artifact, unit, &mut report);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactId, Diagnostic, DiagnosticSink, DirectiveOccurrence, DirectiveValue,
        OccurrenceSource, Rule, Severity, SourceOrigin, UnitView, validate,
    };

    struct EmptyUnit;

    impl UnitView for EmptyUnit {
        fn occurrences<'view>(
            &'view self,
            _section: &str,
            _directive: &str,
        ) -> Box<dyn Iterator<Item = DirectiveOccurrence<'view>> + 'view> {
            Box::new(std::iter::empty())
        }
    }

    struct AlwaysFails;

    impl Rule for AlwaysFails {
        fn check(
            &self,
            artifact: &ArtifactId,
            _unit: &dyn UnitView,
            diagnostics: &mut dyn DiagnosticSink,
        ) {
            diagnostics.emit(Diagnostic::new(
                "nursery.always-fails",
                Severity::Error,
                "injected failure",
                artifact.clone(),
            ));
        }
    }

    #[test]
    fn error_diagnostic_invalidates_report() {
        let artifact = ArtifactId::new("fixture.service");
        let report = validate(&artifact, &EmptyUnit, &[&AlwaysFails]);

        assert!(!report.is_valid());
        assert_eq!(report.diagnostics().len(), 1);
    }

    #[test]
    fn occurrence_retains_raw_and_decoded_values() {
        let occurrence = DirectiveOccurrence::new(
            "Service",
            "Environment",
            DirectiveValue::new("KEY=\"two words\"", "KEY=two words"),
            OccurrenceSource::new(SourceOrigin::BaseUnit),
        );

        assert_eq!(occurrence.raw_value(), "KEY=\"two words\"");
        assert_eq!(occurrence.decoded_value(), "KEY=two words");
    }

    #[test]
    fn secret_diagnostic_debug_output_redacts_message() {
        let diagnostic = Diagnostic::new(
            "nursery.secret",
            Severity::Error,
            "bearer do-not-print",
            ArtifactId::new("fixture.service"),
        )
        .secret();

        assert!(!format!("{diagnostic:?}").contains("do-not-print"));
    }
}
