#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TMP_DIR="/tmp/safe-run-stage6"

usage() {
  cat <<'EOF'
Usage:
  ./scripts/network_allowlist_acceptance.sh

What it does:
  1) Builds an isolated ip netns topology with TAP + bridge + veth + route.
  2) Creates nft allowlist rules inside the test netns (host global output is untouched).
  3) Executes ignored test: stage6_real_network_allowlist_closure.
  4) Cleans test netns/interfaces/tables and leaves host network intact.

Prerequisites:
  - Linux host
  - sudo permissions (or run as root)
  - commands: ip nft sysctl curl cargo
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "ERROR: network allowlist acceptance is only supported on Linux." >&2
  exit 1
fi

for cmd in ip nft sysctl curl cargo; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "ERROR: required command not found: $cmd" >&2
    exit 1
  fi
done

if [[ "$EUID" -eq 0 ]]; then
  SUDO=""
else
  if ! command -v sudo >/dev/null 2>&1; then
    echo "ERROR: sudo is required when not running as root." >&2
    exit 1
  fi
  echo "INFO: requesting sudo authentication..."
  sudo -v
  SUDO="sudo"
fi

mkdir -p "$TMP_DIR"

cat > "$TMP_DIR/setup.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SUDO_CMD="${SUDO_CMD:-sudo}"
NS=s6ns
BR=s6br0
TAP=s6tap0
VETH_HOST=s6vethh
VETH_NS=s6vethn
TABLE=s6testns
CHAIN=output
PORT=18080
BR_IP=198.18.0.1
NS_IP=198.18.0.2
SUBNET=198.18.0.0/24
PID_FILE=/tmp/safe-run-stage6/http.pid

"$SUDO_CMD" /tmp/safe-run-stage6/cleanup.sh >/dev/null 2>&1 || true

"$SUDO_CMD" ip netns add "$NS"
"$SUDO_CMD" ip link add "$BR" type bridge
"$SUDO_CMD" ip link set "$BR" up
"$SUDO_CMD" ip addr add "$BR_IP"/24 dev "$BR"

"$SUDO_CMD" ip tuntap add dev "$TAP" mode tap
"$SUDO_CMD" ip link set "$TAP" master "$BR"
"$SUDO_CMD" ip link set "$TAP" up

"$SUDO_CMD" ip link add "$VETH_HOST" type veth peer name "$VETH_NS"
"$SUDO_CMD" ip link set "$VETH_HOST" master "$BR"
"$SUDO_CMD" ip link set "$VETH_HOST" up
"$SUDO_CMD" ip link set "$VETH_NS" netns "$NS"
"$SUDO_CMD" ip netns exec "$NS" ip link set lo up
"$SUDO_CMD" ip netns exec "$NS" ip addr add "$NS_IP"/24 dev "$VETH_NS"
"$SUDO_CMD" ip netns exec "$NS" ip link set "$VETH_NS" up
"$SUDO_CMD" ip netns exec "$NS" ip route add default via "$BR_IP"

"$SUDO_CMD" sysctl -w net.ipv4.ip_forward=1 >/dev/null
"$SUDO_CMD" nft list table ip s6nat >/dev/null 2>&1 || "$SUDO_CMD" nft add table ip s6nat
"$SUDO_CMD" nft list chain ip s6nat postrouting >/dev/null 2>&1 || "$SUDO_CMD" nft 'add chain ip s6nat postrouting { type nat hook postrouting priority srcnat; policy accept; }'
"$SUDO_CMD" nft add rule ip s6nat postrouting ip saddr "$SUBNET" oifname != "$BR" masquerade >/dev/null 2>&1 || true

python3 -m http.server "$PORT" --bind "$BR_IP" >/tmp/safe-run-stage6/http.log 2>&1 &
echo $! > "$PID_FILE"
sleep 0.5

"$SUDO_CMD" ip netns exec "$NS" nft delete table inet "$TABLE" >/dev/null 2>&1 || true
"$SUDO_CMD" ip netns exec "$NS" nft add table inet "$TABLE"
"$SUDO_CMD" ip netns exec "$NS" nft "add chain inet $TABLE $CHAIN { type filter hook output priority 0; policy drop; }"
"$SUDO_CMD" ip netns exec "$NS" nft add rule inet "$TABLE" "$CHAIN" ip daddr "$BR_IP" tcp dport "$PORT" counter accept
EOF

cat > "$TMP_DIR/allowed_probe.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SUDO_CMD="${SUDO_CMD:-sudo}"
"$SUDO_CMD" ip netns exec s6ns curl -fsS --max-time 3 http://198.18.0.1:18080 >/dev/null
EOF

