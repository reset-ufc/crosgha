#![allow(
    clippy::disallowed_methods,
    reason = "tests stage files and run the binary"
)]

//! `--follow-actions` end to end against the fake GitHub API, which serves each
//! `tests/fixtures/follow/<name>` as the repository `fixture/<name>` at `COMMIT`. The scanned
//! workflows are `follow/caller/.github/workflows/ci.yml` (chains, a cycle, `action.yaml`, a
//! JavaScript and a Docker action, references that are not followed, an ignore) and
//! `follow/deep.yml` (the runner's nesting limit).

mod common;

use common::{Api, COMMIT, Failure, TOKEN, TempDir, bin, fixture, github_api};
use std::fs;
use std::path::PathBuf;
use std::process::Output;
use std::sync::{Arc, Mutex};

type Log = Arc<Mutex<Vec<String>>>;

fn api(failures: &'static [Failure]) -> (String, Log) {
    github_api(Api {
        failures,
        repositories: fixture("follow"),
        ..Api::default()
    })
}

fn workflows() -> PathBuf {
    fixture("follow/caller/.github/workflows")
}

/// A pedantic scan with `--follow-actions` on github.com's terms, so `retired-runtime` runs.
fn follow(api: &str, args: &[&str], inputs: &[PathBuf]) -> Output {
    bin()
        .env("CROSGHA_TEST_HOSTED", "1")
        .args(["--persona", "pedantic", "--follow-actions"])
        .args(["--gh-token", TOKEN, "--gh-hostname", api])
        .args(args)
        .args(inputs)
        .output()
        .unwrap()
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{error}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// A followed file and line, `caller/...` for a scanned file and `<name>:<file>` for
/// `fixture/<name>@COMMIT:<file>`.
fn short(path: &str) -> String {
    let root = format!("{}/", fixture("follow").display());
    let remote = format!("@{COMMIT}:");
    path.trim_start_matches(&root)
        .trim_start_matches("fixture/")
        .replace(&remote, ":")
}

/// `(file:line rule, [chain of file:line])` for each finding in a followed action.
fn followed(report: &serde_json::Value) -> Vec<(String, Vec<Vec<String>>)> {
    let findings = report["findings"].as_array().unwrap();
    findings
        .iter()
        .filter(|finding| finding.get("via").is_some())
        .map(|finding| {
            let at = format!(
                "{}:{} {}",
                short(finding["path"].as_str().unwrap()),
                finding["line"],
                finding["rule"].as_str().unwrap()
            );
            let chains = finding["via"].as_array().unwrap();
            let chains = chains
                .iter()
                .map(|chain| {
                    let links = chain.as_array().unwrap();
                    links
                        .iter()
                        .map(|link| {
                            format!("{}:{}", short(link["path"].as_str().unwrap()), link["line"])
                        })
                        .collect()
                })
                .collect();
            (at, chains)
        })
        .collect()
}

/// The requests that read actions: ref resolution and file contents, sorted.
fn reads(log: &Log) -> Vec<String> {
    let mut reads: Vec<String> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|path| path.contains("/contents/") || path.contains("/commits/"))
        .map(|path| path.replace(COMMIT, "C"))
        .collect();
    reads.sort();
    reads
}

fn chain(links: &[&str]) -> Vec<String> {
    links.iter().map(|link| (*link).to_owned()).collect()
}

const CI: &str = "caller/.github/workflows/ci.yml";

