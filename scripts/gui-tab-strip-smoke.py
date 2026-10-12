#!/usr/bin/env python3
"""Owned X11 tab-strip evidence, with an unchanged HOO-8 comparison mode.

Only synthetic commands/reader data are used. Inspect every PNG separately from
PID/title assertions; --baseline does not claim the new interactions passed.
"""
import argparse
import json
import os
from pathlib import Path
import runpy
import shlex
import subprocess
import time

identity = runpy.run_path(str(Path(__file__).with_name("gui-identity-smoke.py")))
Window, command, wait_for, click, edit = (identity[k] for k in ["Window", "command", "wait_for", "click", "edit"])


def children(window):
    window.assert_owned()
    return [int(pid) for pid in Path(f"/proc/{window.process.pid}/task/{window.process.pid}/children").read_text().split()]


def resize(window, width):
    window.focus()
    command("xdotool", "windowsize", "--sync", window.id, str(width), "700")
    time.sleep(0.3)
    geometry = command("xdotool", "getwindowgeometry", "--shell", window.id)
    values = dict(line.split("=", 1) for line in geometry.splitlines())
    assert (int(values["WIDTH"]), int(values["HEIGHT"])) == (width, 700)
    return values


def move(window, x, y):
    window.focus()
    command("xdotool", "mousemove", "--window", window.id, str(x), str(y))


def initialize(window, index, bash):
    # A fresh shell may still have prompt/startup input pending. Clear its
    # synthetic command line before sending this fixture, then check its state.
    window.key("ctrl+u")
    prompt = "PROMPT_COMMAND=; PS1='BASH> '; " if bash else ""
    window.run(f"unset HISTFILE; {prompt}HOO19_MARKER=SESSION_{index:02d}; printf '\\033[2J\\033[HReview session {index:02d}\\n'")
    window.marker(f"HOO-19 session-{index:02d}-ready")


def long_name(index):
    if index in (1, 2):
        return "Long duplicate session for review".ljust(40, "x")
    if index == 6:
        return "Grüße 界 Unicode session for review".ljust(40, "x")
    return f"Long session {index:02d} for review".ljust(40, "x")


