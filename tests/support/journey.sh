# What every rachuba journey shares; a journey sources this file after
# naming itself in JOURNEY.
#
# RACHUBA names the rachuba binary under test. Cases run in
# .build/journey/$JOURNEY of this checkout, against the tax tables this
# checkout carries; the journey fails when any case fails and names each
# failure.
#
# A defect register that measures a repair by a journey names the defect and
# its criterion in RACHUBA_TEST_DEFECT and RACHUBA_TEST_CRITERION; a passing
# run over a checkout without uncommitted changes then also writes
# criterion.json beside the cases: the defect, the criterion, the committed
# revision, the result, the digests of the binary and of the journey, every
# successful command with its exit, and the final state it observed.

set -u
binary="${RACHUBA:?RACHUBA names the rachuba binary under test}"
checkout="$(cd "$(dirname "$0")/../.." && pwd)"
root="$checkout/.build/journey/${JOURNEY:?JOURNEY names the journey sourcing tests/support/journey.sh}"
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

# succeeds DIR NAME ARG... runs the binary in DIR with ARGs, its output in
# NAME.txt, and records it; a failure is a failed case.
succeeds() {
	in="$1"
	shift
	name="$1"
	shift
	if (cd "$in" && capture "$root/$name.txt" "$binary" "$@"); then
		record "$?" "$@"
		printf 'rachuba %s: succeeded\n' "$*"
	else
		fail "rachuba $* failed: $(cat "$root/$name.txt")"
	fi
}

# company DIR runs rachuba init in DIR and enters the EIN and SSN digits: the
# template's are placeholders rachuba refuses until the operator enters real
# ones, so the journey enters the EIN shape rachuba's own refusal shows and
# SSN digits other than the placeholder.
company() {
	succeeds "$1" init init
	[ -f "$1/rachuba.toml" ] || fail "rachuba init wrote no rachuba.toml"
	[ -f "$1/ledger.toml" ] || fail "rachuba init wrote no ledger.toml"
	sed -e 's/^ein = "00-0000000"$/ein = "12-3456789"/' \
		-e 's/^ssn_last4 = "0000"$/ssn_last4 = "1234"/' \
		"$1/rachuba.toml" >"$1/edited.toml" && mv "$1/edited.toml" "$1/rachuba.toml" ||
		fail "the journey could not enter the EIN and SSN digits in rachuba.toml"
}

# finish OBSERVED ends the journey: it fails naming every failed case, and on
# a pass writes the criterion report when one was asked for, with OBSERVED,
# a JSON object, as the final state the journey observed.
finish() {
	if [ -n "$failures" ]; then
		refuse "$JOURNEY: failed$failures"
		exit
	fi
	printf '%s: passed\n' "$JOURNEY"
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
  "observedFinalState": $1,
  "commands": [$commands]
}
EOF
	printf 'criterion report: %s\n' "$root/criterion.json"
}
