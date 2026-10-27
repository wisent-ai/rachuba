#!/bin/sh
# rachuba sheet writes every recorded run of a year as one CSV sheet, through
# the real binary: an empty ledger writes nothing, a committed run is one row
# under a header naming the fields the ledger records, its amounts the decimal
# strings the ledger keeps, and its deposit day appears once recorded.
#
# Usage: RACHUBA=<path to the built rachuba> sh tests/obligations/sheet.sh
# Inputs, cases and the criterion report are tests/support/journey.sh's.

JOURNEY=runs-sheet
. "$(dirname "$0")/../support/journey.sh"

year="$(date -u +%Y)"
paid="$year-01-15"
one="$(printf '%s\n' "$paid" | grep -c .)"

dir="$(fresh sheet)"
company "$dir"
succeeds "$dir" empty sheet --year "$year"
[ -s "$root/empty.txt" ] && fail "rachuba sheet on an empty ledger wrote something: $(cat "$root/empty.txt")"

succeeds "$dir" run run --pay-date "$paid" --commit
succeeds "$dir" before sheet --year "$year"
read -r header < "$root/before.txt"
for field in pay_date kind gross net_pay federal_income_tax taxes_deposited; do
	case "$header" in
	*"\"$field\""*) ;;
	*) fail "rachuba sheet's header does not name the ledger field $field: $header" ;;
	esac
done
# The header names fields, never a date: every line holding the pay date is a row.
row="$(grep -F "\"$paid\"" "$root/before.txt")" || fail "rachuba sheet did not write the $paid run as a row: $(cat "$root/before.txt")"
[ "$(grep -cF "\"$paid\"" "$root/before.txt")" = "$one" ] \
	|| fail "rachuba sheet wrote other than one row for one run: $(cat "$root/before.txt")"

(cd "$dir" && capture "$root/runs.json" "$binary" --json runs --year "$year")
net="$(sed -n 's/.*"net_pay": "\([^"]*\)".*/\1/p' "$root/runs.json")"
case "$row" in
*"\"$net\""*) ;;
*) fail "rachuba sheet did not carry the net pay $net the ledger records: $row" ;;
esac

succeeds "$dir" deposited deposited --pay-date "$paid" --on "$paid"
succeeds "$dir" after sheet --year "$year"
# Paid and deposited the same day: the row now holds that day twice.
deposits="$(grep -F "\"$paid\"" "$root/after.txt" | grep -o "\"$paid\"" | grep -c .)"
[ "$deposits" -gt "$one" ] || fail "rachuba sheet did not carry the recorded deposit day: $(cat "$root/after.txt")"

finish "{
    \"payDate\": $(json "$paid"),
    \"header\": $(json "$header"),
    \"netPay\": $(json "$net"),
    \"depositCarried\": true
  }"
