// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

//! Translation of POSIX basic regular expressions (the `grep` syntax that
//! options such as `-F` take) into the syntax of the `regex` crate.
//!
//! In a basic regular expression, `\(`, `\)`, `\{`, `\}`, `\|`, `\+` and `\?`
//! are operators while their unescaped forms are ordinary characters, `*` is
//! literal where it has nothing to repeat, `^` only anchors at the start of
//! the expression or of a group or alternative, and `$` only anchors at their
//! end.

use regex::bytes::Regex;

/// A compiled basic regular expression. Two of them are equal when their
/// patterns are.
#[derive(Clone, Debug)]
pub struct Bre {
    pattern: String,
    regex: Regex,
}

impl Bre {
    /// Compiles `pattern`, or returns an error message describing what is
    /// wrong with it.
    pub fn new(pattern: &str) -> Result<Self, String> {
        let regex = Regex::new(&translate(pattern)?).map_err(|error| match error {
            regex::Error::CompiledTooBig(_) => TOO_BIG.to_string(),
            _ => "Invalid regular expression".to_string(),
        })?;
        Ok(Self {
            pattern: pattern.to_string(),
            regex,
        })
    }

    pub fn is_match(&self, text: &[u8]) -> bool {
        self.regex.is_match(text)
    }
}

impl PartialEq for Bre {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl Eq for Bre {}

const TOO_BIG: &str = "Regular expression too big";
/// The largest count of an interval, as in `a\{2,100\}`.
const MAX_REPETITIONS: u32 = 0x7fff;

/// Translates a basic regular expression into `regex` crate syntax.
fn translate(pattern: &str) -> Result<String, String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut translated = String::with_capacity(pattern.len() * 2);
    // Whether the previous token opens an expression, a group or an
    // alternative, where `^` anchors and repetition operators are literal.
    let mut at_expression_start = true;
    let mut open_groups = 0_usize;
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        index += 1;
        let mut opens_expression = false;
        match c {
            '\\' => {
                let Some(&escaped) = chars.get(index) else {
                    return Err("Trailing backslash".to_string());
                };
                index += 1;
                match escaped {
                    '(' => {
                        open_groups += 1;
                        translated.push('(');
                        opens_expression = true;
                    }
                    ')' => {
                        open_groups = open_groups
                            .checked_sub(1)
                            .ok_or_else(|| r"Unmatched ) or \)".to_string())?;
                        translated.push(')');
                    }
                    '|' => {
                        translated.push('|');
                        opens_expression = true;
                    }
                    '{' | '}' | '+' | '?' if at_expression_start => {
                        push_literal(&mut translated, escaped);
                    }
                    '{' => index = translate_interval(&chars, index, &mut translated)?,
                    '+' | '?' => translated.push(escaped),
                    '<' | '>' | 'b' | 'B' | 'w' | 'W' | 's' | 'S' => {
                        translated.push('\\');
                        translated.push(escaped);
                    }
                    '`' => translated.push_str(r"\A"),
                    '\'' => translated.push_str(r"\z"),
                    '1'..='9' => return Err("Invalid back reference".to_string()),
                    _ => push_literal(&mut translated, escaped),
                }
            }
            '^' if at_expression_start => {
                translated.push('^');
                opens_expression = true;
            }
            '$' if ends_expression(&chars[index..]) => translated.push('$'),
            '*' if at_expression_start => translated.push_str(r"\*"),
            '.' | '*' => translated.push(c),
            '[' => index = translate_bracket(&chars, index, &mut translated)?,
            _ => push_literal(&mut translated, c),
        }
        at_expression_start = opens_expression;
    }
    if open_groups > 0 {
        return Err(r"Unmatched ( or \(".to_string());
    }
    Ok(translated)
}

/// Returns whether a `$` followed by `rest` ends an expression, a group or an
/// alternative.
fn ends_expression(rest: &[char]) -> bool {
    matches!(rest, [] | ['\\', ')' | '|', ..])
}

