#!/usr/bin/env sh
# Android's version number (versionCode) for a version: major, minor and
# patch as one number, 1.6.0 as 1006000. It must grow with every release.
#
#   scripts/version-code.sh 1.6.0
set -eu

echo "${1:?usage: version-code.sh VERSION}" | awk -F. '{ printf "%d\n", $1 * 1000000 + $2 * 1000 + $3 }'
