#!/usr/bin/env python3
"""Window-local X11 refresh comparison with synthetic processes only.

GNOME Terminal runs on a private D-Bus session with a private settings directory.
Only the new owned window is captured. Never run this against existing windows.
Pixels are X11 client-drawable observations, not physical-display timestamps.
"""
import argparse
import atexit
import csv
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import pty
import runpy
import select
import shlex
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
import tty
import fcntl

from PIL import Image, ImageChops

SELF = Path(__file__).resolve()
harness = runpy.run_path(str(SELF.with_name("gui-smoke.py")))
Window, command, wait_for = (harness[k] for k in ["Window", "command", "wait_for"])
COMMIT = "170ba6f83719ace935e9cf45085cced5b2c85db7"
DEFAULT_BINARY = Path("/home/patrick/Projekte/hoori-term-builds") / COMMIT / "launch"
PROFILE = "c4f5b2b8-c354-4f58-9175-490cfa7a4415"


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def write_all(data):
    remaining = memoryview(data)
    while remaining:
        count = os.write(1, remaining)
        assert count > 0
        remaining = remaining[count:]


def stats(pid):
    fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    rss = int(Path(f"/proc/{pid}/statm").read_text().split()[1]) * os.sysconf("SC_PAGE_SIZE")
    return {"cpu_seconds": (int(fields[11]) + int(fields[12])) / os.sysconf("SC_CLK_TCK"),
            "rss_mib": rss / 2**20}


def stop(process):
    if process and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=4)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=4)


def worker():
    C.CDLL(None).prctl(15, b"hoo15-fixture", 0, 0, 0)
    while True:
        start = time.monotonic()
        while time.monotonic() - start < 0.03:
            pass
        time.sleep(max(0, 0.10 - (time.monotonic() - start)))


def top_proxy(args):
    """Same nested PTY/timing tap for both terminals; no output contents logged."""
    output = args.output
    size = os.get_terminal_size(0)
    old = termios.tcgetattr(0)
    pid, master = pty.fork()
    if pid == 0:
        os.execv("/usr/bin/top", ["/usr/bin/top", "-d", str(args.interval), "-p", str(args.pid), "-w", str(size.columns)])
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", size.lines, size.columns, 0, 0))
    write_json(output / "top-process.json", {"pid": pid, "proxy_pid": os.getpid(),
               "shell_pid": os.getppid(), "rows": size.lines, "columns": size.columns,
               "interval_seconds": args.interval, "top_binary": "/usr/bin/top",
               "fixture_pid": args.pid})
    os.write(1, b"\x1b]2;HOO-15 top-ready\x07")
    carry = b""
    tty.setraw(0)
    try:
        with (output / "top-output-events.jsonl").open("w") as log:
            while True:
                ready, _, _ = select.select([0, master], [], [], 0.2)
                if master in ready:
                    try:
                        data = os.read(master, 65536)
                    except OSError:
                        break
                    if not data:
                        break
                    before = time.monotonic_ns()
                    count = (carry + data).count(b"\x1b[H")
                    carry = (carry + data)[-2:]
                    write_all(data)
                    log.write(json.dumps({"before_forward_ns": before, "after_forward_ns": time.monotonic_ns(),
                                          "bytes": len(data), "home_markers": count}) + "\n")
                    log.flush()
                if 0 in ready:
                    data = os.read(0, 4096)
                    # During observation only the explicit later interaction
                    # phase may send help/escape/quit to top. Unexpected input
                    # is counted, not recorded or forwarded.
                    allowed = (output / "allow-input").exists()
                    if allowed and data in [b"h", b"\x1b", b"q"]:
                        os.write(master, data)
                    else:
                        with (output / "unexpected-input-count.txt").open("a") as unexpected:
                            unexpected.write(str(len(data)) + "\n")
    finally:
        termios.tcsetattr(0, termios.TCSANOW, old)
        os.close(master)
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        os.waitpid(pid, 0)
        os.write(1, b"\x1b]2;HOO-15 top-done\x07")


