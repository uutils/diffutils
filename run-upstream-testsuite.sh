#!/bin/bash

scriptpath=$(dirname "$(readlink -f "$0")")

# Work in a temporary directory
tempdir=$(mktemp -d)
cd "$tempdir"

# Check out the upstream test suite
git clone -n --depth=1 --filter=tree:0 https://git.savannah.gnu.org/git/diffutils.git
cd diffutils
git sparse-checkout set --no-cone tests
git checkout

# Ensure that calling `diff` invokes the built binary instead of
# the upstream binary that is most likely installed on the system
mkdir src
cd src
ln -s "$scriptpath/target/release/diffutils" diff # XXX: this assumes a release build
cd ../tests

# Get a list of all upstream tests and run only those that invoke `diff`
echo -e '\n\nprinttests:\n\t@echo "${TESTS}"' >> Makefile.am
tests=$(make -f Makefile.am printtests)
export LC_ALL=C
pass="$(tput setaf 2)PASS$(tput sgr0)"
fail="$(tput setaf 1)FAIL$(tput sgr0)"
exitcode=0
for test in $tests
do
  if ! grep -E -s -q "(cmp|diff3|sdiff)" "$test"
  then
    result=$fail
    sh "$test" &> /dev/null && result=$pass || exitcode=1
    printf "%-40s $result\n" "$test"
  fi
done

# Clean up
cd "$scriptpath"
rm -rf "$tempdir"

exit $exitcode
