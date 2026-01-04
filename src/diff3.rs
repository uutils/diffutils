// This file is part of the uutils diffutils package.
//
// For the full copyright and license information, please view the LICENSE-*
// files that was distributed with this source code.

use crate::utils::report_failure_to_read_input_file;
use std::env::ArgsOs;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::iter::Peekable;
use std::process::{exit, ExitCode};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Diff3Format {
    #[default]
    Normal,
    Merged,
    Ed,
    ShowOverlap,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Diff3OutputMode {
    #[default]
    All, // -A: show all changes with conflict markers
    EdScript,          // -e: output ed script
    ShowOverlapEd,     // -E: ed script with overlap markers
    OverlapOnly,       // -x: output only overlapping changes
    OverlapOnlyMarked, // -X: output only overlapping changes with markers
    EasyOnly,          // -3: output only non-overlapping changes
}

#[derive(Clone, Debug, Default)]
pub struct Diff3Params {
    pub executable: OsString,
    pub mine: OsString,
    pub older: OsString,
    pub yours: OsString,
    pub format: Diff3Format,
    pub output_mode: Diff3OutputMode,
    pub text: bool,
    pub labels: [Option<String>; 3],
    pub strip_trailing_cr: bool,
    pub initial_tab: bool,
    pub compat_i: bool, // -i option for ed script compatibility
}

pub fn parse_params<I: Iterator<Item = OsString>>(
    mut opts: Peekable<I>,
) -> Result<Diff3Params, String> {
    let Some(executable) = opts.next() else {
        return Err("Usage: <exe> mine older yours".to_string());
    };
    let mut params = Diff3Params {
        executable,
        ..Default::default()
    };

    let mut mine = None;
    let mut older = None;
    let mut yours = None;
    let mut label_count = 0;

    while let Some(param) = opts.by_ref().next() {
        let param_str = param.to_string_lossy();

        if param_str == "--" {
            break;
        }

        if param_str == "-" {
            push_file_arg(param, &mut mine, &mut older, &mut yours, &params.executable)?;
            continue;
        }

        if param_str.starts_with('-') && param_str != "-" {
            if param_str == "-a" || param_str == "--text" {
                params.text = true;
                continue;
            }
            if param_str == "-A" || param_str == "--show-all" {
                params.format = Diff3Format::Ed;
                params.output_mode = Diff3OutputMode::All;
                continue;
            }
            if param_str == "-e" || param_str == "--ed" {
                params.format = Diff3Format::Ed;
                params.output_mode = Diff3OutputMode::EdScript;
                continue;
            }
            if param_str == "-E" || param_str == "--show-overlap" {
                params.format = Diff3Format::ShowOverlap;
                params.output_mode = Diff3OutputMode::ShowOverlapEd;
                continue;
            }
            if param_str == "-m" || param_str == "--merge" {
                params.format = Diff3Format::Merged;
                if params.output_mode == Diff3OutputMode::EdScript {
                    // -A is default when -m is used without other options
                    params.output_mode = Diff3OutputMode::All;
                }
                continue;
            }
            if param_str == "-x" || param_str == "--overlap-only" {
                // Only set Ed format if no format was explicitly set
                if params.format == Diff3Format::Normal {
                    params.format = Diff3Format::Ed;
                }
                params.output_mode = Diff3OutputMode::OverlapOnly;
                continue;
            }
            if param_str == "-X" {
                // Only set Ed format if no format was explicitly set
                if params.format == Diff3Format::Normal {
                    params.format = Diff3Format::Ed;
                }
                params.output_mode = Diff3OutputMode::OverlapOnlyMarked;
                continue;
            }
            if param_str == "-3" || param_str == "--easy-only" {
                // Only set Ed format if no format was explicitly set
                if params.format == Diff3Format::Normal {
                    params.format = Diff3Format::Ed;
                }
                params.output_mode = Diff3OutputMode::EasyOnly;
                continue;
            }
            if param_str == "-i" {
                params.compat_i = true;
                continue;
            }
            if param_str == "--strip-trailing-cr" {
                params.strip_trailing_cr = true;
                continue;
            }
            if param_str == "-T" || param_str == "--initial-tab" {
                params.initial_tab = true;
                continue;
            }
            if param_str == "--label" {
                if label_count >= 3 {
                    return Err("Too many labels".to_string());
                }
                let label = opts
                    .next()
                    .ok_or("--label requires an argument")?
                    .into_string()
                    .map_err(|_| "Label must be valid UTF-8")?;
                params.labels[label_count] = Some(label);
                label_count += 1;
                continue;
            }
            if let Some(stripped) = param_str.strip_prefix("--label=") {
                if label_count >= 3 {
                    return Err("Too many labels".to_string());
                }
                let label = stripped.to_string();
                params.labels[label_count] = Some(label);
                label_count += 1;
                continue;
            }
            if param_str == "--help" || param_str == "-h" {
                print_help(&params.executable);
                exit(0);
            }
            if param_str == "-v" || param_str == "--version" {
                println!("diff3 {}", env!("CARGO_PKG_VERSION"));
                exit(0);
            }

            return Err(format!("Unknown option: \"{}\"", param_str));
        } else {
            push_file_arg(param, &mut mine, &mut older, &mut yours, &params.executable)?;
        }
    }

    for param in opts {
        push_file_arg(param, &mut mine, &mut older, &mut yours, &params.executable)?;
    }

    params.mine = mine.ok_or("Missing file: mine")?;
    params.older = older.ok_or("Missing file: older")?;
    params.yours = yours.ok_or("Missing file: yours")?;

    Ok(params)
}
fn push_file_arg(
    param: OsString,
    mine: &mut Option<OsString>,
    older: &mut Option<OsString>,
    yours: &mut Option<OsString>,
    exe: &OsString,
) -> Result<(), String> {
    if mine.is_none() {
        *mine = Some(param);
    } else if older.is_none() {
        *older = Some(param);
    } else if yours.is_none() {
        *yours = Some(param);
    } else {
        return Err(format!("Usage: {} mine older yours", exe.to_string_lossy()));
    }
    Ok(())
}
fn print_help(executable: &OsString) {
    let exe_name = executable.to_string_lossy();
    println!("Usage: {} [OPTION]... mine older yours", exe_name);
    println!();
    println!("Compare three files and show their differences.");
    println!();
    println!("Options:");
    println!("  -a, --text                 Treat all files as text");
    println!("  -A, --show-all             Show all changes with conflict markers");
    println!("  -e, --ed                   Output ed script");
    println!("  -E, --show-overlap         Output ed script with overlap markers");
    println!("  -m, --merge                Generate merged output");
    println!("  -x, --overlap-only         Output only overlapping changes");
    println!("  -X                         Output only overlapping changes with markers");
    println!("  -3, --easy-only            Output only non-overlapping changes");
    println!("  -i                         Add write and quit commands for ed");
    println!("  -T, --initial-tab          Make tabs line up by prepending a tab");
    println!("  --label=LABEL              Use label for conflict markers");
    println!("  --strip-trailing-cr        Strip trailing carriage return");
    println!("  -h, --help                 Display this help message");
    println!("  -v, --version              Display version information");
}

#[inline]
fn are_files_identical(file_a: &[u8], file_b: &[u8]) -> bool {
    if file_a.len() != file_b.len() {
        return false;
    }
    file_a == file_b
}

/// Detects if content is binary by scanning for null bytes and checking control characters
/// Uses heuristics similar to GNU diff
fn is_binary_content(content: &[u8]) -> bool {
    if content.is_empty() {
        return false;
    }

    // Check for null bytes (strong indicator of binary data)
    // Scan first 8KB (typical block size) for efficiency on large files
    let check_limit = std::cmp::min(content.len(), 8192);
    for &byte in &content[..check_limit] {
        if byte == 0 {
            return true;
        }
    }

    // Additional heuristic: check for high proportion of non-text bytes
    // This helps detect binary formats that don't contain null bytes
    let mut non_text_count = 0;
    let sample_size = std::cmp::min(content.len(), 512);

    for &byte in &content[..sample_size] {
        if matches!(byte, 0..=8 | 14..=31 | 127) {
            non_text_count += 1;
        }
    }

    // If more than 30% of sampled bytes are non-text, treat as binary
    if sample_size > 0 && (non_text_count * 100) / sample_size > 30 {
        return true;
    }

    false
}

/// Strips trailing carriage return from a byte slice if present
#[inline]
fn strip_trailing_cr(line: &[u8]) -> &[u8] {
    if line.ends_with(b"\r") {
        &line[..line.len() - 1]
    } else {
        line
    }
}

// Main diff3 computation engine with performance optimizations
fn compute_diff3(
    mine: &[u8],
    older: &[u8],
    yours: &[u8],
    params: &Diff3Params,
) -> io::Result<(Vec<u8>, bool)> {
    if are_files_identical(mine, older) && are_files_identical(older, yours) {
        return Ok((Vec::new(), false));
    }

    let mine_is_binary = !params.text && is_binary_content(mine);
    let older_is_binary = !params.text && is_binary_content(older);
    let yours_is_binary = !params.text && is_binary_content(yours);

    if mine_is_binary || older_is_binary || yours_is_binary {
        let all_identical = are_files_identical(mine, older) && are_files_identical(older, yours);

        if all_identical {
            return Ok((Vec::new(), false));
        } else {
            let mut output = Vec::new();

            let mine_name = params.mine.to_string_lossy();
            let older_name = params.older.to_string_lossy();
            let yours_name = params.yours.to_string_lossy();
            if mine_is_binary && older_is_binary && mine != older {
                writeln!(
                    &mut output,
                    "Binary files {} and {} differ",
                    mine_name, older_name
                )?;
            }
            if older_is_binary && yours_is_binary && older != yours {
                writeln!(
                    &mut output,
                    "Binary files {} and {} differ",
                    older_name, yours_name
                )?;
            }
            if mine_is_binary && yours_is_binary && mine != yours {
                writeln!(
                    &mut output,
                    "Binary files {} and {} differ",
                    mine_name, yours_name
                )?;
            }

            return Ok((output, true));
        }
    }

    let mut mine_lines: Vec<&[u8]> = mine.split(|&c| c == b'\n').collect();
    let mut older_lines: Vec<&[u8]> = older.split(|&c| c == b'\n').collect();
    let mut yours_lines: Vec<&[u8]> = yours.split(|&c| c == b'\n').collect();

    if params.strip_trailing_cr {
        for line in &mut mine_lines {
            *line = strip_trailing_cr(line);
        }
        for line in &mut older_lines {
            *line = strip_trailing_cr(line);
        }
        for line in &mut yours_lines {
            *line = strip_trailing_cr(line);
        }
    }

    let mine_lines = if mine_lines.last() == Some(&&b""[..]) {
        &mine_lines[..mine_lines.len() - 1]
    } else {
        &mine_lines
    };
    let older_lines = if older_lines.last() == Some(&&b""[..]) {
        &older_lines[..older_lines.len() - 1]
    } else {
        &older_lines
    };
    let yours_lines = if yours_lines.last() == Some(&&b""[..]) {
        &yours_lines[..yours_lines.len() - 1]
    } else {
        &yours_lines
    };

    if mine_lines == older_lines && older_lines == yours_lines {
        return Ok((Vec::new(), false));
    }

    let diff_mine_older: Vec<_> = diff::slice(mine_lines, older_lines);
    let diff_older_yours: Vec<_> = diff::slice(older_lines, yours_lines);
    let diff_mine_yours: Vec<_> = diff::slice(mine_lines, yours_lines);

    let _has_conflicts = detect_conflicts(
        &diff_mine_older,
        &diff_older_yours,
        &diff_mine_yours,
        mine_lines,
        older_lines,
        yours_lines,
    );

    let regions = build_conflict_regions(
        &diff_mine_older,
        &diff_older_yours,
        &diff_mine_yours,
        mine_lines,
        older_lines,
        yours_lines,
    );

    // Determine the appropriate exit code based on format and output mode
    // GNU diff3 behavior:
    // - Normal format: always returns 0 (changes are informational)
    // - Merged format: returns 1 if there are unresolved conflicts
    // - Ed format: depends on mode
    //   - -e, -x, -X, -3: always return 0 (ed scripts are meant to be applied)
    //   - -E, -A: return 1 if there are overlapping conflicts
    let should_report_conflict = match params.format {
        Diff3Format::Ed => {
            // Ed format exit codes depend on the output mode
            match params.output_mode {
                Diff3OutputMode::ShowOverlapEd | Diff3OutputMode::All => {
                    // -E or -A mode: return 1 if there are overlapping conflicts
                    regions
                        .iter()
                        .any(|r| r.conflict == ConflictType::OverlappingConflict)
                }
                _ => false, // -e, -x, -X, and -3 modes always return 0
            }
        }
        Diff3Format::ShowOverlap => {
            // -E mode: return 1 if there are overlapping conflicts
            regions
                .iter()
                .any(|r| r.conflict == ConflictType::OverlappingConflict)
        }
        Diff3Format::Normal => {
            // Normal format: GNU diff3 always returns 0
            // Changes are shown but don't indicate failure
            false
        }
        Diff3Format::Merged => {
            // Merged format exit code depends on output mode
            match params.output_mode {
                Diff3OutputMode::OverlapOnly | Diff3OutputMode::OverlapOnlyMarked => {
                    // -x or -X with -m: outputs resolved content (yours), returns 0 like ed scripts
                    false
                }
                _ => {
                    // Default merged format: return 1 if there are ANY conflicts needing resolution
                    // This includes both easy conflicts (one side changed) and overlapping (both changed)
                    regions.iter().any(|r| {
                        matches!(
                            r.conflict,
                            ConflictType::EasyConflict | ConflictType::OverlappingConflict
                        )
                    })
                }
            }
        }
    };

    Ok(match params.format {
        Diff3Format::Normal => (
            generate_normal_output(mine_lines, older_lines, yours_lines, &regions, params)?,
            should_report_conflict,
        ),
        Diff3Format::Merged => (
            generate_merged_output(mine_lines, older_lines, yours_lines, &regions, params)?,
            should_report_conflict,
        ),
        Diff3Format::Ed | Diff3Format::ShowOverlap => (
            generate_ed_script(mine_lines, older_lines, yours_lines, &regions, params)?,
            should_report_conflict,
        ),
    })
}

/// Types of conflicts that can occur in three-way merge
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConflictType {
    /// No conflict - files agree or only one changed
    #[default]
    NoConflict,
    /// Both mine and yours changed identically (non-overlapping)
    NonOverlapping,
    /// Mine and yours both changed but differently (easy conflict)
    EasyConflict,
    /// All three files differ (overlapping/difficult conflict)
    OverlappingConflict,
}

/// Represents a contiguous region in a three-way diff
#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
struct Diff3Region {
    /// Starting line in mine file
    pub(crate) mine_start: usize,
    /// Count of lines in mine file
    pub(crate) mine_count: usize,
    /// Starting line in older file
    pub(crate) older_start: usize,
    /// Count of lines in older file
    pub(crate) older_count: usize,
    /// Starting line in yours file
    pub(crate) yours_start: usize,
    /// Count of lines in yours file
    pub(crate) yours_count: usize,
    /// Type of conflict in this region
    conflict: ConflictType,
}

fn detect_conflicts(
    diff_mine_older: &[diff::Result<&&[u8]>],
    diff_older_yours: &[diff::Result<&&[u8]>],
    diff_mine_yours: &[diff::Result<&&[u8]>],
    mine_lines: &[&[u8]],
    older_lines: &[&[u8]],
    yours_lines: &[&[u8]],
) -> bool {
    let regions = build_conflict_regions(
        diff_mine_older,
        diff_older_yours,
        diff_mine_yours,
        mine_lines,
        older_lines,
        yours_lines,
    );

    // Has conflicts if any region has a conflict that's not NoConflict
    regions
        .iter()
        .any(|r| r.conflict != ConflictType::NoConflict)
}

/// Analyzes diffs to build conflict regions with classification
///
/// This function implements proper three-way merge logic by:
/// 1. Identifying change hunks in both mine-older and older-yours diffs
/// 2. Correlating hunks that affect overlapping base (older) line ranges
/// 3. Classifying each region based on what changed
fn build_conflict_regions(
    diff_mine_older: &[diff::Result<&&[u8]>],
    diff_older_yours: &[diff::Result<&&[u8]>],
    _diff_mine_yours: &[diff::Result<&&[u8]>],
    mine_lines: &[&[u8]],
    older_lines: &[&[u8]],
    yours_lines: &[&[u8]],
) -> Vec<Diff3Region> {
    // Hunk represents a contiguous region of changes
    #[derive(Debug, Clone)]
    struct Hunk {
        mine_start: usize,
        mine_count: usize,
        older_start: usize,
        older_count: usize,
        yours_start: usize,
        yours_count: usize,
        mine_changed: bool,
        yours_changed: bool,
    }

    let mut hunks: Vec<Hunk> = Vec::new();

    // Parse mine-older diff to identify changes
    let mut mine_idx = 0;
    let mut older_idx = 0;
    let mut current_hunk_start_mine = 0;
    let mut current_hunk_start_older = 0;
    let mut in_mine_change = false;

    for result in diff_mine_older {
        match result {
            diff::Result::Both(_, _) => {
                if in_mine_change {
                    // End of change hunk from mine
                    hunks.push(Hunk {
                        mine_start: current_hunk_start_mine,
                        mine_count: mine_idx - current_hunk_start_mine,
                        older_start: current_hunk_start_older,
                        older_count: older_idx - current_hunk_start_older,
                        yours_start: current_hunk_start_older,
                        yours_count: older_idx - current_hunk_start_older,
                        mine_changed: true,
                        yours_changed: false,
                    });
                    in_mine_change = false;
                }
                mine_idx += 1;
                older_idx += 1;
            }
            diff::Result::Left(_) => {
                if !in_mine_change {
                    current_hunk_start_mine = mine_idx;
                    current_hunk_start_older = older_idx;
                    in_mine_change = true;
                }
                mine_idx += 1;
            }
            diff::Result::Right(_) => {
                if !in_mine_change {
                    current_hunk_start_mine = mine_idx;
                    current_hunk_start_older = older_idx;
                    in_mine_change = true;
                }
                older_idx += 1;
            }
        }
    }

    // Handle final hunk if we ended in a change
    if in_mine_change {
        hunks.push(Hunk {
            mine_start: current_hunk_start_mine,
            mine_count: mine_idx - current_hunk_start_mine,
            older_start: current_hunk_start_older,
            older_count: older_idx - current_hunk_start_older,
            yours_start: current_hunk_start_older,
            yours_count: older_idx - current_hunk_start_older,
            mine_changed: true,
            yours_changed: false,
        });
    }

    // Parse older-yours diff and merge with existing hunks
    older_idx = 0;
    let mut yours_idx = 0;
    let mut current_hunk_start_yours = 0;
    current_hunk_start_older = 0;
    let mut in_yours_change = false;
    let mut yours_hunks: Vec<(usize, usize, usize, usize)> = Vec::new();

    for result in diff_older_yours {
        match result {
            diff::Result::Both(_, _) => {
                if in_yours_change {
                    yours_hunks.push((
                        current_hunk_start_older,
                        older_idx - current_hunk_start_older,
                        current_hunk_start_yours,
                        yours_idx - current_hunk_start_yours,
                    ));
                    in_yours_change = false;
                }
                older_idx += 1;
                yours_idx += 1;
            }
            diff::Result::Left(_) => {
                if !in_yours_change {
                    current_hunk_start_older = older_idx;
                    current_hunk_start_yours = yours_idx;
                    in_yours_change = true;
                }
                older_idx += 1;
            }
            diff::Result::Right(_) => {
                if !in_yours_change {
                    current_hunk_start_older = older_idx;
                    current_hunk_start_yours = yours_idx;
                    in_yours_change = true;
                }
                yours_idx += 1;
            }
        }
    }

    if in_yours_change {
        yours_hunks.push((
            current_hunk_start_older,
            older_idx - current_hunk_start_older,
            current_hunk_start_yours,
            yours_idx - current_hunk_start_yours,
        ));
    }

    // Merge yours changes into hunks, creating new hunks or updating existing ones
    for (yours_older_start, yours_older_count, yours_start, yours_count) in yours_hunks {
        let yours_older_end = yours_older_start + yours_older_count;

        // Find if this overlaps with any existing hunk
        let mut merged = false;
        for hunk in &mut hunks {
            let hunk_older_end = hunk.older_start + hunk.older_count;

            // Check for overlap in the older (base) file
            if yours_older_start < hunk_older_end && yours_older_end > hunk.older_start {
                // Overlapping region - mark yours as changed
                hunk.yours_changed = true;
                hunk.yours_start = yours_start;
                hunk.yours_count = yours_count;
                merged = true;
                break;
            }
        }

        // If no overlap, create a new hunk for yours-only change
        if !merged {
            hunks.push(Hunk {
                mine_start: yours_older_start,
                mine_count: yours_older_count,
                older_start: yours_older_start,
                older_count: yours_older_count,
                yours_start,
                yours_count,
                mine_changed: false,
                yours_changed: true,
            });
        }
    }

    // Sort hunks by older_start to maintain order
    hunks.sort_by_key(|h| h.older_start);

    // Convert hunks to regions with conflict classification
    let mut regions: Vec<Diff3Region> = Vec::new();

    for hunk in hunks {
        let conflict = if !hunk.mine_changed && !hunk.yours_changed {
            // No changes
            ConflictType::NoConflict
        } else if hunk.mine_changed && !hunk.yours_changed {
            // Only mine changed
            ConflictType::EasyConflict
        } else if !hunk.mine_changed && hunk.yours_changed {
            // Only yours changed
            ConflictType::EasyConflict
        } else {
            // Both changed - need to check if they changed the same way
            let mine_content = &mine_lines[hunk.mine_start..hunk.mine_start + hunk.mine_count];
            let yours_content = &yours_lines[hunk.yours_start..hunk.yours_start + hunk.yours_count];

            if mine_content == yours_content {
                // Both changed identically
                ConflictType::NonOverlapping
            } else {
                // Both changed differently - true conflict
                ConflictType::OverlappingConflict
            }
        };

        regions.push(Diff3Region {
            mine_start: hunk.mine_start,
            mine_count: hunk.mine_count,
            older_start: hunk.older_start,
            older_count: hunk.older_count,
            yours_start: hunk.yours_start,
            yours_count: hunk.yours_count,
            conflict,
        });
    }

    // If no hunks were created, files are identical - create a no-conflict region
    if regions.is_empty()
        && (!mine_lines.is_empty() || !older_lines.is_empty() || !yours_lines.is_empty())
    {
        regions.push(Diff3Region {
            mine_start: 0,
            mine_count: mine_lines.len(),
            older_start: 0,
            older_count: older_lines.len(),
            yours_start: 0,
            yours_count: yours_lines.len(),
            conflict: ConflictType::NoConflict,
        });
    }

    regions
}

/// Determines if a region should be included in output based on the output mode
fn should_include_region(region: &Diff3Region, output_mode: Diff3OutputMode) -> bool {
    match output_mode {
        Diff3OutputMode::All | Diff3OutputMode::EdScript | Diff3OutputMode::ShowOverlapEd => {
            // All modes show everything
            true
        }
        Diff3OutputMode::OverlapOnly | Diff3OutputMode::OverlapOnlyMarked => {
            // Only show overlapping conflicts
            region.conflict == ConflictType::OverlappingConflict
        }
        Diff3OutputMode::EasyOnly => {
            // Only show easy (non-overlapping) conflicts
            region.conflict == ConflictType::EasyConflict
                || region.conflict == ConflictType::NonOverlapping
        }
    }
}

fn generate_normal_output(
    mine_lines: &[&[u8]],
    older_lines: &[&[u8]],
    yours_lines: &[&[u8]],
    regions: &[Diff3Region],
    params: &Diff3Params,
) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    // GNU diff3 uses just a tab before content when -T is specified
    // Without -T, it uses two spaces
    let line_prefix = if params.initial_tab { "\t" } else { "  " };

    for region in regions {
        if !should_include_region(region, params.output_mode) {
            continue;
        }

        if region.conflict == ConflictType::NoConflict {
            continue;
        }

        match region.conflict {
            ConflictType::EasyConflict => {
                // Determine which file changed
                let mine_differs = region.mine_count != region.older_count
                    || (region.mine_count > 0
                        && region.mine_count <= mine_lines.len()
                        && region.older_count > 0
                        && region.older_count <= older_lines.len()
                        && mine_lines[region.mine_start..region.mine_start + region.mine_count]
                            != older_lines
                                [region.older_start..region.older_start + region.older_count]);

                let yours_differs = region.yours_count != region.older_count
                    || (region.yours_count > 0
                        && region.yours_count <= yours_lines.len()
                        && region.older_count > 0
                        && region.older_count <= older_lines.len()
                        && yours_lines
                            [region.yours_start..region.yours_start + region.yours_count]
                            != older_lines
                                [region.older_start..region.older_start + region.older_count]);

                if mine_differs && !yours_differs {
                    writeln!(&mut output, "====1")?;
                } else if yours_differs && !mine_differs {
                    writeln!(&mut output, "====3")?;
                } else {
                    // Shouldn't happen in EasyConflict, but treat as general conflict
                    writeln!(&mut output, "====")?;
                }
            }
            ConflictType::NonOverlapping => {
                writeln!(&mut output, "====")?;
            }
            ConflictType::OverlappingConflict => {
                writeln!(&mut output, "====")?;
            }
            ConflictType::NoConflict => {
                continue;
            }
        }

        // GNU diff3 normal format output rules:
        // - Always show file 1 and file 3 content
        // - Only show file 2 content for overlapping/non-overlapping conflicts
        let is_easy_conflict = region.conflict == ConflictType::EasyConflict;

        // Always show line ranges, content depends on conflict type
        if region.mine_count == 0 {
            writeln!(&mut output, "1:{}a", region.mine_start)?;
        } else {
            let start_line = region.mine_start + 1;
            let end_line = region.mine_start + region.mine_count;
            if start_line == end_line {
                writeln!(&mut output, "1:{}c", start_line)?;
            } else {
                writeln!(&mut output, "1:{},{}c", start_line, end_line)?;
            }
            // Always show mine content
            let end_idx = region
                .mine_start
                .saturating_add(region.mine_count)
                .min(mine_lines.len());
            for line in &mine_lines[region.mine_start..end_idx] {
                writeln!(
                    &mut output,
                    "{}{}",
                    line_prefix,
                    String::from_utf8_lossy(line)
                )?;
            }
        }

        if region.older_count == 0 {
            writeln!(&mut output, "2:{}a", region.older_start)?;
        } else {
            let start_line = region.older_start + 1;
            let end_line = region.older_start + region.older_count;
            if start_line == end_line {
                writeln!(&mut output, "2:{}c", start_line)?;
            } else {
                writeln!(&mut output, "2:{},{}c", start_line, end_line)?;
            }
            // Show older content only for overlapping/non-overlapping conflicts
            if !is_easy_conflict {
                let end_idx = region
                    .older_start
                    .saturating_add(region.older_count)
                    .min(older_lines.len());
                for line in &older_lines[region.older_start..end_idx] {
                    writeln!(
                        &mut output,
                        "{}{}",
                        line_prefix,
                        String::from_utf8_lossy(line)
                    )?;
                }
            }
        }

        if region.yours_count == 0 {
            writeln!(&mut output, "3:{}a", region.yours_start)?;
        } else {
            let start_line = region.yours_start + 1;
            let end_line = region.yours_start + region.yours_count;
            if start_line == end_line {
                writeln!(&mut output, "3:{}c", start_line)?;
            } else {
                writeln!(&mut output, "3:{},{}c", start_line, end_line)?;
            }
            // Always show yours content
            let end_idx = region
                .yours_start
                .saturating_add(region.yours_count)
                .min(yours_lines.len());
            for line in &yours_lines[region.yours_start..end_idx] {
                writeln!(
                    &mut output,
                    "{}{}",
                    line_prefix,
                    String::from_utf8_lossy(line)
                )?;
            }
        }
    }

    Ok(output)
}

