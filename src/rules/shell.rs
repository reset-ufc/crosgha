//! `run` scripts read as simple commands: the program each one runs and its options, where a
//! variable expands unquoted, and the shell that runs the script.

use std::ops::Range;

use yaml_serde::Value;

use super::common::{Job, Step};
use super::{expression, runner};
use crate::workflow::Workflow;

const NOT_EMPTY: &str = "`commands` starts with one command and only grows while splitting";

/// A simple command; `piped` when it reads the previous command's output through `|`.
pub struct Command<'a> {
    pub words: Vec<&'a str>,
    pub piped: bool,
}

/// The simple commands of `script`, split on newlines, `;`, `|`, `&`, `(`, `)`, and backticks.
/// Quotes around a word are dropped, a `${{ }}` expression stays inside one word, a `\` line
/// continuation is a space, and a `#` that starts a word outside quotes ends the line.
// ponytail: separators are not quote-aware, so `echo "a; b"` splits; parse quoting if that
// produces false findings in practice.
pub fn commands(script: &str) -> Vec<Command<'_>> {
    let bytes = script.as_bytes();
    let mut commands = vec![Command {
        words: Vec::new(),
        piped: false,
    }];
    let mut start = None;
    let mut quote = None;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if bytes[index..].starts_with(b"${{") {
            start.get_or_insert(index);
            index =
                expression::end(&script[index + 3..]).map_or(bytes.len(), |end| index + end + 5);
            continue;
        }
        if byte == b'#' && start.is_none() && quote.is_none() {
            index = script[index..]
                .find('\n')
                .map_or(bytes.len(), |end| index + end);
            continue;
        }
        let continuation = byte == b'\\' && bytes.get(index + 1) == Some(&b'\n');
        if continuation || matches!(byte, b' ' | b'\t' | b'\r') {
            end_word(&mut commands, script, &mut start, index);
            index += 1 + usize::from(continuation);
            continue;
        }
        if matches!(byte, b'\n' | b';' | b'|' | b'&' | b'(' | b')' | b'`') {
            end_word(&mut commands, script, &mut start, index);
            quote = quote.filter(|_| byte != b'\n');
            separate(&mut commands, bytes, index);
        } else {
            quote = match (quote, byte) {
                (None, b'\'' | b'"') => Some(byte),
                (Some(open), _) if open == byte => None,
                (open, _) => open,
            };
            start.get_or_insert(index);
        }
        index += 1;
    }
    end_word(&mut commands, script, &mut start, bytes.len());
    commands.retain(|command| !command.words.is_empty());
    commands
}

/// Starts the next command at the separator `bytes[index]`; a single `|` pipes into it.
fn separate(commands: &mut Vec<Command<'_>>, bytes: &[u8], index: usize) {
    if !commands.last().expect(NOT_EMPTY).words.is_empty() {
        commands.push(Command {
            words: Vec::new(),
            piped: false,
        });
    }
    let or = bytes.get(index + 1) == Some(&b'|') || index > 0 && bytes[index - 1] == b'|';
    if bytes[index] == b'|' && !or {
        commands.last_mut().expect(NOT_EMPTY).piped = true;
    }
}

fn end_word<'a>(
    commands: &mut [Command<'a>],
    script: &'a str,
    start: &mut Option<usize>,
    end: usize,
) {
    if let Some(begin) = start.take() {
        let word = script[begin..end].trim_matches(['"', '\'']);
        if !word.is_empty() {
            commands.last_mut().expect(NOT_EMPTY).words.push(word);
        }
    }
}

/// The program a command runs: `path` as written, `name` its file name, and the words after it.
pub struct Program<'a, 'w> {
    pub path: &'a str,
    pub name: &'a str,
    pub args: &'w [&'a str],
}

/// Reserved words that may precede a command.
const KEYWORDS: &[&str] = &[
    "if", "then", "elif", "else", "while", "until", "do", "!", "{", "}", "time",
];

