#!/usr/bin/env python3
"""Real X11 multi-session/clipboard exercise with owned synthetic windows only.

Writes a synthetic clipboard fixture and reads only the expected test selection.
Inspect every screenshot before treating the rendering scenarios as passed.
"""
import argparse
import json
import os
from pathlib import Path
import runpy
import shlex
import subprocess
import time

harness = runpy.run_path(str(Path(__file__).with_name("gui-smoke.py")))
Window, command, wait_for, sample, usage = (harness[k] for k in ["Window", "command", "wait_for", "sample", "usage"])


def clipboard(text):
    subprocess.run(["xclip", "-selection", "clipboard", "-in"], input=text.encode(), check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def click(w, x, y):
    w.focus()
    command("xdotool", "mousemove", "--window", w.id, str(x), str(y), "click", "1")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    assert os.environ.get("XDG_SESSION_TYPE") == "x11"
    binary, output = args.binary.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {"metadata": command(str(binary), "--version"), "session_type": os.environ.get("XDG_SESSION_TYPE"),
              "display": os.environ.get("DISPLAY"), "os": Path("/etc/os-release").read_text(),
              "zsh": command("zsh", "--version"), "bash": command("bash", "--version").splitlines()[0]}
    w = Window(binary, output, "/usr/bin/zsh", ["-f", "-i"])
    w.run("unset HISTFILE; printf '\\033[2J\\033[HSession ONE\\n'")
    w.marker("HOO-7 one-ready")
    first_title = w.title()
    w.key("ctrl+shift+t")
    wait_for(lambda: "zsh · 2" in w.title())
    w.run("unset HISTFILE; printf '\\033[2J\\033[HSession TWO\\n'")
    w.marker("HOO-7 two-ready")
    before_menu = Path(f"/proc/{w.process.pid}/task/{w.process.pid}/children").read_text()
    click(w, 972, 26)  # Fixed Plus target in the owned 1000px window.
    wait_for(lambda: "Neue Sitzung" in w.title())
    assert Path(f"/proc/{w.process.pid}/task/{w.process.pid}/children").read_text() == before_menu
    w.screenshot(output / "00-shell-choice-without-start.png")
    w.key("Down")
    w.key("Return")
    wait_for(lambda: "bash · 3" in w.title())
    w.run("unset HISTFILE; PROMPT_COMMAND=; PS1='BASH> '; printf '\\033[2J\\033[HSession THREE\\n'")
    w.marker("HOO-7 three-ready")
    w.screenshot(output / "01-three-tabs-bash.png")
    children = lambda: [int(p) for p in Path(f"/proc/{w.process.pid}/task/{w.process.pid}/children").read_text().split()]
    shell_pids = children()
    assert len(shell_pids) == 3
    report["shell_pids"] = shell_pids
    w.key("ctrl+Tab")
    wait_for(lambda: "HOO-7 one-ready" in w.title())
    assert w.title() == first_title
    w.screenshot(output / "02-isolated-one.png")
    w.key("ctrl+Tab")
    wait_for(lambda: "HOO-7 two-ready" in w.title())
    w.screenshot(output / "03-isolated-two.png")
    report["three_independent_sessions"] = True

    # High-volume inactive output while repeatedly switching/resizing active tabs.
    load = "import sys,time; [(sys.stdout.write(f'inactive {i:07d} abcdefghijklmnopqrstuvwxyz0123456789\\n'), sys.stdout.flush(), time.sleep(0.002) if i%100==0 else None) for i in range(250000)]"
    w.run("python3 -c " + shlex.quote(load) + "; printf '\\033]2;HOO-7 inactive-done\\007'")
    w.key("ctrl+Tab")
    wait_for(lambda: "bash · 3" in w.title())
    first = sample(w.process.pid)
    for i in range(12):
        command("xdotool", "windowsize", "--sync", w.id, str(900 + i%3*90), str(620 + i%4*30))
        w.key("ctrl+Tab")
        time.sleep(0.08)
    # 12 cycles leave Bash active. Its PTY reports its own current grid.
    w.run("printf '\\033]2;HOO-7 active-size %s\\007' \"$(stty size)\"")
    wait_for(lambda: "HOO-7 active-size " in w.title())
    report["active_size_title"] = w.title()
    w.screenshot(output / "04-responsive-bash-during-load.png")
    report["inactive_load_usage"] = usage(first, sample(w.process.pid))
    w.key("ctrl+shift+Tab")
    wait_for(lambda: "HOO-7 inactive-done" in w.title(), timeout=60)
    w.screenshot(output / "05-latest-inactive-output.png")
    w.key("ctrl+shift+Home")
    time.sleep(0.3)
    w.screenshot(output / "06-bounded-history-top.png")
    w.key("ctrl+shift+End")
    w.key("shift+Page_Up")
    time.sleep(0.2)
    w.screenshot(output / "07-scrollback-page.png")
    w.key("ctrl+shift+End")
    report["inactive_output_resize_scrollback"] = True

    # Return to the clean zsh tab for deterministic clipboard fixtures.
    w.key("ctrl+shift+Tab")
    wait_for(lambda: "zsh · 1" in w.title())
    command("xdotool", "windowsize", "--sync", w.id, "1000", "700")
    w.run("printf '\\033[2J\\033[HCOPY: Gr\\u00fc\\u00dfe \\u754c\\n'")
    w.marker("HOO-7 clipboard-ready")
    clipboard("HOO-7 synthetic pre-copy fixture")
    w.focus()
    command("xdotool", "mousemove", "--window", w.id, "70", "70")
    time.sleep(0.15)
    command("xdotool", "mousedown", "1")
    time.sleep(0.15)
    command("xdotool", "mousemove", "--window", w.id, "150", "70")
    time.sleep(0.25)
    command("xdotool", "mouseup", "1")
    w.screenshot(output / "08-text-selection.png")
    w.key("ctrl+shift+c")
    wait_for(lambda: command("xclip", "-selection", "clipboard", "-out") == "Grüße 界")
    report["copy_exact_utf8"] = True
    reader = output / "paste-reader.py"
    reader.write_text('''import os,sys,termios,tty
from pathlib import Path
old=termios.tcgetattr(0)
tty.setraw(0)
bracketed=sys.argv[2]=='bracketed'
os.write(1, (b'\\x1b[?2004h' if bracketed else b'\\x1b[?2004l')+b'\\x1b]2;HOO-7 paste-ready\\x07')
data=bytearray()
try:
    while not data.endswith(b'\\x1b[201~' if bracketed else b'\\r'):
        data.extend(os.read(0,4096))
    Path(sys.argv[1]).write_bytes(data)
finally:
    termios.tcsetattr(0,termios.TCSANOW,old)
    os.write(1,b'\\x1b[?2004l\\x1b]2;HOO-7 paste-done\\x07')
''')
    for mode, fixture, expected in [("bracketed", "Paste Grüße\nsecond", "\x1b[200~Paste Grüße\nsecond\x1b[201~".encode()),
                                    ("plain", "plain\n", b"plain\r")]:
        proof = output / f"paste-{mode}.bin"
        w.run("python3 " + shlex.quote(str(reader)) + " " + shlex.quote(str(proof)) + " " + mode)
        wait_for(lambda: "HOO-7 paste-ready" in w.title())
        clipboard(fixture)
        w.key("ctrl+shift+v")
        wait_for(lambda: proof.exists())
        assert proof.read_bytes() == expected
        wait_for(lambda: "HOO-7 paste-done" in w.title())
    report["paste_bracketed_and_plain_exact_bytes"] = True
    clipboard("HOO-7 OSC52 unchanged")
    w.run("printf '\\033]52;c;c2VjcmV0\\007\\033]52;c;?\\007'")
    w.marker("HOO-7 OSC52-rejected")
    assert command("xclip", "-selection", "clipboard", "-out") == "HOO-7 OSC52 unchanged"
    report["output_cannot_access_clipboard"] = True

    # Cancel and confirm only the second live tab. Surviving PIDs stay alive.
    w.key("ctrl+Tab")
    wait_for(lambda: "zsh · 2" in w.title())
    w.key("ctrl+shift+w")
    wait_for(lambda: "Schließen bestätigen" in w.title())
    w.screenshot(output / "09-tab-close-confirmation.png")
    w.key("Escape")
    assert all(Path(f"/proc/{p}").exists() for p in shell_pids)
    w.marker("HOO-7 tab-close-cancelled")
    w.key("ctrl+shift+w")
    wait_for(lambda: "Schließen bestätigen" in w.title())
    w.key("Return")
    wait_for(lambda: "bash · 3" in w.title())
    wait_for(lambda: not Path(f"/proc/{shell_pids[1]}").exists())
    assert all(Path(f"/proc/{p}").exists() for p in [shell_pids[0], shell_pids[2]])
    w.marker("HOO-7 survivor-three")
    report["cancel_and_targeted_close"] = True
    w.run("exit 7")
    wait_for(lambda: not Path(f"/proc/{shell_pids[2]}").exists())
    time.sleep(0.2)
    w.screenshot(output / "10-bash-exited-retained.png")
    w.key("ctrl+shift+w")
    wait_for(lambda: "zsh · 1" in w.title())
    assert "Schließen bestätigen" not in w.title()
    w.marker("HOO-7 survivor-one")
    report["exited_tab_closed_without_confirmation"] = True
    # Reopen a tab; the monotonic identity must be 4, never an old closed ID.
    w.key("ctrl+shift+t")
    wait_for(lambda: "zsh · 4" in w.title())
    w.run("unset HISTFILE")
    w.marker("HOO-7 four-ready")
    remaining_pids = children()
    assert len(remaining_pids) == 2
    w.request_close()
    w.screenshot(output / "11-window-close-names.png")
    w.key("Escape")
    assert all(Path(f"/proc/{p}").exists() for p in remaining_pids)
    w.marker("HOO-7 window-close-cancelled")
    first = sample(w.process.pid)
    time.sleep(4)
    report["two_session_idle"] = usage(first, sample(w.process.pid))
    w.request_close()
    w.key("Return")
    w.wait_exit()
    wait_for(lambda: all(not Path(f"/proc/{p}").exists() for p in remaining_pids))
    report["window_cancel_confirm_reaps_all"] = True
    (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