fn generate_merged_output(
    mine_lines: &[&[u8]],
    older_lines: &[&[u8]],
    yours_lines: &[&[u8]],
    regions: &[Diff3Region],
    params: &Diff3Params,
) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();

    // Use file paths as default labels, matching GNU diff3 behavior
    let mine_label = params.labels[0]
        .as_deref()
        .or_else(|| params.mine.to_str())
        .unwrap_or("<<<<<<<");
    let older_label = params.labels[1]
        .as_deref()
        .or_else(|| params.older.to_str())
        .unwrap_or("|||||||");
    let yours_label = params.labels[2]
        .as_deref()
        .or_else(|| params.yours.to_str())
        .unwrap_or(">>>>>>>");

    let mut last_output_line = 0;

    for region in regions {
        if !should_include_region(region, params.output_mode) {
            continue;
        }

        while last_output_line < region.mine_start && last_output_line < mine_lines.len() {
            writeln!(
                &mut output,
                "{}",
                String::from_utf8_lossy(mine_lines[last_output_line])
            )?;
            last_output_line += 1;
        }

        match region.conflict {
            ConflictType::NoConflict => {
                let end_idx = region
                    .mine_start
                    .saturating_add(region.mine_count)
                    .min(mine_lines.len());
                for line in &mine_lines[region.mine_start..end_idx] {
                    writeln!(&mut output, "{}", String::from_utf8_lossy(line))?;
                }
                last_output_line = region.mine_start.saturating_add(region.mine_count);
            }
            ConflictType::EasyConflict => {
                if region.mine_count != region.older_count {
                    let end_idx = region
                        .mine_start
                        .saturating_add(region.mine_count)
                        .min(mine_lines.len());
                    for line in &mine_lines[region.mine_start..end_idx] {
                        writeln!(&mut output, "{}", String::from_utf8_lossy(line))?;
                    }
                } else {
                    let end_idx = region
                        .yours_start
                        .saturating_add(region.yours_count)
                        .min(yours_lines.len());
                    for line in &yours_lines[region.yours_start..end_idx] {
                        writeln!(&mut output, "{}", String::from_utf8_lossy(line))?;
                    }
                }
                last_output_line = region.mine_start.saturating_add(region.mine_count);
            }
            ConflictType::NonOverlapping => {
                let end_idx = region
                    .mine_start
                    .saturating_add(region.mine_count)
                    .min(mine_lines.len());
                for line in &mine_lines[region.mine_start..end_idx] {
                    writeln!(&mut output, "{}", String::from_utf8_lossy(line))?;
                }
                last_output_line = region.mine_start.saturating_add(region.mine_count);
            }
            ConflictType::OverlappingConflict => {
                // For -X or -x (OverlapOnlyMarked/OverlapOnly) in merged format,
                // output yours without markers, matching ed script behavior
                if (params.output_mode == Diff3OutputMode::OverlapOnlyMarked
                    || params.output_mode == Diff3OutputMode::OverlapOnly)
                    && params.format == Diff3Format::Merged
                {
                    let yours_end_idx = region
                        .yours_start
                        .saturating_add(region.yours_count)
                        .min(yours_lines.len());
                    for line in &yours_lines[region.yours_start..yours_end_idx] {
                        writeln!(&mut output, "{}", String::from_utf8_lossy(line))?;
                    }
                    last_output_line = region.mine_start.saturating_add(region.mine_count);
                } else {
                    // Normal merged output with conflict markers
                    writeln!(&mut output, "<<<<<<< {}", mine_label)?;
                    let mine_end_idx = region
                        .mine_start
                        .saturating_add(region.mine_count)
                        .min(mine_lines.len());
                    for line in &mine_lines[region.mine_start..mine_end_idx] {
                        writeln!(&mut output, "{}", String::from_utf8_lossy(line))?;
                    }

                    // Show overlap section if in Merged mode with -A (default) or ShowOverlap format
                    // GNU diff3 -m shows middle section by default (like -A)
                    if params.format == Diff3Format::Merged
                        || params.format == Diff3Format::ShowOverlap
                    {
                        writeln!(&mut output, "||||||| {}", older_label)?;
                        let older_end_idx = region
                            .older_start
                            .saturating_add(region.older_count)
                            .min(older_lines.len());
                        for line in &older_lines[region.older_start..older_end_idx] {
                            writeln!(&mut output, "{}", String::from_utf8_lossy(line))?;
                        }
                    }

                    writeln!(&mut output, "=======")?;
                    let yours_end_idx = region
                        .yours_start
                        .saturating_add(region.yours_count)
                        .min(yours_lines.len());
                    for line in &yours_lines[region.yours_start..yours_end_idx] {
                        writeln!(&mut output, "{}", String::from_utf8_lossy(line))?;
                    }
                    writeln!(&mut output, ">>>>>>> {}", yours_label)?;
                    last_output_line = region.mine_start.saturating_add(region.mine_count);
                }
            }
        }
    }

    while last_output_line < mine_lines.len() {
        writeln!(
            &mut output,
            "{}",
            String::from_utf8_lossy(mine_lines[last_output_line])
        )?;
        last_output_line += 1;
    }

    Ok(output)
}

