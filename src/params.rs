use std::ffi::{OsStr, OsString};

use regex::Regex;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Format {
    #[default]
    Normal,
    Unified,
    Context,
    Ed,
}

#[cfg(unix)]
fn osstr_bytes(osstr: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    osstr.as_bytes()
}

#[cfg(not(unix))]
fn osstr_bytes(osstr: &OsStr) -> Vec<u8> {
    osstr.to_string_lossy().bytes().collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Params {
    pub from: OsString,
    pub to: OsString,
    pub format: Format,
    pub context_count: usize,
    pub report_identical_files: bool,
    pub brief: bool,
    pub expand_tabs: bool,
    pub tabsize: usize,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            from: OsString::default(),
            to: OsString::default(),
            format: Format::default(),
            context_count: 3,
            report_identical_files: false,
            brief: false,
            expand_tabs: false,
            tabsize: 8,
        }
    }
}

pub fn parse_params<I: IntoIterator<Item = OsString>>(opts: I) -> Result<Params, String> {
    let mut opts = opts.into_iter();
    // parse CLI

    let Some(exe) = opts.next() else {
        return Err("Usage: <exe> <from> <to>".to_string());
    };
    let mut params = Params::default();
    let mut from = None;
    let mut to = None;
    let mut format = None;
    let tabsize_re = Regex::new(r"^--tabsize=(?<num>\d+)$").unwrap();
    while let Some(param) = opts.next() {
        if param == "--" {
            break;
        }
        if param == "-" {
            if from.is_none() {
                from = Some(OsString::from("/dev/stdin"));
            } else if to.is_none() {
                to = Some(OsString::from("/dev/stdin"));
            } else {
                return Err(format!("Usage: {} <from> <to>", exe.to_string_lossy()));
            }
            continue;
        }
        if param == "-s" || param == "--report-identical-files" {
            params.report_identical_files = true;
            continue;
        }
        if param == "-q" || param == "--brief" {
            params.brief = true;
            continue;
        }
        if param == "-t" || param == "--expand-tabs" {
            params.expand_tabs = true;
            continue;
        }
        if tabsize_re.is_match(param.to_string_lossy().as_ref()) {
            // Because param matches the regular expression,
            // it is safe to assume it is valid UTF-8.
            let param = param.into_string().unwrap();
            let tabsize_str = tabsize_re
                .captures(param.as_str())
                .unwrap()
                .name("num")
                .unwrap()
                .as_str();
            params.tabsize = match tabsize_str.parse::<usize>() {
                Ok(num) => num,
                Err(_) => return Err(format!("invalid tabsize «{tabsize_str}»")),
            };
            continue;
        }
        let p = osstr_bytes(&param);
        if p.first() == Some(&b'-') && p.get(1) != Some(&b'-') {
            let mut bit = p[1..].iter().copied().peekable();
            // Can't use a for loop because `diff -30u` is supposed to make a diff
            // with 30 lines of context.
            while let Some(b) = bit.next() {
                match b {
                    b'0'..=b'9' => {
                        params.context_count = (b - b'0') as usize;
                        while let Some(b'0'..=b'9') = bit.peek() {
                            params.context_count *= 10;
                            params.context_count += (bit.next().unwrap() - b'0') as usize;
                        }
                    }
                    b'c' => {
                        if format.is_some() && format != Some(Format::Context) {
                            return Err("Conflicting output style options".to_string());
                        }
                        format = Some(Format::Context);
                    }
                    b'e' => {
                        if format.is_some() && format != Some(Format::Ed) {
                            return Err("Conflicting output style options".to_string());
                        }
                        format = Some(Format::Ed);
                    }
                    b'u' => {
                        if format.is_some() && format != Some(Format::Unified) {
                            return Err("Conflicting output style options".to_string());
                        }
                        format = Some(Format::Unified);
                    }
                    b'U' => {
                        if format.is_some() && format != Some(Format::Unified) {
                            return Err("Conflicting output style options".to_string());
                        }
                        format = Some(Format::Unified);
                        let context_count_maybe = if bit.peek().is_some() {
                            String::from_utf8(bit.collect::<Vec<u8>>()).ok()
                        } else {
                            opts.next().map(|x| x.to_string_lossy().into_owned())
                        };
                        if let Some(context_count_maybe) =
                            context_count_maybe.and_then(|x| x.parse().ok())
                        {
                            params.context_count = context_count_maybe;
                            break;
                        }
                        return Err("Invalid context count".to_string());
                    }
                    _ => return Err(format!("Unknown option: {}", String::from_utf8_lossy(&[b]))),
                }
            }
        } else if from.is_none() {
            from = Some(param);
        } else if to.is_none() {
            to = Some(param);
        } else {
            return Err(format!("Usage: {} <from> <to>", exe.to_string_lossy()));
        }
    }
    params.from = if let Some(from) = from {
        from
    } else if let Some(param) = opts.next() {
        param
    } else {
        return Err(format!("Usage: {} <from> <to>", exe.to_string_lossy()));
    };
    params.to = if let Some(to) = to {
        to
    } else if let Some(param) = opts.next() {
        param
    } else {
        return Err(format!("Usage: {} <from> <to>", exe.to_string_lossy()));
    };
    params.format = format.unwrap_or(Format::default());
    Ok(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn os(s: &str) -> OsString {
        OsString::from(s)
    }
    #[test]
    fn basics() {
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("foo"), os("bar")].iter().cloned())
        );
    }
    #[test]
    fn basics_ed() {
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                format: Format::Ed,
                ..Default::default()
            }),
            parse_params([os("diff"), os("-e"), os("foo"), os("bar")].iter().cloned())
        );
    }
    #[test]
    fn context_count() {
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                format: Format::Unified,
                context_count: 54,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("-u54"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
            )
        );
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                format: Format::Unified,
                context_count: 54,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("-U54"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
            )
        );
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                format: Format::Unified,
                context_count: 54,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("-U"), os("54"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
            )
        );
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                format: Format::Context,
                context_count: 54,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("-c54"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
            )
        );
    }
    #[test]
    fn report_identical_files() {
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("foo"), os("bar")].iter().cloned())
        );
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                report_identical_files: true,
                ..Default::default()
            }),
            parse_params([os("diff"), os("-s"), os("foo"), os("bar")].iter().cloned())
        );
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                report_identical_files: true,
                ..Default::default()
            }),
            parse_params(
                [
                    os("diff"),
                    os("--report-identical-files"),
                    os("foo"),
                    os("bar"),
                ]
                .iter()
                .cloned()
            )
        );
    }
    #[test]
    fn brief() {
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("foo"), os("bar")].iter().cloned())
        );
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                brief: true,
                ..Default::default()
            }),
            parse_params([os("diff"), os("-q"), os("foo"), os("bar")].iter().cloned())
        );
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                brief: true,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("--brief"), os("foo"), os("bar"),]
                    .iter()
                    .cloned()
            )
        );
    }
    #[test]
    fn expand_tabs() {
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("foo"), os("bar")].iter().cloned())
        );
        for option in ["-t", "--expand-tabs"] {
            assert_eq!(
                Ok(Params {
                    from: os("foo"),
                    to: os("bar"),
                    expand_tabs: true,
                    ..Default::default()
                }),
                parse_params(
                    [os("diff"), os(option), os("foo"), os("bar")]
                        .iter()
                        .cloned()
                )
            );
        }
    }
    #[test]
    fn tabsize() {
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("foo"), os("bar")].iter().cloned())
        );
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                tabsize: 0,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("--tabsize=0"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
            )
        );
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("bar"),
                tabsize: 42,
                ..Default::default()
            }),
            parse_params(
                [os("diff"), os("--tabsize=42"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
            )
        );
        assert!(parse_params(
            [os("diff"), os("--tabsize"), os("foo"), os("bar")]
                .iter()
                .cloned()
        )
        .is_err());
        assert!(parse_params(
            [os("diff"), os("--tabsize="), os("foo"), os("bar")]
                .iter()
                .cloned()
        )
        .is_err());
        assert!(parse_params(
            [os("diff"), os("--tabsize=r2"), os("foo"), os("bar")]
                .iter()
                .cloned()
        )
        .is_err());
        assert!(parse_params(
            [os("diff"), os("--tabsize=-1"), os("foo"), os("bar")]
                .iter()
                .cloned()
        )
        .is_err());
        assert!(parse_params(
            [os("diff"), os("--tabsize=r2"), os("foo"), os("bar")]
                .iter()
                .cloned()
        )
        .is_err());
        assert!(parse_params(
            [
                os("diff"),
                os("--tabsize=92233720368547758088"),
                os("foo"),
                os("bar")
            ]
            .iter()
            .cloned()
        )
        .is_err());
    }
    #[test]
    fn double_dash() {
        assert_eq!(
            Ok(Params {
                from: os("-g"),
                to: os("-h"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("--"), os("-g"), os("-h")].iter().cloned())
        );
    }
    #[test]
    fn default_to_stdin() {
        assert_eq!(
            Ok(Params {
                from: os("foo"),
                to: os("/dev/stdin"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("foo"), os("-")].iter().cloned())
        );
        assert_eq!(
            Ok(Params {
                from: os("/dev/stdin"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("-"), os("bar")].iter().cloned())
        );
        assert_eq!(
            Ok(Params {
                from: os("/dev/stdin"),
                to: os("/dev/stdin"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("-"), os("-")].iter().cloned())
        );
        assert!(parse_params([os("diff"), os("foo"), os("bar"), os("-")].iter().cloned()).is_err());
        assert!(parse_params([os("diff"), os("-"), os("-"), os("-")].iter().cloned()).is_err());
    }
    #[test]
    fn missing_arguments() {
        assert!(parse_params([os("diff")].iter().cloned()).is_err());
        assert!(parse_params([os("diff"), os("foo")].iter().cloned()).is_err());
    }
    #[test]
    fn unknown_argument() {
        assert!(
            parse_params([os("diff"), os("-g"), os("foo"), os("bar")].iter().cloned()).is_err()
        );
        assert!(parse_params([os("diff"), os("-g"), os("bar")].iter().cloned()).is_err());
        assert!(parse_params([os("diff"), os("-g")].iter().cloned()).is_err());
    }
    #[test]
    fn empty() {
        assert!(parse_params([].iter().cloned()).is_err());
    }
    #[test]
    fn conflicting_output_styles() {
        for (arg1, arg2) in [("-u", "-c"), ("-u", "-e"), ("-c", "-u"), ("-c", "-U42")] {
            assert!(parse_params(
                [os("diff"), os(arg1), os(arg2), os("foo"), os("bar")]
                    .iter()
                    .cloned()
            )
            .is_err());
        }
    }
}
