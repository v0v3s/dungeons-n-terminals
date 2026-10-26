#!/usr/bin/env sh
# Make a pull request a release: from its title (a Conventional Commit, see
# next-version.sh), bump the workspace version and name the changelog's
# Unreleased section after it. Run it on the PR's branch and commit the
# result; when the PR is merged, the release workflow tags and publishes
# that version.
#
#   scripts/release-pr.sh "<PR title>"
set -eu

title=${1:?usage: release-pr.sh "<PR title>"}
cd "$(dirname "$0")/.."
version=$(scripts/next-version.sh "$title")
[ -n "$version" ] || { echo "\"$title\" does not release (only feat:, fix:, perf: and breaking changes do)"; exit 0; }
sed -i "/^\[workspace.package\]/,/^\[/s/^version = \".*\"/version = \"$version\"/" Cargo.toml
cargo update --workspace --quiet
scripts/release-notes.sh cut "$version" "$(date -u +%F)"
echo "v$version: Cargo.toml, Cargo.lock and CHANGELOG.md are ready to commit"
