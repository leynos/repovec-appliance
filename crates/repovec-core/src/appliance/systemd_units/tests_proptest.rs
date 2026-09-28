//! Property-based tests for systemd unit parser and validator invariants.

use proptest::prelude::*;

use super::{
    ParsedUnit, SystemdUnitError,
    tests_proptest_strategies::{
        comment_line, insertion_position, quadlet_source_name, valid_dependency, valid_mcpd_base,
        valid_repovecd_base, valid_repovecd_service_first, valid_target_base,
        valid_target_install_first, whitespace,
    },
    validate_systemd_units,
};

#[derive(Clone, Copy, Debug)]
enum DependencyDirective {
    Wants,
    Requires,
    After,
}

fn validate_bases(target: &str, repovecd: &str, mcpd: &str) -> Result<(), SystemdUnitError> {
    validate_systemd_units(target, repovecd, mcpd)
}

fn insert_line(base: &str, line: &str, pos: usize) -> String {
    let mut lines = base.lines().map(String::from).collect::<Vec<_>>();
    let idx = pos.min(lines.len());
    lines.insert(idx, line.to_owned());
    lines.join("\n") + "\n"
}

fn spaced_target(key_pad: &str, value_pad: &str) -> String {
    format!(
        "[Unit]\n\
         Description=repovec appliance service group\n\
         {key_pad}Wants{key_pad}={value_pad}qdrant.service repovecd.service \
         repovec-mcpd.service cloudflared.service{value_pad}\n\
         \n\
         [Install]\n\
         {key_pad}WantedBy{key_pad}={value_pad}multi-user.target{value_pad}\n"
    )
}

fn dependency_line(key: &str, value: &str) -> String { format!("{key}={value}") }

fn target_with_wants(wants: &str) -> String {
    valid_target_base().replace(
        "Wants=qdrant.service repovecd.service repovec-mcpd.service cloudflared.service",
        &dependency_line("Wants", wants),
    )
}

fn mcpd_with_dependency(key: &str, dependency: &str) -> String {
    valid_mcpd_base().replace(&dependency_line(key, "qdrant.service repovecd.service"), dependency)
}

