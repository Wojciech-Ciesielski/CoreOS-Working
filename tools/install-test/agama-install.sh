#!/bin/bash
# Runs inside the live session (user shrimp, password shrimp) from the test stick made by
# .github/workflows/test-install.yml. Talks to Agama exactly like Setup does (same token,
# same /api paths, config.json from configFromPlan) and logs everything to the serial port.
HERE=$(cd "$(dirname "$0")" && pwd)
S() { echo shrimp | sudo -S -p '' "$@"; }   # the sudo timestamp runs out during the install
S true
exec > >(sudo tee /dev/ttyS0 >/dev/null) 2>&1
set -x
T=$(cat /run/shrimp-setup/token 2>/dev/null)
[ -n "$T" ] || { echo "no Agama token"; ls -l /run/shrimp-setup /run/agama; }
api() { curl -sS -m 60 -H "Authorization: Bearer $T" -H 'Content-Type: application/json' -w '\nHTTP %{http_code}\n' "$@"; }
U=http://127.0.0.1
echo ===AGAMA-START
ls -l "$HERE" /run/initramfs/live /run/initramfs/live/shrimp-transfer 2>&1 | head -40
lsblk -o NAME,SIZE,TYPE,FSTYPE,LABEL,MOUNTPOINTS
# Like the Vivobook (Wi-Fi only, nothing configured in the live session): the VM starts with the
# network cable unplugged, so Agama reads the repositories without internet. Then the cable is
# plugged in (qemu-install-test.py watches for ===NET-UP-PLEASE) and Setup's way of re-reading
# the repositories must make the product visible again.
echo "=== offline: connectivity $(nmcli networking connectivity check 2>&1)"
for p in /api/status /api/issues; do echo "=== offline GET $p"; api "$U$p" | head -c 4000; echo; done
echo "=== API discovery"
for p in /api-docs/openapi.json /api/docs/openapi.json /api/openapi.json /api/v2/openapi.json; do
  echo "--- $p"; api "$U$p" | head -c 200; echo
done
for f in /usr/bin/agama-web-server /usr/bin/agama /usr/libexec/agama*; do
  [ -f "$f" ] && { echo "--- strings $f"; strings "$f" | grep -oE '\b(probe|reprobe|reload|refresh|activate|configure|install|finish)[A-Z][A-Za-z]*\b' | sort -u | head -60; }
