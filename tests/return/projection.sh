#!/bin/sh
# rachuba return projects the employee's federal return through the real
# binary: the IRA eligibility the projection computes reaches the report, the
# projection leaves the ledger rachuba init wrote unchanged, and a
# configuration that does not exist is refused by name.
#
# Usage: RACHUBA=<path to the built rachuba> sh tests/return/projection.sh
# Cases run in .build/journey/return-projection of this checkout; the script
# fails when any case fails and names each failure.

set -u
binary="${RACHUBA:?RACHUBA names the rachuba binary under test}"
checkout="$(cd "$(dirname "$0")/../.." && pwd)"
root="$checkout/.build/journey/return-projection"
# The tax tables the binary reads are the ones this checkout carries.
RACHUBA_TABLES="$checkout/tables"
export RACHUBA_TABLES
failures=""

fail() {
	failures="$failures
$1"
	printf 'FAIL %s\n' "$1"
}

fresh() {
	rm -rf "$root/$1"
	mkdir -p "$root/$1"
	printf '%s\n' "$root/$1"
}

dir="$(fresh fresh)"
if (cd "$dir" && "$binary" init) >"$root/init.txt" 2>&1; then
	printf 'rachuba init: succeeded\n'
else
	fail "rachuba init failed: $(cat "$root/init.txt")"
fi
[ -f "$dir/rachuba.toml" ] || fail "rachuba init wrote no rachuba.toml"
[ -f "$dir/ledger.toml" ] || fail "rachuba init wrote no ledger.toml"
# The template's EIN and SSN digits are placeholders rachuba refuses until the
# operator enters real ones; the journey enters the EIN shape rachuba's own
# refusal shows and SSN digits other than the placeholder.
sed -e 's/^ein = "00-0000000"$/ein = "12-3456789"/' \
	-e 's/^ssn_last4 = "0000"$/ssn_last4 = "1234"/' \
	"$dir/rachuba.toml" >"$dir/edited.toml" && mv "$dir/edited.toml" "$dir/rachuba.toml" ||
	fail "the journey could not enter the EIN and SSN digits in rachuba.toml"
cp "$dir/ledger.toml" "$root/ledger-before.toml" ||
	fail "the journey could not keep the ledger rachuba init wrote"

if (cd "$dir" && "$binary" return) >"$root/return.txt" 2>&1; then
	printf 'rachuba return: succeeded\n'
else
	fail "rachuba return failed: $(cat "$root/return.txt")"
fi
for line in "Modified AGI used for IRA phase-outs" "IRA, employee" "Roth IRA contribution"; do
	grep -qF "$line" "$root/return.txt" || fail "rachuba return printed no '$line'"
done
cmp -s "$root/ledger-before.toml" "$dir/ledger.toml" || fail "rachuba return changed the ledger"

dir="$(fresh absent)"
if (cd "$dir" && "$binary" return --config absent.toml) >"$root/absent.txt" 2>&1; then
	fail "rachuba return succeeded without a configuration"
else
	printf 'rachuba return --config absent.toml: refused\n'
fi
grep -qF "absent.toml" "$root/absent.txt" || fail "the refusal does not name absent.toml: $(cat "$root/absent.txt")"

if [ -n "$failures" ]; then
	printf 'return projection: failed%s\n' "$failures"
	false
else
	printf 'return projection: passed\n'
fi
