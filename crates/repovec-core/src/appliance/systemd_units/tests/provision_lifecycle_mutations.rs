//! Provisioning oneshot lifecycle mutation helpers for validator tests.
//!
//! The oneshot must stay manually restartable, so these mutations cover the
//! `[Unit]` directive that would refuse a restart and the `[Service]`
//! directive that would stop a completed run from re-executing.

use super::{
    scenarios::ValidationScenario,
    unit_set::{UnitFile, UnitSet},
};

const BEFORE_CONSUMERS: &str =
    "Before=repovec-qdrant-api-key.service qdrant.service repovecd.service repovec-mcpd.service\n";

impl ValidationScenario {
    pub(super) fn mutate_provision_lifecycle_directives(self, units: &mut UnitSet) {
        match self {
            Self::ProvisionRemainsActive => units.replace_token(
                UnitFile::Provision,
                "RemainAfterExit=no",
                "RemainAfterExit=yes",
            ),
            Self::ProvisionRefusesManualStop => {
                append_unit_directive(units, "RefuseManualStop=yes");
            }
            Self::ProvisionRefusesManualStopDisabled => {
                append_unit_directive(units, "RefuseManualStop=no");
            }
            _ => panic!("provision lifecycle mutation called for non-lifecycle scenario"),
        }
    }
}

/// Appends a directive to `[Unit]`.
///
/// `RefuseManualStop=` is a `[Unit]` directive, so placing the mutation there
/// reproduces the defect the validator must reject; a `[Service]` placement
/// would test a different, merely misplaced, failure.
fn append_unit_directive(units: &mut UnitSet, directive: &str) {
    units.replace_token(
        UnitFile::Provision,
        BEFORE_CONSUMERS,
        &format!("{BEFORE_CONSUMERS}{directive}\n"),
    );
}
