#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
ARTIFACTS_DIR="${ROOT_DIR}/artifacts"

mkdir -p "${ARTIFACTS_DIR}"
cd "${ARTIFACTS_DIR}"

ARCH="$(uname -m)"
RELEASE_URL="https://github.com/firecracker-microvm/firecracker/releases"
LATEST_VERSION=$(basename "$(curl -fsSLI -o /dev/null -w %{url_effective} ${RELEASE_URL}/latest)")
CI_VERSION=${LATEST_VERSION%.*}

LATEST_KERNEL_KEY=$(curl "http://spec.ccfc.min.s3.amazonaws.com/?prefix=firecracker-ci/${CI_VERSION}/${ARCH}/vmlinux-&list-type=2" \
  | grep -oP "(?<=<Key>)(firecracker-ci/${CI_VERSION}/${ARCH}/vmlinux-[0-9]+\.[0-9]+\.[0-9]{1,3})(?=</Key>)" \
  | sort -V | tail -1)

if [[ -z "${LATEST_KERNEL_KEY}" ]]; then
  echo "ERROR: failed to locate latest kernel image for ${ARCH}" >&2
  exit 1
fi

KERNEL_URL="https://s3.amazonaws.com/spec.ccfc.min/${LATEST_KERNEL_KEY}"

echo "Downloading kernel: ${KERNEL_URL}"
wget -q -O vmlinux "${KERNEL_URL}"

LATEST_UBUNTU_KEY=$(curl "http://spec.ccfc.min.s3.amazonaws.com/?prefix=firecracker-ci/${CI_VERSION}/${ARCH}/ubuntu-&list-type=2" \
  | grep -oP "(?<=<Key>)(firecracker-ci/${CI_VERSION}/${ARCH}/ubuntu-[0-9]+\.[0-9]+\.squashfs)(?=</Key>)" \
  | sort -V | tail -1)

if [[ -z "${LATEST_UBUNTU_KEY}" ]]; then
  echo "ERROR: failed to locate latest Ubuntu rootfs for ${ARCH}" >&2
  exit 1
fi

UBUNTU_VERSION=$(basename "${LATEST_UBUNTU_KEY}" .squashfs | grep -oE '[0-9]+\.[0-9]+')
ROOTFS_SQUASHFS="ubuntu-${UBUNTU_VERSION}.squashfs.upstream"
ROOTFS_URL="https://s3.amazonaws.com/spec.ccfc.min/${LATEST_UBUNTU_KEY}"

if ! command -v unsquashfs >/dev/null 2>&1; then
  echo "ERROR: unsquashfs not found. Install squashfs-tools." >&2
  exit 1
fi

if ! command -v mkfs.ext4 >/dev/null 2>&1; then
  echo "ERROR: mkfs.ext4 not found. Install e2fsprogs." >&2
  exit 1
fi

ROOT_CMD=()
if command -v fakeroot >/dev/null 2>&1; then
  ROOT_CMD=(fakeroot --)
elif [[ ${EUID:-$(id -u)} -eq 0 ]]; then
  ROOT_CMD=()
elif command -v sudo >/dev/null 2>&1; then
  ROOT_CMD=(sudo)
else
  echo "ERROR: fakeroot or sudo is required to set root ownership in rootfs." >&2
  exit 1
fi

run_root_cmd() {
  if ((${#ROOT_CMD[@]})); then
    "${ROOT_CMD[@]}" "$@"
  else
    "$@"
  fi
}

echo "Downloading rootfs: ${ROOTFS_URL}"
wget -q -O "${ROOTFS_SQUASHFS}" "${ROOTFS_URL}"

rm -rf squashfs-root
unsquashfs "${ROOTFS_SQUASHFS}"

run_root_cmd chown -R root:root squashfs-root
truncate -s 1G rootfs.ext4
run_root_cmd mkfs.ext4 -d squashfs-root -F rootfs.ext4

echo "Artifacts ready in ${ARTIFACTS_DIR}:"
ls -1 vmlinux rootfs.ext4
