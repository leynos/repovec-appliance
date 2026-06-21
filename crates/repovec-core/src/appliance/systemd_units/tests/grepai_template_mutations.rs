//! Grepai template mutation helpers for systemd unit validator tests.

use super::{UnitFile, UnitSet, ValidationScenario};

impl ValidationScenario {
    pub(super) fn mutate_grepai_template_unit_section(self, units: &mut UnitSet) {
        match self {
            Self::MissingGrepaiTemplateUnitSection => {
                units.remove_line(UnitFile::GrepaiTemplate, "[Unit]\n");
                units.remove_line(
                    UnitFile::GrepaiTemplate,
                    "Description=repovec grepai indexer for %I\n",
                );
                units.remove_line(
                    UnitFile::GrepaiTemplate,
                    "Requires=qdrant.service repovecd.service\n",
                );
                units.remove_line(
                    UnitFile::GrepaiTemplate,
                    "After=qdrant.service repovecd.service\n",
                );
                units.remove_line(UnitFile::GrepaiTemplate, "PartOf=repovec.target\n");
            }
            _ => panic!("grepai template unit-section mutation called for non-unit scenario"),
        }
    }

    pub(super) fn mutate_grepai_template_dependencies(self, units: &mut UnitSet) {
        match self {
            Self::MissingGrepaiTemplateRequiresQdrant => {
                units.remove_token(UnitFile::GrepaiTemplate, "Requires=", "qdrant.service");
            }
            Self::MissingGrepaiTemplateRequiresRepovecd => {
                units.remove_token(UnitFile::GrepaiTemplate, "Requires=", "repovecd.service");
            }
            Self::MissingGrepaiTemplateAfterQdrant => {
                units.remove_token(UnitFile::GrepaiTemplate, "After=", "qdrant.service");
            }
            Self::MissingGrepaiTemplateAfterRepovecd => {
                units.remove_token(UnitFile::GrepaiTemplate, "After=", "repovecd.service");
            }
            Self::GrepaiTemplateUsesQdrantContainer => {
                units.replace_token(UnitFile::GrepaiTemplate, "qdrant.service", "qdrant.container");
            }
            Self::MissingGrepaiTemplatePartOfTarget => {
                units.remove_line(UnitFile::GrepaiTemplate, "PartOf=repovec.target\n");
            }
            Self::MissingGrepaiTemplateWantedByTarget => {
                units.remove_line(UnitFile::GrepaiTemplate, "WantedBy=repovec.target\n");
            }
            _ => panic!("grepai template dependency mutation called for non-dependency scenario"),
        }
    }
}