def control(args):
    size = os.get_terminal_size(0)
    write_json(args.output / "control-process.json", {"pid": os.getpid(),
               "shell_pid": os.getppid(), "rows": size.lines, "columns": size.columns,
               "interval_seconds": args.interval})
    os.write(1, b"\x1b]2;HOO-15 control-ready\x07\x1b[?1049h\x1b[?25l\x1b[2J")
    start = time.monotonic()
    try:
        with (args.output / "control-emissions.jsonl").open("w") as log:
            for sequence in range(int(args.seconds / args.interval)):
                delay = start + sequence * args.interval - time.monotonic()
                if delay > 0:
                    time.sleep(delay)
                # Two spaces per bit, with a unique blue synchronization block.
                barcode = "\x1b[48;2;10;40;230m  "
                for bit in range(16):
                    color = "0;220;40" if sequence & (1 << bit) else "220;0;40"
                    barcode += f"\x1b[48;2;{color}m  "
                lines = [barcode + "\x1b[0m", f"HOO-15 SYNTHETIC FRAME {sequence:06d}"]
                lines += [f"controlled row {row:02d} frame {sequence:06d} " + "." * 55 for row in range(2, size.lines - 1)]
                frame = ("\x1b[H" + "\r\n".join(line[:size.columns] if not line.startswith("\x1b") else line for line in lines)).encode()
                before = time.monotonic_ns()
                write_all(frame)
                log.write(json.dumps({"sequence": sequence, "before_write_ns": before,
                                      "after_write_ns": time.monotonic_ns(), "bytes": len(frame)}) + "\n")
                log.flush()
    finally:
        os.write(1, b"\x1b[0m\x1b[?1049l\x1b[?25h\x1b]2;HOO-15 control-done\x07")


def setup_profile():
    assert os.environ.get("HOORI_COMPARE_PRIVATE_BUS") == "1"
    schema = f"org.gnome.Terminal.Legacy.Profile:/org/gnome/terminal/legacy/profiles:/:{PROFILE}/"
    for key, value in [("list", f"['{PROFILE}']"), ("default", f"'{PROFILE}'")]:
        command("gsettings", "set", "org.gnome.Terminal.ProfilesList", key, value)
    settings = {"visible-name": "'HOO-15 controlled'", "use-system-font": "false",
                "use-custom-command": "true", "custom-command": "'/usr/bin/zsh -f -i'",
                "font": "'DejaVu Sans Mono 12'", "cursor-blink-mode": "'off'",
                "scrollback-unlimited": "false", "scrollback-lines": "2000",
                "scrollbar-policy": "'never'", "use-theme-colors": "false",
                "background-color": "'rgb(22,24,35)'", "foreground-color": "'rgb(220,224,235)'"}
    for key, value in settings.items():
        command("gsettings", "set", schema, key, value)
    return settings


class GnomeWindow(Window):
    def __init__(self, output):
        self.output = output
        self.log = (output / "gnome-app-stderr.txt").open("w")
        self.process = None
        self.server = subprocess.Popen(["/usr/libexec/gnome-terminal-server"], stdout=self.log, stderr=self.log)
        atexit.register(self.cleanup)

        def server():
            value = command("gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
                            "--object-path", "/org/freedesktop/DBus", "--method",
                            "org.freedesktop.DBus.GetConnectionUnixProcessID", "org.gnome.Terminal", check=False)
            return int(value.split("uint32 ")[1].split(",")[0]) if "uint32 " in value else None

        self.pid = wait_for(server)
        assert self.pid == self.server.pid
        self.process = subprocess.Popen(["gnome-terminal", "--wait", "--hide-menubar", "--geometry=98x29",
                                         f"--profile={PROFILE}", "--title=HOO-15 comparator",
                                         "--", "/usr/bin/zsh", "-f", "-i"], stdout=self.log, stderr=self.log)
        self.id = wait_for(lambda: command("xdotool", "search", "--onlyvisible", "--pid", str(self.pid), check=False).splitlines())[-1]
        self.assert_owned()
        self.focus()
        time.sleep(0.6)

    def assert_owned(self):
        assert self.process.poll() is None
        value = command("xprop", "-id", self.id, "_NET_WM_PID")
        assert value.rsplit("=", 1)[-1].strip() == str(self.pid)

    def cleanup(self):
        stop(self.process)
        stop(self.server)
        self.log.close()


