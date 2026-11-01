// asserts equality of the actual diff and expected diff
// considering datetime varitations
//
// It replaces the modification time in the actual diff
// with placeholder "TIMESTAMP" and then asserts the equality
//
// For eg.
// let brief = "*** fruits_old.txt\t2024-03-24 23:43:05.189597645 +0530\n
//              --- fruits_new.txt\t2024-03-24 23:35:08.922581904 +0530\n";
//
// replaced = "*** fruits_old.txt\tTIMESTAMP\n
//             --- fruits_new.txt\tTIMESTAMP\n";
#[macro_export]
macro_rules! assert_diff_eq {
    ($actual:expr, $expected:expr) => {{
        use regex::Regex;
        use std::str;

        let diff = str::from_utf8(&$actual).unwrap();
        let re = Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d+ [+-]\d{4}").unwrap();
        let actual = re.replacen(diff, 2, "TIMESTAMP");

        assert_eq!(actual, $expected);
    }};
}

/// A safe version of println!() that does not panic if stdout is redirected to /dev/full
#[macro_export]
macro_rules! safe_println {
    () => {
        let _ = ::std::io::Write::write_all(&mut ::std::io::stdout(), b"\n");
    };
    ($($arg:tt)*) => {
        let _ = ::std::io::Write::write_fmt(&mut ::std::io::stdout(), format_args!($($arg)*))
            .and_then(|_| ::std::io::Write::write_all(&mut ::std::io::stdout(), b"\n"));
    };
}
