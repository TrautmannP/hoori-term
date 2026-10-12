#!/usr/bin/env python3
"""Drive only newly created hoori-term X11 windows with synthetic test data.

Screenshots require human inspection; window-title assertions do not replace
visual validation. No existing terminal windows or clipboard contents are read.
"""
import argparse
import atexit
import json
import os
from pathlib import Path
import shlex
import subprocess
import time


def command(*args, check=True):
    return subprocess.run(args, check=check, text=True, capture_output=True).stdout.strip()


def wait_for(predicate, timeout=12):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.1)
    raise AssertionError("GUI condition timed out")


def sample(pid):
    stat = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    rss = int(Path(f"/proc/{pid}/statm").read_text().split()[1]) * os.sysconf("SC_PAGE_SIZE")
    return (time.monotonic(), (int(stat[11]) + int(stat[12])) / os.sysconf("SC_CLK_TCK"), rss)


def usage(first, last):
    return {"seconds": round(last[0] - first[0], 2),
            "cpu_percent_one_core": round((last[1] - first[1]) / (last[0] - first[0]) * 100, 2),
            "rss_mib_start": round(first[2] / 2**20, 2), "rss_mib_end": round(last[2] / 2**20, 2)}


class Window:
    def __init__(self, binary, output, shell, args):
        self.output = output
        self.log = (output / f"{shell.rsplit('/', 1)[-1]}-app-stderr.txt").open("w")
        self.process = subprocess.Popen([str(binary), "--shell", shell, "--", *args], stdout=self.log, stderr=self.log)
        atexit.register(self.cleanup)
        self.id = wait_for(lambda: command("xdotool", "search", "--onlyvisible", "--pid", str(self.process.pid), check=False).splitlines())[-1]
        self.assert_owned()
        wait_for(lambda: self.title().startswith("hoori-term ["))
        command("xdotool", "windowactivate", "--sync", self.id)
        time.sleep(0.6)

    def key(self, key):
        self.focus()
        command("xdotool", "key", "--clearmodifiers", key)

    def type(self, text):
        self.focus()
        ascii_text = ""
        for char in text:
            if ord(char) < 128:
                ascii_text += char
                continue
            if ascii_text:
                command("xdotool", "type", "--clearmodifiers", "--delay", "30", "--", ascii_text)
                ascii_text = ""
            # Let winit consume XKB map changes before xdotool removes the
            # temporary mapping for a character absent from the real layout.
            command("xdotool", "key", "--clearmodifiers", "--delay", "250", f"U{ord(char):04X}")
        if ascii_text:
            command("xdotool", "type", "--clearmodifiers", "--delay", "30", "--", ascii_text)
        assert command("xdotool", "getwindowfocus") == self.id, "Focus changed during test input"

    def focus(self):
        self.assert_owned()
        command("xdotool", "windowactivate", "--sync", self.id)
        assert command("xdotool", "getactivewindow") == self.id
        assert command("xdotool", "getwindowfocus") == self.id

    def assert_owned(self):
        assert self.process.poll() is None
        value = command("xprop", "-id", self.id, "_NET_WM_PID")
        assert value.rsplit("=", 1)[-1].strip() == str(self.process.pid), "Window does not belong to the test process"

    def run(self, text):
        self.type(text)
        self.key("Return")

    def title(self):
        return command("xdotool", "getwindowname", self.id, check=False)

    def marker(self, marker):
        self.run(f"printf '\\033]2;{marker}\\007'")
        try:
            wait_for(lambda: marker in self.title())
        except AssertionError:
            self.screenshot(self.output / "failure.png")
            raise

    def screenshot(self, path):
        self.assert_owned()
        command("import", "-window", self.id, str(path))

    def request_close(self):
        self.focus()
        command("xdotool", "key", "--clearmodifiers", "alt+F4")
        wait_for(lambda: self.process.poll() is not None or "Schließen bestätigen" in self.title())

    def wait_exit(self):
        self.process.wait(timeout=8)
        assert self.process.returncode == 0
        self.log.close()

    def cleanup(self):
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=3)
        self.log.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    assert os.environ.get("XDG_SESSION_TYPE") == "x11", "This harness validates X11 only"
    binary = args.binary.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {"metadata": command(str(binary), "--version"), "display": os.environ.get("DISPLAY"),
              "session_type": os.environ.get("XDG_SESSION_TYPE"), "vim": command("vim", "--version").splitlines()[0]}
    w = Window(binary, output, "/usr/bin/zsh", ["-f", "-i"])
    report["zsh_pid"] = int(Path(f"/proc/{w.process.pid}/task/{w.process.pid}/children").read_text().split()[0])
    w.run("printf '\\033[2J\\033[HUTF-8: Gr\\u00fc\\u00dfe \\u754c e\\u0301\\n\\033[31mROT \\033[32mGR\\u00dcN \\033[34mBLAU\\033[0m\\nCursor: A\\033[2DOK\\n'")
    w.marker("HOO-6 zsh-ready")
    report["title_after_shell_change"] = w.title()
    assert report["metadata"].split("commit: ")[1][:7] in w.title()
    w.screenshot(output / "01-zsh.png")
    first = sample(w.process.pid)
    time.sleep(4)
    report["idle"] = usage(first, sample(w.process.pid))

    proof = output / "vim-proof.txt"
    w.run("vim -Nu NONE -n " + shlex.quote(str(proof)))
    time.sleep(0.7)
    w.type("iVim proof: Grüße 界")
    w.key("Escape")
    w.type(":w")
    w.key("Return")
    for width, height in [(1180, 780), (860, 600), (1100, 740)]:
        command("xdotool", "windowsize", "--sync", w.id, str(width), str(height))
        time.sleep(0.2)
    w.screenshot(output / "02-vim-resize.png")
    w.type(":q")
    w.key("Return")
    wait_for(lambda: proof.exists())
    assert proof.read_text() == "Vim proof: Grüße 界\n"
    w.marker("HOO-6 back-from-vim")
    report["vim_file_verified"] = True
    w.run("printf '\\033]2;HOO-6 size %s\\007' \"$(stty size)\"")
    wait_for(lambda: "HOO-6 size " in w.title())
    report["title_with_pty_size"] = w.title()
    w.screenshot(output / "03-shell-after-vim.png")

    # ~12 MB of synthetic, numbered output plus a final completion marker.
    load_code = "import sys; [sys.stdout.write(f'load {i:07d} abcdefghijklmnopqrstuvwxyz0123456789\\n') for i in range(250000)]"
    w.run("python3 -c " + shlex.quote(load_code) + "; printf '\\033]2;HOO-6 load-done\\007'")
    first = sample(w.process.pid)
    for i in range(12):
        command("xdotool", "windowsize", "--sync", w.id, str(900 + i % 3 * 90), str(620 + i % 4 * 30))
        time.sleep(0.15)
    w.screenshot(output / "04-load-resize.png")
    wait_for(lambda: "HOO-6 load-done" in w.title(), timeout=60)
    report["output_load"] = usage(first, sample(w.process.pid))
    w.marker("HOO-6 responsive-after-load")
    w.request_close()
    assert w.process.poll() is None
    w.screenshot(output / "05-close-confirmation.png")
    w.key("Escape")
    w.marker("HOO-6 close-cancelled")
    report["cancel_preserves_session"] = True
    w.request_close()
    w.key("Return")
    w.wait_exit()
    wait_for(lambda: not Path(f"/proc/{report['zsh_pid']}").exists())
    report["confirmed_close_reaps_shell"] = True

    w = Window(binary, output, "/bin/bash", ["--noprofile", "--norc", "-i"])
    w.marker("HOO-6 bash-ready")
    w.run("printf 'Bash smoke: %s\\n' OK; exit 7")
    time.sleep(0.8)
    w.screenshot(output / "06-bash-exit.png")
    w.request_close()
    w.wait_exit()
    report["bash_smoke_exit_window_closed"] = True

    w = Window(binary, output, "/nonexistent/hoori-test-shell", [])
    w.screenshot(output / "07-start-error.png")
    w.request_close()
    w.wait_exit()
    report["start_error_window_closed"] = True
    (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
