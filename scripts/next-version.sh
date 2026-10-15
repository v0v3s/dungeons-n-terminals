#!/usr/bin/env sh
# Print the next version for a merged pull request, or nothing if it does
# not release. The squash-merge commit carries the PR title (a
# Conventional Commit):
#
#   feat: …            minor   0.3.1 → 0.4.0
#   fix: … / perf: …   patch   0.3.1 → 0.3.2
#   feat!: … (or a "BREAKING CHANGE:" line in the body)
#                      major   0.3.1 → 1.0.0
#   anything else      no release (docs:, chore:, ci:, refactor:, test:, …)
#
#   scripts/next-version.sh "<commit message>" [current version]
#
# The current version defaults to the latest vX.Y.Z tag, else the
# workspace version in Cargo.toml.
set -eu

message=${1:?usage: next-version.sh MESSAGE [VERSION]}
current=${2:-}
if [ -z "$current" ]; then
    current=$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null | sed 's/^v//' || true)
fi
if [ -z "$current" ]; then
    current=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)
fi

subject=$(printf '%s\n' "$message" | head -n 1)
type=$(printf '%s' "$subject" | sed -n 's/^\([a-zA-Z]*\)\(([^)]*)\)\{0,1\}\(!\{0,1\}\): .*/\1/p')
bang=$(printf '%s' "$subject" | sed -n 's/^[a-zA-Z]*\(([^)]*)\)\{0,1\}\(!\{0,1\}\): .*/\2/p')

bump=
if [ "$bang" = "!" ] || printf '%s\n' "$message" | grep -q '^BREAKING[ -]CHANGE:'; then
    bump=major
else
    case "$type" in
        feat) bump=minor ;;
        fix | perf) bump=patch ;;
    esac
fi
[ -n "$bump" ] || exit 0

major=$(echo "$current" | cut -d. -f1)
minor=$(echo "$current" | cut -d. -f2)
patch=$(echo "$current" | cut -d. -f3 | sed 's/[^0-9].*//')
case "$bump" in
    major) echo "$((major + 1)).0.0" ;;
    minor) echo "$major.$((minor + 1)).0" ;;
    patch) echo "$major.$minor.$((patch + 1))" ;;
esac
