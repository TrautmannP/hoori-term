#!/usr/bin/env python3
"""Qualify the current Ubuntu/X11 baseline with owned, synthetic sessions.

Both shell rows use the same clean binary. Inspect every image independently
of byte, PID, title and protocol assertions. Never attaches to an existing tab.
"""
import argparse
import atexit
import base64
import json
import os
from pathlib import Path
import re
import runpy
import shlex
import shutil
import subprocess
import time

helpers = runpy.run_path(str(Path(__file__).with_name("gui-templates-smoke.py")))
Window, command, wait_for, click, start_time = (
    helpers[name] for name in ("Window", "command", "wait_for", "click", "start_time")
)
identity = helpers["base"]
DA = b"\x1b[?6c"


FIXTURE = r'''import base64,json,os,select,signal,sys,termios,time,tty
from pathlib import Path
root=Path(sys.argv[1]); tab=int(sys.argv[2])
old=termios.tcgetattr(0); tty.setraw(0)
state={"pid":os.getpid(),"parent":os.getppid(),"tab":tab,"sequence":0,
       "input_hex":"","queries":[],"input_events":[],"complete":False}
data=bytearray()
def save():
    temporary=root/"proof.tmp"
    temporary.write_text(json.dumps(state,ensure_ascii=False,indent=2))
    temporary.replace(root/"proof.json")
def title(value):
    os.write(1,b"\x1b]2;"+value.encode()+b"\x07")
def paint():
    os.write(1,b"\x1b[2J\x1b[H"+"COPY: Grüße 界\nOwned HOO-10 clipboard fixture\n".encode())
    title(f"HOO10 raw-{tab}")
def query(sequence):
    os.write(1,b"\x1b[c")
    received=bytearray(); deadline=time.monotonic()+12
    while not received.endswith(b"c"):
        if time.monotonic()>deadline: raise RuntimeError("No DA response from own PTY")
        if select.select([0],[],[],.1)[0]: received.extend(os.read(0,4096))
    state["queries"].append({"sequence":sequence,"hex":received.hex(),"at":time.monotonic()})
    if bytes(received)!=b"\x1b[?6c": raise RuntimeError("Unexpected DA/input response")
os.write(1,b"\x1b[?2004h")
paint(); save()
try:
    while True:
        try: instruction=json.loads((root/"command.json").read_text())
        except (OSError,ValueError): instruction=None
        if instruction and instruction["sequence"]>state["sequence"]:
            sequence=instruction["sequence"]; op=instruction["op"]
            if op=="stop": state["sequence"]=sequence; state["complete"]=True; save(); break
            if op=="load":
                state["load_started_at"]=time.monotonic(); save()
                count=instruction["count"]
                os.write(1,b"\x1b[2J\x1b[H")
                for first in range(0,count,50):
                    block="".join(f"HOO10 inactive {i:06d} abcdefghijklmnopqrstuvwxyz\n"
                                  for i in range(first,min(first+50,count)))
                    os.write(1,block.encode())
                    time.sleep(.01)
                title(f"HOO10 inactive-{tab}-done")
                query(sequence)
                state["load_count"]=count; state["load_done_at"]=time.monotonic()
            elif op=="paint": paint(); query(sequence)
            elif op=="emit":
                os.write(1,base64.b64decode(instruction["bytes"])); query(sequence)
            else: raise RuntimeError("Unknown fixture command")
            state["sequence"]=sequence; save()
        if select.select([0],[],[],.02)[0]:
            chunk=os.read(0,4096)
            if not chunk: break
            data.extend(chunk); state["input_hex"]=data.hex()
            state["input_events"].append({"hex":chunk.hex(),"at":time.monotonic()}); save()
except BaseException as error:
    state["error"]=repr(error); save(); raise
finally:
    termios.tcsetattr(0,termios.TCSANOW,old)
    os.write(1,b"\x1b[?2004l")
    title(f"HOO10 raw-{tab}-stopped")
'''


