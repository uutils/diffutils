#[macro_export]
macro_rules! assert_diff_eq {
    ($actual:expr, $expected:expr) => {{
        use regex::Regex;
        use std::str;

        let diff = str::from_utf8(&$actual).unwrap();
        let re = Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d+ [+-]\d{4}").unwrap();
        let actual = re.replace_all(diff, "");

        assert_eq!(actual, $expected);
    }};
}
