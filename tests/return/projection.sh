#!/bin/sh
# rachuba return projects the employee's federal return through the real
# binary: the IRA eligibility the projection computes reaches the report, the
# projection leaves the ledger rachuba init wrote unchanged, and a
# configuration that does not exist is refused by name.
#
# Usage: RACHUBA=<path to the built rachuba> sh tests/return/projection.sh
# Inputs, cases and the criterion report are tests/support/journey.sh's.

JOURNEY=return-projection
. "$(dirname "$0")/../support/journey.sh"

dir="$(fresh fresh)"
company "$dir"
cp "$dir/ledger.toml" "$root/ledger-before.toml" ||
	fail "the journey could not keep the ledger rachuba init wrote"

succeeds "$dir" return return
printed=""
for line in "Modified AGI used for IRA phase-outs" "IRA, employee" "Roth IRA contribution"; do
	if grep -qF "$line" "$root/return.txt"; then
		printed="$printed${printed:+, }$(json "$line")"
	else
		fail "rachuba return printed no '$line'"
	fi
done
cmp -s "$root/ledger-before.toml" "$dir/ledger.toml" || fail "rachuba return changed the ledger"

dir="$(fresh absent)"
refused=""
if (cd "$dir" && capture "$root/absent.txt" "$binary" return --config absent.toml); then
	fail "rachuba return succeeded without a configuration"
else
	refused="$?"
	printf 'rachuba return --config absent.toml: refused\n'
fi
grep -qF "absent.toml" "$root/absent.txt" || fail "the refusal does not name absent.toml: $(cat "$root/absent.txt")"

finish "{
    \"returnPrinted\": [$printed],
    \"ledgerUnchanged\": true,
    \"absentConfigurationExit\": $refused,
    \"absentConfigurationNamed\": true
  }"
