#!/usr/bin/env bash
# End-to-end: deployer schedules nginx onto a worker (verified signed records),
# worker runs it in Podman, deployer's local gateway tunnels HTTP to it.
# Usage: tests/integration/e2e.sh [runs]   (needs podman + cargo; pulls nginx:alpine)
set -u
cd "$(dirname "$0")/../../peer-agent" && cargo build -q || exit 1
B=$PWD/target/debug/flakenet-agent
strip() { sed 's/\x1b\[[0-9;]*m//g'; }
mkid() { timeout 1 "$B" --key-file "$1" --listen /ip4/127.0.0.1/udp/0/quic-v1 2>&1 | strip | grep -o '12D3[A-Za-z0-9]*' | head -1; }
fail=0
for i in $(seq "${1:-3}"); do
  T=$(mktemp -d); P=$((6000+i*4)); HP=$((20000+i)); GP=$((20100+i))
  DID=$(mkid "$T/d.key")
  "$B" --key-file "$T/w.key" --listen /ip4/127.0.0.1/udp/$P/quic-v1 --trust "$DID" >"$T/w.log" 2>&1 & W=$!
  sleep 1; WID=$(strip <"$T/w.log" | grep -o '12D3[A-Za-z0-9]*' | head -1)
  "$B" --key-file "$T/d.key" --listen /ip4/127.0.0.1/udp/0/quic-v1 --bootstrap /ip4/127.0.0.1/udp/$P/quic-v1/p2p/$WID \
     --deploy-image docker.io/library/nginx:alpine --deploy-ports $HP:80 --gateway-listen 127.0.0.1:$GP >"$T/d.log" 2>&1 & D=$!
  sleep 7
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 127.0.0.1:$GP)
  kill -INT $D $W; wait 2>/dev/null; sleep 1
  left=$(podman ps -aq --filter label=flakenet=1 | wc -l)
  if [ "$code" = 200 ] && [ "$left" = 0 ]; then echo "run $i: PASS"; else echo "run $i: FAIL (http=$code leftover=$left)"; fail=1; fi
done
exit $fail
