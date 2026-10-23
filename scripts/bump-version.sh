#!/usr/bin/env bash
# Compute (and optionally apply) the next release version.
#
#   scripts/bump-version.sh [--check|--write] [patch|minor|major] [--set X.Y.Z]
#
# The base version is max(Cargo.toml workspace version, newest v* git tag).
# With no tag yet the current manifest version is released as-is, so the very
# first release is whatever the tree already says. `--write` rewrites
# [workspace.package].version (every crate inherits it) and refreshes Cargo.lock.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/Cargo.toml"

mode=check
level=patch
forced=""
while [ $# -gt 0 ]; do
        case "$1" in
                --check) mode=check ;;
                --write) mode=write ;;
                --set)
                        shift
                        forced="${1:?--set needs X.Y.Z}"
                        ;;
                patch | minor | major) level="$1" ;;
                *)
                        echo "usage: $0 [--check|--write] [patch|minor|major] [--set X.Y.Z]" >&2
                        exit 2
                        ;;
        esac
        shift
done

read_version() {
        awk 'BEGIN { sect = 0 }
             /^\[workspace\.package\]/ { sect = 1; next }
             /^\[/ { sect = 0 }
             sect && /^version = "/ { gsub(/[^0-9.]/, ""); print; exit }' "$MANIFEST"
}

current="$(read_version)"
[ -n "$current" ] || { echo "no [workspace.package] version in $MANIFEST" >&2; exit 1; }

latest="$(git -C "$ROOT" tag --list 'v[0-9]*' --sort=-v:refname | head -n1)"
latest="${latest#v}"

if [ -n "$latest" ]; then
        base="$(printf '%s\n%s\n' "$current" "$latest" | sort -V | tail -n1)"
else
        base=""
fi

if [ -n "$forced" ]; then
        next="$forced"
elif [ -z "$base" ]; then
        # Nothing released yet: ship the manifest version untouched.
        next="$current"
else
        major="${base%%.*}"
        rest="${base#*.}"
        minor="${rest%%.*}"
        patch="${rest#*.}"
        patch="${patch%%[-+]*}" # drop any prerelease suffix
        case "$level" in
                major)
                        major=$((major + 1))
                        minor=0
                        patch=0
                        ;;
                minor)
                        minor=$((minor + 1))
                        patch=0
                        ;;
                *)
                        patch=$((patch + 1))
                        ;;
        esac
        next="$major.$minor.$patch"
fi
if git -C "$ROOT" rev-parse -q --verify "refs/tags/v${next}" >/dev/null; then
        echo "v${next} is already tagged; pass a higher level or --set X.Y.Z" >&2
        exit 1
fi

echo "current=$current latest_tag=${latest:-none} next=$next"

if [ "$mode" = check ]; then
        [ -n "${GITHUB_OUTPUT:-}" ] && echo "version=$next" >>"$GITHUB_OUTPUT"
        exit 0
fi

if [ "$next" = "$current" ]; then
        echo "Cargo.toml already at $next; nothing to write"
else
        awk -v v="$next" 'BEGIN { sect = 0 }
             /^\[workspace\.package\]/ { sect = 1; print; next }
             /^\[/ { sect = 0 }
             sect && /^version = "/ { print "version = \"" v "\""; next }
             { print }' "$MANIFEST" >"$MANIFEST.tmp" && mv "$MANIFEST.tmp" "$MANIFEST"
        echo "wrote $MANIFEST (version = \"$next\")"
fi

cargo update --workspace >/dev/null
echo "refreshed Cargo.lock"
[ -n "${GITHUB_OUTPUT:-}" ] && echo "version=$next" >>"$GITHUB_OUTPUT"
exit 0
