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
    let mut opts = opts.into_iter().peekable();
    // parse CLI

    let Some(exe) = opts.next() else {
        return Err("Usage: <exe> <from> <to>".to_string());
    };
    let mut params = Params::default();
    let mut from = None;
    let mut to = None;
    let mut format = None;
    let mut context_count = None;
    let tabsize_re = Regex::new(r"^--tabsize=(?<num>\d+)$").unwrap();
    let context_re =
        Regex::new(r"^(-[cC](?<num1>\d*)|--context(=(?<num2>\d*))?|-(?<num3>\d+)c)$").unwrap();
    let unified_re =
        Regex::new(r"^(-[uU](?<num1>\d*)|--unified(=(?<num2>\d*))?|-(?<num3>\d+)u)$").unwrap();
    while let Some(param) = opts.next() {
        if param == "--" {
            break;
        }
        if param == "-" {
            if from.is_none() {
                from = Some(param);
            } else if to.is_none() {
                to = Some(param);
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
        if context_re.is_match(param.to_string_lossy().as_ref()) {
            if format.is_some() && format != Some(Format::Context) {
                return Err("Conflicting output style options".to_string());
            }
            format = Some(Format::Context);
            let captures = context_re.captures(param.to_str().unwrap()).unwrap();
            let num = captures
                .name("num1")
                .or(captures.name("num2"))
                .or(captures.name("num3"));
            if num.is_some() && !num.unwrap().as_str().is_empty() {
                context_count = Some(num.unwrap().as_str().parse::<usize>().unwrap());
            }
            if param == "-C" {
                let next_param = opts.peek();
                if next_param.is_some() {
                    let next_value = next_param
                        .unwrap()
                        .to_string_lossy()
                        .as_ref()
                        .parse::<usize>();
                    if next_value.is_ok() {
                        context_count = Some(next_value.unwrap());
                        opts.next();
                    } else {
                        return Err(format!(
                            "invalid context length '{}'",
                            next_param.unwrap().to_string_lossy()
                        ));
                    }
                }
            }
            continue;
        }
        if unified_re.is_match(param.to_string_lossy().as_ref()) {
            if format.is_some() && format != Some(Format::Unified) {
                return Err("Conflicting output style options".to_string());
            }
            format = Some(Format::Unified);
            let captures = unified_re.captures(param.to_str().unwrap()).unwrap();
            let num = captures
                .name("num1")
                .or(captures.name("num2"))
                .or(captures.name("num3"));
            if num.is_some() && !num.unwrap().as_str().is_empty() {
                context_count = Some(num.unwrap().as_str().parse::<usize>().unwrap());
            }
            if param == "-U" {
                let next_param = opts.peek();
                if next_param.is_some() {
                    let next_value = next_param
                        .unwrap()
                        .to_string_lossy()
                        .as_ref()
                        .parse::<usize>();
                    if next_value.is_ok() {
                        context_count = Some(next_value.unwrap());
                        opts.next();
                    } else {
                        return Err(format!(
                            "invalid context length '{}'",
                            next_param.unwrap().to_string_lossy()
                        ));
                    }
                }
            }
            continue;
        }
        let p = osstr_bytes(&param);
        if p.first() == Some(&b'-') && p.get(1) != Some(&b'-') {
            let mut bit = p[1..].iter().copied().peekable();
            while let Some(b) = bit.next() {
                match b {
                    b'e' => {
                        if format.is_some() && format != Some(Format::Ed) {
                            return Err("Conflicting output style options".to_string());
                        }
                        format = Some(Format::Ed);
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
    if context_count.is_some() {
        params.context_count = context_count.unwrap();
    }
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
    fn context_valid() {
        for args in [vec!["-c"], vec!["--context"], vec!["--context="]] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert_eq!(
                Ok(Params {
                    from: os("foo"),
                    to: os("bar"),
                    format: Format::Context,
                    ..Default::default()
                }),
                parse_params(params.iter().map(|x| os(x)))
            );
        }
        for args in [
            vec!["-c42"],
            vec!["-C42"],
            vec!["-C", "42"],
            vec!["--context=42"],
            vec!["-42c"],
        ] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert_eq!(
                Ok(Params {
                    from: os("foo"),
                    to: os("bar"),
                    format: Format::Context,
                    context_count: 42,
                    ..Default::default()
                }),
                parse_params(params.iter().map(|x| os(x)))
            );
        }
    }
    #[test]
    fn context_invalid() {
        for args in [
            vec!["-c", "42"],
            vec!["-c=42"],
            vec!["-c="],
            vec!["-C"],
            vec!["-C=42"],
            vec!["-C="],
            vec!["--context42"],
            vec!["--context", "42"],
            vec!["-42C"],
        ] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert!(parse_params(params.iter().map(|x| os(x))).is_err());
        }
    }
    #[test]
    fn unified_valid() {
        for args in [vec!["-u"], vec!["--unified"], vec!["--unified="]] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert_eq!(
                Ok(Params {
                    from: os("foo"),
                    to: os("bar"),
                    format: Format::Unified,
                    ..Default::default()
                }),
                parse_params(params.iter().map(|x| os(x)))
            );
        }
        for args in [
            vec!["-u42"],
            vec!["-U42"],
            vec!["-U", "42"],
            vec!["--unified=42"],
            vec!["-42u"],
        ] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert_eq!(
                Ok(Params {
                    from: os("foo"),
                    to: os("bar"),
                    format: Format::Unified,
                    context_count: 42,
                    ..Default::default()
                }),
                parse_params(params.iter().map(|x| os(x)))
            );
        }
    }
    #[test]
    fn unified_invalid() {
        for args in [
            vec!["-u", "42"],
            vec!["-u=42"],
            vec!["-u="],
            vec!["-U"],
            vec!["-U=42"],
            vec!["-U="],
            vec!["--unified42"],
            vec!["--unified", "42"],
            vec!["-42U"],
        ] {
            let mut params = vec!["diff"];
            params.extend(args);
            params.extend(["foo", "bar"]);
            assert!(parse_params(params.iter().map(|x| os(x))).is_err());
        }
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
                to: os("-"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("foo"), os("-")].iter().cloned())
        );
        assert_eq!(
            Ok(Params {
                from: os("-"),
                to: os("bar"),
                ..Default::default()
            }),
            parse_params([os("diff"), os("-"), os("bar")].iter().cloned())
        );
        assert_eq!(
            Ok(Params {
                from: os("-"),
                to: os("-"),
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
