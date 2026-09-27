use clap::ValueEnum;

/// Sensitivity levels named after zizmor's personas so users can switch tools
/// without relearning the flag: https://docs.zizmor.sh/usage/#using-personas
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, ValueEnum)]
pub enum Persona {
    /// High-signal, actionable findings.
    Regular,
    /// Adds code smells that are worth cleaning up.
    Pedantic,
    /// Everything, including likely false positives.
    Auditor,
}

impl Persona {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Pedantic => "pedantic",
            Self::Auditor => "auditor",
        }
    }
}

pub struct Settings {
    pub persona: Persona,
    /// Present in online mode: a token is available and `--offline` is not set.
    pub github: Option<crate::github::GitHub>,
    pub config: crate::config::Config,
    /// `false` with `--no-ignores`: inline and config ignores are not applied.
    pub ignores: bool,
    pub min_severity: Option<Severity>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Severity {
    Note,
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum FailOn {
    Any,
    None,
    Note,
    Warning,
    Error,
}

impl FailOn {
    pub fn matches(self, severity: Severity) -> bool {
        match self {
            Self::Any => true,
            Self::None => false,
            Self::Note => severity >= Severity::Note,
            Self::Warning => severity >= Severity::Warning,
            Self::Error => severity >= Severity::Error,
        }
    }
}
