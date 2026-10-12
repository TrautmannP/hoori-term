#!/usr/bin/env python3
"""Bundled HOO-13 X11 exercise in owned windows and isolated configuration.

Only synthetic terminal/input fixtures are used. Inspect all images;
process/title/byte assertions do not establish correct rendering.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import shlex
import signal
import time

base = runpy.run_path(str(Path(__file__).with_name("gui-identity-smoke.py")))
Window, command, wait_for, click, capture = (
    base[k] for k in ["Window", "command", "wait_for", "click", "capture"]
)


def edit(w, label, color_x, key):
    base["open_editor"](w)
    base["name"](w, label)
    time.sleep(.2)  # Settle the last text event before clicking away.
    click(w, color_x, 183)
    capture(w, key)
    w.key("Return")
    wait_for(lambda: "Tab bearbeiten" not in w.title() and label in w.title())


def read_json(path):
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def children(w):
    return sorted(int(p) for p in Path(
        f"/proc/{w.process.pid}/task/{w.process.pid}/children").read_text().split())


def start_time(pid):
    try:
        return Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1].split()[19]
    except OSError:
        return None


def clean_prompt(w, bash=False, state=None):
    prompt = ("PROMPT_COMMAND=;" if bash else
              "precmd_functions=(); preexec_functions=(); unfunction precmd preexec 2>/dev/null;")
    state = f"HOO13_STATE={state};" if state else ""
    w.run(f"unset HISTFILE; {prompt} PS1='HOO13> '; {state} "
          "printf '\\033[2J\\033[HOwned HOO-13 terminal fixture\\n'")


def directory(w, value, bash=False):
    click(w, 171 if bash else 112, 289)
    click(w, 300, 331)
    w.key("ctrl+a")
    w.type(str(value))


def save_current(w, value, bash=False):
    w.key("ctrl+shift+s")
    wait_for(lambda: "Als Vorlage speichern" in w.title())
    directory(w, value, bash)
    w.key("Return")
    wait_for(lambda: "Als Vorlage speichern" not in w.title())


def show_list(w):
    w.key("ctrl+shift+p")
    wait_for(lambda: " — Vorlagen — " in w.title())
    time.sleep(.15)


def edit_saved_directory(w, value, bash=False):
    # Select the first (Web) template using keyboard, independent of active tab.
    show_list(w)
    w.key("Home")
    w.key("e")
    wait_for(lambda: "Vorlage bearbeiten" in w.title())
    directory(w, value, bash)
    w.key("Return")
    wait_for(lambda: " — Vorlagen — " in w.title())
    w.key("Escape")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--resume-after-removal", type=Path,
                        help="Run only remaining scenarios using a preserved post-removal fixture")
    args = parser.parse_args()
    assert os.environ.get("XDG_SESSION_TYPE") == "x11", "This harness proves X11 only"
    binary, output = args.binary.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    windows_dir = output / "windows"
    windows_dir.mkdir()
    config = output / "config"
    file = config / "hoori-term/templates-v1.json"
    snapshots = output / "snapshots"
    snapshots.mkdir()
    literal = output / "space ; literal $(text)"
    bash_directory = output / "bash-home"
    literal.mkdir()
    bash_directory.mkdir()
    metadata = command(str(binary), "--version")
    commit = re.search(r"commit: ([0-9a-f]{40})", metadata).group(1)
    report = {
        "metadata": metadata, "commit": commit,
        "session_type": os.environ.get("XDG_SESSION_TYPE"),
        "display": os.environ.get("DISPLAY"),
        "os": Path("/etc/os-release").read_text(),
        "zsh": command("zsh", "--version"),
        "bash": command("bash", "--version").splitlines()[0],
        "isolated_config": str(config),
        "literal_directory": str(literal), "bash_directory": str(bash_directory),
        "screenshots": [], "phases": [],
    }
    old_config = os.environ.get("XDG_CONFIG_HOME")
    os.environ["XDG_CONFIG_HOME"] = str(config)
    windows, owned = [], {}

    def phase(name):
        report["phases"].append(name)
        (output / "progress.json").write_text(json.dumps(report, indent=2))
        print(json.dumps({"phase": name}), flush=True)

    def make_window(name):
        folder = windows_dir / name
        folder.mkdir()
        w = Window(binary, folder, "/usr/bin/zsh", ["-f", "-i"])
        windows.append(w)
        for pid in children(w):
            owned[pid] = start_time(pid)
        assert f"hoori-term [{commit[:7]}" in w.title()
        return w

    def track(w):
        pids = children(w)
        for pid in pids:
            owned[pid] = start_time(pid)
        return pids

    def shot(w, name):
        w.screenshot(output / name)
        report["screenshots"].append(name)

    def snapshot(name):
        value = read_json(file)
        assert value is not None
        (snapshots / name).write_text(json.dumps(value, ensure_ascii=False, indent=2))
        return value

    def close_window(w, pids, cancel=False):
        w.request_close()
        assert "Schließen bestätigen" in w.title()
        if cancel:
            w.key("Escape")
            wait_for(lambda: "Schließen bestätigen" not in w.title())
            assert children(w) == pids
            w.request_close()
        w.key("Return")
        w.wait_exit()
        wait_for(lambda: all(not Path(f"/proc/{pid}").exists() for pid in pids))

    raw_script = output / "raw-reader.py"
    raw_script.write_text(r'''import os,sys,termios,tty
from pathlib import Path
old=termios.tcgetattr(0)
tty.setraw(0)
os.write(1,b'\x1b]2;HOO-13 raw-ready\x07')
data=bytearray()
try:
    while not data.endswith(b'!'):
        data.extend(os.read(0,4096))
    Path(sys.argv[1]).write_bytes(data)
finally:
    termios.tcsetattr(0,termios.TCSANOW,old)
    os.write(1,b'\x1b]2;HOO-13 raw-done\x07')
''')

    def reader(w, name):
        proof = output / name
        w.run("python3 " + shlex.quote(str(raw_script)) + " " + shlex.quote(str(proof)))
        wait_for(lambda: "HOO-13 raw-ready" in w.title())
        return proof

    try:
        if args.resume_after_removal:
            previous = args.resume_after_removal.resolve()
            previous_report = read_json(previous / "failure-report.json")
            remaining = read_json(previous / "snapshots/03-admin-removed.json")
            assert previous_report and remaining and len(remaining["items"]) == 1
            backend_id, admin_id = previous_report["initial_template_ids"]
            assert remaining["items"][0]["id"] == backend_id
            literal = Path(remaining["items"][0]["directory"])
            bash_directory = Path(previous_report["bash_directory"])
            assert literal.is_dir() and bash_directory.is_dir()
            file.parent.mkdir(parents=True)
            file.write_text(json.dumps(remaining, ensure_ascii=False, indent=2))
            report.update({
                "scope": "remaining-scenarios-after-template-removal",
                "continued_from": str(previous / "failure-report.json"),
                "fixture_source": str(previous / "snapshots/03-admin-removed.json"),
                "literal_directory": str(literal),
                "bash_directory": str(bash_directory),
                "initial_template_ids": [backend_id, admin_id],
            })
            w = make_window("01-remainder-fixture")
            defaults = track(w)
            assert len(defaults) == 1
            wait_for(lambda: "Vorlagen werden geladen" not in w.title())
            time.sleep(.3)
            w.key("F11")
            wait_for(lambda: "Web" in w.title())
            web_pid = list(set(track(w)) - set(defaults))[0]
            assert Path(os.readlink(f"/proc/{web_pid}/cwd")) == literal
            clean_prompt(w, state="REMAINDER_WEB")
            w.marker("HOO-13 remainder Web ready")
            w.key("ctrl+shift+b")
            wait_for(lambda: "bash · 3" in w.title())
            clean_prompt(w, bash=True, state="REMAINDER_ADMIN")
            w.marker("HOO-13 remainder Admin ready")
            pids = track(w)
            assert len(pids) == 3
            remaining = snapshot("03-admin-removed.json")
            phase("remaining-scenarios-fixture-ready")
        else:
            w = make_window("01-initial")
            first = track(w)
            assert len(first) == 1
            # Preserve the established ordinary default startup directory behavior.
            assert Path(os.readlink(f"/proc/{first[0]}/cwd")) == Path(os.environ["HOME"]).resolve()
            clean_prompt(w, state="ONE")
            w.marker("HOO-13 one-ready")
            edit(w, "Backend", 256, "F8")
            w.key("ctrl+shift+s")
            wait_for(lambda: "Als Vorlage speichern" in w.title())
            directory(w, literal)
            shot(w, "01-save-zsh-template.png")
            w.key("Return")
            wait_for(lambda: "Als Vorlage speichern" not in w.title())
            wait_for(lambda: read_json(file) and len(read_json(file)["items"]) == 1)
            assert children(w) == first
            assert Path(os.readlink(f"/proc/{first[0]}/cwd")) != literal
            w.key("ctrl+shift+b")
            wait_for(lambda: "bash · 2" in w.title())
            clean_prompt(w, bash=True, state="TWO")
            w.marker("HOO-13 two-ready")
            edit(w, "Admin", 430, "F9")
            before_save = track(w)
            save_current(w, bash_directory, bash=True)
            wait_for(lambda: read_json(file) and len(read_json(file)["items"]) == 2)
            assert children(w) == before_save
            initial = snapshot("01-two-saved.json")
            backend_id, admin_id = [entry["id"] for entry in initial["items"]]
            report["initial_template_ids"] = [backend_id, admin_id]
            w.key("ctrl+shift+t")
            wait_for(lambda: "zsh · 3" in w.title())
            clean_prompt(w, state="THREE")
            w.marker("HOO-13 ordinary-ready")
            edit(w, "Temporary", 489, "F10")
            pids = track(w)
            assert len(pids) == 3
            assert len(read_json(file)["items"]) == 2
            report["initial_shell_pids"] = pids
            shot(w, "02-two-templates-and-ordinary-tab.png")
            for key, label, state in [("F8", "Backend", "ONE"), ("F9", "Admin", "TWO")]:
                w.key(key)
                wait_for(lambda: label in w.title())
                w.run("printf '\\033]2;HOO-13 retained %s\\007' \"$HOO13_STATE\"")
                wait_for(lambda: f"retained {state}" in w.title())
                assert children(w) == pids
            show_list(w)
            shot(w, "03-saved-list-live-tabs.png")
            w.key("Home")
            w.key("Return")
            wait_for(lambda: "Backend" in w.title() and " — Vorlagen — " not in w.title())
            assert children(w) == pids
            edit(w, "Web", 311, "F11")
            wait_for(lambda: read_json(file)["items"][0]["identity"]["name"] == "Web")
            edited = snapshot("02-auto-edited.json")
            assert edited["items"][0]["id"] == backend_id
            assert edited["items"][0]["identity"]["accent"] == "green"
            assert edited["items"][0]["identity"]["shortcut"]["key"] == "F11"
            w.key("ctrl+shift+e")
            wait_for(lambda: "Tab bearbeiten" in w.title())
            capture(w, "F9")
            shot(w, "04-template-shortcut-conflict.png")
            assert read_json(file) == edited
            w.key("Escape")
            w.key("Escape")
            wait_for(lambda: "Tab bearbeiten" not in w.title())
            assert children(w) == pids
            report["save_link_auto_edit_conflict_and_ordinary_tab"] = True
            phase("saved-linked-configuration-without-extra-processes")
            close_window(w, pids, cancel=True)

            w = make_window("02-restart")
            default_pid = track(w)
            assert len(default_pid) == 1
            time.sleep(.3)
            assert children(w) == default_pid
            assert read_json(file) == edited
            show_list(w)
            shot(w, "05-restart-no-template-autostart.png")
            w.key("Escape")
            w.key("F11")
            wait_for(lambda: "Web" in w.title())
            web_pid = list(set(track(w)) - set(default_pid))[0]
            assert Path(os.readlink(f"/proc/{web_pid}/cwd")) == literal
            assert Path(os.readlink(f"/proc/{web_pid}/exe")).name == "zsh"
            clean_prompt(w, state="REOPENED_WEB")
            w.marker("HOO-13 Web directory verified")
            show_list(w)
            w.key("Home")
            w.key("Return")
            wait_for(lambda: "Web" in w.title() and " — Vorlagen — " not in w.title())
            assert children(w) == sorted([*default_pid, web_pid])
            shot(w, "06-reopened-zsh-literal-directory.png")
            w.key("F9")
            wait_for(lambda: "Admin" in w.title())
            admin_pid = list(set(track(w)) - set(default_pid) - {web_pid})[0]
            assert Path(os.readlink(f"/proc/{admin_pid}/cwd")) == bash_directory
            assert Path(os.readlink(f"/proc/{admin_pid}/exe")).name == "bash"
            clean_prompt(w, bash=True, state="REOPENED_ADMIN")
            w.marker("HOO-13 Bash directory verified")
            pids = track(w)
            shot(w, "07-reopened-bash-directory.png")
            report["restarted_shell_pids"] = pids
            report["reopened_web_pid"] = web_pid
            report["reopened_admin_pid"] = admin_pid
            assert read_json(file) == edited  # Runtime titles are not configuration.
            proof = reader(w, "routing.bin")
            w.key("F11")
            wait_for(lambda: "Web" in w.title())
            w.key("F9")
            wait_for(lambda: "Admin" in w.title() and "raw-ready" in w.title())
            show_list(w)
            w.key("a")
            w.key("Escape")
            w.key("ctrl+shift+e")
            wait_for(lambda: "Tab bearbeiten" in w.title())
            w.type("draft only")
            w.key("Escape")
            w.key("F12")
            w.type("!")
            wait_for(proof.exists)
            assert proof.read_bytes() == b"\x1b[24~!"
            wait_for(lambda: "raw-done" in w.title())
            assert children(w) == pids
            report["restart_no_autostart_exact_directories_repeated_open_and_raw_routing"] = True
            phase("restart-and-literal-directory-routing-proved")

            proof = reader(w, "removed-shortcut.bin")
            show_list(w)
            w.key("Home")
            w.key("Down")
            w.key("Delete")
            wait_for(lambda: len(read_json(file)["items"]) == 1)
            remaining = snapshot("03-admin-removed.json")
            assert remaining["items"][0]["id"] == backend_id
            assert children(w) == pids
            w.key("Escape")
            assert "Admin" in w.title()
            w.key("F9")  # Removed inherited binding now reaches the temporary tab.
            w.type("!")
            wait_for(proof.exists)
            assert proof.read_bytes() == b"\x1b[20~!"
            wait_for(lambda: "raw-done" in w.title())
            shot(w, "08-remove-keeps-running-tab.png")
        edit(w, "Temporary Admin", 430, "F10")
        assert read_json(file) == remaining
        # Explicitly saving the now-temporary tab creates a fresh saved ID.
        save_current(w, bash_directory, bash=True)
        wait_for(lambda: len(read_json(file)["items"]) == 2)
        replacement = snapshot("04-fresh-template-id.json")
        new_id = replacement["items"][1]["id"]
        assert new_id > admin_id
        assert replacement["items"][1]["identity"]["shortcut"]["key"] == "F10"
        assert children(w) == pids
        report["removed_template_keeps_process_clears_binding_and_fresh_id"] = True

        w.key("F11")
        wait_for(lambda: "Web" in w.title())
        w.run("exit 7")
        wait_for(lambda: not Path(f"/proc/{web_pid}").exists())
        exited_pids = track(w)
        w.key("F11")
        assert children(w) == exited_pids
        shot(w, "09-open-exited-template-without-restart.png")
        w.key("ctrl+shift+w")
        wait_for(lambda: "Web" not in w.title())
        assert "Schließen bestätigen" not in w.title()
        w.key("F11")
        wait_for(lambda: "Web" in w.title())
        reopened_pid = list(set(track(w)) - set(exited_pids))[0]
        assert reopened_pid != web_pid
        assert Path(os.readlink(f"/proc/{reopened_pid}/cwd")) == literal
        clean_prompt(w)
        w.marker("HOO-13 reopened after close")
        # Future start data must not restart or relocate this running PTY.
        missing = output / "missing-directory"
        edit_saved_directory(w, missing)
        wait_for(lambda: read_json(file)["items"][0]["directory"] == str(missing))
        w.key("F11")
        assert Path(os.readlink(f"/proc/{reopened_pid}/cwd")) == literal
        w.key("ctrl+shift+w")
        wait_for(lambda: "Schließen bestätigen" in w.title())
        w.key("Return")
        wait_for(lambda: not Path(f"/proc/{reopened_pid}").exists())
        before_failed = track(w)
        w.key("F11")
        wait_for(lambda: "Web" in w.title())
        assert children(w) == before_failed  # Invalid directory never spawns HOME.
        shot(w, "10-invalid-directory-visible-no-fallback.png")
        w.key("F11")
        assert children(w) == before_failed
        edit_saved_directory(w, literal)
        w.key("F11")
        assert children(w) == before_failed  # Correcting data does not restart.
        w.key("ctrl+shift+w")
        wait_for(lambda: "Web" not in w.title())
        assert "Schließen bestätigen" not in w.title()
        w.key("F11")
        wait_for(lambda: "Web" in w.title())
        repaired_pid = list(set(track(w)) - set(before_failed))[0]
        assert Path(os.readlink(f"/proc/{repaired_pid}/cwd")) == literal
        clean_prompt(w)
        w.marker("HOO-13 corrected directory")
        shot(w, "11-corrected-template-reopened.png")
        report["exited_targets_and_invalid_directory_recovery"] = True
        phase("removal-exit-and-directory-error-recovery-proved")

        # Deterministic write failure in this test's own configuration only.
        before_error = file.read_bytes()
        lock = file.with_suffix(".lock")
        saved_lock = lock.with_name("fixture-lock-backup")
        lock.rename(saved_lock)
        lock.mkdir()
        edit(w, "Web Unsaved", 311, "F11")
        time.sleep(.3)
        assert file.read_bytes() == before_error
        assert "Web Unsaved" in w.title()
        shot(w, "12-write-failure-visible-unsaved.png")
        lock.rmdir()
        saved_lock.rename(lock)
        click(w, 85, 94)  # Retry button below the single-line storage error.
        wait_for(lambda: read_json(file)["items"][0]["identity"]["name"] == "Web Unsaved")
        shot(w, "13-write-retry-success.png")
        current = snapshot("05-after-write-retry.json")
        assert current["items"][0]["id"] == backend_id
        assert len(children(w)) == 3
        final_pids = track(w)
        report["write_failure_preserves_file_and_retry_saves_latest_configuration"] = True
        close_window(w, final_pids)
        good = file.read_bytes()
        (snapshots / "valid-final-bytes.json").write_bytes(good)
        phase("write-error-and-retry-proved")

        for index, (name, bad) in enumerate([
            ("corrupt", b'{"version":1,"items":'),
            ("unsupported", json.dumps({**current, "version": 2}).encode()),
        ], start=3):
            file.write_bytes(bad)
            (snapshots / f"{name}-input.txt").write_bytes(bad)
            w = make_window(f"0{index}-{name}")
            one = track(w)
            assert len(one) == 1
            time.sleep(.3)
            w.key("ctrl+shift+s")
            assert "Als Vorlage speichern" not in w.title()
            assert file.read_bytes() == bad
            assert children(w) == one
            shot(w, f"{11+index:02d}-{name}-file-visible-preserved.png")
            close_window(w, one)
            assert file.read_bytes() == bad
        file.write_bytes(good)
        report["corrupt_and_unsupported_files_visible_preserved_without_autostart"] = True
        report["final_config_sha256"] = hashlib.sha256(good).hexdigest()
        report["all_owned_shells_reaped"] = all(
            start_time(pid) != stamp for pid, stamp in owned.items())
        assert report["all_owned_shells_reaped"]
        report["complete"] = True
        phase("complete")
        (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps(report, ensure_ascii=False, indent=2), flush=True)
    except Exception as error:
        report["complete"] = False
        report["failure"] = f"{type(error).__name__}: {error}"
        if windows and windows[-1].process.poll() is None:
            try:
                windows[-1].screenshot(output / "failure.png")
            except Exception as screenshot_error:
                report["failure_screenshot_error"] = type(screenshot_error).__name__
        (output / "failure-report.json").write_text(json.dumps(report, indent=2))
        raise
    finally:
        # Cleanup is constrained to processes descended from these owned windows.
        for w in windows:
            if w.process.poll() is None:
                for pid in children(w):
                    owned[pid] = start_time(pid)
            w.cleanup()
        for pid, stamp in owned.items():
            if stamp and start_time(pid) == stamp:
                try:
                    os.kill(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
        if old_config is None:
            os.environ.pop("XDG_CONFIG_HOME", None)
        else:
            os.environ["XDG_CONFIG_HOME"] = old_config


if __name__ == "__main__":
    main()