def clipboard(value):
    subprocess.run(["xclip", "-selection", "clipboard", "-in"], input=value.encode(),
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def send(w, value):
    # Only our generated command replaces the clipboard. Key events still go
    # through the owned native window and its ordinary paste/input path.
    clipboard(value)
    w.key("ctrl+shift+v")
    time.sleep(.12)
    w.key("Return")


def read_json(path):
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def geometry(w):
    w.assert_owned()
    values = dict(line.split("=", 1) for line in command(
        "xdotool", "getwindowgeometry", "--shell", w.id).splitlines())
    return {key: int(values[key]) for key in ("X", "Y", "WIDTH", "HEIGHT")}


def resize(w, width, height):
    w.focus()
    command("xdotool", "windowsize", "--sync", w.id, str(width), str(height))
    wait_for(lambda: (geometry(w)["WIDTH"], geometry(w)["HEIGHT"]) == (width, height))
    time.sleep(.25)
    return geometry(w)


class DefaultWindow(Window):
    """Use the real no-argument launcher, retaining the ownership guards."""
    def __init__(self, binary, output):
        self.output = output
        self.log = (output / "default-app-stderr.txt").open("w")
        self.process = subprocess.Popen([str(binary)], stdout=self.log, stderr=self.log)
        atexit.register(self.cleanup)
        self.id = wait_for(lambda: command(
            "xdotool", "search", "--onlyvisible", "--pid", str(self.process.pid),
            check=False).splitlines())[-1]
        self.assert_owned()
        wait_for(lambda: self.title().startswith("hoori-term ["))
        self.focus()
        time.sleep(.6)


class Qualification:
    def __init__(self, binary, output):
        self.binary, self.output = binary, output
        self.windows, self.owned = [], {}
        self.report = {
            "metadata": command(str(binary), "--version"),
            "display": os.environ.get("DISPLAY"), "session_type": os.environ.get("XDG_SESSION_TYPE"),
            "wayland_display": os.environ.get("WAYLAND_DISPLAY"),
            "os": Path("/etc/os-release").read_text(),
            "kernel": command("uname", "-r"), "desktop": command("gnome-shell", "--version"),
            "x_server": command("xdpyinfo"),
            "monitors": command("xrandr", "--current"),
            "gpu": (command("nvidia-smi", "--query-gpu=name,driver_version", "--format=csv,noheader")
                    if shutil.which("nvidia-smi") else "See the actual compositor adapter below."),
            "zsh": command("/usr/bin/zsh", "--version"),
            "bash": command("/bin/bash", "--version").splitlines()[0],
            "vim": command("vim", "--version").splitlines()[0],
            "text_scale": command("gsettings", "get", "org.gnome.desktop.interface", "text-scaling-factor"),
            "scaling_factor": command("gsettings", "get", "org.gnome.desktop.interface", "scaling-factor"),
            "screenshots": [], "phases": [], "rows": {}, "complete": False,
            "clipboard_policy": "Only own generated fixtures replace/read the clipboard; old contents are never read.",
            "input_policy": "Newly spawned PID, _NET_WM_PID and native active/focus ownership checked before input/capture.",
        }
        assert "dirty: false" in self.report["metadata"]
        self.commit = re.search(r"commit: ([0-9a-f]{40})", self.report["metadata"]).group(1)
        self.report["commit"] = self.commit
        self.fixture = output / "raw-fixture.py"
        self.fixture.write_text(FIXTURE)

    def phase(self, name):
        self.report["phases"].append(name)
        (self.output / "progress.json").write_text(json.dumps(self.report, ensure_ascii=False, indent=2))
        print(json.dumps({"phase": name}), flush=True)

    def children(self, w):
        ids = helpers["children"](w)
        for pid in ids:
            self.owned.setdefault(pid, start_time(pid))
        return ids

    def create(self, folder, shell=None):
        folder.mkdir()
        if shell:
            executable = "/usr/bin/zsh" if shell == "zsh" else "/bin/bash"
            args = ["-f", "-i"] if shell == "zsh" else ["--noprofile", "--norc", "-i"]
            w = Window(self.binary, folder, executable, args)
        else:
            w = DefaultWindow(self.binary, folder)
        self.windows.append(w)
        assert f"hoori-term [{self.commit[:7]}]" in w.title()
        monitors = self.report["monitors"]
        primary = re.search(r"\S+ connected primary (\d+)x(\d+)([+-]\d+)([+-]\d+)", monitors)
        if primary:
            command("xdotool", "windowmove", "--sync", w.id,
                    str(int(primary.group(3)) + 40), str(int(primary.group(4)) + 40))
        self.children(w)
        return w

    def shot(self, w, path):
        w.assert_owned()
        command("xdotool", "mousemove", "--window", w.id, "500", "350")
        time.sleep(.2)
        w.screenshot(path)
        self.report["screenshots"].append(str(path.relative_to(self.output)))

    def graphics(self, w):
        logs = list(w.output.glob("*-app-stderr.txt"))
        text = wait_for(lambda: next((p.read_text() for p in logs
                                      if "hoori-term graphics: Selected:" in p.read_text()), None))
        adapter = re.search(r'name: "([^"]+)"', text).group(1)
        backend = re.search(r"backend: (\w+)", text).group(1)
        # This is the actual selected compositor adapter, not merely an
        # installed-device inventory or an inferred default.
        return {"adapter": adapter, "backend": backend, "diagnostics": text,
                "native_x11_window": w.id,
                "native_x11_pid": command("xprop", "-id", w.id, "_NET_WM_PID"),
                "geometry": geometry(w)}

    def clean_prompt(self, w, shell, role):
        hooks = ("PROMPT_COMMAND=;" if shell == "bash" else
                 "precmd_functions=(); preexec_functions=(); unfunction precmd preexec 2>/dev/null;")
        send(w, f"unset HISTFILE; {hooks} PS1='HOO10> '; HOO10_ROLE={role}; "
                "printf '\\033[2J\\033[HOwned HOO-10 terminal fixture\\n'; "
                f"printf '\\033]2;HOO10 {role}-ready\\007'")
        wait_for(lambda: f"HOO10 {role}-ready" in w.title())

    def first_command(self, w, folder):
        proof = folder / "first-command.txt"
        send(w, "printf '%s\\n' HOO10_FIRST_COMMAND | tee " + shlex.quote(str(proof)))
        wait_for(lambda: proof.exists() and proof.read_bytes() == b"HOO10_FIRST_COMMAND\n")
        return str(proof.relative_to(self.output))

    def marker(self, w, marker):
        send(w, f"printf '\\033]2;{marker}\\007'")
        wait_for(lambda: marker in w.title())

    def default_probe(self, normal=False):
        self.phase("ordinary-default-launch" if normal else "graphics-probe")
        folder = self.output / ("ordinary-default" if normal else "graphics-probe")
        old = os.environ.get("HOORI_GRAPHICS_INFO")
        if normal:
            os.environ.pop("HOORI_GRAPHICS_INFO", None)
        try:
            w = self.create(folder)
            pids = self.children(w)
            assert len(pids) == 1
            assert Path(f"/proc/{pids[0]}/exe").resolve() == Path("/usr/bin/zsh").resolve()
            first_command = self.first_command(w, folder)
            self.clean_prompt(w, "zsh", "DEFAULT")
            self.shot(w, folder / "01-default-launch.png")
            if normal:
                stderr = (folder / "default-app-stderr.txt").read_text()
                assert "hoori-term graphics:" not in stderr
                assert "Owned HOO-10 terminal fixture" not in stderr
                self.report["ordinary_default_launch"] = {"shell_pid": pids[0],
                    "graphics_diagnostics_enabled": False, "stderr_bytes": len(stderr.encode()),
                    "synthetic_terminal_content_not_logged": True, "title": w.title(),
                    "first_command_proof": first_command}
            else:
                self.report["graphics_probe"] = self.graphics(w)
                self.report["graphics_probe"]["first_command_proof"] = first_command
            w.request_close()
            assert w.process.poll() is None
            w.key("Escape")
            assert self.children(w) == pids
            w.request_close()
            w.key("Return")
            w.wait_exit()
            wait_for(lambda: all(start_time(pid) != self.owned[pid] for pid in pids))
        finally:
            if old is None:
                os.environ.pop("HOORI_GRAPHICS_INFO", None)
            else:
                os.environ["HOORI_GRAPHICS_INFO"] = old

    def startup_probe(self):
        self.phase("menu-zsh-startup")
        folder = self.output / "menu-startup"
        w = self.create(folder, "bash")
        before = self.children(w)
        assert len(before) == 1
        w.key("ctrl+shift+l")
        w.key("End")
        w.key("Tab")
        w.key("Tab")
        w.key("Return")
        wait_for(lambda: "Neue Sitzung" in w.title())
        assert self.children(w) == before
        w.key("Return")  # Explicitly select zsh, initially focused.
        wait_for(lambda: "zsh · 2" in w.title())
        pids = self.children(w)
        assert len(pids) == 2
        pid = (set(pids) - set(before)).pop()
        assert Path(f"/proc/{pid}/exe").resolve() == Path("/usr/bin/zsh").resolve()
        first_command = self.first_command(w, folder)
        self.shot(w, folder / "01-menu-zsh-first-command.png")
        self.clean_prompt(w, "zsh", "MENU")
        self.report["menu_zsh_first_command"] = {"shell_pid": pid,
            "first_command_proof": first_command, "graphics": self.graphics(w)}
        w.request_close()
        w.key("Escape")
        assert self.children(w) == pids
        w.request_close()
        w.key("Return")
        w.wait_exit()
        wait_for(lambda: all(start_time(pid) != self.owned[pid] for pid in pids))

    def shell_row(self, shell):
        self.phase(shell + "-row-start")
        folder = self.output / shell
        w = self.create(folder, shell)
        row = {"graphics": self.graphics(w), "sizes": [], "input_hex": {}, "checks": {}}
        self.report["rows"][shell] = row
        primary_pid = self.children(w)[0]
        self.clean_prompt(w, shell, "ONE")
        probe = ("printf '\\033[2J\\033[HUTF-8: Gr\\u00fc\\u00dfe \\u754c e\\u0301\\n"
                 "\\033[31mROT \\033[32mGR\\u00dcN \\033[34mBLAU\\033[0m\\n"
                 "Cursor: ab\\033[2DOK\\n'; printf '\\033]2;HOO10 vt-ready\\007'")
        send(w, probe)
        wait_for(lambda: "HOO10 vt-ready" in w.title())
        self.shot(w, folder / "01-unicode-colors-cursor.png")
        row["checks"]["real_shell_unicode_color_cursor"] = True

        proof = folder / "vim-proof.txt"
        send(w, "vim -Nu NONE -n " + shlex.quote(str(proof)))
        time.sleep(.8)
        w.type("iVim proof: Grüße 界")
        w.key("Escape")
        w.type(":w")
        w.key("Return")
        wait_for(lambda: proof.exists() and proof.read_bytes() == "Vim proof: Grüße 界\n".encode())
        for width, height in [(1180, 780), (860, 600), (1100, 740)]:
            row["sizes"].append(resize(w, width, height))
        self.shot(w, folder / "02-vim-alternate-screen-resize.png")
        w.type(":q")
        w.key("Return")
        self.marker(w, "HOO10 back-from-vim")
        assert proof.read_bytes() == "Vim proof: Grüße 界\n".encode()
        row["checks"]["vim_real_unicode_file_alternate_resize_return"] = True
        dimensions = []
        for width, height in [(1000, 700), (1280, 780)]:
            row["sizes"].append(resize(w, width, height))
            size = folder / f"stty-{width}.txt"
            send(w, "stty size > " + shlex.quote(str(size)) +
                 f"; printf '\\033]2;HOO10 size-{width}\\007'")
            wait_for(lambda: size.exists() and f"HOO10 size-{width}" in w.title())
            dimensions.append(tuple(int(n) for n in size.read_text().split()))
        assert all(rows > 0 and cols > 0 for rows, cols in dimensions)
        assert dimensions[1][0] > dimensions[0][0] and dimensions[1][1] > dimensions[0][1]
        row["pty_sizes"] = dimensions
        self.shot(w, folder / "03-shell-after-vim-pty-size.png")
        resize(w, 1000, 700)

        w.key("ctrl+shift+t")
        wait_for(lambda: "· 2" in w.title())
        self.clean_prompt(w, shell, "TWO")
        before = self.children(w)
        w.key("ctrl+shift+l")
        w.key("End")
        w.key("Tab")
        w.key("Tab")
        w.key("Return")
        wait_for(lambda: "Neue Sitzung" in w.title())
        assert self.children(w) == before
        self.shot(w, folder / "04-shell-choice-without-start.png")
        other = "bash" if shell == "zsh" else "zsh"
        # The chooser starts at zsh; it supports arrows/Tab, not Home/End.
        if other == "bash":
            w.key("Down")
        w.key("Return")
        wait_for(lambda: "· 3" in w.title())
        self.clean_prompt(w, other, "THREE")
        pids = self.children(w)
        assert len(pids) == 3 and primary_pid in pids
        third_pid = (set(pids) - set(before)).pop()
        assert Path(f"/proc/{third_pid}/exe").resolve() == Path("/usr/bin/" + other).resolve()
        row["shell_pids"] = pids
        row["shell_executables"] = {str(p): str(Path(f"/proc/{p}/exe").resolve()) for p in pids}
        labels = {1: "Backend " + shell, 2: "Worker " + shell, 3: "Admin " + other}
        keys = {1: "F8", 2: "F9", 3: "F10"}

        def activate(tab):
            w.key(keys[tab])
            wait_for(lambda: labels[tab] in w.title() and f"· {tab}" in w.title())
            assert f"[{self.commit[:7]}]" in w.title()

        for tab, color in [(3, 430), (2, 256), (1, 311)]:
            if tab != 3:
                w.key("ctrl+shift+Tab")
                wait_for(lambda: f"· {tab}" in w.title())
            helpers["edit"](w, labels[tab], color, keys[tab])
        assert self.children(w) == pids
        row["identity_titles"] = {}
        for tab, role in [(1, "ONE"), (2, "TWO"), (3, "THREE")]:
            activate(tab)
            send(w, "printf '\\033]2;HOO10 state %s\\007' \"$HOO10_ROLE\"")
            wait_for(lambda: "HOO10 state " + role in w.title())
            row["identity_titles"][str(tab)] = w.title()
        self.shot(w, folder / "05-three-named-colored-stable-tabs.png")
        row["checks"]["three_real_sessions_stable_identity_titles_shortcuts"] = True

        roots, sequences = {}, {}
        for tab in (1, 2, 3):
            activate(tab)
            root = folder / f"raw-{tab}"
            root.mkdir()
            roots[tab] = root
            send(w, "python3 " + shlex.quote(str(self.fixture)) + " " +
                 shlex.quote(str(root)) + " " + str(tab))
            value = wait_for(lambda: read_json(root / "proof.json"))
            wait_for(lambda: f"HOO10 raw-{tab}" in w.title())
            self.owned[value["pid"]] = start_time(value["pid"])

        def raw(tab):
            return read_json(roots[tab] / "proof.json")

        def issue(tab, op, wait=True, **values):
            sequences[tab] = sequences.get(tab, 0) + 1
            instruction = {"sequence": sequences[tab], "op": op, **values}
            temporary = roots[tab] / "command.tmp"
            temporary.write_text(json.dumps(instruction))
            temporary.replace(roots[tab] / "command.json")
            if wait:
                wait_for(lambda: raw(tab).get("sequence") == instruction["sequence"], timeout=25)
            return instruction["sequence"]

        activate(1)
        identity["open_editor"](w)
        identity["name"](w, "Discarded draft")
        identity["capture"](w, "F9")
        w.focus()
        command("xdotool", "mousemove", "--window", w.id, "700", "330",
                "click", "--repeat", "5", "--delay", "80", "5")
        self.shot(w, folder / "06-duplicate-shortcut-rejected.png")
        w.key("ctrl+shift+t")
        assert self.children(w) == pids
        self.shot(w, folder / "07-reserved-shortcut-rejected.png")
        w.key("Escape")
        w.key("Escape")
        wait_for(lambda: "Tab bearbeiten" not in w.title())
        assert labels[1] in w.title() and "Discarded" not in w.title()
        for tab in (2, 3, 1):
            activate(tab)
        assert all(raw(tab)["input_hex"] == "" for tab in (1, 2, 3))
        row["checks"]["conflicts_and_app_actions_never_reach_pty"] = True

        issue(1, "paint")
        time.sleep(.3)
        clipboard("HOO10 synthetic pre-copy fixture")
        w.focus()
        command("xdotool", "mousemove", "--window", w.id, "70", "70")
        command("xdotool", "mousedown", "1")
        time.sleep(.15)
        command("xdotool", "mousemove", "--window", w.id, "150", "70")
        time.sleep(.2)
        command("xdotool", "mouseup", "1")
        self.shot(w, folder / "08-user-selection.png")
        w.key("ctrl+shift+c")
        wait_for(lambda: command("xclip", "-selection", "clipboard", "-out") == "Grüße 界")
        clipboard("Paste Grüße\nsecond")
        w.key("ctrl+shift+v")
        expected = b"\x1b[200~" + "Paste Grüße\nsecond".encode() + b"\x1b[201~"
        wait_for(lambda: raw(1)["input_hex"] == expected.hex())
        issue(1, "emit", bytes=base64.b64encode(b"\x1b[?2004l").decode())
        clipboard("plain\n")
        w.key("ctrl+shift+v")
        expected += b"plain\r"
        wait_for(lambda: raw(1)["input_hex"] == expected.hex())
        clipboard("HOO10 OSC52 unchanged")
        issue(1, "emit", bytes=base64.b64encode(b"\x1b]52;c;c2VjcmV0\x07\x1b]52;c;?\x07").decode())
        assert command("xclip", "-selection", "clipboard", "-out") == "HOO10 OSC52 unchanged"
        assert all(raw(tab)["input_hex"] == "" for tab in (2, 3))
        row["checks"]["explicit_copy_unicode_bracketed_plain_paste_and_osc52"] = True

        self.phase(shell + "-inactive-output")
        issue(2, "load", wait=False, count=6000)
        wait_for(lambda: raw(2).get("load_started_at"))
        w.type("live!")
        expected += b"live!"
        wait_for(lambda: raw(1)["input_hex"] == expected.hex())
        self.shot(w, folder / "09-active-input-during-inactive-output.png")
        for width, height in [(1120, 740), (920, 640), (1000, 700)]:
            row["sizes"].append(resize(w, width, height))
        wait_for(lambda: raw(2).get("load_done_at"), timeout=25)
        event_time = raw(1)["input_events"][-1]["at"]
        assert raw(2)["load_started_at"] < event_time < raw(2)["load_done_at"]
        assert raw(2)["queries"][-1]["hex"] == DA.hex()
        assert labels[1] in w.title() and raw(2)["input_hex"] == "" and raw(3)["input_hex"] == ""
        row["inactive_load"] = raw(2)
        activate(2)
        wait_for(lambda: "HOO10 inactive-2-done" in w.title())
        self.shot(w, folder / "10-inactive-latest-output.png")
        w.key("ctrl+shift+Home")
        time.sleep(.2)
        self.shot(w, folder / "11-bounded-history-top.png")
        w.key("ctrl+shift+End")
        w.key("shift+Page_Up")
        time.sleep(.2)
        self.shot(w, folder / "12-scrollback-page.png")
        w.key("ctrl+shift+End")
        row["checks"]["inactive_output_parsed_and_replied_while_other_tab_interactive"] = True
        row["checks"]["scrollback_top_page_live"] = True

        w.request_close()
        self.shot(w, folder / "13-window-close-all-three-names.png")
        w.key("Escape")
        wait_for(lambda: "Schließen bestätigen" not in w.title())
        assert self.children(w) == pids
        assert all(start_time(pid) == stamp for pid, stamp in self.owned.items()
                   if pid in pids or any(raw(tab)["pid"] == pid for tab in (1, 2, 3)))
        activate(2)
        w.key("ctrl+shift+w")
        wait_for(lambda: "Schließen bestätigen" in w.title())
        self.shot(w, folder / "14-targeted-tab-close.png")
        w.key("Escape")
        issue(2, "emit", bytes=base64.b64encode(b"cancelled close preserves this PTY\r\n").decode())
        assert self.children(w) == pids
        worker_pid = raw(2)["parent"]
        worker_child = raw(2)["pid"]
        w.key("ctrl+shift+w")
        wait_for(lambda: "Schließen bestätigen" in w.title())
        w.key("Return")
        wait_for(lambda: start_time(worker_pid) != self.owned[worker_pid])
        wait_for(lambda: start_time(worker_child) != self.owned[worker_child])
        assert all(start_time(pid) == self.owned[pid] for pid in pids if pid != worker_pid)
        activate(3)
        issue(3, "stop")
        wait_for(lambda: "HOO10 raw-3-stopped" in w.title())
        w.run("exit 7")
        last_pid = raw(3)["parent"]
        wait_for(lambda: start_time(last_pid) != self.owned[last_pid])
        time.sleep(.3)
        self.shot(w, folder / "15-exit-seven-retained.png")
        w.key("ctrl+shift+w")
        wait_for(lambda: labels[1] in w.title())
        assert "Schließen bestätigen" not in w.title()
        w.key("F10")
        expected += b"\x1b[21~"
        wait_for(lambda: raw(1)["input_hex"] == expected.hex())
        w.key("ctrl+shift+t")
        wait_for(lambda: "· 4" in w.title())
        self.clean_prompt(w, shell, "FOUR")
        assert len(self.children(w)) == 2
        activate(1)
        w.key("F9")
        expected += b"\x1b[20~"
        wait_for(lambda: raw(1)["input_hex"] == expected.hex())
        row["input_hex"]["active"] = raw(1)["input_hex"]
        row["input_hex"]["inactive_worker"] = raw(2)["input_hex"]
        row["input_hex"]["other"] = raw(3)["input_hex"]
        assert row["input_hex"]["inactive_worker"] == row["input_hex"]["other"] == ""
        row["checks"]["targeted_cancel_close_exit_and_no_binding_inheritance"] = True
        survivors = self.children(w)
        w.request_close()
        self.shot(w, folder / "16-final-named-window-close.png")
        w.key("Escape")
        assert self.children(w) == survivors
        w.request_close()
        w.key("Return")
        w.wait_exit()
        wait_for(lambda: all(start_time(pid) != self.owned[pid] for pid in survivors))
        row["checks"]["window_cancel_and_confirm_reaps_own_sessions"] = True
        self.phase(shell + "-row-complete")

    def errors(self):
        self.phase("start-error")
        folder = self.output / "start-error"
        folder.mkdir()
        w = Window(self.binary, folder, "/nonexistent/hoo10-test-shell", [])
        self.windows.append(w)
        assert self.children(w) == []
        self.shot(w, folder / "01-start-error.png")
        w.request_close()
        w.wait_exit()
        self.report["visible_start_error_closes_without_live_confirmation"] = True

    def finish(self):
        wait_for(lambda: all(start_time(pid) != stamp for pid, stamp in self.owned.items()))
        self.report["owned_pids_reaped"] = sorted(self.owned)
        self.report["complete"] = True
        (self.output / "report.json").write_text(json.dumps(self.report, ensure_ascii=False, indent=2))
        print(json.dumps({"complete": True, "report": str(self.output / "report.json")}), flush=True)

    def cleanup(self):
        for w in self.windows:
            w.cleanup()
        # A failed native close assertion still must not leave test jobs alive.
        for pid, stamp in self.owned.items():
            if start_time(pid) == stamp:
                try:
                    os.kill(pid, 15)
                except ProcessLookupError:
                    pass


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--only", choices=["all", "graphics", "zsh", "bash", "default", "errors", "startup"],
                        default="all")
    args = parser.parse_args()
    assert os.environ.get("XDG_SESSION_TYPE") == "x11", "This harness proves the native X11 baseline only"
    assert not os.environ.get("WAYLAND_DISPLAY"), "Use the actual X11 desktop"
    binary, output = args.binary.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    config, zdot = output / "config", output / "zsh-config"
    config.mkdir()
    zdot.mkdir()
    # An empty directory alone invokes zsh-newuser-install and can consume
    # the first fixture byte. Only this fresh, owned profile is initialized.
    (zdot / ".zshrc").write_text("")
    # These are isolated fixture directories, never the user's configuration.
    before = {key: os.environ.get(key) for key in ("XDG_CONFIG_HOME", "ZDOTDIR", "HOORI_GRAPHICS_INFO")}
    os.environ.update(XDG_CONFIG_HOME=str(config), ZDOTDIR=str(zdot), HOORI_GRAPHICS_INFO="1")
    q = Qualification(binary, output)
    q.report["scope"] = args.only
    q.report["fixture_environment"] = {"XDG_CONFIG_HOME": str(config), "ZDOTDIR": str(zdot),
                                       "HOORI_GRAPHICS_INFO": "1"}
    try:
        if args.only in ("all", "graphics", "startup"):
            q.default_probe()
        if args.only in ("all", "zsh"):
            q.shell_row("zsh")
        if args.only in ("all", "bash"):
            q.shell_row("bash")
        if args.only in ("all", "errors"):
            q.errors()
        if args.only in ("all", "startup"):
            q.startup_probe()
        if args.only in ("all", "graphics", "default", "startup"):
            q.default_probe(normal=True)
        q.finish()
    except BaseException as error:
        q.report["error"] = repr(error)
        for w in q.windows:
            if w.process.poll() is None:
                q.shot(w, w.output / "failure.png")
        (output / "failure-report.json").write_text(json.dumps(q.report, ensure_ascii=False, indent=2))
        raise
    finally:
        q.cleanup()
        for key, value in before.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


if __name__ == "__main__":
    main()