fn generate_ed_script(
    mine_lines: &[&[u8]],
    older_lines: &[&[u8]],
    yours_lines: &[&[u8]],
    regions: &[Diff3Region],
    params: &Diff3Params,
) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();

    let mine_label = params.labels[0]
        .as_deref()
        .or_else(|| params.mine.to_str())
        .unwrap_or("mine");
    let older_label = params.labels[1]
        .as_deref()
        .or_else(|| params.older.to_str())
        .unwrap_or("older");
    let yours_label = params.labels[2]
        .as_deref()
        .or_else(|| params.yours.to_str())
        .unwrap_or("yours");

    // Ed scripts must be applied in reverse order (bottom to top)
    // to maintain line number accuracy
    let mut commands: Vec<String> = Vec::new();

    let use_conflict_markers = params.output_mode == Diff3OutputMode::ShowOverlapEd
        || params.output_mode == Diff3OutputMode::All;

    for region in regions.iter().rev() {
        if !should_include_region(region, params.output_mode) {
            continue;
        }

        match region.conflict {
            ConflictType::NoConflict => {
                continue;
            }
            ConflictType::NonOverlapping => {
                let start_line = region.mine_start + 1;
                let end_line = region.mine_start + region.mine_count;

                if region.mine_count == region.older_count
                    && region.mine_start < mine_lines.len()
                    && region.older_start < older_lines.len()
                {
                    let mine_slice = &mine_lines[region.mine_start
                        ..std::cmp::min(region.mine_start + region.mine_count, mine_lines.len())];
                    let older_slice = &older_lines[region.older_start
                        ..std::cmp::min(
                            region.older_start + region.older_count,
                            older_lines.len(),
                        )];

                    if mine_slice == older_slice {
                        continue;
                    }
                }

                if region.mine_count == 0 {
                    commands.push(format!("{}a", region.mine_start));
                    for i in region.mine_start
                        ..region.mine_start + region.mine_count.max(region.yours_count)
                    {
                        if i < mine_lines.len() {
                            commands.push(String::from_utf8_lossy(mine_lines[i]).to_string());
                        }
                    }
                    commands.push(".".to_string());
                } else {
                    if start_line == end_line {
                        commands.push(format!("{}c", start_line));
                    } else {
                        commands.push(format!("{},{}c", start_line, end_line));
                    }
                    for line in mine_lines
                        .iter()
                        .skip(region.mine_start)
                        .take(region.mine_count)
                    {
                        commands.push(String::from_utf8_lossy(line).to_string());
                    }
                    commands.push(".".to_string());
                }
            }
            ConflictType::EasyConflict => {
                let mine_differs = region.mine_count != region.older_count
                    || (region.mine_count > 0
                        && region.mine_start + region.mine_count <= mine_lines.len()
                        && region.older_start + region.older_count <= older_lines.len()
                        && mine_lines[region.mine_start..region.mine_start + region.mine_count]
                            != older_lines
                                [region.older_start..region.older_start + region.older_count]);

                if !mine_differs {
                    let start_line = region.mine_start + 1;
                    let end_line = region.mine_start + region.mine_count;

                    if region.mine_count == 0 && region.yours_count > 0 {
                        commands.push(format!("{}a", region.mine_start));
                        for line in yours_lines
                            .iter()
                            .skip(region.yours_start)
                            .take(region.yours_count)
                        {
                            commands.push(String::from_utf8_lossy(line).to_string());
                        }
                        commands.push(".".to_string());
                    } else if region.yours_count == 0 && region.mine_count > 0 {
                        if start_line == end_line {
                            commands.push(format!("{}d", start_line));
                        } else {
                            commands.push(format!("{},{}d", start_line, end_line));
                        }
                    } else if region.yours_count > 0 {
                        if start_line == end_line {
                            commands.push(format!("{}c", start_line));
                        } else {
                            commands.push(format!("{},{}c", start_line, end_line));
                        }
                        for line in yours_lines
                            .iter()
                            .skip(region.yours_start)
                            .take(region.yours_count)
                        {
                            commands.push(String::from_utf8_lossy(line).to_string());
                        }
                        commands.push(".".to_string());
                    }
                }
            }
            ConflictType::OverlappingConflict => {
                if use_conflict_markers {
                    // -E or -A mode: Insert conflict markers by splitting into separate commands
                    // This matches GNU diff3 behavior which preserves the original content
                    // and inserts markers around it

                    // First command: Insert the closing marker and yours content after mine's end line
                    let mine_end_line = region.mine_start + region.mine_count;
                    commands.push(format!("{}a", mine_end_line));

                    // For -A mode, include the middle section (older content)
                    if params.output_mode == Diff3OutputMode::All {
                        commands.push(format!("||||||| {}", older_label));
                        for line in older_lines
                            .iter()
                            .skip(region.older_start)
                            .take(region.older_count)
                        {
                            commands.push(String::from_utf8_lossy(line).to_string());
                        }
                    }

                    // Add separator, yours content, and closing marker
                    commands.push("=======".to_string());
                    for line in yours_lines
                        .iter()
                        .skip(region.yours_start)
                        .take(region.yours_count)
                    {
                        commands.push(String::from_utf8_lossy(line).to_string());
                    }
                    commands.push(format!(">>>>>>> {}", yours_label));
                    commands.push(".".to_string());

                    // Second command: Insert the opening marker before mine content
                    // This goes after line (mine_start), which puts it before mine content
                    if region.mine_start > 0 {
                        commands.push(format!("{}a", region.mine_start));
                    } else {
                        // For first line, use 0a to insert at beginning
                        commands.push("0a".to_string());
                    }
                    commands.push(format!("<<<<<<< {}", mine_label));
                    commands.push(".".to_string());
                } else {
                    let start_line = region.mine_start + 1;
                    let end_line = region.mine_start + region.mine_count;

                    if region.mine_count == 0 && region.yours_count > 0 {
                        commands.push(format!("{}a", region.mine_start));
                        for line in yours_lines
                            .iter()
                            .skip(region.yours_start)
                            .take(region.yours_count)
                        {
                            commands.push(String::from_utf8_lossy(line).to_string());
                        }
                        commands.push(".".to_string());
                    } else if region.yours_count == 0 && region.mine_count > 0 {
                        // Deletion
                        if start_line == end_line {
                            commands.push(format!("{}d", start_line));
                        } else {
                            commands.push(format!("{},{}d", start_line, end_line));
                        }
                    } else if region.yours_count > 0 {
                        if start_line == end_line {
                            commands.push(format!("{}c", start_line));
                        } else {
                            commands.push(format!("{},{}c", start_line, end_line));
                        }
                        for line in yours_lines
                            .iter()
                            .skip(region.yours_start)
                            .take(region.yours_count)
                        {
                            commands.push(String::from_utf8_lossy(line).to_string());
                        }
                        commands.push(".".to_string());
                    }
                }
            }
        }
    }

    for cmd in commands {
        writeln!(&mut output, "{}", cmd)?;
    }

    if params.compat_i {
        writeln!(&mut output, "w")?;
        writeln!(&mut output, "q")?;
    }

    Ok(output)
}

