#!/bin/sh
# Names the next release: bumps the version every crate and Cargo.lock carry, and gives the
# changelog's "## Unreleased" section that number. Pushing the commit to main releases it: the
# release workflow builds and publishes every version that has no release yet.
#
#   scripts/version.sh small   a small update: 2.1.0 to 2.2.0
#   scripts/version.sh big     a big feature release: 2.1.0 to 3.0.0
#
# Write what changed under "## Unreleased" in CHANGELOG.md first; the script stops without it.
set -eu
cd "$(dirname "$0")/.."

kind="${1:-}"
# a checkout on Windows ends its lines in CR LF: the CR is no part of the number
current="$(tr -d '\r' < crates/omega-zero/Cargo.toml | sed -n 's/^version = "\(.*\)"/\1/p' | head -n 1)"
major="${current%%.*}"
rest="${current#*.}"
minor="${rest%%.*}"
case "$kind" in
small) next="$major.$((minor + 1)).0" ;;
big) next="$((major + 1)).0.0" ;;
*)
	echo "usage: scripts/version.sh small|big   (now $current)" >&2
	exit 2
	;;
esac

grep -q '^## Unreleased' CHANGELOG.md || {
	echo "CHANGELOG.md has no \"## Unreleased\" section to name $next" >&2
	exit 1
}

for toml in crates/*/Cargo.toml; do
	sed "0,/^version = \"$current\"/s//version = \"$next\"/" "$toml" > "$toml.new"
	mv "$toml.new" "$toml"
done
# each of the workspace's own crates in the lock file: its name, then its version
awk -v from="$current" -v to="$next" '
	{ sub(/\r$/, "") }
	own && $0 == "version = \"" from "\"" { print "version = \"" to "\""; own = 0; next }
	{ own = ($0 ~ /^name = "omega-/); print }
' Cargo.lock > Cargo.lock.new
mv Cargo.lock.new Cargo.lock
sed "0,/^## Unreleased/s//## omega-zero $next/" CHANGELOG.md > CHANGELOG.md.new
mv CHANGELOG.md.new CHANGELOG.md

echo "omega-zero $current -> $next: commit and push to main to release it"
