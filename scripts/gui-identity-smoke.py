#!/usr/bin/env python3
"""Exercise tab identity/shortcut editing in newly owned X11 windows only.

Uses synthetic shell/PTY data. Inspect all screenshots after completion;
title/process/byte assertions alone do not establish correct rendering.
"""
import argparse
import json
import os
from pathlib import Path
import runpy
import shlex
import time

harness = runpy.run_path(str(Path(__file__).with_name("gui-tabs-smoke.py")))
Window, command, wait_for, click = (harness[k] for k in ["Window", "command", "wait_for", "click"])


def open_editor(w):
    w.key("ctrl+shift+e")
    wait_for(lambda: "Tab bearbeiten" in w.title())
    time.sleep(0.2)


def name(w, value):
    click(w, 240, 114)
    w.key("ctrl+a")
    w.type(value)


def capture(w, key):
    click(w, 830, 222)
    time.sleep(0.15)
    w.key(key)
    time.sleep(0.2)


def edit(w, label, color_x, key):
    open_editor(w)
    name(w, label)
    click(w, color_x, 183)
    capture(w, key)
    w.key("Return")
    wait_for(lambda: "Tab bearbeiten" not in w.title() and label in w.title())


def reader(w, script, proof):
    w.run("python3 " + shlex.quote(str(script)) + " " + shlex.quote(str(proof)))
    wait_for(lambda: "HOO-8 raw-ready" in w.title())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    assert os.environ.get("XDG_SESSION_TYPE") == "x11", "This harness validates X11 only"
    binary, output = args.binary.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {"metadata": command(str(binary), "--version"),
              "session_type": os.environ.get("XDG_SESSION_TYPE"), "display": os.environ.get("DISPLAY"),
              "os": Path("/etc/os-release").read_text(), "zsh": command("zsh", "--version"),
              "bash": command("bash", "--version").splitlines()[0]}
    w = Window(binary, output, "/usr/bin/zsh", ["-f", "-i"])
    try:
        children = lambda: [int(p) for p in Path(f"/proc/{w.process.pid}/task/{w.process.pid}/children").read_text().split()]
        for i in range(3):
            if i:
                w.key("ctrl+shift+t" if i == 1 else "ctrl+shift+b")
                wait_for(lambda: f"· {i + 1}" in w.title())
            # Keep the fixture's OSC markers visible instead of letting the
            # user's Bash prompt hook immediately replace them.
            clean_prompt = "PROMPT_COMMAND=; PS1='BASH> '; " if i == 2 else ""
            w.run(f"unset HISTFILE; {clean_prompt}HOO8_MARKER=SESSION_{i + 1}; printf '\\033[2J\\033[HSession {i + 1} retained\\n'")
            w.marker(f"HOO-8 session-{i + 1}-ready")
        pids = children()
        assert len(pids) == 3
        report["original_shell_pids"] = pids
        edit(w, "Admin", 430, "F10")
        w.key("ctrl+shift+Tab")
        wait_for(lambda: "zsh · 2" in w.title())
        edit(w, "Frontend", 256, "F9")
        w.key("ctrl+shift+Tab")
        wait_for(lambda: "zsh · 1" in w.title())
        edit(w, "Backend", 311, "F8")
        assert children() == pids
        w.screenshot(output / "01-three-named-colored-tabs.png")
        for key, label, marker in [("F8", "Backend", "SESSION_1"), ("F9", "Frontend", "SESSION_2"), ("F10", "Admin", "SESSION_3")]:
            w.key(key)
            wait_for(lambda: label in w.title())
            w.run("printf '\\033]2;HOO-8 preserved %s\\007' \"$HOO8_MARKER\"")
            wait_for(lambda: marker in w.title())
            assert label in w.title()
            assert report["metadata"].split("commit: ")[1][:7] in w.title()
            w.screenshot(output / f"02-{label.lower()}-retained.png")
        report["three_named_colored_sessions_and_dynamic_titles"] = True
        report["original_processes_and_state_preserved"] = True

        raw = output / "raw-reader.py"
        raw.write_text('''import os,sys,termios,tty
from pathlib import Path
old=termios.tcgetattr(0)
tty.setraw(0)
os.write(1,b'\\x1b]2;HOO-8 raw-ready\\x07')
data=bytearray()
try:
    while not data.endswith(b'!'):
        data.extend(os.read(0,4096))
    Path(sys.argv[1]).write_bytes(data)
finally:
    termios.tcsetattr(0,termios.TCSANOW,old)
    os.write(1,b'\\x1b]2;HOO-8 raw-done\\x07')
''')
        proof = output / "dialog-routing.bin"
        reader(w, raw, proof)
        # A direct action while raw mode is active must not also send F10.
        w.key("F8")
        wait_for(lambda: "Backend" in w.title())
        w.key("F10")
        wait_for(lambda: "Admin" in w.title() and "raw-ready" in w.title())
        open_editor(w)
        name(w, "Discarded name")
        w.key("F8")  # In a name field, this must not activate Backend.
        assert "Tab bearbeiten" in w.title() and "Admin" in w.title()
        capture(w, "F9")  # Duplicate Frontend binding: reject visibly.
        w.screenshot(output / "03-duplicate-shortcut-rejected.png")
        assert children() == pids
        w.key("ctrl+shift+t")  # Reserved app action: must not open a tab.
        time.sleep(0.2)
        w.screenshot(output / "04-reserved-shortcut-rejected.png")
        assert children() == pids
        w.key("Escape")  # Cancel capture, retain the existing F10 binding.
        w.key("Escape")  # Discard the whole edit.
        wait_for(lambda: "Tab bearbeiten" not in w.title())
        assert "Admin" in w.title() and "Discarded" not in w.title()
        w.key("F12")  # Ordinary unassigned terminal key remains usable.
        w.type("plain!")
        wait_for(lambda: proof.exists())
        assert proof.read_bytes() == b"\x1b[24~plain!", proof.read_bytes()
        wait_for(lambda: "raw-done" in w.title())
        report["dialog_capture_and_app_actions_do_not_reach_pty"] = True
        report["unassigned_terminal_input_exact_bytes"] = True
        report["duplicate_and_builtin_conflicts_preserve_assignments"] = True

        # Change name/color/shortcut on the same running target.
        w.key("F9")
        wait_for(lambda: "Frontend" in w.title())
        edit(w, "Web", 489, "F11")
        assert children() == pids
        w.marker("HOO-8 renamed-target")
        w.screenshot(output / "05-renamed-recolored-rebound.png")
        w.key("F10")
        wait_for(lambda: "Admin" in w.title())
        proof = output / "rebound-routing.bin"
        reader(w, raw, proof)
        w.key("F9")  # The old binding must not activate Web.
        assert "Admin" in w.title()
        w.key("F11")
        wait_for(lambda: "Web" in w.title())
        w.key("F10")
        wait_for(lambda: "Admin" in w.title())
        w.type("!")
        wait_for(lambda: proof.exists())
        assert proof.read_bytes() == b"\x1b[20~!"
        wait_for(lambda: "raw-done" in w.title())
        report["editing_binding_removes_old_target_and_preserves_process"] = True

        # Remove the tab before Web; its F11 identity is independent of position.
        w.key("F8")
        wait_for(lambda: "Backend" in w.title())
        w.key("ctrl+shift+w")
        wait_for(lambda: "Schließen bestätigen" in w.title())
        w.screenshot(output / "06-custom-name-close-confirmation.png")
        w.key("Escape")
        assert children() == pids
        w.key("ctrl+shift+w")
        wait_for(lambda: "Schließen bestätigen" in w.title())
        w.key("Return")
        wait_for(lambda: not Path(f"/proc/{pids[0]}").exists())
        w.key("F11")
        wait_for(lambda: "Web" in w.title())
        w.run("printf '\\033]2;HOO-8 after-remove %s\\007' \"$HOO8_MARKER\"")
        wait_for(lambda: "SESSION_2" in w.title())
        w.key("ctrl+shift+t")
        wait_for(lambda: "zsh · 4" in w.title())
        w.run("unset HISTFILE")
        w.marker("HOO-8 new-tab-no-binding")
        remaining = children()
        assert len(remaining) == 3 and pids[1] in remaining and pids[2] in remaining
        w.screenshot(output / "07-closed-target-not-inherited.png")
        w.key("F11")
        wait_for(lambda: "Web" in w.title())
        proof = output / "closed-target-routing.bin"
        reader(w, raw, proof)
        w.key("F8")  # Closed Backend binding must not activate the new tab.
        assert "Web" in w.title()
        w.type("!")
        wait_for(lambda: proof.exists())
        assert proof.read_bytes() == b"\x1b[19~!"
        wait_for(lambda: "raw-done" in w.title())
        assert children() == remaining
        report["identity_survives_preceding_removal_and_new_tabs"] = True
        report["closed_binding_not_inherited_or_recreated"] = True

        w.request_close()
        w.screenshot(output / "08-window-close-custom-names.png")
        w.key("Escape")
        assert all(Path(f"/proc/{p}").exists() for p in remaining)
        w.request_close()
        w.key("Return")
        w.wait_exit()
        wait_for(lambda: all(not Path(f"/proc/{p}").exists() for p in remaining))
        report["named_cancel_confirm_and_cleanup"] = True
        (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        print(json.dumps(report, ensure_ascii=False, indent=2))
    except BaseException:
        if w.process.poll() is None:
            w.screenshot(output / "failure.png")
        raise


if __name__ == "__main__":
    main()
