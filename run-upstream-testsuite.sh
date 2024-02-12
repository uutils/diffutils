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
pass="$(tput setaf 2)PASS$(tput sgr0)"
fail="$(tput setaf 1)FAIL$(tput sgr0)"
skip=SKIP
exitcode=0
for test in $tests
do
  result=$fail
  # Run only the tests that invoke `diff`, because other binaries aren't implemented yet
  if ! grep -E -s -q "(cmp|diff3|sdiff)" "$test"
  then
    sh "$test" &> /dev/null && result=$pass || exitcode=1
  else
    result=$skip
  fi
  printf "  %-40s $result\n" "$test"
done

# Clean up
cd "$scriptpath"
rm -rf "$tempdir"

exit $exitcode