class XImage(C.Structure):
    _fields_ = [("width", C.c_int), ("height", C.c_int), ("xoffset", C.c_int), ("format", C.c_int),
                ("data", C.c_void_p), ("byte_order", C.c_int), ("bitmap_unit", C.c_int),
                ("bitmap_bit_order", C.c_int), ("bitmap_pad", C.c_int), ("depth", C.c_int),
                ("bytes_per_line", C.c_int), ("bits_per_pixel", C.c_int),
                ("red_mask", C.c_ulong), ("green_mask", C.c_ulong), ("blue_mask", C.c_ulong)]


class Capture:
    def __init__(self, window):
        self.window = window
        self.lib = C.CDLL("libX11.so.6")
        self.lib.XOpenDisplay.restype = C.c_void_p
        self.lib.XOpenDisplay.argtypes = [C.c_char_p]
        self.display = self.lib.XOpenDisplay(None)
        assert self.display
        self.lib.XGetImage.restype = C.POINTER(XImage)
        self.lib.XGetImage.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_ulong, C.c_int]
        self.lib.XDestroyImage.argtypes = [C.POINTER(XImage)]
        self.lib.XGetInputFocus.argtypes = [C.c_void_p, C.POINTER(C.c_ulong), C.POINTER(C.c_int)]
        self.lib.XQueryTree.argtypes = [C.c_void_p, C.c_ulong, C.POINTER(C.c_ulong), C.POINTER(C.c_ulong),
                                       C.POINTER(C.POINTER(C.c_ulong)), C.POINTER(C.c_uint)]
        self.lib.XFree.argtypes = [C.c_void_p]
        self.lib.XGetGeometry.argtypes = [C.c_void_p, C.c_ulong, C.POINTER(C.c_ulong), C.POINTER(C.c_int), C.POINTER(C.c_int),
                                         C.POINTER(C.c_uint), C.POINTER(C.c_uint), C.POINTER(C.c_uint), C.POINTER(C.c_uint)]
        self.lib.XCloseDisplay.argtypes = [C.c_void_p]
        root, x, y = C.c_ulong(), C.c_int(), C.c_int()
        width, height, border, depth = (C.c_uint() for _ in range(4))
        assert self.lib.XGetGeometry(self.display, int(window.id), C.byref(root), C.byref(x), C.byref(y),
                                    C.byref(width), C.byref(height), C.byref(border), C.byref(depth))
        self.width, self.height = width.value, height.value
        self.barcode = None

    def owns_focus(self):
        # GTK gives keyboard focus to a child X window. Check its ancestry,
        # rather than accepting an arbitrary other focused window.
        focus, revert = C.c_ulong(), C.c_int()
        self.lib.XGetInputFocus(self.display, C.byref(focus), C.byref(revert))
        current = focus.value
        for _ in range(16):
            if current == int(self.window.id):
                return True
            if current in (0, 1):
                return False
            root, parent, children, count = C.c_ulong(), C.c_ulong(), C.POINTER(C.c_ulong)(), C.c_uint()
            if not self.lib.XQueryTree(self.display, current, C.byref(root), C.byref(parent), C.byref(children), C.byref(count)):
                return False
            if children:
                self.lib.XFree(children)
            if current == root.value:
                return False
            current = parent.value
        return False

    def grab(self):
        assert self.owns_focus(), "Owned measurement window lost focus; capture stopped"
        assert self.window.process.poll() is None
        start = time.monotonic_ns()
        pointer = self.lib.XGetImage(self.display, int(self.window.id), 0, 0, self.width, self.height, C.c_ulong(-1), 2)
        assert pointer
        try:
            value = pointer.contents
            assert value.bits_per_pixel == 32 and value.byte_order == 0
            assert (value.red_mask, value.green_mask, value.blue_mask) == (0xFF0000, 0xFF00, 0xFF)
            data = C.string_at(value.data, value.bytes_per_line * value.height)
            picture = Image.frombytes("RGB", (value.width, value.height), data, "raw", "BGRX", value.bytes_per_line, 1)
            return picture, start, time.monotonic_ns()
        finally:
            self.lib.XDestroyImage(pointer)

    def sequence(self, picture):
        if self.barcode is None:
            red, green, blue = picture.split()
            mask = ImageChops.multiply(red.point(lambda p: 255 if p == 10 else 0),
                                      ImageChops.multiply(green.point(lambda p: 255 if p == 40 else 0),
                                                          blue.point(lambda p: 255 if p == 230 else 0)))
            box = mask.getbbox()
            if box is None:
                return None
            x, y, right, bottom = box
            self.barcode = (x, (y + bottom) // 2, right - x)
        x, y, width = self.barcode
        sequence = 0
        for bit in range(16):
            red, green, blue = picture.getpixel((int(x + (bit + 1.5) * width), y))
            if green > 180 and red < 40:
                sequence |= 1 << bit
            elif not (red > 180 and green < 40):
                return None
        return sequence

    def close(self):
        self.lib.XCloseDisplay(self.display)


def observe(window, output, name, seconds, pids, frequency, counter=False):
    capture = Capture(window)
    window.assert_owned()
    start = time.monotonic()
    next_sample = start
    rows, resources, pictures = [], [], []
    next_resource = start
    last_hash = None
    try:
        while time.monotonic() - start < seconds:
            delay = next_sample - time.monotonic()
            if delay > 0:
                time.sleep(delay)
            picture, before, after = capture.grab()
            digest = hashlib.sha256(picture.tobytes()).hexdigest()
            sequence = capture.sequence(picture) if counter else None
            rows.append({"capture_start_ns": before, "capture_end_ns": after, "elapsed_seconds": before / 1e9 - start,
                         "capture_ms": (after - before) / 1e6, "sha256": digest,
                         "changed": int(digest != last_hash), "sequence": sequence})
            last_hash = digest
            if len(pictures) < 3 and before / 1e9 - start >= len(pictures) * max(1, seconds / 2 - 0.1):
                pictures.append(picture.copy())
            if time.monotonic() >= next_resource:
                resources.append({"monotonic_ns": time.monotonic_ns(),
                                  "processes": {label: stats(pid) for label, pid in pids.items()}})
                next_resource += 1
            next_sample += 1 / frequency
        resources.append({"monotonic_ns": time.monotonic_ns(),
                          "processes": {label: stats(pid) for label, pid in pids.items()}})
    finally:
        capture.close()
    with (output / f"{name}-samples.csv").open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)
    write_json(output / f"{name}-resources.json", resources)
    for index, picture in enumerate(pictures):
        picture.save(output / f"{name}-{index:02d}.png")
    duration = (resources[-1]["monotonic_ns"] - resources[0]["monotonic_ns"]) / 1e9
    usage = {}
    for label in pids:
        samples = [r["processes"][label] for r in resources]
        usage[label] = {"cpu_percent_one_core": 100 * (samples[-1]["cpu_seconds"] - samples[0]["cpu_seconds"]) / duration,
                        "rss_mib_min": min(s["rss_mib"] for s in samples), "rss_mib_max": max(s["rss_mib"] for s in samples)}
    summary = {"capture_span_seconds": (rows[-1]["capture_end_ns"] - rows[0]["capture_start_ns"]) / 1e9,
               "resource_span_seconds": duration, "requested_seconds": seconds, "samples": len(rows), "resources": usage,
               "capture_size": [capture.width, capture.height], "poll_hz": frequency}
    write_json(output / f"{name}-summary.json", summary)
    print(name, json.dumps(summary), flush=True)
    return summary


