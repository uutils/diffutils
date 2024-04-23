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

# By default it expects a release build of the diffutils binary, but a
# different build profile can be specified as an argument
# (e.g. 'dev' or 'test').
# Unless overridden by the $TESTS environment variable, all tests in the test
# suite will be run. Tests targeting a command that is not yet implemented
# (e.g. cmp, diff3 or sdiff) are skipped.

scriptpath=$(dirname "$(readlink -f "$0")")
rev=$(git rev-parse HEAD)

# Allow passing a specific profile as parameter (default to "release")
profile="release"
[[ -n $1 ]] && profile="$1"

# Verify that the diffutils binary was built for the requested profile
binary="$scriptpath/../target/$profile/diffutils"
if [[ ! -x "$binary" ]]
then
  echo "Missing build for profile $profile"
  exit 1
fi

# Work in a temporary directory
tempdir=$(mktemp -d)
cd "$tempdir"

# Check out the upstream test suite
gitserver="https://git.savannah.gnu.org"
testsuite="$gitserver/git/diffutils.git"
echo "Fetching upstream test suite from $testsuite"
git clone -n --depth=1 --filter=tree:0 "$testsuite" &> /dev/null
cd diffutils
git sparse-checkout set --no-cone tests &> /dev/null
git checkout &> /dev/null
upstreamrev=$(git rev-parse HEAD)

# Ensure that calling `diff` invokes the built `diffutils` binary instead of
# the upstream `diff` binary that is most likely installed on the system
mkdir src
cd src
ln -s "$binary" diff
cd ../tests

if [[ -n "$TESTS" ]]
then
  tests="$TESTS"
else
  # Get a list of all upstream tests (default if $TESTS isn't set)
  echo -e '\n\nprinttests:\n\t@echo "${TESTS}"' >> Makefile.am
  tests=$(make -f Makefile.am printtests)
fi
total=$(echo "$tests" | wc -w)
echo "Running $total tests"
export LC_ALL=C
export KEEP=yes
exitcode=0
timestamp=$(date -Iseconds)
urlroot="$gitserver/cgit/diffutils.git/tree/tests/"
passed=0
failed=0
skipped=0
normal="$(tput sgr0)"
for test in $tests
do
  result="FAIL"
  url="$urlroot$test?id=$upstreamrev"
  # Run only the tests that invoke `diff`,
  # because other binaries aren't implemented yet
  if ! grep -E -s -q "(cmp|diff3|sdiff)" "$test"
  then
    sh "$test" 1> stdout.txt 2> stderr.txt && result="PASS" || exitcode=1
    json+="{\"test\":\"$test\",\"result\":\"$result\","
    json+="\"url\":\"$url\","
    json+="\"stdout\":\"$(base64 -w0 < stdout.txt)\","
    json+="\"stderr\":\"$(base64 -w0 < stderr.txt)\","
    json+="\"files\":{"
    cd gt-$test.*
    # Note: this doesn't include the contents of subdirectories,
    # but there isn't much value added in doing so
    for file in *
    do
      [[ -f "$file" ]] && json+="\"$file\":\"$(base64 -w0 < "$file")\","
    done
    json="${json%,}}},"
    cd - > /dev/null
    [[ "$result" = "PASS" ]] && (( passed++ ))
    [[ "$result" = "FAIL" ]] && (( failed++ ))
  else
    result="SKIP"
    (( skipped++ ))
    json+="{\"test\":\"$test\",\"url\":\"$url\",\"result\":\"$result\"},"
  fi
  color=2 # green
  [[ "$result" = "FAIL" ]] && color=1 # red
  [[ "$result" = "SKIP" ]] && color=3 # yellow
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
metadata+="\"upstream-revision\":\"$upstreamrev\","
if [[ -n "$GITHUB_ACTIONS" ]]
then
  metadata+="\"branch\":\"$GITHUB_REF\","
fi
json="{$metadata $json}"

# Clean up
cd "$scriptpath"
rm -rf "$tempdir"

resultsfile="test-results.json"
echo "$json" | jq > "$resultsfile"
echo "Results written to $scriptpath/$resultsfile"

exit $exitcode
