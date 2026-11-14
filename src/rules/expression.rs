//! `${{ }}` expressions: where they are in text, the context paths they read, which of those an
//! outsider controls, and how a condition compares or negates them.

/// Byte ranges of every `${{ ... }}` in `text`, from `$` to the closing `}}`. A `}}` inside a
/// single-quoted string literal (`''` escapes a quote) does not close the expression:
/// https://docs.github.com/en/actions/reference/workflows-and-actions/expressions#literals
pub fn spans(text: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find("${{") {
        let start = from + offset;
        let Some(end) = end(&text[start + 3..]) else {
            break;
        };
        from = start + 3 + end + 2;
        found.push((start, from));
    }
    found
}

/// Whether `text` is one `${{ ... }}` and nothing else, the only form GitHub evaluates as a
/// condition once `${{` appears in it. Any other text makes the value a string.
pub fn single(text: &str) -> bool {
    spans(text) == [(0, text.len())]
}

/// `text` with each `${{ ... }}` replaced by its body.
pub fn strip_delimiters(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for (start, end) in spans(text) {
        out.push_str(&text[from..start]);
        out.push_str(&text[start + 3..end - 2]);
        from = end;
    }
    out.push_str(&text[from..]);
    out
}

/// The bodies of every `${{ ... }}` expression in `text`.
pub fn expressions(text: &str) -> Vec<&str> {
    spans(text)
        .into_iter()
        .map(|(start, end)| text[start + 3..end - 2].trim())
        .collect()
}

/// Offset of the `}}` that closes the expression `body` starts, outside string literals.
pub fn end(body: &str) -> Option<usize> {
    let bytes = body.as_bytes();
    let mut quoted = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            // An escaped `''` reads as closing and reopening the literal.
            b'\'' => quoted = !quoted,
            b'}' if !quoted && bytes.get(index + 1) == Some(&b'}') => return Some(index),
            _ => {}
        }
        index += 1;
    }
    None
}

/// Context paths GitHub documents as attacker-controllable: `github.head_ref` and
/// `github.event.*` values ending in one of these names.
/// https://docs.github.com/en/actions/concepts/security/script-injections
const UNTRUSTED_SUFFIXES: &[&str] = &[
    "body",
    "default_branch",
    "email",
    "head_ref",
    "label",
    "message",
    "name",
    "page_name",
    "ref",
    "title",
    // `workflow_run.head_branch` is the head_ref of the run's pull request under another name.
    "head_branch",
    // `workflow_run.display_title` defaults to the commit message or pull request title:
    // https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#run-name
    // and `display_title` in https://docs.github.com/en/rest/actions/workflow-runs
    "display_title",
];

/// `changes.<field>.from` of an `edited` event is the field's previous text, so an untrusted
/// `<field>` stays untrusted (`webhook-issues-edited` and the other `edited` payloads):
/// https://docs.github.com/en/webhooks/webhook-events-and-payloads
fn previous_untrusted_text(segments: &[&str]) -> bool {
    matches!(segments, [.., "changes", field, "from"] if UNTRUSTED_SUFFIXES.contains(field))
}

/// Event payload objects that carry the free text above; `toJSON` of one, or of `github` or
/// `github.event` as a whole, serializes that text into the script.
const FREE_TEXT_OBJECTS: &[&str] = &[
    "comment",
    "commits",
    "discussion",
    "head_commit",
    "issue",
    "pages",
    "pull_request",
    "review",
    "workflow_run",
];

/// Payload fields that hold a number, flag, hash, enum, timestamp, or GitHub URL, so `toJSON` of
/// one carries no free text; any other field below a free-text object may be an object holding it.
fn is_scalar_leaf(field: &str) -> bool {
    const SCALARS: &[&str] = &[
        "additions",
        "author_association",
        "changed_files",
        "comments",
        "commits",
        "deletions",
        "draft",
        "id",
        "locked",
        "merged",
        "number",
        "review_comments",
        "run_attempt",
        "run_number",
        "sha",
        "state",
        "url",
    ];
    SCALARS.contains(&field)
        || ["_at", "_count", "_id", "_sha", "_url"]
            .iter()
            .any(|suffix| field.ends_with(suffix))
}

