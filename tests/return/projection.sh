#!/bin/sh
# rachuba return projects the employee's federal return through the real
# binary: the IRA eligibility the projection computes reaches the report, the
# projection leaves the ledger rachuba init wrote unchanged, and a
# configuration that does not exist is refused by name.
#
# Usage: RACHUBA=<path to the built rachuba> sh tests/return/projection.sh
# Cases run in .build/journey/return-projection of this checkout; the script
# fails when any case fails and names each failure.
#
# A defect register that measures a repair by this journey names the defect
# and its criterion in RACHUBA_TEST_DEFECT and RACHUBA_TEST_CRITERION; a
# passing run over a checkout without uncommitted changes then also writes
# criterion.json beside the cases: the defect, the criterion, the committed
# revision, the result, the digests of the binary and of this script, every
# successful command with its exit, and the final state it observed.

set -u
binary="${RACHUBA:?RACHUBA names the rachuba binary under test}"
checkout="$(cd "$(dirname "$0")/../.." && pwd)"
root="$checkout/.build/journey/return-projection"
# The tax tables the binary reads are the ones this checkout carries.
RACHUBA_TABLES="$checkout/tables"
export RACHUBA_TABLES
asked=false
if [ -n "${RACHUBA_TEST_DEFECT+named}" ] || [ -n "${RACHUBA_TEST_CRITERION+named}" ]; then
	defect="${RACHUBA_TEST_DEFECT:?RACHUBA_TEST_DEFECT names the defect the criterion report measures, together with RACHUBA_TEST_CRITERION}"
	criterion="${RACHUBA_TEST_CRITERION:?RACHUBA_TEST_CRITERION names the criterion the report measures, together with RACHUBA_TEST_DEFECT}"
	asked=true
fi
failures=""
commands=""

# capture FILE COMMAND... runs COMMAND with its output and errors in FILE.
capture() {
	out="$1"
	shift
	# https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html#tag_18_07_06
	"$@" >"$out" 2>&1
}

# refuse MESSAGE prints MESSAGE on standard error and fails.
refuse() {
	# https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html#tag_18_07_06
	printf '%s\n' "$1" >&2
	false
}

fail() {
	failures="$failures
$1"
	printf 'FAIL %s\n' "$1"
}

json() {
	printf '"%s"' "$(printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g')"
}

# digest FILE prints FILE's SHA-256, the digest Oko's criterion report names.
digest() {
	# https://github.com/wisent-ai/oko/blob/main/rust/src/defects/probe/criterion.rs
	shasum -a 256 "$1" | { read -r sum rest && printf '%s' "$sum"; }
}

# record EXIT ARG... keeps one successful invocation of the binary for the
# criterion report.
record() {
	code="$1"
	shift
	args=""
	for arg in "$@"; do
		args="$args${args:+, }$(json "$arg")"
	done
	commands="$commands${commands:+, }{\"executable\": $(json "$binary"), \"args\": [$args], \"exit\": $code}"
}

fresh() {
	rm -rf "$root/$1"
	mkdir -p "$root/$1"
	printf '%s\n' "$root/$1"
}

dir="$(fresh fresh)"
if (cd "$dir" && capture "$root/init.txt" "$binary" init); then
	record "$?" init
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

if (cd "$dir" && capture "$root/return.txt" "$binary" return); then
	record "$?" return
	printf 'rachuba return: succeeded\n'
else
	fail "rachuba return failed: $(cat "$root/return.txt")"
fi
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
if (cd "$dir" && capture "$root/absent.txt" "$binary" return --config absent.toml); then
	fail "rachuba return succeeded without a configuration"
else
	refused="$?"
	printf 'rachuba return --config absent.toml: refused\n'
fi
grep -qF "absent.toml" "$root/absent.txt" || fail "the refusal does not name absent.toml: $(cat "$root/absent.txt")"

if [ -n "$failures" ]; then
	refuse "return projection: failed$failures"
	exit
fi
printf 'return projection: passed\n'
if ! $asked; then
	exit
fi

git -C "$checkout" diff --quiet HEAD -- ||
	{ refuse "a criterion report binds committed source, and $checkout has uncommitted changes"; exit; }
revision="$(git -C "$checkout" rev-parse HEAD)" || exit
binary_sha="$(digest "$binary")" || exit
test_sha="$(digest "$0")" || exit
cat >"$root/criterion.json" <<EOF || exit
{
  "defectId": $(json "$defect"),
  "criterion": $(json "$criterion"),
  "sourceRevision": $(json "$revision"),
  "result": "passed",
  "binarySha256": $(json "$binary_sha"),
  "testSha256": $(json "$test_sha"),
  "observedFinalState": {
    "returnPrinted": [$printed],
    "ledgerUnchanged": true,
    "absentConfigurationExit": $refused,
    "absentConfigurationNamed": true
  },
  "commands": [$commands]
}
EOF
printf 'criterion report: %s\n' "$root/criterion.json"
