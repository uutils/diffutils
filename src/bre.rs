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
                        translated.push_str("(?:");
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
    use rstest::rstest;

    #[rstest]
    // Groups and alternatives.
    #[case::alternative_first(r"^ *\(sub\|fn\) ", "  fn main", true)]
    #[case::alternative_second(r"^ *\(sub\|fn\) ", "sub x", true)]
    #[case::alternative_none(r"^ *\(sub\|fn\) ", "  let x", false)]
    // Unescaped operators of extended expressions are ordinary characters.
    #[case::ere_operators_are_literal(r"f(x)|y+?", "f(x)|y+?", true)]
    #[case::ere_operators_do_not_group(r"f(x)|y+?", "fx", false)]
    // Intervals and repetitions.
    #[case::interval(r"^ab\{2,3\}c$", "abbc", true)]
    #[case::interval_too_few(r"^ab\{2,3\}c$", "abc", false)]
    #[case::one_or_more(r"^x\+$", "xxx", true)]
    #[case::zero_or_one(r"^y\?z$", "z", true)]
    // `*` with nothing to repeat is literal.
    #[case::leading_star(r"*q", "a*q", true)]
    #[case::star_after_anchor(r"^*q", "*q", true)]
    #[case::star_after_group_start(r"\(*\)", "*", true)]
    #[case::star_repeats(r"to*k", "tk", true)]
    // Anchors only at the ends.
    #[case::caret_in_middle(r"a^b", "a^b", true)]
    #[case::dollar_in_middle(r"a$b", "a$b", true)]
    #[case::dollar_at_end(r"end$", "the end", true)]
    #[case::dollar_at_end_no_match(r"end$", "endless", false)]
    #[case::anchors_in_alternatives(r"\(x$\|^y\)", "yes", true)]
    // Bracket expressions.
    #[case::class_with_dollar(r"^[[:alpha:]$_]", "$var", true)]
    #[case::class_with_underscore(r"^[[:alpha:]$_]", "_x", true)]
    #[case::class_no_match(r"^[[:alpha:]$_]", " x", false)]
    #[case::leading_bracket(r"[]x]", "]", true)]
    #[case::negated_leading_bracket(r"[^]x]", "y", true)]
    #[case::backslash_in_bracket(r"[a\]", "\\", true)]
    #[case::range(r"[a-c]", "b", true)]
    #[case::trailing_dash(r"[a-]", "-", true)]
    #[case::doubled_set_operators(r"[&&~~[]", "[", true)]
    // Operators with nothing to repeat are literal.
    #[case::leading_interval(r"\{3\}", "{3}", true)]
    #[case::plus_after_group_start(r"\(\+x\)", "+x", true)]
    #[case::interval_without_min(r"q\{,2\}r", "r", true)]
    #[case::interval_without_max(r"^q\{2,\}$", "qqqq", true)]
    #[case::collating_symbol(r"[[.-.][=k=]]", "-", true)]
    #[case::equivalence_class(r"[[.-.][=k=]]", "k", true)]
    fn matches(#[case] pattern: &str, #[case] text: &str, #[case] expected: bool) {
        assert_eq!(
            Bre::new(pattern).unwrap().is_match(text.as_bytes()),
            expected
        );
    }

    // Only is_match() is used and back references are rejected, so a group
    // has nothing to capture; capturing would cost memory for each of them.
    #[test]
    fn groups_do_not_capture() {
        let bre = Bre::new(r"^\(a\(b\)\)\{2\}\(c\|d\)$").unwrap();
        assert_eq!(bre.regex.captures_len(), 1);
        assert!(bre.is_match(b"ababd"));
        assert!(!bre.is_match(b"abd"));
    }

    #[rstest]
    #[case::trailing_backslash("ab\\", "Trailing backslash")]
    #[case::unmatched_bracket("[ab", "Unmatched [, [^, [:, [., or [=")]
    #[case::unmatched_class_bracket("[[:alpha:]", "Unmatched [, [^, [:, [., or [=")]
    #[case::back_reference(r"\(a\)\1", "Invalid back reference")]
    #[case::unmatched_open_group(r"x\(y", r"Unmatched ( or \(")]
    #[case::unmatched_close_group(r"x\)y", r"Unmatched ) or \)")]
    #[case::unmatched_interval(r"k\{4", r"Unmatched \{")]
    #[case::empty_interval(r"k\{\}", r"Invalid content of \{\}")]
    #[case::non_numeric_interval(r"k\{2,z\}", r"Invalid content of \{\}")]
    #[case::reversed_interval(r"k\{5,2\}", r"Invalid content of \{\}")]
    #[case::huge_interval(r"k\{40000\}", "Regular expression too big")]
    #[case::unknown_class("[[:vowel:]]", "Invalid character class name")]
    #[case::multi_char_collation("[[.ab.]]", "Invalid collation character")]
    #[case::reversed_range("[m-c]", "Invalid range end")]
    fn errors(#[case] pattern: &str, #[case] message: &str) {
        assert_eq!(Bre::new(pattern).err().as_deref(), Some(message));
    }
}
