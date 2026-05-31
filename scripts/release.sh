#!/usr/bin/env bash
# Bump gpucap version, publish to crates.io, and update the Homebrew formula.
#
# Usage:
#   ./scripts/release.sh [patch|minor|major|X.Y.Z] [options]
#
# Options:
#   --dry-run       Bump, build, and package only; do not publish or edit the formula
#   --skip-tests    Skip cargo test
#   --no-publish    Update formula from local cargo package but skip crates.io upload
#   --formula PATH  Homebrew formula (default: /Volumes/Code/homebrew-tap/Formula/gpucap.rb)
#
# Prerequisites:
#   • rust/cargo, logged in for publish (`cargo login`)
#   • clean git tree in gpucap (or use --dry-run to inspect first)
#   • homebrew-tap checkout at HOMEBREW_TAP_FORMULA path
#
# After running:
#   • commit and push gpucap (tag vX.Y.Z recommended)
#   • commit and push homebrew-tap
#   • brew update && brew upgrade digital-defiance/tap/gpucap
#
# If `brew install gpucap` reports duplicate taps, remove the local dev tap:
#   brew untap digital-defiance/tap-local

set -euo pipefail

GPUCAP_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FORMULA_FILE="${HOMEBREW_TAP_FORMULA:-/Volumes/Code/homebrew-tap/Formula/gpucap.rb}"
CRATE_NAME="gpucap"

DRY_RUN=0
SKIP_TESTS=0
NO_PUBLISH=0
BUMP="patch"

usage() {
  sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'
  exit "${1:-0}"
}

log() {
  printf '==> %s\n' "$*"
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

read_version() {
  grep -E '^version[[:space:]]*=' "${GPUCAP_ROOT}/Cargo.toml" \
    | head -1 \
    | sed -E 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/'
}

bump_version() {
  local current="$1" kind="$2"
  local major minor patch
  IFS=. read -r major minor patch <<<"$current"

  case "$kind" in
    patch) patch=$((patch + 1)) ;;
    minor) minor=$((minor + 1)); patch=0 ;;
    major) major=$((major + 1)); minor=0; patch=0 ;;
    *)
      if [[ ! "$kind" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        die "invalid version or bump kind: $kind"
      fi
      printf '%s' "$kind"
      return
      ;;
  esac

  printf '%s.%s.%s' "$major" "$minor" "$patch"
}

set_version() {
  local version="$1"
  local tmp
  tmp="$(mktemp)"
  sed -E "s/^version[[:space:]]*=[[:space:]]*\"[^\"]+\"/version = \"${version}\"/" \
    "${GPUCAP_ROOT}/Cargo.toml" >"$tmp"
  mv "$tmp" "${GPUCAP_ROOT}/Cargo.toml"
}

update_formula() {
  local version="$1" sha256="$2"
  [[ -f "$FORMULA_FILE" ]] || die "formula not found: $FORMULA_FILE"

  local url="https://static.crates.io/crates/${CRATE_NAME}/${CRATE_NAME}-${version}.crate"
  local tmp
  tmp="$(mktemp)"

  awk -v ver="$version" -v url="$url" -v sha="$sha256" '
    /^  url / { print "  url \"" url "\""; next }
    /^  sha256 / { print "  sha256 \"" sha "\""; next }
    { print }
  ' "$FORMULA_FILE" >"$tmp"
  mv "$tmp" "$FORMULA_FILE"

  log "updated ${FORMULA_FILE}"
  log "  url    ${url}"
  log "  sha256 ${sha256}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    patch|minor|major) BUMP="$1"; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --skip-tests) SKIP_TESTS=1; shift ;;
    --no-publish) NO_PUBLISH=1; shift ;;
    --formula)
      FORMULA_FILE="$2"
      shift 2
      ;;
    -h|--help) usage 0 ;;
    *)
      if [[ "$1" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        BUMP="${1#v}"
      else
        die "unknown argument: $1 (try --help)"
      fi
      shift
      ;;
  esac
done

cd "$GPUCAP_ROOT"
export CARGO_TARGET_DIR="${GPUCAP_ROOT}/target"

CURRENT="$(read_version)"
NEW="$(bump_version "$CURRENT" "$BUMP")"

log "gpucap ${CURRENT} -> ${NEW}"
set_version "$NEW"
if [[ "$DRY_RUN" -eq 1 ]]; then
  restore_cargo_version() {
    set_version "$CURRENT"
  }
  trap restore_cargo_version EXIT
fi

if [[ "$SKIP_TESTS" -eq 0 ]]; then
  log "running tests"
  cargo test
fi

log "release build"
cargo build --release

log "packaging crate"
rm -f "${CARGO_TARGET_DIR}/package/${CRATE_NAME}-${NEW}.crate"
cargo package --allow-dirty
PACKAGE="${CARGO_TARGET_DIR}/package/${CRATE_NAME}-${NEW}.crate"
if [[ ! -f "$PACKAGE" ]]; then
  PACKAGE="$(find "${CARGO_TARGET_DIR}/package" -maxdepth 1 -name "${CRATE_NAME}-${NEW}.crate" -print -quit 2>/dev/null || true)"
fi
[[ -n "${PACKAGE:-}" && -f "$PACKAGE" ]] || die "package not found under ${CARGO_TARGET_DIR}/package"

SHA256="$(shasum -a 256 "$PACKAGE" | awk '{print $1}')"
log "crate sha256: ${SHA256}"

if [[ "$DRY_RUN" -eq 1 ]]; then
  log "dry run — skipping publish and formula update"
  log "package: ${PACKAGE}"
  exit 0
fi

if [[ "$NO_PUBLISH" -eq 0 ]]; then
  log "publishing to crates.io"
  cargo publish --allow-dirty
else
  log "skipping crates.io publish (--no-publish)"
fi

update_formula "$NEW" "$SHA256"

cat <<EOF

Done.

Next steps:
  1. In gpucap:
       git add Cargo.toml Cargo.lock
       git commit -m "Release v${NEW}"
       git tag v${NEW}
       git push && git push origin v${NEW}

  2. In homebrew-tap:
       git add Formula/gpucap.rb
       git commit -m "gpucap ${NEW}"
       git push

  3. Install/upgrade (use fully-qualified name if tap-local is also tapped):
       brew untap digital-defiance/tap-local   # optional, avoids duplicate formula
       brew update
       brew upgrade digital-defiance/tap/gpucap   # command: bgpucap (gpucap symlink)

EOF
