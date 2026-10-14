#!/bin/bash

# Run the GNU upstream test suite for diffutils against a local build of the
# Rust implementation, print out a summary of the test results, and writes a
# JSON file ('test-results.json') containing detailed information about the
# test run.

# The JSON file contains metadata about the test run, and for each test the
# result as well as the contents of stdout, stderr, and of all the files
# written by the test script, if any (excluding subdirectories).

# The script takes a shortcut to fetch only the test suite from the upstream
# repository and carefully avoids running the autotools machinery which is
# time-consuming and resource-intensive, and doesn't offer the option to not
# build the upstream binaries. As a consequence, the environment in which the
# tests are run might not match exactly that used when the upstream tests are
# run through the autotools.

# Exit codes: 0 if all tests passed, 1 if at least one test failed, and 2 if
# the test suite could not be run at all (e.g. the upstream repository couldn't
# be fetched). Callers must treat 2 as an infrastructure error: no meaningful
# result was produced.

# By default it expects a release build of the diffutils binary, but a
# different build profile can be specified as an argument
# (e.g. 'dev' or 'test').
# Unless overridden by the $TESTS environment variable, all tests in the test
# suite will be run. Tests targeting a command that is not yet implemented
# (e.g. diff3 or sdiff) are skipped.

scriptpath=$(dirname "$(readlink -f "$0")")
rev=$(git rev-parse HEAD)
gnu_version="3.12"
gnu_tarball_blake2b=5b4593b39da71578d7f975603abe9359be215b9ac76548a6ab0d6e3838bb103c7ffcddf7fa01abcd5c6289db9a2f16b43aa3d5e846a9aa4b8db866763c2660de

# Report an infrastructure error: the test suite could not be run at all
die() {
  echo "ERROR: $*" >&2
  exit 2
}

# Allow passing a specific profile as parameter (default to "release")
profile="release"
[[ -n $1 ]] && profile="$1"

# Verify that the diffutils binary was built for the requested profile
binary="$scriptpath/../target/$profile/diffutils"
if [[ ! -x "$binary" ]]
then
  die "Missing build for profile $profile"
fi

# Work in a temporary directory
tempdir=$(mktemp -d)
trap 'rm -rf "$tempdir"' EXIT
cd "$tempdir" || die "Cannot enter temporary directory $tempdir"

# Fetch the upstream test suite
echo "Fetching upstream test suite"
curl -fsSL --retry 5 https://ftpmirror.gnu.org/diffutils/diffutils-${gnu_version}.tar.xz -o gnu-diffutils.tar.xz || die "Failed to fetch test suite"
echo ${gnu_tarball_blake2b}  gnu-diffutils.tar.xz \
 | b2sum --check || die "Failed to verify checksum of tarball of GNU diffutils"
mkdir -p diffutils
tar xJf gnu-diffutils.tar.xz --strip-components=1 -C diffutils || die "Failed to extract test suite"

cd diffutils || die "Test suite src is broken"

# Ensure that calling `diff` invokes the built `diffutils` binary instead of
# the upstream `diff` binary that is most likely installed on the system
cd src || die "Missing the directory holding the diff and cmp symlinks"
ln -s "$binary" diff
ln -s "$binary" cmp
cd ../tests || die "Cannot enter the upstream tests directory"

if [[ -n "$TESTS" ]]
then
  tests="$TESTS"
else
  # Get a list of all upstream tests (default if $TESTS isn't set)
  echo -e '\n\nprinttests:\n\t@echo "${TESTS}"' >> Makefile.am
  tests=$(make -f Makefile.am printtests)
fi
total=$(echo "$tests" | wc -w)
(( total > 0 )) || die "No test to run: the upstream test list is empty"
echo "Running $total tests"
export LC_ALL=C
export KEEP=yes
timestamp=$(date -Iseconds)
passed=0
failed=0
skipped=0
normal="$(tput sgr0)"
for test in $tests
do
  result="FAIL"
  # Run only the tests that invoke `diff` or `cmp`,
  # because other binaries aren't implemented yet
  if ! grep -E -s -q "(diff3|sdiff)" "$test"
  then
    sh "$test" 1> stdout.txt 2> stderr.txt && result="PASS"
    if [[ $? = 77 ]]
    then
      result="SKIP"
    else
      json+="{\"test\":\"$test\",\"result\":\"$result\","
      json+="\"stdout\":\"$(base64 -w0 < stdout.txt)\","
      json+="\"stderr\":\"$(base64 -w0 < stderr.txt)\","
      json+="\"files\":{"
      cd gt-$test.*
      # Note: this doesn't include the contents of subdirectories,
      # but there isn't much value added in doing so
      for file in *
      do
        # Encode the name with jq: some tests create files whose name contains
        # quotes or control characters, which would produce invalid JSON
        [[ -f "$file" ]] && json+="$(jq -Rn --arg name "$file" '$name'):\"$(base64 -w0 < "$file")\","
      done
      json="${json%,}}},"
      cd - > /dev/null
      [[ "$result" = "PASS" ]] && (( passed++ ))
      [[ "$result" = "FAIL" ]] && (( failed++ ))
    fi
  else
    result="SKIP"
  fi
  color=2 # green
  [[ "$result" = "FAIL" ]] && color=1 # red
  if [[ $result = "SKIP" ]]
  then
    (( skipped++ ))
    json+="{\"test\":\"$test\",\"result\":\"$result\"},"
    color=3 # yellow
  fi
  printf "  %-40s $(tput setaf $color)$result$(tput sgr0)\n" "$test"
done
echo ""
echo -n "Summary: TOTAL: $total / "
echo -n "$(tput setaf 2)PASS$normal: $passed / "
echo -n "$(tput setaf 1)FAIL$normal: $failed / "
echo "$(tput setaf 3)SKIP$normal: $skipped"
echo ""

json="\"tests\":[${json%,}]"
metadata="\"timestamp\":\"$timestamp\","
metadata+="\"revision\":\"$rev\","
metadata+="\"upstream-version\":\"$gnu_version\","
if [[ -n "$GITHUB_ACTIONS" ]]
then
  metadata+="\"branch\":\"$GITHUB_REF\","
fi
json="{$metadata $json}"

# Clean up
cd "$scriptpath"

# Write the results out only once they are known to be valid JSON, so that a
# malformed (or truncated) file is never left behind for the caller to consume
resultsfile="test-results.json"
if ! echo "$json" | jq > "$resultsfile.tmp"
then
  rm -f "$resultsfile.tmp"
  die "Generated invalid JSON results"
fi
mv "$resultsfile.tmp" "$resultsfile"
echo "Results written to $scriptpath/$resultsfile"

(( failed > 0 )) && exit 1
exit 0
