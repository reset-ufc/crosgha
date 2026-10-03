//! `run` scripts read as simple commands: the program each one runs and its options.

use super::expression;

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
    if !commands.last().unwrap().words.is_empty() {
        commands.push(Command {
            words: Vec::new(),
            piped: false,
        });
    }
    let or = bytes.get(index + 1) == Some(&b'|') || index > 0 && bytes[index - 1] == b'|';
    if bytes[index] == b'|' && !or {
        commands.last_mut().unwrap().piped = true;
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
            commands.last_mut().unwrap().words.push(word);
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
