#!/bin/sh
# Every rachuba command that answers a question answers it as JSON under the
# global --json flag and as prose without it, through the real binary: after
# one committed run, run, explain, stub, ytd, owed, calendar, check, form w2
# and return each print a document python3's json.tool reads, and ytd without
# --json prints prose that json.tool refuses.
#
# Usage: RACHUBA=<path to the built rachuba> sh tests/cli/json.sh
# Inputs, cases and the criterion report are tests/support/journey.sh's.

JOURNEY=cli-json
. "$(dirname "$0")/../support/journey.sh"

year="$(date -u +%Y)"
paid="$year-01-15"

# document NAME says whether NAME.txt is one JSON document.
document() {
	python3 -m json.tool "$root/$1.txt" >/dev/null 2>&1
}

dir="$(fresh json)"
company "$dir"
succeeds "$dir" commit run --pay-date "$paid" --commit

answered=""
for case in "run:run --pay-date $paid" "explain:explain --pay-date $paid" "stub:stub --pay-date $paid" \
	"ytd:ytd --year $year" "calendar:calendar --year $year" "check:check --year $year" \
	"w2:form w2 --year $year" "return:return --year $year"; do
	name="${case%%:*}"
	# shellcheck disable=SC2086 # the case's arguments are words
	succeeds "$dir" "$name" --json ${case#*:}
	if document "$name"; then
		answered="$answered${answered:+, }$(json "$name")"
	else
		fail "rachuba --json ${case#*:} printed no JSON: $(cat "$root/$name.txt")"
	fi
done

# The run is overdue, so owed fails naming that on standard error; its
# answer on standard output is still the JSON document.
(cd "$dir" && "$binary" --json owed --year "$year" >"$root/owed.txt" 2>"$root/owed-refusal.txt")
if document owed; then
	answered="$answered, $(json owed)"
else
	fail "rachuba --json owed printed no JSON: $(cat "$root/owed.txt")"
fi

succeeds "$dir" prose ytd --year "$year"
if document prose; then
	fail "rachuba ytd without --json printed JSON: $(cat "$root/prose.txt")"
fi

finish "{
    \"answeredJson\": [$answered],
    \"proseWithoutJson\": true
  }"
