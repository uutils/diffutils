#!/bin/bash

# Print the test results written to a JSON file
# by run-upstream-testsuite.sh in a markdown format.

json="test-results.json"
[[ -n $1 ]] && json="$1"

codeblock () { echo -e "\`\`\`\n$1\n\`\`\`"; }

jq -c '.tests[]' "$json" | while read -r test
do
  name=$(echo "$test" | jq -r '.test')
  echo "# test: $name"
  result=$(echo "$test" | jq -r '.result')
  echo "result: $result"
  url=$(echo "$test" | jq -r '.url')
  echo "url: $url"
  if [[ "$result" != "SKIP" ]]
  then
    stdout=$(echo "$test" | jq -r '.stdout' | base64 -d)
    if [[ -n "$stdout" ]]
    then
      echo "## stdout"
      codeblock "$stdout"
    fi
    stderr=$(echo "$test" | jq -r '.stderr' | base64 -d)
    if [[ -n "$stderr" ]]
    then
      echo "## stderr"
      codeblock "$stderr"
    fi
  fi
  echo ""
done