/// Translates the interval whose content starts at `index` (just after the
/// `\{`), and returns the index following its closing `\}`.
fn translate_interval(
    chars: &[char],
    index: usize,
    translated: &mut String,
) -> Result<usize, String> {
    let end = (index..chars.len().saturating_sub(1))
        .find(|&end| chars[end] == '\\' && chars[end + 1] == '}')
        .ok_or_else(|| r"Unmatched \{".to_string())?;
    let content: String = chars[index..end].iter().collect();
    let invalid = || r"Invalid content of \{\}".to_string();
    let count = |digits: &str| -> Result<Option<u32>, String> {
        if digits.is_empty() {
            return Ok(None);
        }
        if !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
        }
        match digits.parse::<u32>() {
            Ok(count) if count <= MAX_REPETITIONS => Ok(Some(count)),
            _ => Err(TOO_BIG.to_string()),
        }
    };
    match content.split_once(',') {
        None => {
            let exact = count(&content)?.ok_or_else(invalid)?;
            translated.push_str(&format!("{{{exact}}}"));
        }
        Some((min, max)) => {
            let min = count(min)?.unwrap_or(0);
            match count(max)? {
                Some(max) if max < min => return Err(invalid()),
                Some(max) => translated.push_str(&format!("{{{min},{max}}}")),
                None => translated.push_str(&format!("{{{min},}}")),
            }
        }
    }
    Ok(end + 2)
}

/// Characters with a meaning in `regex` crate syntax, inside or outside of
/// bracket expressions.
const META_CHARACTERS: &str = r"\.+*?()|[]{}^$#&-~";

/// The names of the character classes of bracket expressions, as in
/// `[[:alpha:]]`.
const CLASS_NAMES: [&str; 12] = [
    "alnum", "alpha", "blank", "cntrl", "digit", "graph", "lower", "print", "punct", "space",
    "upper", "xdigit",
];

fn push_literal(translated: &mut String, c: char) {
    if META_CHARACTERS.contains(c) {
        translated.push('\\');
    }
    translated.push(c);
}