def check_interaction(window, output, terminal, process):
    """Qualitative checks after measurement; their times are not latency data."""
    window.key("ctrl+shift+t")
    wait_for(lambda: "HOO-15 top-ready" not in window.title())
    proof = output / "secondary-shell.pid"
    window.run("unset HISTFILE; PS1='SECOND> '; printf '%s' $$ > " + shlex.quote(str(proof)) +
               "; printf '\\033]2;HOO-15 secondary-ready\\007'")
    wait_for(lambda: proof.exists() and "HOO-15 secondary-ready" in window.title())
    secondary = int(proof.read_text())
    assert secondary != process["shell_pid"]
    assert Path(f"/proc/{secondary}").exists()
    window.screenshot(output / "secondary-tab-response.png")
    window.key("ctrl+shift+Tab" if terminal == "hoori" else "ctrl+Page_Up")
    wait_for(lambda: "HOO-15 top-ready" in window.title())
    assert Path(f"/proc/{process['pid']}").exists()
    time.sleep(0.3)
    window.screenshot(output / "top-after-tab-return.png")
    (output / "allow-input").touch()
    window.key("h")
    time.sleep(0.3)
    window.screenshot(output / "top-help-response.png")
    window.key("Escape")
    time.sleep(0.3)
    window.key("q")
    wait_for(lambda: "HOO-15 top-done" in window.title())
    window.key("ctrl+Tab" if terminal == "hoori" else "ctrl+Page_Down")
    wait_for(lambda: "HOO-15 secondary-ready" in window.title())
    window.run("exit")
    wait_for(lambda: not Path(f"/proc/{secondary}").exists())
    if terminal == "hoori":
        window.key("ctrl+shift+w")
    wait_for(lambda: "HOO-15 top-done" in window.title())
    write_json(output / "interaction.json", {"new_tab_and_return": True, "top_survived_inactive_tab": True,
               "help_response_requires_visual_check": "top-help-response.png", "quit_returned_to_shell": True,
               "secondary_shell_pid": secondary, "secondary_shell_reaped": True,
               "note": "Qualitative post-measurement check, no response-time claim"})