/// Byte ranges of the `github...` context paths in `text`, which must already be lowercase and
/// dotted. A path runs over names, dots, and `[*]` filters.
pub fn context_paths(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find("github") {
        let start = from + offset;
        let mut end = start;
        while end < bytes.len() && (is_context_char(bytes[end]) || b".[]*".contains(&bytes[end])) {
            end += 1;
        }
        from = end.max(start + 1);
        let preceded = start > 0 && (is_context_char(bytes[start - 1]) || bytes[start - 1] == b'.');
        let github = text[start..end].split(['.', '[']).next() == Some("github");
        if !preceded && github {
            found.push((start, end));
        }
    }
    found
}

/// Untrusted context paths read by `expression`, lowercased and in dotted form. Contexts accept
/// `github['event']` as well as `github.event`:
/// https://docs.github.com/en/actions/reference/workflows-and-actions/contexts
/// and the runner looks keys up case-insensitively (`StringComparer.OrdinalIgnoreCase` in
/// actions/runner `DictionaryContextData.cs` at d7bc179baf11a02110b46cfbbc4040f74ac3f60a).
pub fn untrusted_contexts(expression: &str) -> Vec<String> {
    let expression = dotted(&expression.to_ascii_lowercase());
    if only_boolean(&expression) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (start, end) in context_paths(&expression) {
        let path = &expression[start..end];
        let segments: Vec<&str> = path
            .split('.')
            .map(|segment| segment.split('[').next().unwrap_or(segment))
            .filter(|segment| !segment.is_empty() && *segment != "*")
            .collect();
        let whole_object = expression[..start].trim_end().ends_with("tojson(")
            && match segments.as_slice() {
                ["github"] | ["github", "event"] => true,
                ["github", "event", object] => FREE_TEXT_OBJECTS.contains(object),
                ["github", "event", object, .., leaf] => {
                    FREE_TEXT_OBJECTS.contains(object) && !is_scalar_leaf(leaf)
                }
                _ => false,
            };
        let untrusted = segments.as_slice() == ["github", "head_ref"]
            || (segments.len() > 2
                && segments[1] == "event"
                && (segments
                    .last()
                    .is_some_and(|last| UNTRUSTED_SUFFIXES.contains(last))
                    || previous_untrusted_text(&segments)))
            || whole_object;
        if untrusted {
            found.push(path.to_owned());
        }
    }
    found
}

/// Whether `expression` (lowercase) can only evaluate to `true` or `false`, so none of the text it
/// reads comes out. `&&` and `||` return an operand, so every operand must be boolean too. Operators
/// and the functions that return booleans, loosest operator first:
/// https://docs.github.com/en/actions/reference/workflows-and-actions/expressions
fn only_boolean(expression: &str) -> bool {
    let expression = expression.trim();
    for operator in ["||", "&&"] {
        let operands = split_outside_groups(expression, operator);
        if operands.len() > 1 {
            return operands.into_iter().all(only_boolean);
        }
    }
    let compares = ["==", "!=", "<", ">"]
        .iter()
        .any(|operator| split_outside_groups(expression, operator).len() > 1);
    if compares || expression.starts_with('!') {
        return true;
    }
    match expression
        .strip_suffix(')')
        .and_then(|body| body.split_once('('))
    {
        Some(("", group)) => only_boolean(group),
        Some((name, _)) => ["contains", "startswith", "endswith"].contains(&name.trim_end()),
        None => false,
    }
}

