use std::path::PathBuf;

use clap::ValueEnum;

use crate::engine::Inspection;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Format {
    Text,
    Json,
    Sarif,
}

impl Format {
    pub fn ensure_available(self) -> Result<(), &'static str> {
        match self {
            Self::Text => Ok(()),
            Self::Json | Self::Sarif => Err("JSON and SARIF output are not implemented"),
        }
    }
}

pub struct FileReport {
    pub path: PathBuf,
    pub inspection: Inspection,
}

pub fn text(files: &[FileReport], fix: bool) -> String {
    let mut output = String::new();
    for file in files {
        let (line, column) = file.inspection.jobs_start;
        output.push_str(&format!(
            "OK {}: YAML parsed; `on` and `jobs` found; `jobs` starts at {line}:{column}\n",
            file.path.display()
        ));
    }

    if fix {
        let fixed: usize = files
            .iter()
            .filter_map(|file| file.inspection.fix.as_ref())
            .map(|outcome| outcome.fixed_count)
            .sum();
        let remaining: usize = files
            .iter()
            .filter_map(|file| file.inspection.fix.as_ref())
            .map(|outcome| outcome.remaining.len())
            .sum();
        output.push_str(&format!(
            "Fixed {fixed} finding(s) in {} workflow file(s).\n",
            files.len()
        ));
        if remaining > 0 {
            output.push_str(&format!("Remaining {remaining} finding(s).\n"));
        }
        return output;
    }

    let findings: usize = files
        .iter()
        .map(|file| file.inspection.findings.len())
        .sum();
    for file in files {
        for finding in &file.inspection.findings {
            output.push_str(&format!(
                "{} [{}:{}] {}:{}: job `{}`, step {} {}\n",
                finding.rule.id,
                finding.rule.tag,
                finding.rule.severity.as_str(),
                file.path.display(),
                finding.line,
                finding.job,
                finding.step + 1,
                finding.message
            ));
        }
    }
    output.push_str(&format!(
        "Checked {} workflow file(s); {findings} finding(s).\n",
        files.len()
    ));
    output
}
