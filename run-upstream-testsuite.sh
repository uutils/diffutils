#!/bin/bash

scriptpath=$(dirname "$(readlink -f "$0")")

# Allow passing a specific profile as parameter (default to "release")
profile="release"
[[ -n $1 ]] && profile="$1"

# Verify that the diffutils binary was built for the requested profile
binary="$scriptpath/target/$profile/diffutils"
if [[ ! -x "$binary" ]]
then
  echo "Missing build for profile $profile"
  exit 1
fi

# Work in a temporary directory
tempdir=$(mktemp -d)
cd "$tempdir"

# Check out the upstream test suite
testsuite="https://git.savannah.gnu.org/git/diffutils.git"
echo "Fetching upstream test suite from $testsuite"
git clone -n --depth=1 --filter=tree:0 "$testsuite" &> /dev/null
cd diffutils
git sparse-checkout set --no-cone tests &> /dev/null
git checkout &> /dev/null

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
echo "Running $(echo "$tests" | wc -w) tests"
export LC_ALL=C
export KEEP=yes
exitcode=0
json=""
for test in $tests
do
  result="FAIL"
  # Run only the tests that invoke `diff`, because other binaries aren't implemented yet
  if ! grep -E -s -q "(cmp|diff3|sdiff)" "$test"
  then
    sh "$test" 1> stdout.txt 2> stderr.txt && result="PASS" || exitcode=1
    json+="{\"test\":\"$test\",\"result\":\"$result\","
    json+="\"stdout\":\"$(base64 -w0 < stdout.txt)\","
    json+="\"stderr\":\"$(base64 -w0 < stderr.txt)\","
    json+="\"files\":{"
    cd gt-$test.*
    for file in *
    do
      [[ -f "$file" ]] && json+="\"$file\":\"$(base64 -w0 < "$file")\","
    done
    json="${json%,}}},"
    cd - > /dev/null
  else
    result="SKIP"
    json+="{\"test\":\"$test\",\"result\":\"$result\"},"
  fi
  color=2 # green
  [[ "$result" = "FAIL" ]] && color=1 # red
  [[ "$result" = "SKIP" ]] && color=3 # yellow
  printf "  %-40s $(tput setaf $color)$result$(tput sgr0)\n" "$test"
done
json="[${json%,}]"

# Clean up
cd "$scriptpath"
rm -rf "$tempdir"

resultsfile="test-results.json"
echo "$json" | jq > "$resultsfile"
echo "Results written to $resultsfile"

exit $exitcode
