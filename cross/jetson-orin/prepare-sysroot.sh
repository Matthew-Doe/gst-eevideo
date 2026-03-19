#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /absolute/path/to/output-sysroot" >&2
  exit 2
fi

SYSROOT="$1"

if [[ "$SYSROOT" != /* ]]; then
  echo "error: sysroot path must be absolute: $SYSROOT" >&2
  exit 2
fi

if [[ "$SYSROOT" == "/" ]]; then
  echo "error: refusing to populate / as a generated sysroot" >&2
  exit 2
fi

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

LISTS_DIR="$WORK_DIR/state/lists"
STATUS_FILE="$WORK_DIR/state/status"
SOURCEPARTS_DIR="$WORK_DIR/etc/sourceparts"
PKGCACHE_FILE="$WORK_DIR/cache/pkgcache.bin"
SRCPKGCACHE_FILE="$WORK_DIR/cache/srcpkgcache.bin"
DOWNLOAD_DIR="$WORK_DIR/downloads"

mkdir -p "$LISTS_DIR/partial" "$WORK_DIR/cache" "$SOURCEPARTS_DIR" "$DOWNLOAD_DIR"
: > "$STATUS_FILE"

cat > "$SOURCEPARTS_DIR/ubuntu-archive-amd64.sources" <<'EOF'
Types: deb
URIs: http://azure.archive.ubuntu.com/ubuntu
Suites: noble noble-updates noble-backports noble-security
Components: main universe restricted multiverse
Architectures: amd64
Targets: Packages
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg
EOF

cat > "$SOURCEPARTS_DIR/ubuntu-ports-arm64.sources" <<'EOF'
Types: deb
URIs: http://ports.ubuntu.com/ubuntu-ports
Suites: noble noble-updates noble-backports noble-security
Components: main universe restricted multiverse
Architectures: arm64
Targets: Packages
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg
EOF

APT_OPTS=(
  -o Acquire::Languages=none
  -o Dir::Etc::sourcelist=/dev/null
  -o Dir::Etc::sourceparts="$SOURCEPARTS_DIR"
  -o Dir::State::lists="$LISTS_DIR"
  -o Dir::State::status="$STATUS_FILE"
  -o Dir::Cache::pkgcache="$PKGCACHE_FILE"
  -o Dir::Cache::srcpkgcache="$SRCPKGCACHE_FILE"
)

echo "Updating package indexes for sysroot assembly"
apt-get "${APT_OPTS[@]}" update

SEED_PACKAGES=(
  libc6-dev:arm64
  libglib2.0-dev:arm64
  libgstreamer1.0-dev:arm64
  libgstreamer-plugins-base1.0-dev:arm64
)

package_arch() {
  local package="$1"
  local metadata

  metadata="$(apt-cache "${APT_OPTS[@]}" show "$package" 2>/dev/null || true)"

  sed -n 's/^Architecture: //p' <<<"$metadata" | head -n 1
}

mapfile -t raw_candidates < <(
  apt-cache "${APT_OPTS[@]}" depends \
    --recurse \
    --no-recommends \
    --no-suggests \
    --no-conflicts \
    --no-breaks \
    --no-replaces \
    --no-enhances \
    "${SEED_PACKAGES[@]}" |
    perl -ne '
      if (/^([^[:space:]][^[:space:]]*)$/) {
        print "$1\n";
        next;
      }
      if (/^[[:space:]]+\|?(?:Pre)?Depends:\s+<?([^>[:space:]]+)>?/) {
        print "$1\n";
        next;
      }
      if (/^[[:space:]]+([^<[:space:]][^[:space:]]*)$/) {
        print "$1\n";
      }
    '
)

mapfile -t unique_candidates < <(
  printf '%s\n' "${raw_candidates[@]}" | awk 'NF && !seen[$0]++'
)

download_packages=()
for package in "${unique_candidates[@]}"; do
  if [[ "$package" == *:arm64 ]]; then
    arch="$(package_arch "$package")"
    if [[ "$arch" == "arm64" ]]; then
      download_packages+=("$package")
      continue
    fi

    base_package="${package%:arm64}"
    arch="$(package_arch "$base_package")"
    if [[ "$arch" == "all" ]]; then
      download_packages+=("$base_package")
    fi
    continue
  fi

  arch="$(package_arch "$package")"
  if [[ "$arch" == "all" ]]; then
    download_packages+=("$package")
  fi
done

mapfile -t unique_downloads < <(
  printf '%s\n' "${download_packages[@]}" | awk 'NF && !seen[$0]++'
)

echo "Downloading ${#unique_downloads[@]} arm64/all packages into sysroot staging"
(
  cd "$DOWNLOAD_DIR"
  apt-get "${APT_OPTS[@]}" download "${unique_downloads[@]}"
)

mkdir -p "$SYSROOT"

echo "Extracting downloaded packages into $SYSROOT"
find "$DOWNLOAD_DIR" -maxdepth 1 -name '*.deb' -print0 |
  while IFS= read -r -d '' deb; do
    dpkg-deb -x "$deb" "$SYSROOT"
  done
