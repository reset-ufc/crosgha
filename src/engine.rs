use std::{
    num::NonZero,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

use crate::{
    filesystem::{self, Source},
    fix::{self, FixOutcome},
    policy::Settings,
    rules::{self, Finding},
    workflow::Workflow,
};

pub struct Inspection {
    /// The source the findings' lines point into.
    pub source: Source,
    pub findings: Vec<Finding>,
    pub fix: Option<FixOutcome>,
    /// Local actions the workflow calls, for the repository rules.
    pub local_actions: Vec<String>,
}

pub fn inspect(
    source: Source,
    apply_fixes: bool,
    settings: &Settings,
    file: &str,
) -> Result<Inspection, String> {
    let workflow = Workflow::parse(source.text.clone())?;
    let findings = rules::detect_all(&workflow, settings, file)?;
    let local_actions = rules::local_actions(&workflow);
    // Without a proposal the fix pass would only scan the same source again.
    let fix = if apply_fixes && findings.iter().any(|finding| finding.fix.is_some()) {
        Some(fix::apply(workflow, &findings, settings, file)?)
    } else {
        None
    };
    Ok(Inspection {
        source,
        findings,
        fix,
        local_actions,
    })
}

/// Reads and inspects `paths` in parallel.
pub fn inspect_files(
    paths: &[PathBuf],
    apply_fixes: bool,
    settings: &Settings,
) -> Vec<Result<Inspection, String>> {
    parallel_map(paths, |path| {
        let file = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        // A symbolic link is refused only if a fix has to be written through it.
        filesystem::read(path)
            .map_err(|error| format!("cannot read workflow: {error}"))
            .and_then(|source| inspect(source, apply_fixes, settings, &file))
    })
}

/// Maps `items` on every available core. Results keep the order of `items`, so reports stay
/// deterministic.
pub fn parallel_map<T: Sync, R: Send>(items: &[T], map: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let workers = thread::available_parallelism()
        .map_or(1, NonZero::get)
        .min(items.len());
    let mut done: Vec<(usize, R)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    // Items differ widely in cost, so each worker takes the next one when free.
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(index) else {
                            break done;
                        };
                        done.push((index, map(item)));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| {
                worker
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    done.sort_by_key(|(index, _)| *index);
    done.into_iter().map(|(_, result)| result).collect()
}