def configure_long(window, label, color, key):
    identity["open_editor"](window)
    click(window, 240, 114)
    window.key("ctrl+a")
    # Use an exact synthetic UTF-8 clipboard fixture for long names, avoiding
    # temporary XKB Unicode remapping for this exact-name fixture.
    subprocess.run(["xclip", "-selection", "clipboard", "-in"], input=label.encode(),
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    window.key("ctrl+v")
    time.sleep(0.25)
    click(window, color, 183)
    identity["capture"](window, key)
    window.key("Return")
    wait_for(lambda: "Tab bearbeiten" not in window.title() and label in window.title())


def plus(window, width):
    click(window, width - 28, 26)
    wait_for(lambda: "Neue Sitzung" in window.title())


def menu_and_focus(window, output, pids):
    window.key("F2")
    wait_for(lambda: "Frontend" in window.title())
    raw = output / "strip-reader.py"
    raw.write_text('''import os,sys,termios,tty
from pathlib import Path
old=termios.tcgetattr(0)
tty.setraw(0)
os.write(1,b'\\x1b]2;HOO-19 raw-ready\\x07')
data=bytearray()
try:
    while not data.endswith(b'!'):
        data.extend(os.read(0,4096))
    Path(sys.argv[1]).write_bytes(data)
finally:
    termios.tcsetattr(0,termios.TCSANOW,old)
    os.write(1,b'\\x1b]2;HOO-19 raw-done\\x07')
''')
    proof = output / "strip-routing.bin"
    window.run("python3 " + shlex.quote(str(raw)) + " " + shlex.quote(str(proof)))
    wait_for(lambda: "HOO-19 raw-ready" in window.title())
    move(window, 100, 24)
    time.sleep(0.15)
    window.screenshot(output / "03-hover-inactive.png")
    move(window, 800, 400)
    window.key("ctrl+shift+l")
    window.key("Home")
    assert "Frontend" in window.title()  # Focus alone does not activate Backend.
    window.screenshot(output / "04-keyboard-focus-inactive.png")
    window.key("Right")
    window.key("Return")
    assert "Frontend" in window.title()
    plus(window, 1280)
    assert children(window) == pids
    window.screenshot(output / "05-menu-without-start.png")
    window.type("discarded")
    window.key("ctrl+shift+t")
    assert children(window) == pids
    window.key("Escape")
    wait_for(lambda: "Neue Sitzung" not in window.title())
    plus(window, 1280)
    window.key("Up")  # Wrap from zsh to Abbrechen.
    window.key("Return")
    assert children(window) == pids
    window.key("F1")
    window.key("F2")
    window.key("F12")  # Still unassigned with three tabs.
    window.type("ok!")
    wait_for(lambda: proof.exists())
    assert proof.read_bytes() == b"\x1b[24~ok!", "Strip/menu actions leaked or terminal input changed"
    wait_for(lambda: "HOO-19 raw-done" in window.title())
    plus(window, 1280)
    window.key("Down")
    window.key("Return")
    wait_for(lambda: "bash · 4" in window.title())
    wait_for(lambda: len(children(window)) == 4)
    assert children(window)[:3] == pids
    time.sleep(0.2)
    assert len(children(window)) == 4


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--baseline", action="store_true")
    args = parser.parse_args()
    assert os.environ.get("XDG_SESSION_TYPE") == "x11", "X11-only evidence"
    output, binary = args.output.resolve(), args.binary.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {"metadata": command(str(binary), "--version"), "baseline_only": args.baseline,
              "session_type": os.environ.get("XDG_SESSION_TYPE"), "display": os.environ.get("DISPLAY"),
              "os": Path("/etc/os-release").read_text(), "geometry": [],
              "scale": command("gsettings", "get", "org.gnome.desktop.interface", "text-scaling-factor"),
              "xft_dpi": command("xrdb", "-query"), "visual_inspection_required": True}
    window = Window(binary, output, "/usr/bin/zsh", ["-f", "-i"])
    try:
        for index, label, color in [(1, "Backend", 256), (2, "Frontend", 311), (3, "Deploy", 430)]:
            if index > 1:
                window.key("ctrl+shift+t" if index == 2 else "ctrl+shift+b")
                wait_for(lambda: f"· {index}" in window.title())
            initialize(window, index, index == 3)
            edit(window, label, color, f"F{index}")
        original_pids = children(window)
        assert len(original_pids) == 3
        report["three_original_shell_pids"] = original_pids
        window.key("F1")
        for width in [640, 1280]:
            report["geometry"].append(resize(window, width))
            move(window, width - 100, 400)
            window.screenshot(output / f"01-three-tabs-{width}.png")
        print("Three-tab comparison views complete", flush=True)
        if not args.baseline:
            menu_and_focus(window, output, original_pids)
            report["menu_open_cancel_no_process_explicit_choice_one_bash"] = True
            report["keyboard_focus_and_menu_exact_raw_input"] = True
        for index in range(1, 13):
            if index <= 3:
                window.key(f"F{index}")
            elif index == 4 and not args.baseline:
                window.key("ctrl+Tab")  # From configured third to the chosen fourth shell.
                wait_for(lambda: "bash · 4" in window.title())
                initialize(window, index, True)
            else:
                window.key("ctrl+shift+b" if index % 2 == 0 else "ctrl+shift+t")
                wait_for(lambda: f"· {index}" in window.title())
                initialize(window, index, index % 2 == 0)
            # Preserve the existing editor geometry while configuring fixtures.
            resize(window, 1000)
            configure_long(window, long_name(index), [256, 311, 430][(index - 1) % 3], f"F{index}")
        pids = children(window)
        assert len(pids) == 12 and pids[:3] == original_pids
        report["twelve_shell_pids"] = pids
        for index in range(1, 13):
            window.key(f"F{index}")
            window.run("printf '\\033]2;HOO-19 preserved %s\\007' \"$HOO19_MARKER\"")
            wait_for(lambda: f"SESSION_{index:02d}" in window.title())
        assert children(window) == pids
        report["twelve_long_names_original_state_and_shortcut_targets"] = True
        for width in [640, 1280]:
            report["geometry"].append(resize(window, width))
            for index in [1, 12]:
                window.key(f"F{index}")
                wait_for(lambda: f"SESSION_{index:02d}" in window.title())
                move(window, width - 100, 400)
                window.screenshot(output / f"06-twelve-tabs-{width}-target-{index:02d}.png")
        print("Twelve-tab comparison views complete", flush=True)
        if not args.baseline:
            resize(window, 640)
            window.key("F12")
            move(window, 410, 25)
            time.sleep(0.9)
            window.screenshot(output / "07-full-name-inspection.png")
            # Scroll the strip without changing the active terminal. Close the
            # first of two equal names through its own mouse target.
            for _ in range(12):
                click(window, 20, 26)
                time.sleep(0.04)
            assert "SESSION_12" in window.title()
            click(window, 272, 30)
            wait_for(lambda: "Schließen bestätigen" in window.title())
            window.screenshot(output / "08-inactive-duplicate-name-close.png")
            window.key("Escape")
            assert children(window) == pids
            click(window, 272, 30)
            wait_for(lambda: "Schließen bestätigen" in window.title())
            window.key("Return")
            wait_for(lambda: not Path(f"/proc/{pids[0]}").exists())
            assert all(Path(f"/proc/{p}").exists() for p in pids[1:])
            window.key("F2")
            window.run("printf '\\033]2;HOO-19 preserved %s\\007' \"$HOO19_MARKER\"")
            wait_for(lambda: "SESSION_02" in window.title())
            window.screenshot(output / "09-duplicate-survivor.png")
            window.key("F11")
            window.run("exit 7")
            wait_for(lambda: not Path(f"/proc/{pids[10]}").exists())
            time.sleep(0.3)
            window.screenshot(output / "10-exited-tab-marker.png")
            window.key("ctrl+shift+w")
            wait_for(lambda: " · 11" not in window.title())
            assert "Schließen bestätigen" not in window.title()
            report["inactive_mouse_close_cancel_targeted_duplicate_survives"] = True
            report["exited_tab_closes_without_process_confirmation"] = True
        alive = children(window)
        window.request_close()
        window.screenshot(output / "11-window-close-named.png")
        window.key("Escape")
        assert children(window) == alive
        window.request_close()
        window.key("Return")
        window.wait_exit()
        wait_for(lambda: all(not Path(f"/proc/{p}").exists() for p in pids))
        report["window_cancel_and_owned_process_cleanup"] = True
    except Exception:
        if window.process.poll() is None:
            (output / "failure-title.txt").write_text(window.title() + "\n")
            window.screenshot(output / "failure.png")
        raise
    finally:
        window.cleanup()
    if not args.baseline:
        failed = Window(binary, output, "/nonexistent/hoo19-test-shell", [])
        try:
            resize(failed, 640)
            failed.screenshot(output / "12-start-error-marker.png")
            failed.request_close()
            failed.wait_exit()
            report["start_error_visible_without_new_process"] = True
        finally:
            failed.cleanup()
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2), flush=True)


if __name__ == "__main__":
    main()