/// Commands that run the next operand as the program, with their options that take a value:
/// sudo 1.9 and GNU env `--help`; bash's `exec -a name`.
const WRAPPERS: &[(&str, Spec)] = &[
    (
        "sudo",
        Spec {
            short: &['C', 'D', 'g', 'h', 'p', 'R', 'r', 't', 'T', 'U', 'u'],
            long: &[
                "close-from",
                "chdir",
                "group",
                "host",
                "prompt",
                "chroot",
                "role",
                "type",
                "command-timeout",
                "other-user",
                "user",
            ],
        },
    ),
    (
        "env",
        Spec {
            short: &['u', 'C', 'S'],
            long: &["unset", "chdir", "split-string"],
        },
    ),
    (
        "exec",
        Spec {
            short: &['a'],
            long: &[],
        },
    ),
    ("command", Spec::NONE),
    ("nohup", Spec::NONE),
];

/// The program `words` runs, past reserved words, variable assignments, and wrappers such as
/// `sudo` or `env` with their options.
pub fn program<'a, 'w>(words: &'w [&'a str]) -> Option<Program<'a, 'w>> {
    let mut rest = words;
    loop {
        let (&word, after) = rest.split_first()?;
        let name = basename(word);
        if KEYWORDS.contains(&word) || is_assignment(word) {
            rest = after;
        } else if let Some((_, spec)) = WRAPPERS.iter().find(|(wrapper, _)| *wrapper == name) {
            rest = leading_options(after, spec).1;
        } else {
            return Some(Program {
                path: word,
                name,
                args: after,
            });
        }
    }
}

pub fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        name.starts_with(|char: char| char.is_ascii_alphabetic() || char == '_')
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    })
}

/// The options of a program that take a value: short letters and long names without `--`.
pub struct Spec {
    pub short: &'static [char],
    pub long: &'static [&'static str],
}

impl Spec {
    pub const NONE: Spec = Spec {
        short: &[],
        long: &[],
    };
}

pub enum Arg<'a> {
    Short(char, Option<&'a str>),
    Long(&'a str, Option<&'a str>),
    Operand(&'a str),
}

/// Reads arguments as options and operands, getopt style: `-xvf` sets each letter, a valued
/// letter or long option takes the rest of its word or the next word (`--name=value` too), and
/// `--` ends the options. Options may follow operands.
pub struct Args<'a, 'w> {
    words: &'w [&'a str],
    spec: &'w Spec,
    cluster: &'a str,
    ended: bool,
}

pub fn args<'a, 'w>(words: &'w [&'a str], spec: &'w Spec) -> Args<'a, 'w> {
    Args {
        words,
        spec,
        cluster: "",
        ended: false,
    }
}

impl<'a> Args<'a, '_> {
    fn take(&mut self) -> Option<&'a str> {
        let (&first, rest) = self.words.split_first()?;
        self.words = rest;
        Some(first)
    }
}

impl<'a> Iterator for Args<'a, '_> {
    type Item = Arg<'a>;

    fn next(&mut self) -> Option<Arg<'a>> {
        if let Some(letter) = self.cluster.chars().next() {
            let attached = &self.cluster[letter.len_utf8()..];
            if !self.spec.short.contains(&letter) {
                self.cluster = attached;
                return Some(Arg::Short(letter, None));
            }
            self.cluster = "";
            let value = match attached.is_empty() {
                true => self.take(),
                false => Some(attached),
            };
            return Some(Arg::Short(letter, value));
        }
        let word = self.take()?;
        if self.ended || word == "-" || !word.starts_with('-') {
            return Some(Arg::Operand(word));
        }
        if word == "--" {
            self.ended = true;
            return self.next();
        }
        if let Some(long) = word.strip_prefix("--") {
            return Some(match long.split_once('=') {
                Some((name, value)) => Arg::Long(name, Some(value)),
                None if self.spec.long.contains(&long) => Arg::Long(long, self.take()),
                None => Arg::Long(long, None),
            });
        }
        self.cluster = &word[1..];
        self.next()
    }
}