#[test]
fn followed_actions_report_their_findings_in_the_action_file_with_every_chain() {
    let (api, log) = api(&[]);
    let output = follow(&api, &["--no-config", "--format", "json"], &[workflows()]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    let report = json(&output);
    assert_eq!(report["errors"], serde_json::json!([]), "{report:#}");
    // outer, sub, inner, node, docker, yamlonly, ping, pong.
    assert_eq!(report["summary"]["referenced_actions"], 8, "{report:#}");
    let root = |line: u32| format!("{CI}:{line}");
    let expected = vec![
        // `fixture/Outer/@v1` is the same action as `fixture/outer@v1`; the ignore on line 35
        // drops that chain for `unpinned-action` only.
        (
            "outer:action.yml:6 unpinned-action",
            vec![vec![root(12)], vec![root(14)]],
        ),
        ("node:action.yml:4 retired-runtime", vec![vec![root(16)]]),
        ("docker:action.yml:5 unpinned-action", vec![vec![root(18)]]),
        (
            "yamlonly:action.yaml:6 unpinned-action",
            vec![vec![root(20)]],
        ),
        ("ping:action.yml:6 unpinned-action", vec![vec![root(22)]]),
        // The author's ignore comment on inner's line 11 is not honored.
        (
            "inner:action.yml:11 unpinned-action",
            vec![
                chain(&[&root(12), "outer:action.yml:6"]),
                chain(&[&root(14), "outer:action.yml:6"]),
            ],
        ),
        (
            "inner:action.yml:12 script-injection",
            vec![
                chain(&[&root(12), "outer:action.yml:6"]),
                chain(&[&root(14), "outer:action.yml:6"]),
                chain(&[&root(35), "outer:action.yml:6"]),
            ],
        ),
        (
            "pong:action.yml:6 unpinned-action",
            vec![chain(&[&root(22), "ping:action.yml:6"])],
        ),
    ];
    let expected: Vec<(String, Vec<Vec<String>>)> = expected
        .into_iter()
        .map(|(at, chains)| (at.to_owned(), chains))
        .collect();
    assert_eq!(followed(&report), expected, "{report:#}");

    // Each action is read once at its commit: `fixture/ping@v1` is called from ci.yml and from
    // pong, and `fixture/Outer/@v1` is `fixture/outer@v1`. A full SHA needs no ref request,
    // `$/sub` is outer's own repository at outer's commit, and `action.yaml` is read only after
    // `action.yml` is missing. `./`, `docker://`, `${{ }}`, and the reusable workflow are not
    // requested at all.
    let c = "ref=C";
    let missing = "ref=0123456789012345678901234567890123456789";
    let expected_reads = [
        "/repos/fixture/docker/commits/v1".to_owned(),
        format!("/repos/fixture/docker/contents/action.yml?{c}"),
        "/repos/fixture/inner/commits/main".to_owned(),
        format!("/repos/fixture/inner/contents/action.yml?{c}"),
        "/repos/fixture/missing/commits/v1".to_owned(),
        format!("/repos/fixture/node/contents/action.yaml?{missing}"),
        format!("/repos/fixture/node/contents/action.yml?{missing}"),
        format!("/repos/fixture/node/contents/action.yml?{c}"),
        "/repos/fixture/outer/commits/v1".to_owned(),
        format!("/repos/fixture/outer/contents/action.yml?{c}"),
        format!("/repos/fixture/outer/contents/sub/action.yml?{c}"),
        "/repos/fixture/ping/commits/v1".to_owned(),
        format!("/repos/fixture/ping/contents/action.yml?{c}"),
        "/repos/fixture/pong/commits/v1".to_owned(),
        format!("/repos/fixture/pong/contents/action.yml?{c}"),
        "/repos/fixture/yamlonly/commits/v1".to_owned(),
        format!("/repos/fixture/yamlonly/contents/action.yaml?{c}"),
        format!("/repos/fixture/yamlonly/contents/action.yml?{c}"),
        "/repos/octo-org/lint/commits/main".to_owned(),
        "/repos/octo-org/setup-tool/commits/v2".to_owned(),
    ];
    assert_eq!(reads(&log), expected_reads);

    // What could not be read is named once, with the line that calls it; none fails the run.
    for (line, uses, reason) in [
        (
            26,
            "fixture/missing@v1",
            "the repository or ref does not exist",
        ),
        (
            24,
            "fixture/node@0123456789012345678901234567890123456789",
            "no action.yml or action.yaml at that commit",
        ),
    ] {
        let warning = format!("ci.yml:{line}: `{uses}`: ");
        let named: Vec<&str> = stderr
            .lines()
            .filter(|text| text.contains(&warning))
            .collect();
        assert_eq!(named.len(), 1, "{warning}: {stderr}");
        assert!(named[0].contains(reason), "{stderr}");
    }
}

#[test]
fn every_format_names_the_chain_and_points_annotations_at_the_scanned_uses_line() {
    let (api, _) = api(&[]);
    let inner = format!("fixture/inner@{COMMIT}:action.yml");
    let outer = format!("fixture/outer@{COMMIT}:action.yml");
    let ci = "tests/fixtures/follow/caller/.github/workflows/ci.yml";

    let text = follow(&api, &["--no-config", "--color", "never"], &[workflows()]);
    let text = String::from_utf8(text.stdout).unwrap();
    let block = format!(
        "error[script-injection]: untrusted `inputs.name` expands into a script\n  \
         --> {inner}:12:7\n   |\n12 |     - run: echo \"hello ${{{{ inputs.name }}}}\"\n   |       \
         ^^^ step 2\n   |\n   = via: {ci}:12 `fixture/outer@v1` > {outer}:6 `fixture/inner@main`\n   \
         = via: 2 more scanned `uses:` lines lead here (`--format json` lists every chain)\n"
    );
    assert!(text.contains(&block), "{text}");
    assert!(
        text.contains("Checked 1 workflow file, 1 action file, and 8 referenced actions: "),
        "{text}"
    );

    // Code scanning shows only files of the scanned repository, so a result sits on the first
    // chain's `uses:` line and its message names the action's file and line.
    let sarif = follow(&api, &["--no-config", "--format", "sarif"], &[workflows()]);
    let sarif: serde_json::Value = serde_json::from_slice(&sarif.stdout).unwrap();
    let results = sarif["runs"][0]["results"].as_array().unwrap();
    let result = results
        .iter()
        .find(|result| {
            result["ruleId"] == "script-injection"
                && result["message"]["text"]
                    .as_str()
                    .unwrap()
                    .starts_with(&inner)
        })
        .unwrap_or_else(|| panic!("{sarif:#}"));
    assert_eq!(
        result["message"]["text"],
        format!(
            "{inner}:12: step 2: untrusted `inputs.name` expands into a script (via {ci}:12 \
             `fixture/outer@v1` > {outer}:6 `fixture/inner@main`; 2 more scanned `uses:` lines \
             lead here)"
        )
    );
    assert_eq!(
        result["locations"][0]["physicalLocation"],
        serde_json::json!({
            "artifactLocation": { "uri": ci },
            "region": { "startLine": 12, "startColumn": 9, "endLine": 12, "endColumn": 13 },
        })
    );

    for (format, prefix) in [
        (
            "github",
            format!(
                "::error file={ci},line=12,endLine=12,col=9,endColumn=13,title=script-injection::\
                 {inner}:12: step 2: "
            ),
        ),
        (
            "agent",
            format!("{ci}:12:9: error script-injection: {inner}:12: step 2: "),
        ),
    ] {
        let output = follow(&api, &["--no-config", "--format", format], &[workflows()]);
        let output = String::from_utf8(output.stdout).unwrap();
        assert!(
            output.lines().any(|line| line.starts_with(&prefix)
                && line.contains(&format!("(via {ci}:12 `fixture/outer@v1` > {outer}:6"))),
            "{format}: {output}"
        );
    }
}

#[test]
fn ignores_on_the_scanned_uses_line_and_the_config_cover_the_chain_below_it() {
    let (api, _) = api(&[]);
    let temp = TempDir::new("follow-config");
    let config = temp.join("crosgha.yml");
    // Lines 12, 14, and 35 are the three `uses:` lines that reach inner; line 14 stays.
    fs::write(
        &config,
        "rules:\n  script-injection:\n    ignore:\n      - ci.yml:12\n      - ci.yml:35\n  \
         retired-runtime:\n    disable: true\n  unpinned-action:\n    remap:\n      severity: high\n",
    )
    .unwrap();
    let config = config.to_str().unwrap();
    let report = json(&follow(
        &api,
        &["--config", config, "--format", "json", "--fail-on", "none"],
        &[workflows()],
    ));
    let found = followed(&report);
    let injection: Vec<&Vec<Vec<String>>> = found
        .iter()
        .filter(|(at, _)| at.ends_with("script-injection"))
        .map(|(_, chains)| chains)
        .collect();
    assert_eq!(
        injection,
        [&vec![chain(&[&format!("{CI}:14"), "outer:action.yml:6"])]],
        "{report:#}"
    );
    assert!(
        found.iter().all(|(at, _)| !at.ends_with("retired-runtime")),
        "{report:#}"
    );
    let severities: Vec<&serde_json::Value> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding.get("via").is_some() && finding["rule"] == "unpinned-action")
        .map(|finding| &finding["severity"])
        .collect();
    assert!(
        !severities.is_empty() && severities.iter().all(|severity| *severity == "error"),
        "{report:#}"
    );

    // `--no-ignores` keeps the chain through the line that ignores `unpinned-action`.
    let report = json(&follow(
        &api,
        &[
            "--no-config",
            "--no-ignores",
            "--format",
            "json",
            "--fail-on",
            "none",
        ],
        &[workflows()],
    ));
    let outer = followed(&report)
        .into_iter()
        .find(|(at, _)| at == "outer:action.yml:6 unpinned-action")
        .unwrap();
    assert_eq!(outer.1.len(), 3, "{report:#}");
}

