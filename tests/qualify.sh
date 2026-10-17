#!/bin/sh
# Qualifies one committed revision of rachuba by its journeys, for a defect
# register's revision-bound completion probe:
#
#   sh tests/qualify.sh <revision> <journey>...
#
# where a journey is a path below tests/ without .sh (return/projection,
# obligations/owed, cli/json). Stado builds the revision from its own export
# (stado product cargo build --revision), and each journey runs from that
# export against the binary built there, so neither another session's
# uncommitted edit nor a later test reads in. Standard output lists the
# journeys that failed, one per line, and is empty when every one passed;
# the build's and the journeys' own output goes to standard error. A
# revision Stado cannot build ends the run as failed.

set -u
revision="${1:?usage: sh tests/qualify.sh <revision> <journey>...}"
: "${2:?usage: sh tests/qualify.sh <revision> <journey>...}"
shift
cd "$(dirname "$0")/.." || exit

# refuse MESSAGE prints MESSAGE on standard error and fails.
refuse() {
	# https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html#tag_18_07_06
	printf '%s\n' "$1" >&2
	false
}

evidence="$(stado product cargo build --revision "$revision" --json)" ||
	{ refuse "stado product cargo build --revision $revision failed"; exit; }
manifest="$(printf '%s\n' "$evidence" | sed -n 's/^  "manifest_path": "\(.*\)",$/\1/p')"
[ -n "$manifest" ] || { refuse "stado named no manifest for $revision: $evidence"; exit; }
exported="$(dirname "$manifest")"
binary="$exported/target/debug/rachuba"
[ -x "$binary" ] || { refuse "stado's build of $revision left no rachuba at $binary"; exit; }

for journey in "$@"; do
	script="$exported/tests/$journey.sh"
	if [ ! -f "$script" ]; then
		printf '%s: no such journey at %s\n' "$journey" "$revision"
		continue
	fi
	RACHUBA="$binary" sh "$script" >&2 || printf '%s: failed\n' "$journey"
done