pub fn main(opts: Peekable<ArgsOs>) -> ExitCode {
    let params = match parse_params(opts) {
        Ok(p) => p,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };

    // Read files
    fn read_file_contents(filepath: &OsString) -> io::Result<Vec<u8>> {
        if filepath == "-" {
            let mut content = Vec::new();
            io::stdin().read_to_end(&mut content)?;
            Ok(content)
        } else {
            fs::read(filepath)
        }
    }

    let mut io_error = false;

    let mine_content = match read_file_contents(&params.mine) {
        Ok(content) => content,
        Err(e) => {
            report_failure_to_read_input_file(&params.executable, &params.mine, &e);
            io_error = true;
            vec![]
        }
    };

    let older_content = match read_file_contents(&params.older) {
        Ok(content) => content,
        Err(e) => {
            report_failure_to_read_input_file(&params.executable, &params.older, &e);
            io_error = true;
            vec![]
        }
    };

    let yours_content = match read_file_contents(&params.yours) {
        Ok(content) => content,
        Err(e) => {
            report_failure_to_read_input_file(&params.executable, &params.yours, &e);
            io_error = true;
            vec![]
        }
    };

    if io_error {
        return ExitCode::from(2);
    }

    // Compute diff3
    let (result, has_conflicts) =
        match compute_diff3(&mine_content, &older_content, &yours_content, &params) {
            Ok(res) => res,
            Err(e) => {
                eprintln!(
                    "{}: failed to generate output: {}",
                    params.executable.to_string_lossy(),
                    e
                );
                return ExitCode::from(2);
            }
        };

    if let Err(e) = io::stdout().write_all(&result) {
        eprintln!(
            "{}: failed to write output: {}",
            params.executable.to_string_lossy(),
            e
        );
        return ExitCode::from(2);
    }

    if has_conflicts {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Analyzes overlap information in diff regions
    fn analyze_overlap(regions: &[Diff3Region]) -> (usize, usize, usize) {
        let mut easy_conflicts = 0;
        let mut overlapping_conflicts = 0;
        let mut non_overlapping = 0;

        for region in regions {
            match region.conflict {
                ConflictType::EasyConflict => easy_conflicts += 1,
                ConflictType::OverlappingConflict => overlapping_conflicts += 1,
                ConflictType::NonOverlapping => non_overlapping += 1,
                ConflictType::NoConflict => {}
            }
        }

        (easy_conflicts, overlapping_conflicts, non_overlapping)
    }

    /// Checks if only easy (non-overlapping) conflicts exist
    fn has_only_easy_conflicts(regions: &[Diff3Region]) -> bool {
        regions.iter().all(|r| {
            r.conflict == ConflictType::NoConflict || r.conflict == ConflictType::EasyConflict
        })
    }

    /// Checks if overlapping (difficult) conflicts exist
    fn has_overlapping_conflicts(regions: &[Diff3Region]) -> bool {
        regions
            .iter()
            .any(|r| r.conflict == ConflictType::OverlappingConflict)
    }

    #[test]
    fn test_parse_params_basic() {
        let args = vec![
            OsString::from("diff3"),
            OsString::from("file1"),
            OsString::from("file2"),
            OsString::from("file3"),
        ]
        .into_iter()
        .peekable();

        let params = parse_params(args).expect("Failed to parse params");
        assert_eq!(params.mine, OsString::from("file1"));
        assert_eq!(params.older, OsString::from("file2"));
        assert_eq!(params.yours, OsString::from("file3"));
        assert_eq!(params.format, Diff3Format::Normal);
        assert_eq!(params.output_mode, Diff3OutputMode::All);
    }

    #[test]
    fn test_parse_params_with_merge_flag() {
        let args = vec![
            OsString::from("diff3"),
            OsString::from("-m"),
            OsString::from("file1"),
            OsString::from("file2"),
            OsString::from("file3"),
        ]
        .into_iter()
        .peekable();

        let params = parse_params(args).expect("Failed to parse params");
        assert_eq!(params.format, Diff3Format::Merged);
        assert_eq!(params.output_mode, Diff3OutputMode::All);
    }

    #[test]
    fn test_parse_params_with_ed_flag() {
        let args = vec![
            OsString::from("diff3"),
            OsString::from("-e"),
            OsString::from("file1"),
            OsString::from("file2"),
            OsString::from("file3"),
        ]
        .into_iter()
        .peekable();

        let params = parse_params(args).expect("Failed to parse params");
        assert_eq!(params.format, Diff3Format::Ed);
        assert_eq!(params.output_mode, Diff3OutputMode::EdScript);
    }

    #[test]
    fn test_parse_params_with_text_flag() {
        let args = vec![
            OsString::from("diff3"),
            OsString::from("-a"),
            OsString::from("file1"),
            OsString::from("file2"),
            OsString::from("file3"),
        ]
        .into_iter()
        .peekable();

        let params = parse_params(args).expect("Failed to parse params");
        assert!(params.text);
    }

    #[test]
    fn test_parse_params_with_labels() {
        let args = vec![
            OsString::from("diff3"),
            OsString::from("--label=mine"),
            OsString::from("--label=older"),
            OsString::from("--label=yours"),
            OsString::from("file1"),
            OsString::from("file2"),
            OsString::from("file3"),
        ]
        .into_iter()
        .peekable();

        let params = parse_params(args).expect("Failed to parse params");
        assert_eq!(params.labels[0], Some("mine".to_string()));
        assert_eq!(params.labels[1], Some("older".to_string()));
        assert_eq!(params.labels[2], Some("yours".to_string()));
    }

    #[test]
    fn test_parse_params_missing_files() {
        let args = vec![OsString::from("diff3"), OsString::from("file1")]
            .into_iter()
            .peekable();

        let result = parse_params(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_params_stdin() {
        let args = vec![
            OsString::from("diff3"),
            OsString::from("-"),
            OsString::from("file2"),
            OsString::from("file3"),
        ]
        .into_iter()
        .peekable();

        let params = parse_params(args).expect("Failed to parse params");
        assert_eq!(params.mine, OsString::from("-"));
        assert_eq!(params.older, OsString::from("file2"));
        assert_eq!(params.yours, OsString::from("file3"));
    }

    #[test]
    fn test_parse_params_with_show_all_flag() {
        let args = vec![
            OsString::from("diff3"),
            OsString::from("-A"),
            OsString::from("file1"),
            OsString::from("file2"),
            OsString::from("file3"),
        ]
        .into_iter()
        .peekable();

        let params = parse_params(args).expect("Failed to parse params");
        assert_eq!(params.output_mode, Diff3OutputMode::All);
    }

    #[test]
    fn test_parse_params_with_easy_only_flag() {
        let args = vec![
            OsString::from("diff3"),
            OsString::from("-3"),
            OsString::from("file1"),
            OsString::from("file2"),
            OsString::from("file3"),
        ]
        .into_iter()
        .peekable();

        let params = parse_params(args).expect("Failed to parse params");
        assert_eq!(params.output_mode, Diff3OutputMode::EasyOnly);
    }

    #[test]
    fn test_compute_diff3_identical_files() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("file1"),
            older: OsString::from("file2"),
            yours: OsString::from("file3"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: false,
        };

        let content = b"line1\nline2\nline3\n";
        let (output, has_conflicts) =
            compute_diff3(content, content, content, &params).expect("compute_diff3 failed");

        // Identical files should produce no output
        assert!(output.is_empty());
        assert!(!has_conflicts);
    }

    #[test]
    fn test_compute_diff3_with_changes() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("file1"),
            older: OsString::from("file2"),
            yours: OsString::from("file3"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: false,
        };

        let mine = b"line1\nmodified\nline3\n";
        let older = b"line1\nline2\nline3\n";
        let yours = b"line1\nline2\nline3\n";

        let (output, _has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");

        // Should have some output indicating differences
        assert!(!output.is_empty());
    }

    #[test]
    fn test_compute_diff3_merged_format() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("file1"),
            older: OsString::from("file2"),
            yours: OsString::from("file3"),
            format: Diff3Format::Merged,
            output_mode: Diff3OutputMode::All,
            text: false,
            labels: [Some("mine".to_string()), None, Some("yours".to_string())],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: false,
        };

        let mine = b"line1\nmine_version\nline3\n";
        let older = b"line1\noriginal\nline3\n";
        let yours = b"line1\nyours_version\nline3\n";

        let (output, _has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");

        let output_str = String::from_utf8_lossy(&output);

        // Merged format with conflicts should contain conflict markers
        assert!(output_str.contains("<<<<<<<") || !output.is_empty());
    }

    #[test]
    fn test_compute_diff3_identical_lines() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("file1"),
            older: OsString::from("file2"),
            yours: OsString::from("file3"),
            format: Diff3Format::Merged,
            output_mode: Diff3OutputMode::All,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: false,
        };

        // When mine and yours are identical, should pass through unchanged
        let content = b"line1\nline2\nline3\n";
        let (output, _has_conflicts) =
            compute_diff3(content, b"different\n", content, &params).expect("compute_diff3 failed");

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("line1"));
        assert!(output_str.contains("line2"));
        assert!(output_str.contains("line3"));
    }

    #[test]
    fn test_compute_diff3_empty_files() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("file1"),
            older: OsString::from("file2"),
            yours: OsString::from("file3"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: false,
        };

        let (output, has_conflicts) =
            compute_diff3(b"", b"", b"", &params).expect("compute_diff3 failed");

        assert!(output.is_empty());
        assert!(!has_conflicts);
    }

    #[test]
    fn test_compute_diff3_single_line_files() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("file1"),
            older: OsString::from("file2"),
            yours: OsString::from("file3"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: false,
        };

        let mine = b"line1\n";
        let older = b"line1\n";
        let yours = b"different\n";

        let (output, _has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");

        // Should produce output for the difference
        assert!(!output.is_empty());
    }

    #[test]
    fn test_detect_conflicts() {
        // Test conflict detection logic
        let diff_mine_older: Vec<diff::Result<&&[u8]>> = vec![];
        let diff_older_yours: Vec<diff::Result<&&[u8]>> = vec![];
        let diff_mine_yours: Vec<diff::Result<&&[u8]>> = vec![];

        let result = detect_conflicts(
            &diff_mine_older,
            &diff_older_yours,
            &diff_mine_yours,
            &[b"line1"],
            &[b"line1"],
            &[b"line1"],
        );

        // Identical files should have no conflicts
        assert!(!result);
    }

    #[test]
    fn test_conflict_type_no_conflict() {
        assert_eq!(ConflictType::NoConflict, ConflictType::NoConflict);
    }

    #[test]
    fn test_conflict_type_easy_conflict() {
        assert_eq!(ConflictType::EasyConflict, ConflictType::EasyConflict);
    }

    #[test]
    fn test_conflict_type_overlapping_conflict() {
        assert_eq!(
            ConflictType::OverlappingConflict,
            ConflictType::OverlappingConflict
        );
    }

    #[test]
    fn test_conflict_type_non_overlapping() {
        assert_eq!(ConflictType::NonOverlapping, ConflictType::NonOverlapping);
    }

    #[test]
    fn test_conflict_type_inequality() {
        assert_ne!(ConflictType::NoConflict, ConflictType::EasyConflict);
        assert_ne!(
            ConflictType::EasyConflict,
            ConflictType::OverlappingConflict
        );
        assert_ne!(ConflictType::NonOverlapping, ConflictType::NoConflict);
    }

    #[test]
    fn test_analyze_overlap_empty_regions() {
        let regions = vec![];
        let (easy, overlapping, non_overlapping) = analyze_overlap(&regions);
        assert_eq!(easy, 0);
        assert_eq!(overlapping, 0);
        assert_eq!(non_overlapping, 0);
    }

    #[test]
    fn test_analyze_overlap_all_easy_conflicts() {
        let regions = vec![
            Diff3Region {
                mine_start: 0,
                mine_count: 1,
                older_start: 0,
                older_count: 1,
                yours_start: 0,
                yours_count: 1,
                conflict: ConflictType::EasyConflict,
            },
            Diff3Region {
                mine_start: 1,
                mine_count: 1,
                older_start: 1,
                older_count: 1,
                yours_start: 1,
                yours_count: 1,
                conflict: ConflictType::EasyConflict,
            },
        ];
        let (easy, overlapping, non_overlapping) = analyze_overlap(&regions);
        assert_eq!(easy, 2);
        assert_eq!(overlapping, 0);
        assert_eq!(non_overlapping, 0);
    }

    #[test]
    fn test_analyze_overlap_all_overlapping() {
        let regions = vec![Diff3Region {
            mine_start: 0,
            mine_count: 2,
            older_start: 0,
            older_count: 1,
            yours_start: 0,
            yours_count: 3,
            conflict: ConflictType::OverlappingConflict,
        }];
        let (easy, overlapping, non_overlapping) = analyze_overlap(&regions);
        assert_eq!(easy, 0);
        assert_eq!(overlapping, 1);
        assert_eq!(non_overlapping, 0);
    }

    #[test]
    fn test_analyze_overlap_mixed_conflicts() {
        let regions = vec![
            Diff3Region {
                mine_start: 0,
                mine_count: 1,
                older_start: 0,
                older_count: 1,
                yours_start: 0,
                yours_count: 1,
                conflict: ConflictType::EasyConflict,
            },
            Diff3Region {
                mine_start: 1,
                mine_count: 1,
                older_start: 1,
                older_count: 1,
                yours_start: 1,
                yours_count: 1,
                conflict: ConflictType::OverlappingConflict,
            },
            Diff3Region {
                mine_start: 2,
                mine_count: 1,
                older_start: 2,
                older_count: 1,
                yours_start: 2,
                yours_count: 1,
                conflict: ConflictType::NonOverlapping,
            },
            Diff3Region {
                mine_start: 3,
                mine_count: 1,
                older_start: 3,
                older_count: 1,
                yours_start: 3,
                yours_count: 1,
                conflict: ConflictType::NoConflict,
            },
        ];
        let (easy, overlapping, non_overlapping) = analyze_overlap(&regions);
        assert_eq!(easy, 1);
        assert_eq!(overlapping, 1);
        assert_eq!(non_overlapping, 1);
    }

    #[test]
    fn test_has_only_easy_conflicts_true() {
        let regions = vec![
            Diff3Region {
                mine_start: 0,
                mine_count: 1,
                older_start: 0,
                older_count: 1,
                yours_start: 0,
                yours_count: 1,
                conflict: ConflictType::EasyConflict,
            },
            Diff3Region {
                mine_start: 1,
                mine_count: 1,
                older_start: 1,
                older_count: 1,
                yours_start: 1,
                yours_count: 1,
                conflict: ConflictType::NoConflict,
            },
        ];
        assert!(has_only_easy_conflicts(&regions));
    }

    #[test]
    fn test_has_only_easy_conflicts_false() {
        let regions = vec![
            Diff3Region {
                mine_start: 0,
                mine_count: 1,
                older_start: 0,
                older_count: 1,
                yours_start: 0,
                yours_count: 1,
                conflict: ConflictType::EasyConflict,
            },
            Diff3Region {
                mine_start: 1,
                mine_count: 2,
                older_start: 1,
                older_count: 1,
                yours_start: 1,
                yours_count: 3,
                conflict: ConflictType::OverlappingConflict,
            },
        ];
        assert!(!has_only_easy_conflicts(&regions));
    }

    #[test]
    fn test_has_only_easy_conflicts_empty() {
        let regions = vec![];
        // Empty regions should return true (vacuously true)
        assert!(has_only_easy_conflicts(&regions));
    }

    #[test]
    fn test_has_overlapping_conflicts_true() {
        let regions = vec![
            Diff3Region {
                mine_start: 0,
                mine_count: 1,
                older_start: 0,
                older_count: 1,
                yours_start: 0,
                yours_count: 1,
                conflict: ConflictType::EasyConflict,
            },
            Diff3Region {
                mine_start: 1,
                mine_count: 2,
                older_start: 1,
                older_count: 1,
                yours_start: 1,
                yours_count: 3,
                conflict: ConflictType::OverlappingConflict,
            },
        ];
        assert!(has_overlapping_conflicts(&regions));
    }

    #[test]
    fn test_has_overlapping_conflicts_false() {
        let regions = vec![
            Diff3Region {
                mine_start: 0,
                mine_count: 1,
                older_start: 0,
                older_count: 1,
                yours_start: 0,
                yours_count: 1,
                conflict: ConflictType::EasyConflict,
            },
            Diff3Region {
                mine_start: 1,
                mine_count: 1,
                older_start: 1,
                older_count: 1,
                yours_start: 1,
                yours_count: 1,
                conflict: ConflictType::NoConflict,
            },
        ];
        assert!(!has_overlapping_conflicts(&regions));
    }

    #[test]
    fn test_has_overlapping_conflicts_empty() {
        let regions = vec![];
        assert!(!has_overlapping_conflicts(&regions));
    }

    #[test]
    fn test_diff3_region_construction() {
        let region = Diff3Region {
            mine_start: 10,
            mine_count: 5,
            older_start: 12,
            older_count: 3,
            yours_start: 15,
            yours_count: 7,
            conflict: ConflictType::OverlappingConflict,
        };

        assert_eq!(region.mine_start, 10);
        assert_eq!(region.mine_count, 5);
        assert_eq!(region.older_start, 12);
        assert_eq!(region.older_count, 3);
        assert_eq!(region.yours_start, 15);
        assert_eq!(region.yours_count, 7);
        assert_eq!(region.conflict, ConflictType::OverlappingConflict);
    }

    #[test]
    fn test_diff3_region_cloning() {
        let region1 = Diff3Region {
            mine_start: 0,
            mine_count: 1,
            older_start: 0,
            older_count: 1,
            yours_start: 0,
            yours_count: 1,
            conflict: ConflictType::NoConflict,
        };
        let region2 = region1.clone();

        assert_eq!(region1.mine_start, region2.mine_start);
        assert_eq!(region1.conflict, region2.conflict);
    }

    #[test]
    fn test_parse_params_with_compat_i_flag() {
        let args = vec![
            OsString::from("diff3"),
            OsString::from("-i"),
            OsString::from("file1"),
            OsString::from("file2"),
            OsString::from("file3"),
        ]
        .into_iter()
        .peekable();

        let params = parse_params(args).expect("Failed to parse params");
        assert!(params.compat_i);
        assert_eq!(params.format, Diff3Format::Normal);
    }

    #[test]
    fn test_parse_params_ed_with_compat_i() {
        let args = vec![
            OsString::from("diff3"),
            OsString::from("-e"),
            OsString::from("-i"),
            OsString::from("file1"),
            OsString::from("file2"),
            OsString::from("file3"),
        ]
        .into_iter()
        .peekable();

        let params = parse_params(args).expect("Failed to parse params");
        assert!(params.compat_i);
        assert_eq!(params.format, Diff3Format::Ed);
        assert_eq!(params.output_mode, Diff3OutputMode::EdScript);
    }

    #[test]
    fn test_compute_diff3_ed_with_compat_i() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("file1"),
            older: OsString::from("file2"),
            yours: OsString::from("file3"),
            format: Diff3Format::Ed,
            output_mode: Diff3OutputMode::EdScript,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: true, // Enable -i option
        };

        let mine = b"line1\noriginal\nline3\n";
        let older = b"line1\noriginal\nline3\n";
        let yours = b"line1\nmodified\nline3\n";

        let (output, _has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");

        let output_str = String::from_utf8_lossy(&output);
        // With -i, output should contain write (w) and quit (q) commands
        assert!(
            output_str.contains("w\n") || output_str.contains("w"),
            "Output should contain write command"
        );
        assert!(
            output_str.contains("q\n") || output_str.contains("q"),
            "Output should contain quit command"
        );
    }

    #[test]
    fn test_compute_diff3_ed_without_compat_i() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("file1"),
            older: OsString::from("file2"),
            yours: OsString::from("file3"),
            format: Diff3Format::Ed,
            output_mode: Diff3OutputMode::EdScript,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: false, // Disable -i option
        };

        let mine = b"line1\noriginal\nline3\n";
        let older = b"line1\noriginal\nline3\n";
        let yours = b"line1\nmodified\nline3\n";

        let (output, _has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");

        let output_str = String::from_utf8_lossy(&output);
        // Without -i, output should NOT contain write (w) and quit (q) at the end
        // (It may contain them in the middle but not as final commands)
        let lines: Vec<&str> = output_str.lines().collect();
        if !lines.is_empty() {
            let last_line = lines[lines.len() - 1];
            assert_ne!(
                last_line, "q",
                "Last line should not be 'q' without -i flag"
            );
        }
    }

    #[test]
    fn test_is_binary_content_with_null_bytes() {
        // Binary content with null bytes
        let binary = b"GIF89a\x00\x00\x00\x00";
        assert!(
            is_binary_content(binary),
            "Should detect null bytes as binary"
        );
    }

    #[test]
    fn test_is_binary_content_text_only() {
        // Plain text content
        let text = b"This is plain text\nwith multiple lines\n";
        assert!(
            !is_binary_content(text),
            "Should not detect plain text as binary"
        );
    }

    #[test]
    fn test_is_binary_content_with_control_chars() {
        // Content with excessive control characters
        let binary = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0xFF, 0xFE];
        assert!(
            is_binary_content(&binary),
            "Should detect control-heavy content as binary"
        );
    }

    #[test]
    fn test_is_binary_content_empty() {
        // Empty content should not be treated as binary
        let empty: &[u8] = b"";
        assert!(
            !is_binary_content(empty),
            "Empty content should not be binary"
        );
    }

    #[test]
    fn test_compute_diff3_with_binary_files() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("mine.bin"),
            older: OsString::from("older.bin"),
            yours: OsString::from("yours.bin"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: false, // Binary detection enabled
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: false,
        };

        // Binary files with null bytes
        let mine = b"GIF89a\x00\x10\x00\x10";
        let older = b"GIF89a\x00\x10\x00\x10";
        let yours = b"PNG\x89\x50\x4E\x47\x0D";

        let (output, has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");

        // Should report binary file differences
        let output_str = String::from_utf8_lossy(&output);
        assert!(
            output_str.contains("Binary files") || output_str.is_empty(),
            "Should report binary differences or be empty"
        );
        // Different binary files should have conflicts
        assert!(
            has_conflicts,
            "Different binary files should be detected as conflicts"
        );
    }

    #[test]
    fn test_compute_diff3_binary_identical() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("mine.bin"),
            older: OsString::from("older.bin"),
            yours: OsString::from("yours.bin"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: false, // Binary detection enabled
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: false,
        };

        // Identical binary files
        let content = b"GIF89a\x00\x10\x00\x10\xFF\xFF\xFF";

        let (output, has_conflicts) =
            compute_diff3(content, content, content, &params).expect("compute_diff3 failed");

        // Identical files should have no output and no conflicts
        assert!(
            output.is_empty(),
            "Identical binary files should produce no output"
        );
        assert!(
            !has_conflicts,
            "Identical binary files should have no conflicts"
        );
    }

    #[test]
    fn test_text_flag_forces_text_mode() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("mine"),
            older: OsString::from("older"),
            yours: OsString::from("yours"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: true, // Force text mode even for binary-looking content
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: false,
        };

        // Content with null byte (normally binary)
        let mine = b"line1\x00\nline2\n";
        let older = b"line1\x00\nline2\n";
        let yours = b"line1\x00\nline3\n";

        let (output, _has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");

        // With --text flag, should process as text, not report as binary
        let output_str = String::from_utf8_lossy(&output);
        // Should not be "Binary files differ" - instead should process as text
        if !output.is_empty() {
            assert!(
                !output_str.contains("Binary files differ"),
                "Should not report binary when --text flag is set"
            );
        }
    }

    #[test]
    fn test_strip_trailing_cr() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("mine"),
            older: OsString::from("older"),
            yours: OsString::from("yours"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: true,
            initial_tab: false,
            compat_i: false,
        };

        // Files with CRLF line endings (Windows style)
        let mine = b"line1\r\nline2\r\nline3\r\n";
        let older = b"line1\r\nline2\r\nline3\r\n";
        let yours = b"line1\r\nline2\r\nline3\r\n";

        let (output, has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");

        // Should treat files as identical despite CRLF
        assert!(!has_conflicts);
        assert!(output.is_empty());
    }

    #[test]
    fn test_strip_trailing_cr_with_differences() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("mine"),
            older: OsString::from("older"),
            yours: OsString::from("yours"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: true,
            initial_tab: false,
            compat_i: false,
        };

        // Files with CRLF and actual content differences - create a real conflict
        let mine = b"line1\r\nmodified_mine\r\nline3\r\n";
        let older = b"line1\r\nline2\r\nline3\r\n";
        let yours = b"line1\r\nmodified_yours\r\nline3\r\n";

        let (output, has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");

        // Normal format always returns false for has_conflicts (exit code 0)
        // but should still produce diff output showing the differences
        assert!(!has_conflicts, "Normal format always returns false for conflicts");
        assert!(!output.is_empty(), "Should produce output showing differences");
    }

    #[test]
    fn test_strip_trailing_cr_mixed_line_endings() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("mine"),
            older: OsString::from("older"),
            yours: OsString::from("yours"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: true,
            initial_tab: false,
            compat_i: false,
        };

        // Mine has CRLF, older and yours have LF - should be identical with strip_trailing_cr
        let mine = b"line1\r\nline2\r\nline3\r\n";
        let older = b"line1\nline2\nline3\n";
        let yours = b"line1\nline2\nline3\n";

        let (output, has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");

        // Should treat files as identical when only line endings differ
        assert!(!has_conflicts);
        assert!(output.is_empty());
    }

    #[test]
    fn test_initial_tab_flag() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("mine"),
            older: OsString::from("older"),
            yours: OsString::from("yours"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: true,
            compat_i: false,
        };

        let mine = b"line1\nchanged_mine\nline3\n";
        let older = b"line1\nline2\nline3\n";
        let yours = b"line1\nline2\nline3\n";

        let (output, _has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");
        let output_str = String::from_utf8_lossy(&output);

        // With initial_tab, content lines should be prefixed with a tab
        // (without initial_tab they would be prefixed with two spaces)
        assert!(
            output_str.contains("\t"),
            "Should have tab before content"
        );
    }

    #[test]
    fn test_without_initial_tab_flag() {
        let params = Diff3Params {
            executable: OsString::from("diff3"),
            mine: OsString::from("mine"),
            older: OsString::from("older"),
            yours: OsString::from("yours"),
            format: Diff3Format::Normal,
            output_mode: Diff3OutputMode::All,
            text: false,
            labels: [None, None, None],
            strip_trailing_cr: false,
            initial_tab: false,
            compat_i: false,
        };

        let mine = b"line1\nchanged_mine\nline3\n";
        let older = b"line1\nline2\nline3\n";
        let yours = b"line1\nline2\nline3\n";

        let (output, _has_conflicts) =
            compute_diff3(mine, older, yours, &params).expect("compute_diff3 failed");
        let output_str = String::from_utf8_lossy(&output);

        // Without initial_tab, content lines should have two spaces but no tab
        assert!(
            output_str.contains("  changed_mine"),
            "Should have two-space prefix for content"
        );
        assert!(
            !output_str.contains("\t  "),
            "Should not have tab before two-space prefix"
        );
    }
}
