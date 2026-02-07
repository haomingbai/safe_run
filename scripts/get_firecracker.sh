#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ARTIFACTS_DIR="${ROOT_DIR}/artifacts"
BIN_DIR="${ARTIFACTS_DIR}/bin"
CACHE_DIR="${ARTIFACTS_DIR}/cache"

usage() {
  cat <<'EOF'
Usage:
  ./scripts/get_firecracker.sh [version]

Examples:
  ./scripts/get_firecracker.sh           # download latest release
  ./scripts/get_firecracker.sh v1.13.1   # download specific version

Behavior:
  - Downloads Firecracker release tarball from GitHub.
  - Installs both firecracker and jailer into ./artifacts/bin.
  - Does not require system package installation of firecracker/jailer.
  - Uses local proxy http://127.0.0.1:7890 by default when no proxy env is set.
    Set SAFE_RUN_USE_LOCAL_PROXY=0 to disable, or SAFE_RUN_PROXY_URL to override.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "ERROR: curl is required." >&2
  exit 1
fi

if ! command -v tar >/dev/null 2>&1; then
  echo "ERROR: tar is required." >&2
  exit 1
fi

USE_LOCAL_PROXY="${SAFE_RUN_USE_LOCAL_PROXY:-1}"
PROXY_URL="${SAFE_RUN_PROXY_URL:-http://127.0.0.1:7890}"
HAS_PROXY_ENV=0
if [[ -n "${HTTP_PROXY:-}" || -n "${HTTPS_PROXY:-}" || -n "${http_proxy:-}" || -n "${https_proxy:-}" ]]; then
  HAS_PROXY_ENV=1
fi

curl_fetch() {
  if [[ "${HAS_PROXY_ENV}" == "1" ]]; then
    curl "$@"
    return
  fi

  if [[ "${USE_LOCAL_PROXY}" == "1" ]]; then
    if curl --proxy "${PROXY_URL}" "$@"; then
      return
    fi
    echo "WARN: request via proxy '${PROXY_URL}' failed, retrying without proxy." >&2
  fi

  curl "$@"
}

ARCH_RAW="$(uname -m)"
case "${ARCH_RAW}" in
  x86_64|aarch64)
    ARCH="${ARCH_RAW}"
    ;;
  *)
    echo "ERROR: unsupported architecture '${ARCH_RAW}'. Expected x86_64 or aarch64." >&2
    exit 1
    ;;
esac

resolve_version() {
  local requested="${1:-}"
  if [[ -n "${requested}" ]]; then
    if [[ "${requested}" == v* ]]; then
      printf '%s\n' "${requested}"
    else
      printf 'v%s\n' "${requested}"
    fi
    return 0
  fi

  curl_fetch --silent --show-error --fail --location \
    "https://api.github.com/repos/firecracker-microvm/firecracker/releases/latest" \
    | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -n 1
}

VERSION="$(resolve_version "${1:-}")"
if [[ -z "${VERSION}" ]]; then
  echo "ERROR: failed to resolve latest Firecracker version." >&2
  exit 1
fi

TARBALL="firecracker-${VERSION}-${ARCH}.tgz"
URL="https://github.com/firecracker-microvm/firecracker/releases/download/${VERSION}/${TARBALL}"

mkdir -p "${BIN_DIR}" "${CACHE_DIR}"
TMP_DIR="$(mktemp -d "${CACHE_DIR}/fc-tmp-XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

ARCHIVE_PATH="${CACHE_DIR}/${TARBALL}"

echo "Downloading ${URL}"
curl_fetch --fail --location --retry 3 --output "${ARCHIVE_PATH}" "${URL}"

echo "Extracting ${ARCHIVE_PATH}"
tar -xzf "${ARCHIVE_PATH}" -C "${TMP_DIR}"

RELEASE_DIR="${TMP_DIR}/release-${VERSION}-${ARCH}"
if [[ ! -d "${RELEASE_DIR}" ]]; then
  echo "ERROR: release directory not found in archive: ${RELEASE_DIR}" >&2
  exit 1
fi

for binary in firecracker jailer; do
  SRC="${RELEASE_DIR}/${binary}-${VERSION}-${ARCH}"
  DST="${BIN_DIR}/${binary}"
  if [[ ! -f "${SRC}" ]]; then
    echo "ERROR: binary not found in archive: ${SRC}" >&2
    exit 1
  fi
  cp "${SRC}" "${DST}"
  chmod 0755 "${DST}"
done

cat > "${BIN_DIR}/VERSION" <<EOF
${VERSION}
EOF

echo "Installed local binaries:"
echo "  ${BIN_DIR}/firecracker"
echo "  ${BIN_DIR}/jailer"
echo
echo "Use them in current shell:"
echo "  export PATH=\"${BIN_DIR}:\$PATH\""
