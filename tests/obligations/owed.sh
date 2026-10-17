#!/bin/sh
# rachuba owed reports an employment-tax deposit that is past its due date as
# overdue, through the real binary: a run committed on a pay date whose EFTPS
# deposit date has passed is reported as an ERROR naming that payroll and the
# date it was due, owed fails and --json says overdue; once the deposit is
# recorded the run's taxes are no longer reported as undeposited.
#
# Usage: RACHUBA=<path to the built rachuba> sh tests/obligations/owed.sh
# Inputs, cases and the criterion report are tests/support/journey.sh's.

JOURNEY=owed-overdue
. "$(dirname "$0")/../support/journey.sh"

# A pay date early in the year the binary reads tables for, so its deposit
# date has passed whenever the journey runs later in that year.
year="$(date -u +%Y)"
paid="$year-01-15"
undeposited="from the $paid payroll has not been recorded as deposited"

dir="$(fresh overdue)"
company "$dir"
succeeds "$dir" run run --pay-date "$paid" --commit

overdue_exit=""
if (cd "$dir" && capture "$root/owed.txt" "$binary" owed --year "$year"); then
	fail "rachuba owed succeeded with an overdue deposit: $(cat "$root/owed.txt")"
else
	overdue_exit="$?"
	printf 'rachuba owed: refused with an overdue deposit\n'
fi
reported="$(grep -F "$undeposited" "$root/owed.txt")"
case "$reported" in
ERROR*"was due"*) ;;
*) fail "rachuba owed did not report the $paid deposit as an overdue ERROR with its due date: $(cat "$root/owed.txt")" ;;
esac

if (cd "$dir" && capture "$root/owed.json" "$binary" --json owed --year "$year"); then
	fail "rachuba --json owed succeeded with an overdue deposit"
fi
grep -qF '"overdue": true' "$root/owed.json" || fail "rachuba --json owed did not say overdue: $(cat "$root/owed.json")"

succeeds "$dir" deposited deposited --pay-date "$paid" --on "$paid"
(cd "$dir" && capture "$root/owed-after.txt" "$binary" owed --year "$year")
if grep -qF "$undeposited" "$root/owed-after.txt"; then
	fail "rachuba owed still reports the deposit recorded for $paid: $(cat "$root/owed-after.txt")"
fi

finish "{
    \"payDate\": $(json "$paid"),
    \"overdueReported\": $(json "$reported"),
    \"owedExit\": $overdue_exit,
    \"jsonOverdue\": true,
    \"reportedAfterDeposit\": false
  }"