done
for d in /usr/share/agama/web_ui /usr/share/agama/web-ui /usr/share/agama-web-ui; do
  [ -d "$d" ] && { echo "--- web ui $d"; zcat -f "$d"/*.js* 2>/dev/null | grep -oE '"(probe|reprobe|reload|refresh|activate|configure)[A-Z][A-Za-z]*"' | sort -u | head -60; }
done
rpm -q agama agama-web-ui agama-cli 2>&1
# 1. Offline, exactly what Setup's "check" does: select the product. Agama cannot read the repositories.
echo "=== offline: PUT /api/config"; api -X PUT --data @"$HERE/config.json" "$U/api/config"
for i in $(seq 240); do s=$(api "$U/api/status"); echo "$s" | grep -q '"tasks":\[\],"progresses":\[\]' && break; sleep 0.5; done
echo "=== offline issues"; off=$(api "$U/api/issues"); echo "$off"
echo "$off" | grep -q '"scope":"software"' && echo "===OFFLINE-ISSUE yes" || echo "===OFFLINE-ISSUE no"
# 1b. Setup's Wi-Fi step runs nmcli as the live user, before any account or password exists.
# Adding a connection is the step that asked for the administrator password on the Vivobook (Beta 3.5),
# so do the same here without sudo and without an authentication agent answering.
echo "=== live user $(id -un): nmcli connection add"
if timeout 30 nmcli --wait 10 connection add type dummy ifname shrimpauth0 con-name shrimp-auth-test autoconnect no; then
  echo "===NM-AUTH ok"; timeout 30 nmcli connection delete shrimp-auth-test || true
else
  echo "===NM-AUTH no"
fi
# 2. The cable goes in (Setup's "Connect to a network" step does this on a real laptop).
echo ===NET-UP-PLEASE
for i in $(seq 90); do [ "$(nmcli networking connectivity check 2>/dev/null)" = full ] && break; sleep 2; done
echo "=== online after $((i*2)) s: $(nmcli networking connectivity check 2>&1)"
# 3. Setup's reprobe(): probe, select the product again, wait, read the issues.
# Same body as PROBE_SOFTWARE in Setup (agama.ts). {"probe":null} was refused with HTTP 422 up to Beta 4,
# and only the PUT below hid it here; on the Vivobook it made Setup say "no internet" after Wi-Fi connected.
echo "=== POST probe"; pr=$(api -X POST --data '{"probe":{"only":["software"]}}' "$U/api/action"); echo "$pr"
case "$pr" in *"HTTP 200"*) echo "===PROBE ok";; *) echo "===PROBE refused";; esac
for i in $(seq 240); do s=$(api "$U/api/status"); echo "$s" | grep -q '"tasks":\[\],"progresses":\[\]' && break; sleep 0.5; done
echo "=== PUT /api/config again"; api -X PUT --data @"$HERE/config.json" "$U/api/config"
for i in $(seq 240); do s=$(api "$U/api/status"); echo "$s" | grep -q '"tasks":\[\],"progresses":\[\]' && break; sleep 0.5; done
iss=$(api "$U/api/issues"); echo "=== issues after probe"; echo "$iss"
if echo "$iss" | grep -q '"scope":"software"'; then reprobed=no; else reprobed=probe; fi
case "$pr" in *"HTTP 200"*) ;; *) reprobed=no;; esac
echo "=== REPROBE $reprobed"
for p in /api/status /api/issues /api/config /api/system; do
  echo "=== GET $p"; api "$U$p" | head -c 12000; echo
done
echo "=== config.json"; cat "$HERE/config.json"
echo "=== PUT /api/config"; api -X PUT --data @"$HERE/config.json" "$U/api/config"
# same wait as Setup's settle(): no progresses and no tasks
for i in $(seq 240); do
  s=$(api "$U/api/status"); echo "$s" | grep -q '"tasks":\[\],"progresses":\[\]' && break; sleep 0.5
done
echo "settled after $((i/2)) s"
echo "=== status after config"; api "$U/api/status"
echo "=== issues after config"; api "$U/api/issues"
echo "=== POST install"; r=$(api -X POST --data '{"install":null}' "$U/api/action"); echo "$r"
stage=unknown
case "$r" in *"HTTP 200"*) n=360;; *) stage=refused; n=0;; esac   # Agama refused: don't wait 90 min
for i in $(seq $n); do   # up to 90 min
  sleep 15
  s=$(api "$U/api/status")
  echo "[$((i*15))s] $s" | tr -d '\n' | head -c 1500; echo
  case "$s" in *'"stage":"finished"'*) stage=finished; break;; *'"stage":"failed"'*) stage=failed; break;; esac
done
echo "=== issues at end"; api "$U/api/issues"
echo "=== journal"
S journalctl -b -u agama -u agama-web-server --no-pager -o short-monotonic | grep -v -iE 'pubkey|TmpPath|keyring|Tkey|zypp.tmp|hardlink|database|Pid [0-9]+ |launched' | tail -n 400
echo "=== post-install / target"
S ls -la /mnt /mnt/var/lib/shrimp-setup /mnt/usr/bin/shrimp-setup /mnt/home 2>&1 | head -40
S find /var/log/agama* /run/agama -maxdepth 2 -name '*post*' -o -maxdepth 2 -name '*script*' 2>/dev/null | head
for f in $(S find /var/log/agama* /run/agama /tmp -maxdepth 3 -path '*scripts*' -type f 2>/dev/null | head -20); do echo "--- $f"; S tail -n 60 "$f"; done
echo "===AGAMA-DONE $stage"