proptest! {
    // ------ Parser robustness ------

    /// Verifies that horizontal whitespace around keys and values does not
    /// change the validation result for required systemd directives.
    #[test]
    fn whitespace_tolerance(key_pad in whitespace(), value_pad in whitespace()) {
        validate_bases(&spaced_target(&key_pad, &value_pad), &valid_repovecd_base(), &valid_mcpd_base())
            .expect("systemd units with whitespace around required directives should remain valid");
    }

    /// Verifies that `#` comment lines can appear anywhere in a valid unit.
    #[test]
    fn hash_comment_injection_invariance(
        text in r"[^\n]{0,40}",
        pos in insertion_position(&valid_repovecd_base()),
    ) {
        let repovecd = insert_line(&valid_repovecd_base(), &format!("#{text}"), pos);
        validate_bases(&valid_target_base(), &repovecd, &valid_mcpd_base())
            .expect("hash comments should not change validation");
    }

    /// Verifies that `;` comment lines can appear anywhere in a valid unit.
    #[test]
    fn semicolon_comment_injection_invariance(
        text in r"[^\n]{0,40}",
        pos in insertion_position(&valid_mcpd_base()),
    ) {
        let mcpd = insert_line(&valid_mcpd_base(), &format!(";{text}"), pos);
        validate_bases(&valid_target_base(), &valid_repovecd_base(), &mcpd)
            .expect("semicolon comments should not change validation");
    }

    /// Verifies that generated comment lines with either supported prefix can
    /// appear anywhere in a valid unit.
    #[test]
    fn generated_comment_line_invariance(
        comment in comment_line(),
        pos in insertion_position(&valid_target_base()),
    ) {
        let target = insert_line(&valid_target_base(), &comment, pos);
        validate_bases(&target, &valid_repovecd_base(), &valid_mcpd_base())
            .expect("generated comments should not change validation");
    }

    /// Verifies that empty lines can appear anywhere in a valid unit.
    #[test]
    fn empty_line_invariance(
        pos in insertion_position(&valid_target_base()),
        count in 0_usize..=4,
    ) {
        let mut target = valid_target_base();
        for _ in 0..count {
            target = insert_line(&target, "", pos);
        }
        validate_bases(&target, &valid_repovecd_base(), &valid_mcpd_base())
            .expect("empty lines should not change validation");
    }

    /// Verifies that the validator does not rely on a specific section order.
    #[test]
    fn section_ordering_invariance(
        target_first in prop::bool::ANY,
        service_first in prop::bool::ANY,
    ) {
        let target = if target_first {
            valid_target_base()
        } else {
            valid_target_install_first()
        };
        let repovecd = if service_first {
            valid_repovecd_base()
        } else {
            valid_repovecd_service_first()
        };
        validate_bases(&target, &repovecd, &valid_mcpd_base())
            .expect("section ordering should not change validation");
    }

    // ------ Pre-section rejection ------

    /// Verifies that a key-value directive before any section is rejected.
    #[test]
    fn property_before_section_rejected(key in r"[A-Za-z][A-Za-z0-9]{0,12}", value in r"[A-Za-z0-9._/-]{1,24}") {
        let contents = format!("{key}={value}\n{}", valid_repovecd_base());
        let error = ParsedUnit::parse("repovecd.service", &contents)
            .expect_err("property before the first section should be rejected");

        prop_assert_eq!(
            error,
            SystemdUnitError::PropertyBeforeSection {
                unit: "repovecd.service",
                line_number: 1,
                line: format!("{key}={value}"),
            }
        );
    }

    // ------ Dependency tokenisation ------

    /// Verifies dependency tokenisation for `Wants=`, `Requires=`, and
    /// `After=` regardless of whitespace around the directive value.
    #[test]
    fn dependency_tokenisation_ignores_surrounding_whitespace(
        key in prop::sample::select(vec![
            DependencyDirective::Wants,
            DependencyDirective::Requires,
            DependencyDirective::After,
        ]),
        prefix in whitespace(),
        suffix in whitespace(),
    ) {
        let value = format!("{prefix}qdrant.service repovecd.service repovec-mcpd.service cloudflared.service{suffix}");
        let result = match key {
            DependencyDirective::Wants => {
                validate_bases(&target_with_wants(&value), &valid_repovecd_base(), &valid_mcpd_base())
            }
            DependencyDirective::Requires => validate_bases(
                &valid_target_base(),
                &valid_repovecd_base(),
                &mcpd_with_dependency("Requires", &dependency_line("Requires", &value)),
            ),
            DependencyDirective::After => validate_bases(
                &valid_target_base(),
                &valid_repovecd_base(),
                &mcpd_with_dependency("After", &dependency_line("After", &value)),
            ),
        };
        result.expect("dependency tokenisation should ignore surrounding whitespace");
    }

    /// Verifies that all generated dependency tokens are recognised when they
    /// appear together in a space-separated directive value.
    #[test]
    fn multiple_dependency_tokens_all_recognised(extra in valid_dependency()) {
        let value = format!("qdrant.service repovecd.service repovec-mcpd.service cloudflared.service {extra}");
        validate_bases(&target_with_wants(&value), &valid_repovecd_base(), &valid_mcpd_base())
            .expect("all required dependency tokens should be recognised among surrounding tokens");
    }

    // ------ Quadlet source rejection ------

    /// Verifies that a lone Quadlet source dependency is rejected.
    #[test]
    fn quadlet_source_dependency_rejected_alone(source in quadlet_source_name()) {
        let target = target_with_wants(&source);
        let error = validate_bases(&target, &valid_repovecd_base(), &valid_mcpd_base())
            .expect_err("Quadlet source dependency should be rejected");

        prop_assert_eq!(
            error,
            SystemdUnitError::UsesQuadletSourceDependency {
                unit: "repovec.target",
                section: "Unit",
                key: "Wants",
                dependency: source,
            }
        );
    }

    /// Verifies that any Quadlet source token is rejected even when surrounded
    /// by otherwise valid dependency tokens.
    #[test]
    fn quadlet_source_dependency_rejected_among_valid_tokens(
        source in quadlet_source_name(),
        before in valid_dependency(),
        after in valid_dependency(),
    ) {
        let value = format!("{before} qdrant.service {source} repovecd.service repovec-mcpd.service cloudflared.service {after}");
        let target = target_with_wants(&value);
        let error = validate_bases(&target, &valid_repovecd_base(), &valid_mcpd_base())
            .expect_err("Quadlet source dependency should be rejected among valid tokens");

        prop_assert_eq!(
            error,
            SystemdUnitError::UsesQuadletSourceDependency {
                unit: "repovec.target",
                section: "Unit",
                key: "Wants",
                dependency: source,
            }
        );
    }
}
