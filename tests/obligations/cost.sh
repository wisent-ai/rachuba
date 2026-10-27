#!/bin/sh
# rachuba cost says what payroll cost the company per month and in the year,
# through the real binary: an empty ledger says no runs are recorded, and two
# committed runs in one month are one month holding both runs, whose total is
# the year's total.
#
# Usage: RACHUBA=<path to the built rachuba> sh tests/obligations/cost.sh
# Inputs, cases and the criterion report are tests/support/journey.sh's.

JOURNEY=payroll-cost
. "$(dirname "$0")/../support/journey.sh"

year="$(date -u +%Y)"
first="$year-01-15"
second="$year-01-31"
month="${first%-*}"

dir="$(fresh cost)"
company "$dir"
succeeds "$dir" empty cost --year "$year"
grep -qF "No runs recorded in $year" "$root/empty.txt" || fail "rachuba cost on an empty ledger did not say so: $(cat "$root/empty.txt")"

succeeds "$dir" first run --pay-date "$first" --commit
succeeds "$dir" second run --pay-date "$second" --commit
(cd "$dir" && capture "$root/cost.json" "$binary" --json cost --year "$year")
months="$(jq -c '[.months[].month]' "$root/cost.json")"
[ "$months" = "[\"$month\"]" ] || fail "rachuba cost did not hold both runs in $month: $(cat "$root/cost.json")"
runs="$(jq '.months | first | .runs' "$root/cost.json")"
[ "$runs" = "$(printf '%s\n' "$first" "$second" | grep -c .)" ] || fail "rachuba cost counted $runs runs, not the two committed"
month_total="$(jq -r '.months | first | .total' "$root/cost.json")"
year_total="$(jq -r '.total.total' "$root/cost.json")"
[ "$month_total" = "$year_total" ] || fail "rachuba cost's year total $year_total is not its one month's $month_total"
gross="$(jq -r '.months | first | .gross' "$root/cost.json")"
[ "$month_total" != "$gross" ] || fail "rachuba cost's total $month_total carries no employer tax over the gross $gross"

succeeds "$dir" text cost --year "$year"
grep -qF "$month  " "$root/text.txt" || fail "rachuba cost did not print the month $month: $(cat "$root/text.txt")"

finish "{
    \"month\": $(json "$month"),
    \"monthTotal\": $(json "$month_total"),
    \"yearTotal\": $(json "$year_total")
  }"