/// Translates the bracket expression whose content starts at `index` (just
/// after the `[`), and returns the index following its closing `]`.
fn translate_bracket(
    chars: &[char],
    mut index: usize,
    translated: &mut String,
) -> Result<usize, String> {
    const UNMATCHED: &str = "Unmatched [, [^, [:, [., or [=";
    translated.push('[');
    if chars.get(index) == Some(&'^') {
        translated.push('^');
        index += 1;
    }
    let content_start = index;
    // The last single character member, which can start a range.
    let mut range_start = None;
    loop {
        let Some(&c) = chars.get(index) else {
            return Err(UNMATCHED.to_string());
        };
        // A `]` first in the list is an ordinary member.
        if c == ']' && index > content_start {
            translated.push(']');
            return Ok(index + 1);
        }
        if c == '[' && matches!(chars.get(index + 1), Some(':' | '.' | '=')) {
            let delimiter = chars[index + 1];
            let end = (index + 2..chars.len().saturating_sub(1))
                .find(|&end| chars[end] == delimiter && chars[end + 1] == ']')
                .ok_or_else(|| UNMATCHED.to_string())?;
            let name: String = chars[index + 2..end].iter().collect();
            if delimiter == ':' {
                // A character class such as `[:alpha:]`, kept verbatim.
                if !CLASS_NAMES.contains(&name.as_str()) {
                    return Err("Invalid character class name".to_string());
                }
                translated.extend(&chars[index..end + 2]);
                range_start = None;
            } else {
                // A collating element or equivalence class such as `[.a.]`
                // or `[=a=]`, which only stands for a single character here.
                let mut name_chars = name.chars();
                let (Some(member), None) = (name_chars.next(), name_chars.next()) else {
                    return Err("Invalid collation character".to_string());
                };
                push_literal(translated, member);
                range_start = Some(member);
            }
            index = end + 2;
            continue;
        }
        let is_range = c == '-'
            && index > content_start
            && chars.get(index + 1).is_some_and(|&next| next != ']');
        if is_range {
            if let (Some(start), Some(&end)) = (range_start, chars.get(index + 1)) {
                if end < start {
                    return Err("Invalid range end".to_string());
                }
            }
            translated.push('-');
            range_start = None;
        } else {
            // Backslashes are ordinary members of a bracket expression, and
            // the `regex` crate gives meaning to `[`, `&&`, `--` and `~~`.
            push_literal(translated, c);
            range_start = Some(c);
        }
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn matches(pattern: &str, text: &str) -> bool {
        Bre::new(pattern).unwrap().is_match(text.as_bytes())
    }

    #[test]
    fn groups_and_alternatives() {
        assert!(matches(r"^ *\(sub\|fn\) ", "  fn main"));
        assert!(matches(r"^ *\(sub\|fn\) ", "sub x"));
        assert!(!matches(r"^ *\(sub\|fn\) ", "  let x"));
        // Unescaped operators of extended expressions are ordinary characters.
        assert!(matches(r"f(x)|y+?", "f(x)|y+?"));
        assert!(!matches(r"f(x)|y+?", "fx"));
    }

    #[test]
    fn intervals_and_repetitions() {
        assert!(matches(r"^ab\{2,3\}c$", "abbc"));
        assert!(!matches(r"^ab\{2,3\}c$", "abc"));
        assert!(matches(r"^x\+$", "xxx"));
        assert!(matches(r"^y\?z$", "z"));
        // `*` with nothing to repeat is literal.
        assert!(matches(r"*q", "a*q"));
        assert!(matches(r"^*q", "*q"));
        assert!(matches(r"\(*\)", "*"));
        assert!(matches(r"to*k", "tk"));
    }

    #[test]
    fn anchors_only_at_the_ends() {
        assert!(matches(r"a^b", "a^b"));
        assert!(matches(r"a$b", "a$b"));
        assert!(matches(r"end$", "the end"));
        assert!(!matches(r"end$", "endless"));
        assert!(matches(r"\(x$\|^y\)", "yes"));
    }

    #[test]
    fn bracket_expressions() {
        assert!(matches(r"^[[:alpha:]$_]", "$var"));
        assert!(matches(r"^[[:alpha:]$_]", "_x"));
        assert!(!matches(r"^[[:alpha:]$_]", " x"));
        assert!(matches(r"[]x]", "]"));
        assert!(matches(r"[^]x]", "y"));
        assert!(matches(r"[a\]", "\\"));
        assert!(matches(r"[a-c]", "b"));
        assert!(matches(r"[a-]", "-"));
        assert!(matches(r"[&&~~[]", "["));
    }

    #[test]
    fn errors() {
        for (pattern, message) in [
            ("ab\\", "Trailing backslash"),
            ("[ab", "Unmatched [, [^, [:, [., or [="),
            ("[[:alpha:]", "Unmatched [, [^, [:, [., or [="),
            (r"\(a\)\1", "Invalid back reference"),
            (r"x\(y", r"Unmatched ( or \("),
            (r"x\)y", r"Unmatched ) or \)"),
            (r"k\{4", r"Unmatched \{"),
            (r"k\{\}", r"Invalid content of \{\}"),
            (r"k\{2,z\}", r"Invalid content of \{\}"),
            (r"k\{5,2\}", r"Invalid content of \{\}"),
            (r"k\{40000\}", "Regular expression too big"),
            ("[[:vowel:]]", "Invalid character class name"),
            ("[[.ab.]]", "Invalid collation character"),
            ("[m-c]", "Invalid range end"),
        ] {
            assert_eq!(
                Bre::new(pattern).err().as_deref(),
                Some(message),
                "{pattern}"
            );
        }
    }

    #[test]
    fn literal_operators_with_nothing_to_repeat() {
        assert!(matches(r"\{3\}", "{3}"));
        assert!(matches(r"\(\+x\)", "+x"));
        assert!(matches(r"q\{,2\}r", "r"));
        assert!(matches(r"^q\{2,\}$", "qqqq"));
        assert!(matches(r"[[.-.][=k=]]", "-"));
        assert!(matches(r"[[.-.][=k=]]", "k"));
    }
}