#[test]
fn a_remote_input_follows_the_same_actions_as_its_local_copy() {
    let (api, _) = api(&[]);
    let args = ["--no-config", "--format", "json", "--fail-on", "none"];
    let local = followed(&json(&follow(&api, &args, &[workflows()])));
    let remote = followed(&json(&follow(
        &api,
        &args,
        &[PathBuf::from("fixture/caller")],
    )));
    let rooted = |found: Vec<(String, Vec<Vec<String>>)>| -> Vec<(String, Vec<Vec<String>>)> {
        found
            .into_iter()
            .map(|(at, chains)| {
                let chains = chains
                    .into_iter()
                    .map(|chain| {
                        chain
                            .into_iter()
                            .map(|link| link.replace("caller:.github", "caller/.github"))
                            .collect()
                    })
                    .collect();
                (at, chains)
            })
            .collect()
    };
    assert!(!local.is_empty());
    assert_eq!(rooted(remote), local);
}

#[test]
fn nesting_stops_at_the_runners_composite_depth_limit() {
    let (api, log) = api(&[]);
    let output = follow(
        &api,
        &["--no-config", "--format", "json", "--fail-on", "none"],
        &[fixture("follow/deep.yml")],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    let report = json(&output);
    let found = followed(&report);
    let levels: Vec<&str> = found.iter().map(|(at, _)| at.as_str()).collect();
    let expected: Vec<String> = (1..=10)
        .map(|level| format!("deep:{level}/action.yml:6 unpinned-action"))
        .collect();
    assert_eq!(levels, expected, "{report:#}");
    let deepest = &found[9].1;
    assert_eq!(deepest.len(), 1);
    assert_eq!(deepest[0].len(), 10, "{deepest:?}");
    let requests = log.lock().unwrap().clone();
    assert!(
        !requests.iter().any(|path| path.contains("deep/11")),
        "{requests:?}"
    );
    assert!(
        stderr.contains(&format!(
            "fixture/deep@{COMMIT}:10/action.yml: its 1 call(s) nest deeper than the runner's \
             limit of 9 composite actions, so they were not followed"
        )),
        "{stderr}"
    );
}

#[test]
fn a_failed_read_skips_only_that_action_and_a_rate_limit_stops_the_walk() {
    const DOCKER: &str =
        "/repos/fixture/docker/contents/action.yml?ref=f00df00df00df00df00df00df00df00df00df00d";
    const LEVEL_5: &str =
        "/repos/fixture/deep/contents/5/action.yml?ref=f00df00df00df00df00df00df00df00df00df00d";
    let (api, log) = api(&[
        (DOCKER, "502 Bad Gateway", "", "Server Error"),
        (
            LEVEL_5,
            "403 Forbidden",
            "x-ratelimit-remaining: 0\r\nx-ratelimit-reset: 9999999999\r\n",
            "API rate limit exceeded",
        ),
    ]);
    let inputs = [workflows().join("ci.yml"), fixture("follow/deep.yml")];
    let output = follow(&api, &["--no-config", "--format", "json"], &inputs);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    let report = json(&output);
    let found = followed(&report);
    let levels: Vec<&str> = found
        .iter()
        .map(|(at, _)| at.as_str())
        .filter(|at| at.starts_with("deep:"))
        .collect();
    assert_eq!(
        levels,
        (1..=4)
            .map(|level| format!("deep:{level}/action.yml:6 unpinned-action"))
            .collect::<Vec<_>>(),
        "{report:#}"
    );
    assert!(
        found.iter().all(|(at, _)| !at.starts_with("docker:")),
        "{report:#}"
    );
    assert!(
        found.iter().any(|(at, _)| at.starts_with("inner:")),
        "{report:#}"
    );
    let errors: Vec<&str> = report["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|error| error["message"].as_str().unwrap())
        .collect();
    assert_eq!(errors.len(), 2, "{report:#}");
    assert!(
        errors[0].ends_with(&format!(
            "ci.yml:18: `fixture/docker@v1` was not checked: GitHub API {DOCKER}: HTTP 502 \
             (Server Error)"
        )),
        "{errors:?}"
    );
    assert!(
        errors[1].starts_with(&format!(
            "GitHub API {LEVEL_5}: HTTP 403 (API rate limit exceeded)"
        )),
        "{errors:?}"
    );
    assert!(
        errors[1].ends_with("; 1 referenced action(s) were not read"),
        "{errors:?}"
    );
    // After the rate limit, nothing more is requested.
    let requests = log.lock().unwrap().clone();
    let limited = requests.iter().position(|path| path == LEVEL_5).unwrap();
    assert_eq!(limited, requests.len() - 1, "{requests:?}");

    let text = follow(&api, &["--no-config"], &inputs);
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("The scan is incomplete"), "{text}");
}

#[test]
fn following_is_opt_in_and_needs_the_api() {
    let (api, log) = api(&[]);
    let plain = bin()
        .args(["--persona", "pedantic", "--no-config", "--fail-on", "none"])
        .args(["--gh-token", TOKEN, "--gh-hostname", &api])
        .arg(workflows())
        .output()
        .unwrap();
    assert_eq!(plain.status.code(), Some(0));
    assert_eq!(reads(&log), Vec::<String>::new());

    let tokenless = bin()
        .args(["--follow-actions", "--gh-hostname", &api])
        .arg(workflows())
        .output()
        .unwrap();
    assert_eq!(tokenless.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&tokenless.stderr)
            .contains("--follow-actions requires --gh-token, GH_TOKEN, or GITHUB_TOKEN")
    );
    for flag in ["--offline", "--no-online-audits", "--fix"] {
        let refused = bin()
            .args([
                "--follow-actions",
                flag,
                "--gh-token",
                TOKEN,
                "--gh-hostname",
                &api,
            ])
            .arg(workflows())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&refused.stderr);
        assert_eq!(refused.status.code(), Some(2), "{flag}: {stderr}");
        assert!(stderr.contains("cannot be used with"), "{flag}: {stderr}");
    }
    assert_eq!(reads(&log), Vec::<String>::new());
}