cat > "$TMP_DIR/blocked_probe.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SUDO_CMD="${SUDO_CMD:-sudo}"
"$SUDO_CMD" ip netns exec s6ns curl -fsS --max-time 2 http://1.1.1.1:80 >/dev/null
EOF

cat > "$TMP_DIR/audit_probe.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SUDO_CMD="${SUDO_CMD:-sudo}"
"$SUDO_CMD" ip netns exec s6ns nft list chain inet s6testns output | grep -E "packets [1-9]"
EOF

cat > "$TMP_DIR/cleanup_probe.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SUDO_CMD="${SUDO_CMD:-sudo}"
NS=s6ns
BR=s6br0
TAP=s6tap0
VETH_HOST=s6vethh
PID_FILE=/tmp/safe-run-stage6/http.pid
if [[ -f "$PID_FILE" ]]; then
  kill "$(cat "$PID_FILE")" >/dev/null 2>&1 || true
  rm -f "$PID_FILE"
fi
"$SUDO_CMD" ip netns del "$NS" >/dev/null 2>&1 || true
"$SUDO_CMD" ip link del "$BR" >/dev/null 2>&1 || true
"$SUDO_CMD" ip link del "$TAP" >/dev/null 2>&1 || true
"$SUDO_CMD" ip link del "$VETH_HOST" >/dev/null 2>&1 || true
"$SUDO_CMD" nft delete table ip s6nat >/dev/null 2>&1 || true
! "$SUDO_CMD" ip netns list | grep -q "^${NS}\\b"
! ip link show "$BR" >/dev/null 2>&1
! ip link show "$TAP" >/dev/null 2>&1
! ip link show "$VETH_HOST" >/dev/null 2>&1
! "$SUDO_CMD" nft list table ip s6nat >/dev/null 2>&1
EOF

cat > "$TMP_DIR/cleanup.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SUDO_CMD="${SUDO_CMD:-sudo}"
NS=s6ns
BR=s6br0
TAP=s6tap0
VETH_HOST=s6vethh
PID_FILE=/tmp/safe-run-stage6/http.pid
if [[ -f "$PID_FILE" ]]; then
  kill "$(cat "$PID_FILE")" >/dev/null 2>&1 || true
  rm -f "$PID_FILE"
fi
"$SUDO_CMD" ip netns del "$NS" >/dev/null 2>&1 || true
"$SUDO_CMD" ip link del "$BR" >/dev/null 2>&1 || true
"$SUDO_CMD" ip link del "$TAP" >/dev/null 2>&1 || true
"$SUDO_CMD" ip link del "$VETH_HOST" >/dev/null 2>&1 || true
"$SUDO_CMD" nft delete table ip s6nat >/dev/null 2>&1 || true
EOF

chmod +x "$TMP_DIR"/*.sh

export SUDO_CMD="${SUDO:-sudo}"
export SAFE_RUN_STAGE6_SETUP_CMD="$TMP_DIR/setup.sh"
export SAFE_RUN_STAGE6_ALLOWED_PROBE_CMD="$TMP_DIR/allowed_probe.sh"
export SAFE_RUN_STAGE6_BLOCKED_PROBE_CMD="$TMP_DIR/blocked_probe.sh"
export SAFE_RUN_STAGE6_AUDIT_PROBE_CMD="$TMP_DIR/audit_probe.sh"
export SAFE_RUN_STAGE6_CLEANUP_PROBE_CMD="$TMP_DIR/cleanup_probe.sh"
export SAFE_RUN_STAGE6_CLEANUP_CMD="$TMP_DIR/cleanup.sh"

cd "$ROOT_DIR"
echo "INFO: running network allowlist ignored acceptance test..."
if [[ "$EUID" -eq 0 ]]; then
  cargo test -p sr-runner stage6_real_network_allowlist_closure -- --ignored --nocapture
else
  sudo --preserve-env=SAFE_RUN_STAGE6_SETUP_CMD,SAFE_RUN_STAGE6_ALLOWED_PROBE_CMD,SAFE_RUN_STAGE6_BLOCKED_PROBE_CMD,SAFE_RUN_STAGE6_AUDIT_PROBE_CMD,SAFE_RUN_STAGE6_CLEANUP_PROBE_CMD,SAFE_RUN_STAGE6_CLEANUP_CMD,SUDO_CMD,PATH cargo test -p sr-runner stage6_real_network_allowlist_closure -- --ignored --nocapture
fi

echo "INFO: network allowlist acceptance succeeded."