//! Typed error constructors for systemd unit validator tests.

use crate::appliance::systemd_units::SystemdUnitError;

/// Declares constructors that fill a [`SystemdUnitError`] variant from test
/// fixtures.
///
/// The six constructors differ only in the variant they build and in which
/// fixture fields are owned, so the shared wrapper is written once here rather
/// than repeated around each struct literal.
macro_rules! unit_error_constructors {
    ($(
        $name:ident($($parameter:ident: $type:ty,)*) -> $variant:ident {
            $($field:ident: $value:expr,)*
        }
    )*) => {
        $(
            pub(super) fn $name($($parameter: $type),*) -> SystemdUnitError {
                SystemdUnitError::$variant { $($field: $value),* }
            }
        )*
    };
}

unit_error_constructors! {
    missing(
        unit: &'static str,
        key: &'static str,
        dependency: &'static str,
    ) -> MissingDependency {
        unit: unit,
        section: "Unit",
        key: key,
        dependency: dependency,
    }
    missing_install(unit: &'static str, dependency: &'static str,) -> MissingDependency {
        unit: unit,
        section: "Install",
        key: "WantedBy",
        dependency: dependency,
    }
    quadlet_source(
        unit: &'static str,
        key: &'static str,
        dependency: &str,
    ) -> UsesQuadletSourceDependency {
        unit: unit,
        section: "Unit",
        key: key,
        dependency: dependency.to_owned(),
    }
    exec_start(
        unit: &'static str,
        expected: &'static str,
        actual: &str,
    ) -> IncorrectExecStart {
        unit: unit,
        expected: expected,
        actual: actual.to_owned(),
    }
    service_directive(
        unit: &'static str,
        key: &'static str,
        expected: &'static str,
        actual: &str,
    ) -> IncorrectServiceDirective {
        unit: unit,
        key: key,
        expected: expected,
        actual: actual.to_owned(),
    }
    forbidden_directive(
        unit: &'static str,
        section: &'static str,
        key: &'static str,
        actual: &str,
    ) -> ForbiddenDirective {
        unit: unit,
        section: section,
        key: key,
        actual: actual.to_owned(),
    }
}
