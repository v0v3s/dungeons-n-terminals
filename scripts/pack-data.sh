#!/usr/bin/env sh
# Pack the data the app reads into one zip, for Data › Import a zip…
#
#   scripts/pack-data.sh [--data DIR] [--homebrew DIR] [-o OUT.zip]
#
#   --data DIR      a data repository (with data/books.json) or data folder
#   --homebrew DIR  a homebrew collection: homebrew files in any layout
#   -o OUT.zip      where to write it (default: dnt-data.zip)
#
# At least one of --data and --homebrew. Only what dnt reads is packed:
# the data files and their indexes, and homebrew files with classes,
# spells, items and the like, each kept to those entities. Images,
# creatures, adventure text and tooling folders are left out.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
out=dnt-data.zip
set -- "$@" --end
while [ "$1" != --end ]; do
  case "$1" in
    --data | --homebrew)
      [ $# -ge 3 ] || { echo "$1 needs a folder" >&2; exit 2; }
      [ -d "$2" ] || { echo "$2: not a folder" >&2; exit 2; }
      dir=$(cd "$2" && pwd)
      set -- "$@" "$1" "$dir"
      shift 2
      ;;
    -o)
      [ $# -ge 3 ] || { echo "-o needs a file name" >&2; exit 2; }
      out=$2
      shift 2
      ;;
    -h | --help)
      sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "unknown argument: $1 (see --help)" >&2
      exit 2
      ;;
  esac
done
shift # --end; the folders follow
[ $# -gt 0 ] || { echo "give --data and/or --homebrew (see --help)" >&2; exit 2; }
case "$out" in /*) ;; *) out="$(pwd)/$out" ;; esac

cargo run --quiet --release --manifest-path "$here/Cargo.toml" -p dnt-core --example pack -- "$@" "$out"
