use std::path::PathBuf;

use clap::ValueEnum;

use crate::{engine::Inspection, rules::Located};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Format {
    /// Human-readable lines; `plain` is zizmor's name for it.
    #[value(alias = "plain")]
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

pub fn text(files: &[FileReport], repository: &[Located], fix: bool) -> String {
    let mut output = String::new();
    let append_repository = |output: &mut String| {
        for located in repository {
            append_findings(
                output,
                &located.path,
                std::slice::from_ref(&located.finding),
            );
        }
    };
    if fix {
        let fixed: usize = files
            .iter()
            .filter_map(|file| file.inspection.fix.as_ref())
            .map(|outcome| outcome.fixed_count)
            .sum();
        let remaining = repository.len()
            + files
                .iter()
                .filter_map(|file| file.inspection.fix.as_ref())
                .map(|outcome| outcome.remaining.len())
                .sum::<usize>();
        let changed = files
            .iter()
            .filter(|file| {
                file.inspection
                    .fix
                    .as_ref()
                    .is_some_and(|outcome| outcome.fixed_count > 0)
            })
            .count();
        output.push_str(&format!(
            "Fixed {fixed} finding(s) in {changed} workflow file(s).\n"
        ));
        if remaining > 0 {
            output.push_str(&format!("Remaining {remaining} finding(s).\n"));
            for file in files {
                if let Some(outcome) = &file.inspection.fix {
                    append_findings(&mut output, &file.path, &outcome.remaining);
                }
            }
            append_repository(&mut output);
        }
        return output;
    }

    let findings = repository.len()
        + files
            .iter()
            .map(|file| file.inspection.findings.len())
            .sum::<usize>();
    for file in files {
        append_findings(&mut output, &file.path, &file.inspection.findings);
    }
    append_repository(&mut output);
    output.push_str(&format!(
        "Checked {} workflow file(s); {findings} finding(s).\n",
        files.len()
    ));
    output
}

fn append_findings(
    output: &mut String,
    path: &std::path::Path,
    findings: &[crate::rules::Finding],
) {
    for finding in findings {
        output.push_str(&format!(
            "{} [{}:{}] {}:{}: {}: {}\n",
            finding.rule.id,
            finding.rule.tag,
            finding.severity.as_str(),
            path.display(),
            finding.line,
            finding.location,
            finding.message
        ));
    }
}

pub fn rules() -> String {
    crate::rules::all()
        .map(|(rule, scope)| {
            format!(
                "{} {} {} {} {scope}\n",
                rule.id,
                rule.tag,
                rule.severity.as_str(),
                rule.persona.as_str()
            )
        })
        .collect()
}
