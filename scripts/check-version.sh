#!/usr/bin/env sh
# Check a pull request's version against its title (see next-version.sh):
# a release (feat:, fix:, perf:, breaking) carries exactly the next version
# and its changelog section (scripts/release-pr.sh makes both); anything
# else leaves the version as released.
#
#   scripts/check-version.sh "<PR title>"
set -eu

title=${1:?usage: check-version.sh "<PR title>"}
cd "$(dirname "$0")/.."
released=$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null | sed 's/^v//' || true)
version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)
next=$(scripts/next-version.sh "$title" "$released")
if [ -n "$next" ]; then
    [ "$version" = "$next" ] || { echo "\"$title\" releases v$next: run scripts/release-pr.sh \"$title\" and commit (Cargo.toml says $version)" >&2; exit 1; }
    grep -q "^## v$next " CHANGELOG.md || { echo "CHANGELOG.md has no '## v$next' section: run scripts/release-pr.sh" >&2; exit 1; }
    echo "releases v$next"
else
    [ -z "$released" ] || [ "$version" = "$released" ] || { echo "\"$title\" does not release, but Cargo.toml says $version (released: $released)" >&2; exit 1; }
    echo "no release"
fi