def run_one(args):
    assert os.environ.get("XDG_SESSION_TYPE") == "x11"
    assert not (Path.home() / ".toprc").exists(), "Legacy top config requires a separately controlled setup"
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    for name in ["GNOME_TERMINAL_SERVICE", "GNOME_TERMINAL_SCREEN"]:
        os.environ.pop(name, None)
    os.environ.update({"XDG_CONFIG_HOME": str(output / "config"), "TERM": "xterm-256color", "LC_ALL": "C.UTF-8",
                       "GDK_BACKEND": "x11", "GDK_SCALE": "1", "GDK_DPI_SCALE": "1", "NO_AT_BRIDGE": "1"})
    metadata = {"terminal": args.terminal, "os": Path("/etc/os-release").read_text(),
                "display": os.environ.get("DISPLAY"), "session_type": os.environ.get("XDG_SESSION_TYPE"),
                "top_version": command("top", "-V"), "shell_version": command("zsh", "--version"),
                "binary_metadata": command(str(args.binary), "--version"), "worker_duty_cycle": 0.30,
                "top_sha256": hashlib.sha256(Path("/usr/bin/top").read_bytes()).hexdigest(),
                "harness_sha256": hashlib.sha256(SELF.read_bytes()).hexdigest(),
                "font": "DejaVu Sans Mono, 16px (hoori), 12pt at 96dpi (GNOME)",
                "sample_hz": args.frequency, "requested_grid": [98, 29], "warmup_seconds": args.warmup,
                "locale": "C.UTF-8", "measured_tab_count": 1, "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
    assert f"commit: {COMMIT}" in metadata["binary_metadata"] and "dirty: false" in metadata["binary_metadata"]
    if args.terminal == "gnome":
        metadata["gnome_version"] = command("gnome-terminal", "--version")
        metadata["private_profile"] = setup_profile()
    write_json(output / "metadata.json", metadata)
    fixture = subprocess.Popen([sys.executable, str(SELF), "worker"])
    window = None
    complete = False
    owned_pids = [fixture.pid]
    try:
        window = GnomeWindow(output) if args.terminal == "gnome" else Window(args.binary.resolve(), output, "/usr/bin/zsh", ["-f", "-i"])
        pid = window.pid if args.terminal == "gnome" else window.process.pid
        owned_pids.append(pid)
        metadata.update({"terminal_pid": pid, "window_id": window.id, "fixture_pid": fixture.pid})
        window.run("unset HISTFILE; PS1='TEST> '; printf '\\033[?25l\\033]2;HOO-15 idle-ready\\007'")
        wait_for(lambda: "HOO-15 idle-ready" in window.title())
        metadata["idle_title"] = window.title()
        if args.terminal == "hoori":
            assert f"[{COMMIT[:7]}]" in metadata["idle_title"]
        time.sleep(args.warmup)
        observe(window, output, "idle", args.idle, {"terminal": pid, "observer": os.getpid(), "fixture": fixture.pid}, args.frequency)
        launch = f"python3 {shlex.quote(str(SELF))} proxy {shlex.quote(str(output))} --pid {fixture.pid} --interval {args.interval}"
        window.run(launch)
        wait_for(lambda: (output / "top-process.json").exists())
        process = json.loads((output / "top-process.json").read_text())
        owned_pids.extend(process[key] for key in ["pid", "proxy_pid", "shell_pid"])
        assert (process["columns"], process["rows"]) == (98, 29), process
        wait_for(lambda: "HOO-15 top-ready" in window.title())
        metadata["top_title"] = window.title()
        time.sleep(args.warmup)
        observe(window, output, "top", args.seconds, {"terminal": pid, "top": process["pid"], "proxy": process["proxy_pid"],
                                                     "shell": process["shell_pid"], "observer": os.getpid(), "fixture": fixture.pid}, args.frequency)
        assert not (output / "unexpected-input-count.txt").exists(), "Unexpected input invalidates the controlled run"
        check_interaction(window, output, args.terminal, process)
        if args.control:
            launch = f"python3 {shlex.quote(str(SELF))} control {shlex.quote(str(output))} --seconds {args.control + args.warmup + 2} --interval 0.1"
            window.run(launch)
            wait_for(lambda: "HOO-15 control-ready" in window.title())
            time.sleep(args.warmup)
            control_process = json.loads((output / "control-process.json").read_text())
            owned_pids.append(control_process["pid"])
            assert (control_process["columns"], control_process["rows"]) == (98, 29)
            observe(window, output, "control", args.control, {"terminal": pid, "generator": control_process["pid"],
                                                             "observer": os.getpid(), "fixture": fixture.pid}, args.frequency, counter=True)
            wait_for(lambda: "HOO-15 control-done" in window.title())
        metadata["final_title"] = window.title()
        if args.terminal == "hoori":
            assert all(f"[{COMMIT[:7]}]" in metadata[key] for key in ["idle_title", "top_title", "final_title"])
        metadata["finished_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        write_json(output / "metadata.json", metadata)
        window.run("exit")
        if args.terminal == "hoori":
            wait_for(lambda: not Path(f"/proc/{process['shell_pid']}").exists())
            window.request_close()
        window.wait_exit()
        complete = True
    finally:
        if window:
            window.cleanup()
        stop(fixture)
        # GTK/PulseAudio can create incidental cookies in the private config.
        # The explicit profile is already in metadata; keep no credentials.
        shutil.rmtree(output / "config", ignore_errors=True)
        if complete:
            wait_for(lambda: all(not Path(f"/proc/{pid}").exists() for pid in owned_pids))
            write_json(output / "COMPLETE.json", {"complete": True, "terminal": args.terminal,
                       "owned_processes_reaped": True, "owned_pids": owned_pids})


def distribution(values):
    values = sorted(values)
    if not values:
        return None
    def percentile(fraction):
        position = (len(values) - 1) * fraction
        low = int(position)
        return values[low] + (values[min(low + 1, len(values) - 1)] - values[low]) * (position - low)
    return {"count": len(values), "min": values[0], "p50": percentile(0.5), "p95": percentile(0.95), "max": values[-1]}


def analyze_run(output):
    """Summarize drawable changes; deliberately no physical latency estimate."""
    assert json.loads((output / "COMPLETE.json").read_text())["complete"]
    assert not (output / "unexpected-input-count.txt").exists()
    result = {"metadata": json.loads((output / "metadata.json").read_text()), "phases": {}}
    for name in ["idle", "top", "control"]:
        source = output / f"{name}-samples.csv"
        if not source.exists():
            continue
        rows = list(csv.DictReader(source.open()))
        times = [int(row["capture_start_ns"]) for row in rows]
        end = int(rows[-1]["capture_end_ns"])
        changes = [int(row["capture_start_ns"]) for row in rows[1:] if row["changed"] == "1"]
        phase = {"summary": json.loads((output / f"{name}-summary.json").read_text()),
                 "observed_changes": len(changes),
                 "observed_changes_per_second": len(changes) / ((end - times[0]) / 1e9),
                 "change_intervals_ms": distribution([(b - a) / 1e6 for a, b in zip(changes, changes[1:])]),
                 "sample_intervals_ms": distribution([(b - a) / 1e6 for a, b in zip(times, times[1:])]),
                 "capture_ms": distribution([float(row["capture_ms"]) for row in rows])}
        if name == "top":
            events = [json.loads(line) for line in (output / "top-output-events.jsonl").read_text().splitlines()]
            events = [event for event in events if times[0] <= event["before_forward_ns"] <= end]
            homes = [event["before_forward_ns"] for event in events if event["home_markers"]]
            phase.update({"pty_bytes_in_window": sum(event["bytes"] for event in events),
                          "pty_home_markers_in_window": sum(event["home_markers"] for event in events),
                          "pty_home_event_intervals_ms": distribution([(b - a) / 1e6 for a, b in zip(homes, homes[1:])])})
        if name == "control":
            emissions = {event["sequence"]: event for event in map(json.loads, (output / "control-emissions.jsonl").read_text().splitlines())}
            distinct, transitions, lower, upper = [], [], [], []
            missing = 0
            previous = None
            for row in rows:
                if not row["sequence"]:
                    missing += 1
                    previous = row
                    continue
                sequence = int(row["sequence"])
                assert sequence in emissions, "Decoded barcode does not correspond to an emitted frame"
                if not distinct or sequence != distinct[-1]:
                    distinct.append(sequence)
                    transitions.append(int(row["capture_start_ns"]))
                    if previous is not None:
                        emission = emissions[sequence]
                        # The barcode may become visible anywhere between the
                        # preceding and current capture, and write() also spans
                        # an interval. Keep those bounds rather than midpoint
                        # estimates. The very first observed frame is omitted.
                        lower.append(max(0, int(previous["capture_start_ns"]) - emission["after_write_ns"]) / 1e6)
                        upper.append((int(row["capture_end_ns"]) - emission["before_write_ns"]) / 1e6)
                previous = row
            interior = {seq for seq, event in emissions.items() if times[0] <= event["before_write_ns"] < end - 200_000_000}
            phase.update({"decoded_distinct_frames": len(distinct), "undecodable_samples": missing,
                          "skipped_or_backwards_transitions": sum(b != a + 1 for a, b in zip(distinct, distinct[1:])),
                          "unobserved_interior_emissions": sorted(interior - set(distinct)),
                          "decoded_frame_intervals_ms": distribution([(b - a) / 1e6 for a, b in zip(transitions, transitions[1:])]),
                          "drawable_delay_lower_bound_ms": distribution(lower), "drawable_delay_upper_bound_ms": distribution(upper),
                          "delay_note": "Interval-censored first barcode visibility, not physical display latency or full-frame completion"})
        result["phases"][name] = phase
    write_json(output / "analysis.json", result)
    return result


def suite(args):
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    order = ["hoori", "gnome", "gnome", "hoori", "hoori", "gnome"]
    manifest = {"order": order, "runs": [], "complete": False,
                "note": "One terminal at a time; synthetic control included in the first run of each terminal"}
    write_json(output / "suite.json", manifest)
    for index, terminal in enumerate(order):
        name = f"{index + 1:02d}-{terminal}"
        destination = output / name
        print("Starting", name, flush=True)
        control_seconds = args.control if index < 2 else 0
        with (output / f"{name}.log").open("w") as log:
            child = subprocess.Popen([sys.executable, str(SELF), "run", terminal, str(destination),
                                      "--seconds", str(args.seconds), "--interval", str(args.interval),
                                      "--idle", str(args.idle), "--warmup", str(args.warmup),
                                      "--control", str(control_seconds), "--frequency", str(args.frequency)], stdout=log, stderr=log)
            try:
                code = child.wait()
            finally:
                stop(child)
        manifest["runs"].append({"name": name, "terminal": terminal, "returncode": code})
        write_json(output / "suite.json", manifest)
        assert code == 0, f"{name} incomplete; preserve its log and use a fresh suite directory for a repeat"
        analyze_run(destination)
        print("Completed", name, flush=True)
    manifest["complete"] = True
    write_json(output / "suite.json", manifest)
    write_json(output / "analysis.json", {"runs": [json.loads((output / run["name"] / "analysis.json").read_text()) for run in manifest["runs"]]})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    sub.add_parser("worker")
    proxy = sub.add_parser("proxy")
    proxy.add_argument("output", type=Path)
    proxy.add_argument("--pid", type=int, required=True)
    proxy.add_argument("--interval", type=float, default=0.2)
    synthetic = sub.add_parser("control")
    synthetic.add_argument("output", type=Path)
    synthetic.add_argument("--seconds", type=float, default=40)
    synthetic.add_argument("--interval", type=float, default=0.1)
    run = sub.add_parser("run")
    run.add_argument("terminal", choices=["hoori", "gnome"])
    run.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    batch = sub.add_parser("suite")
    for target in [run, batch]:
        target.add_argument("output", type=Path)
        target.add_argument("--seconds", type=float, default=60)
        target.add_argument("--interval", type=float, default=0.2)
        target.add_argument("--idle", type=float, default=10)
        target.add_argument("--control", type=float, default=30 if target is batch else 0)
        target.add_argument("--warmup", type=float, default=5)
        target.add_argument("--frequency", type=float, default=20)
    analysis = sub.add_parser("analyze")
    analysis.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.action == "worker":
        worker()
    elif args.action == "proxy":
        top_proxy(args)
    elif args.action == "control":
        assert 0 < args.seconds / args.interval < 65536
        control(args)
    elif args.action == "analyze":
        if (args.output / "suite.json").exists():
            manifest = json.loads((args.output / "suite.json").read_text())
            assert manifest["complete"]
            write_json(args.output / "analysis.json", {"runs": [analyze_run(args.output / run["name"]) for run in manifest["runs"]]})
        else:
            analyze_run(args.output)
    elif args.action == "suite":
        assert args.seconds >= 1 and args.idle >= 1 and args.interval > 0 and args.warmup >= 0
        assert 1 <= args.frequency <= 100 and 0 <= args.control <= 3600
        suite(args)
    elif args.action == "run":
        assert args.seconds >= 1 and args.idle >= 1 and args.interval > 0 and args.warmup >= 0
        assert 1 <= args.frequency <= 100 and 0 <= args.control <= 3600
        if args.terminal == "gnome" and os.environ.get("HOORI_COMPARE_PRIVATE_BUS") != "1":
            environment = dict(os.environ, HOORI_COMPARE_PRIVATE_BUS="1", XDG_CONFIG_HOME=str(args.output.resolve() / "config"),
                               GDK_BACKEND="x11", GDK_SCALE="1", GDK_DPI_SCALE="1", NO_AT_BRIDGE="1",
                               GTK_USE_PORTAL="0", GIO_USE_VFS="local", GSETTINGS_BACKEND="keyfile")
            for name in ["GNOME_TERMINAL_SERVICE", "GNOME_TERMINAL_SCREEN"]:
                environment.pop(name, None)
            # No service autoactivation: opening a comparator must not start
            # portals/keyrings or connect to an existing terminal server.
            with tempfile.TemporaryDirectory(prefix="hoo15-bus-") as directory:
                config = Path(directory) / "bus.conf"
                config.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen>'
                                  '<auth>EXTERNAL</auth><policy context="default">'
                                  '<allow send_destination="*"/><allow receive_sender="*"/>'
                                  '<allow own="*"/></policy></busconfig>')
                raise SystemExit(subprocess.call(["dbus-run-session", f"--config-file={config}", "--",
                                                  sys.executable, str(SELF), *sys.argv[1:]], env=environment))
        run_one(args)


if __name__ == "__main__":
    main()
