#!/bin/sh

# Author: Raul Correia
# Purpose: prepare a local release commit and tag without pushing.

set -eu

usage() {
  cat <<'EOF'
Prepare a local release commit and tag.

Author:
  Raul Correia

Usage:
  scripts/release.sh <version>
  scripts/release.sh --help

Actions:
  - update Cargo.toml
  - refresh Cargo.lock
  - prepend CHANGELOG.md
  - create a release commit
  - create an annotated tag

The script does not push. It prints the final push commands instead.
EOF
}

die() {
  printf '%s\n' "$1" >&2
  exit 1
}

require_clean_worktree() {
  git diff --quiet || die "Working tree is not clean. Commit or stash your changes first."
  git diff --cached --quiet || die "Index is not clean. Commit or unstage your changes first."
}

read_package_version() {
  sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1
}

validate_version() {
  version="$1"

  case "$version" in
    *[!0-9A-Za-z.-]* | '' | .* | *..* | *.-* | *-. | *.)
      return 1
      ;;
    *)
      ;;
  esac

  old_ifs=$IFS
  IFS=.
  set -- $version
  IFS=$old_ifs

  [ "$#" -ge 3 ] || return 1

  for part in "$1" "$2" "$3"; do
    case "$part" in
      '' | *[!0-9]*)
        return 1
        ;;
    esac
  done
}

build_release_notes() {
  previous_tag="$(git describe --tags --abbrev=0 2>/dev/null || true)"

  if ! git rev-parse --verify HEAD >/dev/null 2>&1; then
    return 0
  fi

  if [ -n "$previous_tag" ]; then
    git log --reverse --format='- %s' "$previous_tag..HEAD"
  else
    git log --reverse --format='- %s' HEAD
  fi
}

update_cargo_version() {
  version="$1"
  current_version="$2"
  tmp_file="$(mktemp "${TMPDIR:-/tmp}/d2rlauncher-cargo.XXXXXX")"

  sed "s/^version = \"$current_version\"$/version = \"$version\"/" Cargo.toml > "$tmp_file"
  mv "$tmp_file" Cargo.toml
}

update_changelog() {
  version="$1"
  today="$2"
  notes="$3"
  tmp_file="$(mktemp "${TMPDIR:-/tmp}/d2rlauncher-changelog.XXXXXX")"

  {
    printf '# Changelog\n\n'
    printf 'All notable changes to this project will be documented in this file.\n\n'
    printf '## [%s] - %s\n\n' "$version" "$today"
    printf '%s\n\n' "$notes"

    if [ -f CHANGELOG.md ]; then
      tail -n +4 CHANGELOG.md
    fi
  } > "$tmp_file"

  mv "$tmp_file" CHANGELOG.md
}

main() {
  case "${1:-}" in
    -h|--help)
      usage
      exit 0
      ;;
  esac

  [ "$#" -eq 1 ] || {
    usage >&2
    exit 1
  }

  version="$1"
  tag="v$version"
  today="$(date +%Y-%m-%d)"

  validate_version "$version" || die "Invalid version: $version"
  require_clean_worktree

  git rev-parse "$tag" >/dev/null 2>&1 && die "Tag already exists: $tag"

  current_version="$(read_package_version)"
  [ -n "$current_version" ] || die "Failed to read the current version from Cargo.toml."
  [ "$current_version" != "$version" ] || die "Cargo.toml is already set to version $version."

  release_notes="$(build_release_notes)"
  [ -n "$release_notes" ] || release_notes="- No user-facing changes."

  update_cargo_version "$version" "$current_version"
  cargo generate-lockfile --offline
  update_changelog "$version" "$today" "$release_notes"

  git add Cargo.toml Cargo.lock CHANGELOG.md
  git commit -m "chore(release): $tag"
  git tag -a "$tag" -m "Release $tag"

  cat <<EOF
Release prepared locally.

Next steps:
  git push origin main
  git push origin $tag
EOF
}

main "$@"
