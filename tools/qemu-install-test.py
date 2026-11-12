#!/usr/bin/env python3
"""Install Shrimp in QEMU the way a user does after Shrimp Install, then boot the result.

Usage: qemu-install-test.py <usb.img> <outdir>
  usb.img  FAT32 stick made like Shrimp Install does (ISO files, label SHRIMP_LIVE,
           shrimp-transfer/ with shrimp.json and files) plus shrimp-test/agama-install.sh.

Phase 0 boots the stick with the Setup guard ON (no SHRIMP_TEST drive) and checks that a mouse
click reaches the Setup window (Beta 3.2: the pointer moved, clicks did nothing). The journal is
forwarded to the serial port (kernel parameters added to the stick's GRUB by the workflow).
Phase 1 boots the stick (UEFI) with an empty 40 GB disk, waits for the Setup window, then
runs shrimp-test/agama-install.sh, which drives Agama with Setup's own configuration and
logs to the serial port. Phase 2 boots the installed disk and captures the first boot
(Setup, first configuration). Everything lands in <outdir> (serial*.log, result.json); no screenshots.
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import time

USB, OUT = sys.argv[1], sys.argv[2]
USB_GUARD = os.path.join(os.path.dirname(USB) or '.', 'usb-guard.img')
if not os.path.exists(USB_GUARD):
    USB_GUARD = USB
os.makedirs(OUT, exist_ok=True)
QMP_SOCK = os.path.join(OUT, "qmp.sock")
W, H = 1920, 1080
DISK = os.path.join(OUT, "disk.qcow2")
VARS = os.path.join(OUT, "vars.fd")
subprocess.run(["qemu-img", "create", "-f", "qcow2", DISK, "40G"], check=True)
shutil.copy("/usr/share/OVMF/OVMF_VARS_4M.fd", VARS)


def noguard_drive(outdir):
    """Small FAT drive labelled SHRIMP_TEST: switches off the Shrimp Setup guard (GNOME Shell
    extension shrimp-setup-guard@shrimp), so the test can close Setup and use the desktop."""
    img = os.path.join(outdir, "noguard.img")
    if not os.path.exists(img):
        subprocess.run(["mkfs.vfat", "-C", "-n", "SHRIMP_TEST", img, "2048"], check=True, stdout=subprocess.DEVNULL)
    return ["-drive", f"file={img},if=none,id=noguard,format=raw", "-device", "usb-storage,drive=noguard"]


def start(phase, with_usb, guard=False, offline=False):
    if os.path.exists(QMP_SOCK):
        os.remove(QMP_SOCK)
    cmd = [
        "qemu-system-x86_64", "-enable-kvm", "-cpu", "host", "-smp", "4", "-m", "6144",
        "-machine", "q35",
        "-drive", "if=pflash,format=raw,unit=0,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd",
        "-drive", f"if=pflash,format=raw,unit=1,file={VARS}",
        "-device", f"virtio-vga,edid=on,xres={W},yres={H}",
        "-device", "qemu-xhci,id=xhci", "-device", "usb-tablet", "-device", "usb-kbd",
        "-netdev", "user,id=net0", "-device", "virtio-net-pci,netdev=net0,id=nic0",
        "-drive", f"file={DISK},if=none,id=hd,format=qcow2", "-device", "virtio-blk-pci,drive=hd,bootindex=2",
        "-display", "none",
        "-qmp", f"unix:{QMP_SOCK},server=on,wait=off",
        "-serial", f"file:{os.path.join(OUT, f'serial-{phase}.log')}",
    ]
    if with_usb:
        cmd += ["-drive", f"file={USB_GUARD if guard else USB},if=none,id=stick,format=raw",
                "-device", "usb-storage,bus=xhci.0,drive=stick,removable=on,bootindex=1"]
    if not guard:
        cmd += noguard_drive(OUT)
    print("+", " ".join(cmd), flush=True)
    return subprocess.Popen(cmd)


def set_net(up):
    """Network cable in or out (like a laptop with Wi-Fi only before anything is configured)."""
    print(f"[{t():6.1f}s] network {'up' if up else 'down'}", flush=True)
    qmp.cmd("set_link", name="nic0", up=up)


T0 = time.time()


def t():
    return time.time() - T0


class VMGone(Exception):
    """QEMU exited (e.g. the guest powered off)."""


class QMP:
    def __init__(self, path):
        for _ in range(100):
            try:
                self.s = socket.socket(socket.AF_UNIX)
                self.s.connect(path)
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise SystemExit("QMP socket never appeared")
        self.f = self.s.makefile("rw")
        self.f.readline()  # greeting
        self.cmd("qmp_capabilities")

    def cmd(self, _name, **args):
        msg = {"execute": _name}
        if args:
            msg["arguments"] = args
        try:
            self.f.write(json.dumps(msg) + "\n")
            self.f.flush()
        except OSError:  # BrokenPipeError etc.: QEMU already exited (the guest powered off)
            raise VMGone()
        while True:
            line = self.f.readline()
            if not line:
                raise VMGone()
            r = json.loads(line)
            if "return" in r:
                return r["return"]
            if "error" in r:
                print("QMP error:", _name, r["error"], flush=True)
                return None




# ---------------------------------------------------------------- stages ----
# Zrzutów ekranu nie robimy i nie zapisujemy (zasada właściciela). Etapy trafiają tylko do logu.
def stage(label, keep_duplicates=False):
    """Marks a stage of the run in the log (no screenshot)."""
    if qemu.poll() is not None:
        return
    print(f"[{t():6.1f}s] stage {label}", flush=True)


def wait_stages(until, every, label):
    """Waits until `until` (seconds since start) or until QEMU exits; logs the stage once."""
    stage(label)
    while t() < until and qemu.poll() is None:
        time.sleep(min(every, 1))


# ---------------------------------------------------------------- input ----
SHIFTED = {'|': 'backslash', '>': 'dot', '<': 'comma', '&': '7', '"': 'apostrophe',
           '_': 'minus', ':': 'semicolon', '*': '8', '(': '9', ')': '0', '~': 'grave_accent',
           '$': '4', '@': '2', '{': 'bracket_left', '}': 'bracket_right', '!': '1',
           '?': 'slash', '+': 'equal', '%': '5', '^': '6', '#': '3'}
PLAIN = {' ': 'spc', '-': 'minus', '/': 'slash', '.': 'dot', ';': 'semicolon', "'": 'apostrophe',
         '=': 'equal', ',': 'comma', '\n': 'ret', '\\': 'backslash', '[': 'bracket_left',
         ']': 'bracket_right', '`': 'grave_accent', '\t': 'tab'}


def _key_events(names, down):
    return [{"type": "key", "data": {"down": down, "key": {"type": "qcode", "data": n}}} for n in names]


def press(names):
    """Press a chord: modifiers down in order, then released in reverse, one event at a time
    (send-key overran the guest keyboard queue and left Shift stuck)."""
    for n in names:
        qmp.cmd("input-send-event", events=_key_events([n], True))
        time.sleep(0.03)
    for n in reversed(names):
        qmp.cmd("input-send-event", events=_key_events([n], False))
        time.sleep(0.03)
    time.sleep(0.06)


def keys(*names):
    print(f"[{t():6.1f}s] keys {'+'.join(names)}", flush=True)
    press(list(names))
    time.sleep(0.2)


def type_text(s):
    print(f"[{t():6.1f}s] type {s!r}", flush=True)
    for c in s:
        if c.isalpha() and c.isupper():
            k = ["shift", c.lower()]
        elif c.isalnum():
            k = [c]
        elif c in PLAIN:
            k = [PLAIN[c]]
        elif c in SHIFTED:
            k = ["shift", SHIFTED[c]]
        else:
            raise ValueError(f"no key for {c!r}")
        press(k)


def click(x, y):
    print(f"[{t():6.1f}s] click {x},{y}", flush=True)
    ax, ay = int(x * 32767 / (W - 1)), int(y * 32767 / (H - 1))
    qmp.cmd("input-send-event", events=[
        {"type": "abs", "data": {"axis": "x", "value": ax}},
        {"type": "abs", "data": {"axis": "y", "value": ay}}])
    time.sleep(0.2)
    for down in (True, False):
        qmp.cmd("input-send-event", events=[{"type": "btn", "data": {"down": down, "button": "left"}}])
        time.sleep(0.1)


def pause(sec):
    time.sleep(sec)


def run_dialog(command):
    keys("alt", "f2")
    pause(2)
    type_text(command)
    pause(0.5)
    keys("ret")




def serial_has(phase, text):
    try:
        with open(os.path.join(OUT, f"serial-{phase}.log"), errors="replace") as f:
            return text in f.read()
    except OSError:
        return False


def stop():
    if qemu.poll() is None:
        try:
            qmp.cmd("quit")
        except VMGone:
            pass
        qemu.wait(20)


result = "unknown"
guard = "unknown"
try:
    # ---------------------------------------------------------- phase 0 ----
    # Setup guard on, like on a real computer: a click in the middle of the Setup window must reach Setup.
    qemu = start("guard", True, guard=True)
    qmp = QMP(QMP_SOCK)
    deadline = t() + 360
    while t() < deadline and qemu.poll() is None and not serial_has("guard", "shrimp-setup-guard: window:"):
        time.sleep(5)
    stage("guard-setup", keep_duplicates=True)
    pause(20)  # Setup page loaded (Agama token, first screen)
    for _ in range(3):
        if serial_has("guard", "shrimp-setup: pierwsze klikni"):
            break
        click(W // 2, H // 2)
        pause(8)
    if serial_has("guard", "shrimp-setup: pierwsze klikni"):
        guard = "click-ok"
    elif serial_has("guard", "shrimp-setup-guard: window:"):
        guard = "click-blocked"
    print(f"[{t():6.1f}s] Setup guard: {guard}", flush=True)
    stop()

    # ---------------------------------------------------------- phase 1 ----
    qemu = start("live", True)
    qmp = QMP(QMP_SOCK)
    set_net(False)  # Vivobook: no cable, Wi-Fi not set up yet
    wait_stages(240, 15, "live")
    stage("setup", keep_duplicates=True)
    # Setup window up: drive Agama from a terminal (same token and config as Setup).
    run_dialog("pkill -f shrimp-setup")
    pause(5)
    run_dialog("kgx")
    pause(10)
    # Where the stick is mounted is part of what we check: log it, fall back to mounting by label.
    type_text("echo shrimp | sudo -S true; findmnt -l | sudo tee /dev/ttyS0; "
              "f=$(find /run /media -maxdepth 5 -name agama-install.sh 2>/dev/null | head -n1); "
              "[ -n \"$f\" ] || { sudo mkdir -p /tmp/stick; sudo mount -o ro LABEL=SHRIMP_LIVE /tmp/stick; "
              "f=/tmp/stick/shrimp-test/agama-install.sh; }; echo \"script: $f\" | sudo tee /dev/ttyS0; bash \"$f\"\n")
    pause(20)
    stage("terminal", keep_duplicates=True)
    deadline = t() + 100 * 60
    started = t()
    net_up = False
    while t() < deadline and qemu.poll() is None and not serial_has("live", "===AGAMA-DONE"):
        if not net_up and serial_has("live", "===NET-UP-PLEASE"):
            set_net(True)
            net_up = True
        if t() - started > 240 and not serial_has("live", "===AGAMA-START"):
            print(f"[{t():6.1f}s] test script did not start", flush=True)
            stage("no-script", keep_duplicates=True)
            break
        wait_stages(t() + 10, 10, "install")
    for r in ("finished", "failed"):
        if serial_has("live", "===AGAMA-DONE " + r):
            result = r
    print(f"[{t():6.1f}s] Agama: {result}", flush=True)
    stage("install-end", keep_duplicates=True)
    stop()

    # ---------------------------------------------------------- phase 2 ----
    if result == "finished":
        qemu = start("installed", False)
        qmp = QMP(QMP_SOCK)
        wait_stages(t() + 300, 10, "firstboot")
        stage("firstboot", keep_duplicates=True)
        # What the installed system got: files from Windows, Setup state, autostart, journal.
        run_dialog("kgx")
        pause(10)
        type_text("echo shrimp | sudo -S true; { echo ===DIAG; ls -la ~ ~/Dokumenty ~/Documents; "
                  "sudo ls -la /var/lib/shrimp-setup; sudo cat /var/lib/shrimp-setup/install.json; "
                  "ls /etc/xdg/autostart /usr/etc/xdg/autostart; cat /etc/gdm/custom.conf; "
                  "plymouth-set-default-theme; ls /usr/share/plymouth/themes; gnome-extensions list --details; "
                  "echo ===MISSING; for f in /usr/share/fonts/truetype/inter /usr/share/grub2/themes/shrimp "
                  "/usr/share/plymouth/themes/shrimp /usr/share/shrimp /usr/share/icons/MoreWaita /usr/bin/cocktail /usr/bin/panko "
                  "/usr/bin/shrimp-apps; do [ -e $f ] || echo missing $f; done; "
                  "rpm -q typelib-1_0-GnomeDesktop-3_0 gvfs plymouth-plugin-script plymouth-plugin-label adwaita-icon-theme "
                  "| grep 'not installed'; echo ===FAILED; systemctl --failed --no-pager; "
                  "sudo journalctl -b --no-pager -o short-monotonic | grep -iE 'shrimp|welcome|JS ERROR|ding|typelib|localsearch' | tail -n 120; "
                  "echo ===DIAG-END; } 2>&1 | sudo tee /dev/ttyS0 >/dev/null\n")
        pause(20)
        stage("diag", keep_duplicates=True)
        stop()
except VMGone:
    print(f"[{t():6.1f}s] QEMU exited", flush=True)
finally:
    try:
        stop()
    except Exception:
        pass
    with open(os.path.join(OUT, "result.json"), "w") as f:
        json.dump({"agama": result, "guard": guard}, f, indent=1)
    for p in ("vars.fd", "qmp.sock", "disk.qcow2"):
        try:
            os.remove(os.path.join(OUT, p))
        except OSError:
            pass
    # The serial logs are only an artifact; print the parts worth reading into the job log too.
    try:
        with open(os.path.join(OUT, "serial-guard.log"), errors="replace") as f:
            glines = [l for l in f.read().splitlines() if "shrimp-setup" in l or "JS ERROR" in l]
        print("::group::serial-guard: Setup guard and Setup", flush=True)
        print("\n".join(glines[-80:]))
        print("::endgroup::", flush=True)
    except OSError:
        pass
    for phase, start_mark, end_mark in (("live", "=== offline: connectivity", "=== REPROBE"),
                                        ("live", "=== issues after config", "=== POST install"),
                                        ("live", "=== post-install / target", "===AGAMA-DONE"),
                                        ("installed", "===DIAG", "===DIAG-END")):
        try:
            with open(os.path.join(OUT, f"serial-{phase}.log"), errors="replace") as f:
                text = f.read()
        except OSError:
            continue
        lines = [l for l in text.splitlines() if "shrimp-post-install" in l] if phase == "live" else []
        a = text.rfind(start_mark)
        b = text.find(end_mark, a) if a >= 0 else -1
        print(f"::group::serial-{phase}: {start_mark}", flush=True)
        print("\n".join(lines[-60:]))
        if a >= 0:
            print(text[a:b if b > a else a + 20000][:20000])
        print("::endgroup::", flush=True)
    print(f"done, Agama {result}, Setup guard {guard}, logs in {OUT}", flush=True)
offline = "yes" if serial_has("live", "===OFFLINE-ISSUE yes") else "no"
reprobe = "ok" if serial_has("live", "=== REPROBE probe") else "FAILED"
nm_auth = "ok" if serial_has("live", "===NM-AUTH ok") else "FAILED"
print(f"live user can add a network connection without a password: {nm_auth}", flush=True)
print(f"offline repositories issue seen: {offline}; after network + probe: {reprobe}", flush=True)
if nm_auth != "ok":
    print("::error::On the live medium, connecting to a network asks for a password (polkit, NetworkManager)", flush=True)
    sys.exit(1)
if reprobe != "ok":
    print("::error::After the network came up, Agama still does not see the repositories (action probe)", flush=True)
    sys.exit(1)
if result != "finished":
    print(f"::error::Agama did not finish the installation ({result})", flush=True)
    sys.exit(1)
if guard == "click-blocked":
    print("::error::Setup guard on: a click did not reach the Setup window", flush=True)
    sys.exit(1)
