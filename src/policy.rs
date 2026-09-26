use clap::ValueEnum;

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
