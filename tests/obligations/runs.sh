#!/bin/sh
# rachuba runs lists every recorded run of a year with whether its money
# moved, through the real binary: an empty ledger says no runs are recorded,
# a committed run is listed with its taxes pending, and once its deposit is
# recorded the list and --json carry the day it was made.
#
# Usage: RACHUBA=<path to the built rachuba> sh tests/obligations/runs.sh
# Inputs, cases and the criterion report are tests/support/journey.sh's.

JOURNEY=runs-history
. "$(dirname "$0")/../support/journey.sh"

year="$(date -u +%Y)"
paid="$year-01-15"

dir="$(fresh history)"
company "$dir"
succeeds "$dir" empty runs --year "$year"
grep -qF "No runs recorded in $year" "$root/empty.txt" || fail "rachuba runs on an empty ledger did not say so: $(cat "$root/empty.txt")"

succeeds "$dir" run run --pay-date "$paid" --commit
succeeds "$dir" listed runs --year "$year"
listed="$(grep -F "$paid" "$root/listed.txt")"
case "$listed" in
*"deposited pending"*) ;;
*) fail "rachuba runs did not list the $paid run with its taxes pending: $(cat "$root/listed.txt")" ;;
esac

succeeds "$dir" deposited deposited --pay-date "$paid" --on "$paid"
succeeds "$dir" after runs --year "$year"
grep -qF "deposited $paid" "$root/after.txt" || fail "rachuba runs did not carry the recorded deposit: $(cat "$root/after.txt")"
(cd "$dir" && capture "$root/runs.json" "$binary" --json runs --year "$year")
grep -qF "\"taxes_deposited\": \"$paid\"" "$root/runs.json" || fail "rachuba --json runs did not carry the deposit day: $(cat "$root/runs.json")"
succeeds "$dir" bonuses runs --year "$year" --kind BONUS
grep -qF "No runs of kind BONUS recorded in $year" "$root/bonuses.txt" || fail "rachuba runs --kind BONUS listed a regular run: $(cat "$root/bonuses.txt")"
succeeds "$dir" regular runs --year "$year" --kind Regular
grep -qF "$paid" "$root/regular.txt" || fail "rachuba runs --kind Regular, case ignored, left out the regular run: $(cat "$root/regular.txt")"
succeeds "$dir" pending runs --year "$year" --pending
if grep -qF "deposited $paid" "$root/pending.txt" && ! grep -qF "remitted pending" "$root/pending.txt"; then
	fail "rachuba runs --pending listed a run whose money all moved: $(cat "$root/pending.txt")"
fi

finish "{
    \"payDate\": $(json "$paid"),
    \"listedBeforeDeposit\": $(json "$listed"),
    \"depositCarried\": true
  }"
