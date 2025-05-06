// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::utils::limited_string;
use diff::Result;
use std::{
    io::{stdout, StdoutLock, Write},
    vec,
};

fn push_output(
    output: &mut StdoutLock,
    left_ln: &[u8],
    right_ln: &[u8],
    symbol: &[u8],
    tab_size: usize,
) -> std::io::Result<()> {
    // The reason why this function exists, is that we cannot
    // assume a enconding for our left or right line, and the
    // writeln!() macro obligattes us to do it.

    // side-by-side diff usually prints the output like:
    // {left_line}{tab}{space_char}{symbol(|, < or >)}{space_char}{right_line}{EOL}

    // recalculate how many spaces are nescessary, cause we need to take into
    // consideration the lenght of the word before print it.
    let tab_size = (tab_size as isize - left_ln.len() as isize).max(0);
    let ident = vec![b' '; tab_size as usize];
    output.write_all(left_ln)?; // {left_line}
    output.write_all(&ident)?; // {tab}
    output.write_all(b" ")?; // {space_char}
    output.write_all(symbol)?; // {symbol}
    output.write_all(b" ")?; // {space_char}
    output.write_all(right_ln)?; // {right_line}

    writeln!(output)?; // {EOL}

    Ok(())
}

pub fn diff(from_file: &[u8], to_file: &[u8]) -> Vec<u8> {
    //      ^ The left file  ^ The right file

    let mut output = stdout().lock();
    let left_lines: Vec<&[u8]> = from_file.split(|&c| c == b'\n').collect();
    let right_lines: Vec<&[u8]> = to_file.split(|&c| c == b'\n').collect();
    let tab_size = 61; // for some reason the tab spaces are 61 not 60
    for result in diff::slice(&left_lines, &right_lines) {
        match result {
            Result::Left(left_ln) => {
                push_output(
                    &mut output,
                    limited_string(left_ln, tab_size),
                    &[],
                    b"<",
                    tab_size,
                )
                .unwrap();
            }
            Result::Right(right_ln) => {
                push_output(
                    &mut output,
                    &[],
                    limited_string(right_ln, tab_size),
                    b">",
                    tab_size,
                )
                .unwrap();
            }
            Result::Both(left_ln, right_ln) => {
                push_output(
                    &mut output,
                    limited_string(left_ln, tab_size),
                    limited_string(right_ln, tab_size),
                    b" ",
                    tab_size,
                )
                .unwrap();
            }
        }
    }

    vec![]
}
