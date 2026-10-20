#!/usr/bin/env sh
# The changelog's part in a release. CHANGELOG.md starts with an
# "## Unreleased" section that pull requests fill in; a release names it:
#
#   scripts/release-notes.sh cut VERSION DATE
#       Rename "## Unreleased" to "## vVERSION - DATE" and start a new,
#       empty Unreleased section above it. If nothing was written there,
#       the commit subjects since the last release are used instead.
#
#   scripts/release-notes.sh notes VERSION
#       Print the notes of vVERSION (for the release page), with the
#       downloads.
#
#   scripts/release-notes.sh check
#       Fail if the changelog has no Unreleased section, or a version
#       twice (as a merge gone wrong leaves it).
set -eu

file=CHANGELOG.md
cmd=${1:?usage: release-notes.sh cut VERSION DATE | notes VERSION | check}
if [ "$cmd" = check ]; then
    grep -q '^## Unreleased$' "$file" || { echo "$file has no '## Unreleased' section" >&2; exit 1; }
    twice=$(grep '^## v' "$file" | cut -d' ' -f2 | sort | uniq -d)
    [ -z "$twice" ] || { echo "$file has these versions twice: $twice" >&2; exit 1; }
    exit 0
fi
version=${2:?version}

# The body of a section: from its heading to the next "## " heading.
section() {
    awk -v head="$1" '
        index($0, head) == 1 { on = 1; next }
        on && /^## / { exit }
        on { print }
    ' "$file" | sed -e '/./,$!d' | sed -e ':a' -e '/^\n*$/{$d;N;ba' -e '}'
}

case "$cmd" in
cut)
    date=${3:?date}
    body=$(section "## Unreleased")
    if [ -z "$body" ]; then
        last=$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null || true)
        body=$(git log --format='- %s' ${last:+"$last"..}HEAD | grep -v '^- chore(release)' |
            sed 's/^- [a-z]*\(([^)]*)\)\{0,1\}!\{0,1\}: /- /' || true)
    fi
    tmp=$(mktemp)
    BODY=$body awk -v version="$version" -v date="$date" '
        !done && /^## Unreleased/ {
            print "## Unreleased"
            print ""
            print "## v" version " - " date
            print ""
            print ENVIRON["BODY"]
            print ""
            skip = 1
            done = 1
            next
        }
        skip && /^## / { skip = 0 }
        !skip { print }
    ' "$file" >"$tmp"
    mv "$tmp" "$file"
    ;;
notes)
    body=$(section "## v$version ")
    [ -n "$body" ] || { echo "no section for v$version in $file" >&2; exit 1; }
    printf '%s\n' "$body"
    cat <<'EOF'

### Downloads

| File | For |
| --- | --- |
| `dnt-app-windows-x86_64-setup.exe` | the app on Windows (installer) |
| `dnt-app-android.apk` | the app on Android phones and tablets |
| `dnt-linux-x86_64`, `dnt-windows-x86_64.exe` | dnt in the terminal |

The app reads data you import as a zip; `scripts/pack-data.sh` makes one with just what it needs.
EOF
    ;;
*)
    echo "unknown command: $cmd" >&2
    exit 2
    ;;
esac