/// `text` split on `operator` where it stands outside string literals, parentheses, and indexes.
fn split_outside_groups<'a>(text: &'a str, operator: &str) -> Vec<&'a str> {
    let bytes = text.as_bytes();
    let mut parts = Vec::new();
    let (mut start, mut depth, mut quoted) = (0, 0, false);
    for index in 0..bytes.len() {
        match bytes[index] {
            b'\'' => quoted = !quoted,
            _ if quoted => {}
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            _ if depth == 0
                && index >= start
                && bytes[index..].starts_with(operator.as_bytes()) =>
            {
                parts.push(&text[start..index]);
                start = index + operator.len();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

pub fn untrusted_in_text(text: &str) -> Vec<String> {
    expressions(text)
        .into_iter()
        .flat_map(untrusted_contexts)
        .collect()
}

/// Whether `expression` reads one named secret, in any casing and in property or index syntax. The
/// text between single quotes is a literal (`''` reads as closing and reopening one), so it is skipped.
pub fn reads_named_secret(expression: &str) -> bool {
    let text = dotted(&expression.to_ascii_lowercase());
    text.split('\'').step_by(2).any(|outside| {
        outside.match_indices("secrets").any(|(start, name)| {
            let preceded = outside[..start]
                .bytes()
                .next_back()
                .is_some_and(|byte| is_context_char(byte) || byte == b'.');
            !preceded && outside[start + name.len()..].trim_start().starts_with('.')
        })
    })
}

/// Rewrites index syntax with a literal name (`['body']`, `[ 'body' ]`) to property syntax
/// (`.body`).
pub fn dotted(expression: &str) -> String {
    let mut out = String::with_capacity(expression.len());
    let mut rest = expression;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let index = after.trim_start().strip_prefix('\'').and_then(|name| {
            let close = name.find('\'')?;
            let tail = name[close + 1..].trim_start().strip_prefix(']')?;
            name[..close]
                .bytes()
                .all(is_context_char)
                .then_some((&name[..close], tail))
        });
        match index {
            Some((name, tail)) => {
                out.push('.');
                out.push_str(name);
                rest = tail;
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn is_context_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

/// What a condition does with one context path: compares it with an operand, or reads it as a
/// boolean, possibly negated.
pub struct Test<'a> {
    pub operator: Operator<'a>,
    pub negated: bool,
}

/// An operand is the text of a single-quoted literal or the word next to the operator.
pub enum Operator<'a> {
    Equals(&'a str),
    Differs(&'a str),
    Truthy,
}

/// How the condition `text` (lowercase and dotted) tests the path at `text[start..end]`. A test is
/// negated by an odd number of `!`, `== false`, or `!= true` around it and its enclosing groups.
pub fn test(text: &str, start: usize, end: usize) -> Test<'_> {
    let before = text[..start].trim_end();
    let after = text[end..].trim_start();
    let operator = if let Some(rest) = after.strip_prefix("==") {
        Operator::Equals(operand_after(rest.trim_start()))
    } else if let Some(rest) = after.strip_prefix("!=") {
        Operator::Differs(operand_after(rest.trim_start()))
    } else if let Some(rest) = before.strip_suffix("==") {
        Operator::Equals(operand_before(rest.trim_end()))
    } else if let Some(rest) = before.strip_suffix("!=") {
        Operator::Differs(operand_before(rest.trim_end()))
    } else {
        Operator::Truthy
    };
    let bang = matches!(operator, Operator::Truthy) && before.ends_with('!');
    let negations = usize::from(bang) + negations(text, start);
    Test {
        operator,
        negated: !negations.is_multiple_of(2),
    }
}

fn is_operand_char(char: char) -> bool {
    char.is_ascii_alphanumeric() || "_-.[]*".contains(char)
}

fn operand_after(text: &str) -> &str {
    match text.strip_prefix('\'') {
        Some(rest) => rest.find('\'').map_or(rest, |close| &rest[..close]),
        None => {
            &text[..text
                .find(|char| !is_operand_char(char))
                .unwrap_or(text.len())]
        }
    }
}

fn operand_before(text: &str) -> &str {
    match text.strip_suffix('\'') {
        Some(rest) => rest.rfind('\'').map_or(rest, |open| &rest[open + 1..]),
        None => {
            let start = text
                .char_indices()
                .rev()
                .find(|(_, char)| !is_operand_char(*char))
                .map_or(0, |(index, char)| index + char.len_utf8());
            &text[start..]
        }
    }
}

/// How many of the parenthesized groups and calls around `text[at]` are negated by a leading
/// `!`, a trailing `== false` or `!= true`, or a leading `false ==` or `true !=`.
fn negations(text: &str, mut at: usize) -> usize {
    let mut count = 0;
    while let Some((open, close)) = enclosing_group(text, at) {
        let head = text[..open].trim_end();
        let name_start = head
            .rfind(|char: char| !(char.is_ascii_alphanumeric() || char == '_'))
            .map_or(0, |index| index + 1);
        let before = head[..name_start].trim_end();
        let after = text[close + 1..].trim_start();
        let compared_false = ["== false", "!= true"]
            .iter()
            .any(|suffix| after.starts_with(suffix))
            || ["false ==", "true !="]
                .iter()
                .any(|prefix| before.ends_with(prefix));
        count += usize::from(before.ends_with('!')) + usize::from(compared_false);
        at = name_start;
    }
    count
}

/// Byte offsets of the `(` and `)` of the innermost group around `text[at]`.
fn enclosing_group(text: &str, at: usize) -> Option<(usize, usize)> {
    let mut depth = 0;
    let open = text[..at].char_indices().rev().find_map(|(index, char)| {
        match char {
            ')' => depth += 1,
            '(' if depth == 0 => return Some(index),
            '(' => depth -= 1,
            _ => {}
        }
        None
    })?;
    let mut depth = 0;
    let close = text[open..].char_indices().find_map(|(index, char)| {
        match char {
            '(' => depth += 1,
            ')' if depth == 1 => return Some(open + index),
            ')' => depth -= 1,
            _ => {}
        }
        None
    })?;
    Some((open, close))
}

/// Name and full text of the innermost function call around `text[at]`.
pub fn enclosing_call(text: &str, at: usize) -> Option<(&str, &str)> {
    let (open, close) = enclosing_group(text, at)?;
    let head = text[..open].trim_end();
    let name_start = head
        .rfind(|char: char| !(char.is_ascii_alphanumeric() || char == '_'))
        .map_or(0, |index| index + 1);
    Some((&head[name_start..], &text[name_start..=close]))
}

/// A call to a status check function in a condition.
pub struct StatusCheck {
    pub name: &'static str,
    /// Preceded by `!`.
    pub negated: bool,
}

/// Status check function calls in `condition`, inside `${{ }}` or not, outside string literals:
/// https://docs.github.com/en/actions/reference/workflows-and-actions/expressions#status-check-functions
/// The runner looks function names up ignoring case (`WellKnownFunctions` in actions/runner
/// `src/Sdk/DTExpressions2/Expressions2/ExpressionConstants.cs`, same commit as `Binding`).
pub fn status_checks(condition: &str) -> Vec<StatusCheck> {
    const NAMES: [&str; 4] = ["always", "success", "failure", "cancelled"];
    let text = strip_delimiters(condition).to_ascii_lowercase();
    let mut found = Vec::new();
    // `''` escapes a quote inside a literal; it reads as closing and reopening one.
    for outside in text.split('\'').step_by(2) {
        for name in NAMES {
            for (start, _) in outside.match_indices(name) {
                let preceded = outside[..start]
                    .bytes()
                    .next_back()
                    .is_some_and(|byte| is_context_char(byte) || byte == b'.');
                let call = outside[start + name.len()..]
                    .trim_start()
                    .strip_prefix('(')
                    .is_some_and(|arguments| arguments.trim_start().starts_with(')'));
                if !preceded && call {
                    found.push(StatusCheck {
                        name,
                        negated: outside[..start].trim_end().ends_with('!'),
                    });
                }
            }
        }
    }
    found
}

/// How tightly a binary operator binds, loosest first. The runner ranks `<`, `>` above `==`,
/// `!=`; one rank for both only adds parentheses (`Precedence` in actions/runner
/// `src/Sdk/DTExpressions2/Expressions2/Tokens/Token.cs` at
/// 8cfa1049f86781d141056082d73802d56cd2b433).
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Binding {
    Or,
    And,
    Comparison,
}

/// A token of a condition whose `${{ }}` segments each count as one operand.
enum Token {
    Operand,
    Segment,
    Not,
    Open,
    Close,
    Operator(Binding),
}

/// `text`, a condition that mixes `${{ }}` with other text, as one expression in a single
/// `${{ }}`: each segment's body takes the place of the segment, in parentheses when an operator
/// next to it binds tighter than the body's own. `None` unless the text outside the segments holds
/// only operators, parentheses, literals, and whitespace that join the segments into one
/// expression: a context or function outside `${{ }}` was never evaluated, so nothing says it
/// exists. Whitespace around the condition stays outside the new `${{ }}`.
pub fn merge_segments(text: &str) -> Option<String> {
    let spans = spans(text);
    let &(_, last) = spans.last()?;
    if text[last..].contains("${{") {
        return None;
    }
    let mut tokens = Vec::new();
    let mut segments = Vec::new();
    let mut from = 0;
    for &(start, end) in &spans {
        lex(&text[from..start], &mut tokens)?;
        if text[start + 3..end - 2].trim().is_empty() {
            return None;
        }
        segments.push(tokens.len());
        tokens.push(Token::Segment);
        from = end;
    }
    lex(&text[from..], &mut tokens)?;
    let (mut operand, mut depth) = (true, 0_usize);
    for token in &tokens {
        match (operand, token) {
            (true, Token::Not) => {}
            (true, Token::Open) => depth += 1,
            (true, Token::Operand | Token::Segment) => operand = false,
            (false, Token::Operator(_)) => operand = true,
            (false, Token::Close) => depth = depth.checked_sub(1)?,
            _ => return None,
        }
    }
    if operand || depth > 0 {
        return None;
    }

    let mut joined = String::with_capacity(text.len());
    let mut from = 0;
    for (&(start, end), &at) in spans.iter().zip(&segments) {
        joined.push_str(&text[from..start]);
        let body = text[start + 3..end - 2].trim();
        let before = at.checked_sub(1).map(|index| &tokens[index]);
        if needs_group(body, before, tokens.get(at + 1)) {
            joined.push('(');
            joined.push_str(body);
            joined.push(')');
        } else {
            joined.push_str(body);
        }
        from = end;
    }
    joined.push_str(&text[from..]);
    let core = joined.trim();
    let lead = joined.len() - joined.trim_start().len();
    Some(format!(
        "{}${{{{ {core} }}}}{}",
        &joined[..lead],
        &joined[lead + core.len()..]
    ))
}

/// Appends the tokens of `text`, text outside `${{ }}`, or returns `None` at anything but an
/// operator, a parenthesis, or a literal.
fn lex(text: &str, tokens: &mut Vec<Token>) -> Option<()> {
    let mut rest = text.trim_start();
    while let Some(first) = rest.chars().next() {
        let (token, length) = match first {
            '(' => (Token::Open, 1),
            ')' => (Token::Close, 1),
            '\'' => (Token::Operand, string_length(rest)?),
            _ if rest.starts_with("||") => (Token::Operator(Binding::Or), 2),
            _ if rest.starts_with("&&") => (Token::Operator(Binding::And), 2),
            _ if ["==", "!=", "<=", ">="]
                .iter()
                .any(|operator| rest.starts_with(operator)) =>
            {
                (Token::Operator(Binding::Comparison), 2)
            }
            '<' | '>' => (Token::Operator(Binding::Comparison), 1),
            '!' => (Token::Not, 1),
            _ => (Token::Operand, keyword_length(rest)?),
        };
        tokens.push(token);
        rest = rest[length..].trim_start();
    }
    Some(())
}

/// Length of the string literal `text` starts with, where `''` is an escaped quote.
fn string_length(text: &str) -> Option<usize> {
    let mut index = 1;
    loop {
        index += text[index..].find('\'')? + 1;
        if !text[index..].starts_with('\'') {
            return Some(index);
        }
        index += 1;
    }
}

/// Length of the number (`10`, `-1.5`) or keyword literal `text` starts with. The runner reads
/// keywords case-sensitively (`LexicalAnalyzer.cs`, same commit as `Binding`).
fn keyword_length(text: &str) -> Option<usize> {
    let length = text
        .find(|char: char| !(char.is_ascii_alphanumeric() || "_-.".contains(char)))
        .unwrap_or(text.len());
    let word = &text[..length];
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let unsigned = word.strip_prefix('-').unwrap_or(word);
    let number = match unsigned.split_once('.') {
        Some((whole, fraction)) => digits(whole) && digits(fraction),
        None => digits(unsigned),
    };
    let keyword = ["true", "false", "null", "NaN", "Infinity"].contains(&word);
    (number || keyword).then_some(length)
}

/// Whether `body`, a segment's expression, needs parentheses once `before` and `after` stand next
/// to it: a `!` before it, or an operator that binds tighter than the loosest one at its top
/// level. Chained comparisons are grouped too, since they do not associate like `&&` and `||`.
fn needs_group(body: &str, before: Option<&Token>, after: Option<&Token>) -> bool {
    let splits = |operator: &str| split_outside_groups(body, operator).len() > 1;
    let loosest = if splits("||") {
        Binding::Or
    } else if splits("&&") {
        Binding::And
    } else if ["==", "!=", "<", ">"].into_iter().any(splits) {
        Binding::Comparison
    } else {
        return false;
    };
    let tighter = |token: Option<&Token>| match token {
        Some(Token::Operator(binding)) => {
            *binding > loosest || (*binding == loosest && loosest == Binding::Comparison)
        }
        _ => false,
    };
    matches!(before, Some(Token::Not)) || tighter(before) || tighter(after)
}
