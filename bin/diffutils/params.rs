use std::ffi::{OsStr, OsString};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
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
}

pub fn parse_params<I: IntoIterator<Item = OsString>>(opts: I) -> Result<Params, String> {
    let mut opts = opts.into_iter();
    // parse CLI
    let exe = match opts.next() {
        Some(from) => from,
        None => {
            return Err(format!("Usage: <exe> <from> <to>"));
        }
    };
    let mut from = None;
    let mut to = None;
    let mut format = None;
    let mut context_count = 3;
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
        let p = osstr_bytes(&param);
        if p.get(0) == Some(&b'-') && p.get(1) != Some(&b'-') {
            let mut bit = p[1..].into_iter().copied().peekable();
            // Can't use a for loop because `diff -30u` is supposed to make a diff
            // with 30 lines of context.
            while let Some(b) = bit.next() {
                match b {
                    b'0'..=b'9' => {
                        context_count = (b - b'0') as usize;
                        while let Some(b'0'..=b'9') = bit.peek() {
                            context_count = context_count * 10;
                            context_count += (bit.next().unwrap() - b'0') as usize;
                        }
                    }
                    b'c' => {
                        if format.is_some() && format != Some(Format::Context) {
                            return Err(format!("Conflicting output style options"));
                        }
                        format = Some(Format::Context);
                    }
                    b'e' => {
                        if format.is_some() && format != Some(Format::Ed) {
                            return Err(format!("Conflicting output style options"));
                        }
                        format = Some(Format::Ed);
                    }
                    b'u' => {
                        if format.is_some() && format != Some(Format::Unified) {
                            return Err(format!("Conflicting output style options"));
                        }
                        format = Some(Format::Unified);
                    }
                    b'U' => {
                        if format.is_some() && format != Some(Format::Unified) {
                            return Err(format!("Conflicting output style options"));
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
                            context_count = context_count_maybe;
                            break;
                        } else {
                            return Err(format!("Invalid context count"));
                        }
                    }
                    _ => return Err(format!("Unknown option: {}", String::from_utf8_lossy(&[b]))),
                }
            }
        } else if from.is_none() {
            from = Some(param);
        } else if to.is_none() {
            to = Some(param)
        } else {
            return Err(format!("Usage: {} <from> <to>", exe.to_string_lossy()));
        }
    }
    let from = if let Some(from) = from {
        from
    } else if let Some(param) = opts.next() {
        param
    } else {
        return Err(format!("Usage: {} <from> <to>", exe.to_string_lossy()));
    };
    let to = if let Some(to) = to {
        to
    } else if let Some(param) = opts.next() {
        param
    } else {
        return Err(format!("Usage: {} <from> <to>", exe.to_string_lossy()));
    };
    let format = format.unwrap_or(Format::Normal);
    Ok(Params {
        from,
        to,
        format,
        context_count,
    })
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
                format: Format::Normal,
                context_count: 3,
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
                context_count: 3,
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
            }),
            parse_params(
                [os("diff"), os("-c54"), os("foo"), os("bar")]
                    .iter()
                    .cloned()
            )
        );
    }
    #[test]
    fn double_dash() {
        assert_eq!(
            Ok(Params {
                from: os("-g"),
                to: os("-h"),
                format: Format::Normal,
                context_count: 3,
            }),
            parse_params([os("diff"), os("--"), os("-g"), os("-h")].iter().cloned())
        );
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
}
