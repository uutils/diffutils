#!/usr/bin/env python3

"""
Compare the current GNU test results to the last results gathered from the main branch to
highlight if a PR is making the results better/worse.
Don't exit with error code if all failing tests are in the ignore-intermittent.txt list.
"""

import json
import sys
import argparse
from pathlib import Path


def load_ignore_list(ignore_file):
    """Load list of intermittent test names to ignore from file."""
    ignore_set = set()
    if ignore_file and Path(ignore_file).exists():
        with open(ignore_file, "r") as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#"):
                    ignore_set.add(line)
    return ignore_set


def extract_test_results(json_data):
    """Extract test results from a diffutils test-results.json.

    Note: unlike sed, diffutils JSON has no 'summary' object — results are
    computed from the 'tests' array using the 'result' and 'test' fields.
    """
    tests = json_data.get("tests", [])
    passed  = sum(1 for t in tests if t.get("result") == "PASS")
    failed  = sum(1 for t in tests if t.get("result") == "FAIL")
    skipped = sum(1 for t in tests if t.get("result") == "SKIP")
    summary = {"total": len(tests), "passed": passed, "failed": failed, "skipped": skipped}
    failed_tests = [t["test"] for t in tests if t.get("result") == "FAIL"]
    return summary, failed_tests


def compare_results(current_file, reference_file, ignore_file=None, output_file=None):
    """Compare current results with reference results."""
    ignore_set = load_ignore_list(ignore_file)

    try:
        with open(current_file, "r") as f:
            current_data = json.load(f)
        current_summary, current_failed = extract_test_results(current_data)
    except Exception as e:
        print(f"Error loading current results: {e}")
        return 1

    try:
        with open(reference_file, "r") as f:
            reference_data = json.load(f)
        reference_summary, reference_failed = extract_test_results(reference_data)
    except Exception as e:
        print(f"Error loading reference results: {e}")
        return 1

    # Calculate differences
    pass_diff  = int(current_summary.get("passed",  0)) - int(reference_summary.get("passed",  0))
    fail_diff  = int(current_summary.get("failed",  0)) - int(reference_summary.get("failed",  0))
    total_diff = int(current_summary.get("total",   0)) - int(reference_summary.get("total",   0))

    # Find new failures and improvements
    current_failed_set   = set(current_failed)
    reference_failed_set = set(reference_failed)

    new_failures  = current_failed_set   - reference_failed_set
    improvements  = reference_failed_set - current_failed_set

    # Filter out intermittent failures
    non_intermittent_new_failures = new_failures - ignore_set

    # Check if results are identical (no changes)
    no_changes = (
        pass_diff == 0
        and fail_diff == 0
        and total_diff == 0
        and not new_failures
        and not improvements
    )

    # If no changes, write empty output to prevent comment posting
    if no_changes:
        if output_file:
            with open(output_file, "w") as f:
                f.write("")
        return 0

    # Prepare output message
    output_lines = []

    output_lines.append("Test results comparison:")
    output_lines.append(
        f"  Current:   TOTAL: {current_summary.get('total', 0)} / PASSED: {current_summary.get('passed', 0)} / FAILED: {current_summary.get('failed', 0)} / SKIPPED: {current_summary.get('skipped', 0)}"
    )
    output_lines.append(
        f"  Reference: TOTAL: {reference_summary.get('total', 0)} / PASSED: {reference_summary.get('passed', 0)} / FAILED: {reference_summary.get('failed', 0)} / SKIPPED: {reference_summary.get('skipped', 0)}"
    )
    output_lines.append("")

    if pass_diff != 0 or fail_diff != 0 or total_diff != 0:
        output_lines.append("Changes from main branch:")
        output_lines.append(f"  TOTAL: {total_diff:+d}")
        output_lines.append(f"  PASSED: {pass_diff:+d}")
        output_lines.append(f"  FAILED: {fail_diff:+d}")
        output_lines.append("")

    if new_failures:
        output_lines.append(f"New test failures ({len(new_failures)}):")
        for test in sorted(new_failures):
            if test in ignore_set:
                output_lines.append(f"  - {test} (intermittent)")
            else:
                output_lines.append(f"  - {test}")
        output_lines.append("")

    if improvements:
        output_lines.append(f"Test improvements ({len(improvements)}):")
        for test in sorted(improvements):
            output_lines.append(f"  + {test}")
        output_lines.append("")

    output_text = "\n".join(output_lines)
    if output_file:
        with open(output_file, "w") as f:
            f.write(output_text)
    else:
        print(output_text)

    if non_intermittent_new_failures:
        print(
            f"ERROR: Found {len(non_intermittent_new_failures)} new non-intermittent test failures"
        )
        return 1

    return 0


def main():
    parser = argparse.ArgumentParser(description="Compare GNU diffutils test results")
    parser.add_argument("current",   help="Current test results JSON file")
    parser.add_argument("reference", help="Reference test results JSON file")
    parser.add_argument(
        "--ignore-file", help="File containing intermittent test names to ignore"
    )
    parser.add_argument("--output",  help="Output file for comparison results")

    args = parser.parse_args()

    return compare_results(args.current, args.reference, args.ignore_file, args.output)


if __name__ == "__main__":
    sys.exit(main())
