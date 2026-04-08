//! Shared domain types and configuration scaffolding for repovec services.

use std::fmt;

use camino::{Utf8Path, Utf8PathBuf};

/// Service types: daemons (`Repovecd`, `RepovecMcpd`), terminal UI (`RepovecTui`), and CLI (`Repovectl`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceKind {
    /// The repository control-plane daemon.
    Repovecd,
    /// The external MCP bridge daemon.
    RepovecMcpd,
    /// The terminal user interface.
    RepovecTui,
    /// The deployment and operator CLI.
    Repovectl,
}

impl ServiceKind {
    /// Returns the crate and binary name for the service.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::ServiceKind;
    ///
    /// assert_eq!(ServiceKind::Repovecd.binary_name(), "repovecd");
    /// ```
    #[must_use]
    pub const fn binary_name(self) -> &'static str {
        match self {
            Self::Repovecd => "repovecd",
            Self::RepovecMcpd => "repovec-mcpd",
            Self::RepovecTui => "repovec-tui",
            Self::Repovectl => "repovectl",
        }
    }
}

impl fmt::Display for ServiceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.binary_name())
    }
}

impl AsRef<str> for ServiceKind {
    fn as_ref(&self) -> &str {
        self.binary_name()
    }
}

/// Canonical filesystem roots used by appliance components.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePaths {
    config_root: Utf8PathBuf,
    data_root: Utf8PathBuf,
}

impl RuntimePaths {
    /// Creates a set of runtime paths from explicit config and data roots.
    ///
    /// # Examples
    ///
    /// ```
    /// use camino::Utf8PathBuf;
    /// use repovec_core::RuntimePaths;
    ///
    /// let paths = RuntimePaths::new(
    ///     Utf8PathBuf::from("/etc/repovec"),
    ///     Utf8PathBuf::from("/var/lib/repovec"),
    /// );
    ///
    /// assert_eq!(paths.git_mirrors_root().as_str(), "/var/lib/repovec/git-mirrors");
    /// ```
    #[must_use]
    pub const fn new(config_root: Utf8PathBuf, data_root: Utf8PathBuf) -> Self {
        Self { config_root, data_root }
    }

    /// Returns the standard appliance runtime layout.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::RuntimePaths;
    ///
    /// let paths = RuntimePaths::appliance_defaults();
    ///
    /// assert_eq!(paths.config_root().as_str(), "/etc/repovec");
    /// ```
    #[must_use]
    pub fn appliance_defaults() -> Self {
        Self::new(Utf8PathBuf::from("/etc/repovec"), Utf8PathBuf::from("/var/lib/repovec"))
    }

    /// Returns the configuration directory root.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::RuntimePaths;
    ///
    /// let paths = RuntimePaths::appliance_defaults();
    ///
    /// assert_eq!(paths.config_root().as_str(), "/etc/repovec");
    /// ```
    #[must_use]
    pub fn config_root(&self) -> &Utf8Path {
        self.config_root.as_path()
    }

    /// Returns an immutable borrowed path to the appliance data directory root.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::RuntimePaths;
    ///
    /// let paths = RuntimePaths::appliance_defaults();
    ///
    /// assert_eq!(paths.data_root().as_str(), "/var/lib/repovec");
    /// ```
    #[must_use]
    pub fn data_root(&self) -> &Utf8Path {
        self.data_root.as_path()
    }

    /// Private helper for constructing data subdirectory paths.
    #[inline]
    fn data_child(&self, name: &str) -> Utf8PathBuf {
        self.data_root.join(name)
    }

    /// Returns the bare-mirror directory root.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::RuntimePaths;
    ///
    /// let paths = RuntimePaths::appliance_defaults();
    ///
    /// assert_eq!(paths.git_mirrors_root().as_str(), "/var/lib/repovec/git-mirrors");
    /// ```
    #[must_use]
    pub fn git_mirrors_root(&self) -> Utf8PathBuf {
        self.data_child("git-mirrors")
    }

    /// Returns the worktree directory root.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::RuntimePaths;
    ///
    /// let paths = RuntimePaths::appliance_defaults();
    ///
    /// assert_eq!(paths.worktrees_root().as_str(), "/var/lib/repovec/worktrees");
    /// ```
    #[must_use]
    pub fn worktrees_root(&self) -> Utf8PathBuf {
        self.data_child("worktrees")
    }

    /// Returns the grepai data directory root.
    ///
    /// Note: grepai configuration is stored under the data root rather than the
    /// configuration root, as it contains both configuration and mutable index data.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::RuntimePaths;
    ///
    /// let paths = RuntimePaths::appliance_defaults();
    ///
    /// assert_eq!(paths.grepai_root().as_str(), "/var/lib/repovec/.grepai");
    /// ```
    #[must_use]
    pub fn grepai_root(&self) -> Utf8PathBuf {
        self.data_child(".grepai")
    }
}

#[cfg(test)]
mod tests {
    //! Regression tests for the shared scaffolding surface.

    use camino::Utf8PathBuf;
    use rstest::rstest;

    use super::{RuntimePaths, ServiceKind};

    #[rstest]
    #[case(ServiceKind::Repovecd, "repovecd")]
    #[case(ServiceKind::RepovecMcpd, "repovec-mcpd")]
    #[case(ServiceKind::RepovecTui, "repovec-tui")]
    #[case(ServiceKind::Repovectl, "repovectl")]
    fn service_binary_names_remain_stable(#[case] kind: ServiceKind, #[case] expected: &str) {
        assert_eq!(kind.binary_name(), expected);
    }

    #[test]
    fn runtime_paths_expand_from_the_data_root() {
        let paths =
            RuntimePaths::new(Utf8PathBuf::from("/tmp/config"), Utf8PathBuf::from("/srv/repovec"));

        assert_eq!(paths.git_mirrors_root().as_str(), "/srv/repovec/git-mirrors");
        assert_eq!(paths.worktrees_root().as_str(), "/srv/repovec/worktrees");
        assert_eq!(paths.grepai_root().as_str(), "/srv/repovec/.grepai");
    }
}