/// The options before the first operand, and the words from that operand on.
pub fn leading_options<'a, 'w>(
    words: &'w [&'a str],
    spec: &'w Spec,
) -> (Vec<Arg<'a>>, &'w [&'a str]) {
    let mut options = Vec::new();
    let mut reader = args(words, spec);
    while let Some(arg) = reader.next() {
        if let Arg::Operand(_) = arg {
            return (options, &words[words.len() - reader.words.len() - 1..]);
        }
        options.push(arg);
    }
    (options, &[])
}

/// git's global options that take their value as the next word:
/// https://git-scm.com/docs/git#_options
const GIT: Spec = Spec {
    short: &['C', 'c'],
    long: &["git-dir", "work-tree", "namespace", "config-env"],
};

/// The subcommand `program` runs when it is `git`, and the words after it.
pub fn git_subcommand<'a, 'w>(program: &Program<'a, 'w>) -> Option<(&'a str, &'w [&'a str])> {
    if program.name != "git" {
        return None;
    }
    let (_, rest) = leading_options(program.args, &GIT);
    let (&subcommand, args) = rest.split_first()?;
    Some((subcommand, args))
}

fn default_shell(scope: &Value) -> Option<&str> {
    scope
        .get("defaults")
        .and_then(|defaults| defaults.get("run"))
        .and_then(|run| run.get("shell"))
        .and_then(Value::as_str)
}

/// Whether bash or sh runs `step`'s script: its `shell`, else the job's or the workflow's
/// `defaults.run.shell`, else the runner's default. A custom shell such as `/usr/bin/bash -e {0}`
/// names it by path, and `/usr/bin/env [options] [NAME=value] bash {0}` looks it up.
pub fn runs_posix(workflow: &Workflow, job: &Job, step: &Step) -> bool {
    // `run` defaults to pwsh on Windows and to bash elsewhere, so a matrix over both OSes still
    // runs bash on some entries:
    // https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idstepsshell
    let labels: Vec<String> = runner::job_labels(job)
        .iter()
        .map(|label| label.to_ascii_lowercase())
        .collect();
    let windows = labels.iter().any(|label| label.starts_with("windows"))
        && !labels.iter().any(|label| {
            ["ubuntu", "macos", "linux"]
                .iter()
                .any(|os| label.starts_with(os))
        });
    let shell = step
        .value
        .get("shell")
        .and_then(Value::as_str)
        .or_else(|| default_shell(job.value))
        .or_else(|| default_shell(&workflow.value))
        .or(windows.then_some("pwsh"))
        .unwrap_or("bash");
    let words: Vec<&str> = shell.split_whitespace().collect();
    program(&words).is_some_and(|program| matches!(program.name, "bash" | "sh"))
}

/// Whether the step's `run` scalar holds `script` as written, so a line of it can be rewritten in
/// place: a literal block scalar, or a one-line plain scalar outside a flow mapping, where `{`,
/// `}`, and `,` would end it. YAML folds `>` scalars and unescapes quoted ones.
pub fn verbatim(workflow: &Workflow, step: &Step, script: &str) -> bool {
    let document = &workflow.document;
    let (Ok(Some(node)), Ok(Some(mapping))) = (
        document.query_exact(&step.key("run")),
        document.query_exact(&step.route),
    ) else {
        return false;
    };
    let source = document.extract(&node);
    source.starts_with('|') || (source == script && !document.extract(&mapping).starts_with('{'))
}

/// Where the shell reads a `run` script, the innermost one last.
enum Frame {
    /// Shell code with the parentheses still open in it; `$(` opens one.
    Code(usize),
    /// `((` or `$((`, where quotes are not shell quoting.
    Arithmetic(usize),
    Backtick,
    Double,
    /// `${...}` of another variable, whose `#`, `%`, and `/` operands are patterns.
    Parameter,
}

/// A heredoc whose body starts on the next line.
struct Heredoc {
    delimiter: String,
    /// `<<-`, whose delimiter line may start with tabs.
    strip_tabs: bool,
}

const OPEN: &str = "the script's own frame is never closed";

/// The `$NAME` and `${NAME}` expansions in `script`, for a name in `names`, that the shell reads
/// unquoted: outside quotes, comments, heredoc bodies, backticks, and arithmetic, where quoting
/// them keeps the path one word. `None` when one of them may be a pattern, which quoting turns
/// into a literal: inside `[[ ]]`, `case ... esac`, or another variable's `${...}`. Also `None`
/// when the script holds quoting this reader does not follow: `$'...'`, an unclosed quote, `$(`,
/// or `${`, or a heredoc without its delimiter line.
// ponytail: `)` closes `$(` without parsing `case` patterns inside it, and any `[[ ]]` or `case`
// operand counts as a pattern; parse the shell grammar if that misreads real scripts.
pub fn unquoted_expansions(script: &str, names: &[&str]) -> Option<Vec<Range<usize>>> {
    let bytes = script.as_bytes();
    let mut frames = vec![Frame::Code(0)];
    let mut heredocs: Vec<Heredoc> = Vec::new();
    let mut found = Vec::new();
    // Open `[[` and `case` constructs.
    let mut patterns = 0_usize;
    let mut index = 0;
    while index < bytes.len() {
        let rest = &bytes[index..];
        if rest.starts_with(b"${{") {
            index =
                expression::end(&script[index + 3..]).map_or(bytes.len(), |end| index + end + 5);
            continue;
        }
        let nested = frames.len() > 1;
        match frames.last_mut().expect(OPEN) {
            Frame::Double => match rest {
                [b'\\', ..] => index += 1,
                [b'"', ..] => {
                    frames.pop();
                }
                [b'`', ..] => frames.push(Frame::Backtick),
                [b'$', b'(', b'(', ..] => {
                    frames.push(Frame::Arithmetic(2));
                    index += 2;
                }
                [b'$', b'(', ..] => {
                    frames.push(Frame::Code(1));
                    index += 1;
                }
                _ => {}
            },
            Frame::Backtick => match rest {
                [b'\\', ..] => index += 1,
                [b'`', ..] => {
                    frames.pop();
                }
                _ => {}
            },
            Frame::Parameter => match rest {
                [b'\\', ..] => index += 1,
                [b'}', ..] => {
                    frames.pop();
                }
                [b'$', ..] if expansion_length(&script[index..], names).is_some() => return None,
                [b'$', b'{', ..] => {
                    frames.push(Frame::Parameter);
                    index += 1;
                }
                _ => {}
            },
            Frame::Arithmetic(depth) => match rest {
                [b'(', ..] => *depth += 1,
                [b')', ..] if *depth == 1 => {
                    frames.pop();
                }
                [b')', ..] => *depth -= 1,
                _ => {}
            },
            Frame::Code(depth) => match rest {
                [b'\\', ..] => index += 1,
                [b'\'', ..] => index += script[index + 1..].find('\'')? + 1,
                [b'"', ..] => frames.push(Frame::Double),
                [b'`', ..] => frames.push(Frame::Backtick),
                [b'#', ..] if starts_word(bytes, index) => {
                    index = script[index..]
                        .find('\n')
                        .map_or(bytes.len(), |end| index + end);
                    continue;
                }
                [b'(', b'(', ..] | [b'$', b'(', b'(', ..] => {
                    index += usize::from(rest[0] == b'$') + 1;
                    frames.push(Frame::Arithmetic(2));
                }
                [b'$', b'(', ..] => {
                    frames.push(Frame::Code(1));
                    index += 1;
                }
                [b'(', ..] => *depth += 1,
                [b')', ..] if *depth == 1 && nested => {
                    frames.pop();
                }
                [b')', ..] => *depth = depth.saturating_sub(1),
                [b'$', b'\'', ..] => return None,
                [b'$', b'$' | b'#', ..] => index += 1,
                [b'$', ..] if let Some(length) = expansion_length(&script[index..], names) => {
                    if patterns > 0 {
                        return None;
                    }
                    found.push(index..index + length);
                    index += length - 1;
                }
                [b'$', b'{', ..] => {
                    frames.push(Frame::Parameter);
                    index += 1;
                }
                [b'[', b'[', ..] if keyword(bytes, index, b"[[") => {
                    patterns += 1;
                    index += 1;
                }
                [b'c', ..] if keyword(bytes, index, b"case") => patterns += 1,
                [b']', b']', ..] if keyword(bytes, index, b"]]") => {
                    patterns = patterns.saturating_sub(1);
                    index += 1;
                }
                [b'e', ..] if keyword(bytes, index, b"esac") => {
                    patterns = patterns.saturating_sub(1);
                }
                [b'<', b'<', b'<', ..] => index += 2,
                [b'<', b'<', ..] => {
                    let (heredoc, end) = heredoc(script, index + 2)?;
                    heredocs.push(heredoc);
                    index = end;
                    continue;
                }
                [b'\n', ..] if !heredocs.is_empty() => {
                    index = heredoc_bodies_end(script, index + 1, &heredocs)?;
                    heredocs.clear();
                    continue;
                }
                _ => {}
            },
        }
        index += 1;
    }
    (frames.len() == 1 && heredocs.is_empty()).then_some(found)
}

/// Whether a `#` at `index` starts a comment rather than sitting inside a word.
fn starts_word(bytes: &[u8], index: usize) -> bool {
    index == 0
        || matches!(
            bytes[index - 1],
            b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'('
        )
}

/// Whether `word` stands as a word of its own at `index`.
fn keyword(bytes: &[u8], index: usize, word: &[u8]) -> bool {
    starts_word(bytes, index)
        && bytes[index..].starts_with(word)
        && bytes
            .get(index + word.len())
            .is_none_or(|next| matches!(next, b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b')'))
}

/// The length of `$NAME` or `${NAME}` at the start of `text`, for a name in `names` that is not
/// the start of a longer name.
fn expansion_length(text: &str, names: &[&str]) -> Option<usize> {
    let rest = text.strip_prefix('$')?;
    names.iter().find_map(|name| {
        if rest
            .strip_prefix('{')
            .and_then(|braced| braced.strip_prefix(name))
            .is_some_and(|after| after.starts_with('}'))
        {
            return Some(name.len() + 3);
        }
        let after = rest.strip_prefix(name)?;
        let whole = !after.starts_with(|next: char| next.is_ascii_alphanumeric() || next == '_');
        whole.then_some(name.len() + 1)
    })
}

/// The heredoc whose `<<` ends right before `start`, and the index after its delimiter word.
/// Quotes in the word only turn off expansion in the body, so the delimiter line omits them.
fn heredoc(script: &str, start: usize) -> Option<(Heredoc, usize)> {
    let strip_tabs = script[start..].starts_with('-');
    let after = script[start + usize::from(strip_tabs)..].trim_start_matches([' ', '\t']);
    let word_start = script.len() - after.len();
    let word_end = word_start
        + after
            .find(|next: char| next.is_whitespace() || ";&|<>()".contains(next))
            .unwrap_or(after.len());
    let delimiter: String = script[word_start..word_end]
        .chars()
        .filter(|next| !matches!(next, '\'' | '"' | '\\'))
        .collect();
    if delimiter.is_empty() {
        return None;
    }
    let heredoc = Heredoc {
        delimiter,
        strip_tabs,
    };
    Some((heredoc, word_end))
}

/// The index after the bodies of `heredocs`, which start at `start` one after another, or `None`
/// when one has no delimiter line.
fn heredoc_bodies_end(script: &str, mut start: usize, heredocs: &[Heredoc]) -> Option<usize> {
    for heredoc in heredocs {
        loop {
            if start >= script.len() {
                return None;
            }
            let end = script[start..]
                .find('\n')
                .map_or(script.len(), |at| start + at);
            let line = match heredoc.strip_tabs {
                true => script[start..end].trim_start_matches('\t'),
                false => &script[start..end],
            };
            start = end + 1;
            if line == heredoc.delimiter {
                break;
            }
        }
    }
    Some(start.min(script.len()))
}
