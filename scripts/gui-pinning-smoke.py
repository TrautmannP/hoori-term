#!/usr/bin/env python3
"""Bundled HOO-17 pinning exercise on X11, using owned synthetic windows.

Inspect every image separately from PID/config/byte assertions. The isolated
configuration and all failure attempts remain available with their source hash.
--only supports targeted repetition of an affected phase; restart needs a
previous valid catalog snapshot. No existing user clipboard is read or changed.
"""
import argparse
import json
import os
from pathlib import Path
import re
import runpy
import shlex
import signal
import time

base = runpy.run_path(str(Path(__file__).with_name('gui-templates-smoke.py')))
Window, command, wait_for, click, edit, children, start_time, read_json = (
    base[key] for key in ['Window', 'command', 'wait_for', 'click', 'edit',
                         'children', 'start_time', 'read_json'])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('binary', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--only', choices=['all', 'live', 'restart', 'errors'], default='all')
    parser.add_argument('--catalog-from', type=Path)
    args = parser.parse_args()
    assert os.environ.get('XDG_SESSION_TYPE') == 'x11', 'This exercise proves X11 only'
    binary, output = args.binary.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    windows_dir, snapshots = output / 'windows', output / 'snapshots'
    windows_dir.mkdir()
    snapshots.mkdir()
    config = output / 'config'
    file = config / 'hoori-term/templates-v1.json'
    literal, bash_dir = output / 'space ; literal $(text)', output / 'bash-home'
    literal.mkdir()
    bash_dir.mkdir()
    if args.catalog_from:
        source = read_json(args.catalog_from.resolve())
        assert source and len(source['items']) == 2
        # Keep real, original literal directories when repeating restart only.
        literal, bash_dir = [Path(t['directory']) for t in source['items']]
        assert literal.is_dir() and bash_dir.is_dir()
        file.parent.mkdir(parents=True)
        file.write_text(json.dumps(source, ensure_ascii=False, indent=2))
    metadata = command(str(binary), '--version')
    commit = re.search(r'commit: ([0-9a-f]{40})', metadata).group(1)
    report = {'commit': commit, 'metadata': metadata, 'scope': args.only,
              'catalog_from': str(args.catalog_from) if args.catalog_from else None,
              'session_type': os.environ.get('XDG_SESSION_TYPE'), 'display': os.environ.get('DISPLAY'),
              'os': Path('/etc/os-release').read_text(), 'screenshots': [], 'phases': [],
              'isolated_config': str(config), 'literal_directory': str(literal),
              'bash_directory': str(bash_dir)}
    old_config = os.environ.get('XDG_CONFIG_HOME')
    os.environ['XDG_CONFIG_HOME'] = str(config)
    windows, owned = [], {}

    def phase(label):
        report['phases'].append(label)
        (output / 'progress.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps({'phase': label}), flush=True)

    def track(w):
        ids = children(w)
        for pid in ids:
            owned.setdefault(pid, start_time(pid))
        return ids

    def make_window(label, width=1280):
        directory = windows_dir / label
        directory.mkdir()
        w = Window(binary, directory, '/usr/bin/zsh', ['-f', '-i'])
        windows.append(w)
        track(w)
        assert f'hoori-term [{commit[:7]}' in w.title()
        command('xdotool', 'windowsize', '--sync', w.id, str(width), '700')
        time.sleep(.3)
        return w

    def shot(w, name):
        command('xdotool', 'mousemove', '--window', w.id, '600', '420')
        time.sleep(.15)
        w.screenshot(output / name)
        report['screenshots'].append(name)

    def snapshot(name):
        value = read_json(file)
        assert value
        (snapshots / name).write_text(json.dumps(value, ensure_ascii=False, indent=2))
        return value

    def activate(w, key, label):
        w.key(key)
        wait_for(lambda: label in w.title())
        time.sleep(.12)

    def clean(w, state, bash=False):
        w.key('ctrl+u')
        base['clean_prompt'](w, bash=bash)
        w.run(f"HOO17_STATE={state}; printf '\\033[2J\\033[HOwned HOO-17 {state}\\n'")
        w.marker(f'HOO-17 {state}-ready')

    def pin(w):
        click(w, 65, 635)  # Bottom toolbar in the 700px client; never a title-bar action.
        time.sleep(.2)

    def explicit_close(w):
        click(w, 235, 635)

    def save(w, directory, bash=False):
        base['save_current'](w, directory, bash=bash)

    def close_window(w, cancel=False, image=None):
        ids = track(w)
        w.request_close()
        assert 'Schließen bestätigen' in w.title()
        if image:
            shot(w, image)
        if cancel:
            w.key('Escape')
            wait_for(lambda: 'Schließen bestätigen' not in w.title())
            assert track(w) == ids
            w.request_close()
        w.key('Return')
        w.wait_exit()
        wait_for(lambda: all(start_time(pid) != owned[pid] for pid in ids))

    raw = output / 'raw-reader.py'
    raw.write_text(r'''import json,os,sys,termios,tty
from pathlib import Path
old=termios.tcgetattr(0)
tty.setraw(0)
Path(sys.argv[2]).write_text(json.dumps({'pid':os.getpid()}))
os.write(1,b'\x1b]2;HOO-17 raw-ready\x07')
data=bytearray()
try:
 while not data.endswith(b'!'): data.extend(os.read(0,4096))
 Path(sys.argv[1]).write_bytes(data)
finally:
 termios.tcsetattr(0,termios.TCSANOW,old)
 os.write(1,b'\x1b]2;HOO-17 raw-done\x07')
''')

    def reader(w, name):
        proof, pid_file = output / name, output / (name + '.pid.json')
        w.run('python3 ' + shlex.quote(str(raw)) + ' ' + shlex.quote(str(proof)) + ' ' + shlex.quote(str(pid_file)))
        wait_for(lambda: 'HOO-17 raw-ready' in w.title() and read_json(pid_file))
        pid = read_json(pid_file)['pid']
        owned[pid] = start_time(pid)
        return proof

    try:
        if args.only in ['all', 'live']:
            w = make_window('01-live', width=1000)
            first = track(w)
            assert len(first) == 1
            clean(w, 'BACKEND')
            edit(w, 'Backend', 311, 'F8')
            pin(w)
            save(w, literal)
            wait_for(lambda: read_json(file) and len(read_json(file)['items']) == 1)
            assert read_json(file)['items'][0]['pinned'] is True
            assert track(w) == first
            w.key('ctrl+shift+b')
            wait_for(lambda: 'bash · 2' in w.title())
            admin_pid = list(set(track(w)) - set(first))[0]
            clean(w, 'ADMIN', bash=True)
            edit(w, 'Admin', 430, 'F9')
            save(w, bash_dir, bash=True)
            wait_for(lambda: len(read_json(file)['items']) == 2)
            assert read_json(file)['items'][1]['pinned'] is False
            initial_ids = [t['id'] for t in read_json(file)['items']]
            pin(w)
            wait_for(lambda: read_json(file)['items'][1]['pinned'] is True)
            w.key('ctrl+shift+t')
            wait_for(lambda: 'zsh · 3' in w.title())
            scratch_pid = list(set(track(w)) - set(first) - {admin_pid})[0]
            clean(w, 'SCRATCH')
            edit(w, 'Scratch', 256, 'F10')
            pin(w)  # Temporary: this must not create a third template.
            w.key('ctrl+shift+b')
            wait_for(lambda: 'bash · 4' in w.title())
            clean(w, 'LOGS', bash=True)
            edit(w, 'Logs', 370, 'F11')
            pids = track(w)
            assert len(pids) == 4 and len(read_json(file)['items']) == 2
            report['initial_shell_pids'] = pids
            report['original_target_pids'] = {'Backend': first[0], 'Admin': admin_pid, 'Scratch': scratch_pid}
            command('xdotool', 'windowsize', '--sync', w.id, '1280', '700')
            shot(w, '01-mixed-pinned-linked-and-ordinary.png')
            activate(w, 'F8', 'Backend')
            pin(w)  # Backend to ordinary: Admin/Scratch retain their relative order.
            wait_for(lambda: read_json(file)['items'][0]['pinned'] is False)
            activate(w, 'F10', 'Scratch')
            pin(w)  # Scratch to end of ordinary group.
            activate(w, 'F8', 'Backend')
            pin(w)
            wait_for(lambda: read_json(file)['items'][0]['pinned'] is True)
            activate(w, 'F10', 'Scratch')
            pin(w)
            assert track(w) == pids
            state = snapshot('01-pinned-templates.json')
            assert [t['id'] for t in state['items']] == initial_ids
            assert all(t['pinned'] for t in state['items'])
            w.key('ctrl+shift+l')
            w.key('Home')
            w.key('Return')
            order = [('Admin', 'ADMIN'), ('Backend', 'BACKEND'), ('Scratch', 'SCRATCH'), ('Logs', 'LOGS')]
            for index, (label, marker) in enumerate(order):
                if index:
                    w.key('ctrl+Tab')
                wait_for(lambda: label + ' · ' in w.title())
                w.run("printf '\\033]2;HOO-17 retained %s\\007' \"$HOO17_STATE\"")
                wait_for(lambda: 'retained ' + marker in w.title())
                assert track(w) == pids
            report['observed_group_order'] = [label for label, _ in order]
            report['pin_moves_preserve_ids_shortcuts_names_colors_processes_and_state'] = True
            shot(w, '02-stable-group-order-after-pin-changes.png')
            command('xdotool', 'windowsize', '--sync', w.id, '640', '700')
            activate(w, 'F10', 'Scratch')
            shot(w, '03-minimum-width-pinned-shortcut-target.png')
            command('xdotool', 'windowsize', '--sync', w.id, '1280', '700')
            activate(w, 'F9', 'Admin')
            ticker = output / 'inactive-output.py'
            ticker_pid = output / 'inactive-output-pid.json'
            ticker.write_text("import json,os,sys,time\nfrom pathlib import Path\nPath(sys.argv[1]).write_text(json.dumps({'pid':os.getpid()}))\nfor i in range(40):\n print('HOO17 inactive line',i,flush=True); time.sleep(.025)\n")
            w.run('python3 ' + shlex.quote(str(ticker)) + ' ' + shlex.quote(str(ticker_pid)) + "; printf '\\033]2;HOO-17 inactive-done\\007'")
            wait_for(lambda: read_json(ticker_pid))
            pid = read_json(ticker_pid)['pid']
            owned[pid] = start_time(pid)
            activate(w, 'F10', 'Scratch')
            proof = reader(w, 'protected-routing.bin')
            pin(w)
            pin(w)  # Both property actions also remain outside the raw PTY.
            w.key('ctrl+shift+w')
            assert 'Schließen bestätigen' not in w.title()
            shot(w, '04-normal-close-protected.png')
            click(w, 660, 30)  # Scratch × in Admin/Backend/Scratch/Logs order.
            assert 'Schließen bestätigen' not in w.title()
            w.key('ctrl+shift+l')
            w.key('Delete')
            assert 'Schließen bestätigen' not in w.title()
            assert track(w) == pids
            explicit_close(w)
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            shot(w, '05-explicit-pinned-close-confirmation.png')
            w.type('ignored')
            w.key('Escape')
            wait_for(lambda: 'Schließen bestätigen' not in w.title())
            assert track(w) == pids and not proof.exists()
            activate(w, 'F8', 'Backend')
            activate(w, 'F10', 'Scratch')
            w.key('F12')
            w.type('plain!')
            wait_for(proof.exists)
            assert proof.read_bytes() == b'\x1b[24~plain!', proof.read_bytes()
            wait_for(lambda: 'raw-done' in w.title())
            report['normal_shortcut_mouse_and_delete_protected_explicit_cancel_no_hidden_pty_bytes'] = True
            activate(w, 'F9', 'Admin')
            wait_for(lambda: 'inactive-done' in w.title())
            shot(w, '06-inactive-pinned-output-consumed.png')
            report['inactive_output_consumed_after_group_changes'] = True
            w.request_close()
            shot(w, '07-window-confirmation-includes-pins-and-ordinary.png')
            w.key('Escape')
            wait_for(lambda: 'Schließen bestätigen' not in w.title())
            assert track(w) == pids and read_json(file) == state
            report['window_cancel_retains_all_pins_processes_and_configuration'] = True
            activate(w, 'F10', 'Scratch')
            pin(w)
            w.key('ctrl+shift+w')
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            shot(w, '08-unpin-restores-normal-named-close.png')
            w.key('Escape')
            wait_for(lambda: 'Schließen bestätigen' not in w.title())
            pin(w)
            explicit_close(w)
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            w.key('Return')
            wait_for(lambda: start_time(scratch_pid) != owned[scratch_pid])
            survivors = sorted(set(pids) - {scratch_pid})
            assert track(w) == survivors
            report['explicit_confirm_closes_only_running_target'] = True
            activate(w, 'F9', 'Admin')
            w.run('exit 7')
            wait_for(lambda: start_time(admin_pid) != owned[admin_pid])
            time.sleep(.25)
            w.key('ctrl+shift+w')
            assert 'Schließen bestätigen' not in w.title() and 'Admin · 2' in w.title()
            activate(w, 'F9', 'Admin · 2')  # Exited linked tab must not restart.
            assert track(w) == sorted(set(survivors) - {admin_pid})
            w.key('ctrl+shift+w')
            shot(w, '09-exited-pinned-tab-still-protected.png')
            explicit_close(w)
            wait_for(lambda: 'Admin' not in w.title())
            assert 'Schließen bestätigen' not in w.title()
            assert read_json(file) == state
            before = track(w)
            activate(w, 'F9', 'Admin · 5')
            new_pid = list(set(track(w)) - set(before))[0]
            assert new_pid != admin_pid and Path(os.readlink(f'/proc/{new_pid}/cwd')) == bash_dir
            shot(w, '10-explicit-closed-template-reopens-pinned.png')
            assert read_json(file) == state
            report['exited_pin_protected_repeated_open_no_restart_explicit_close_keeps_template'] = True
            snapshot('02-before-restart.json')
            close_window(w)
            phase('live-pin-groups-protection-explicit-close-and-window-rules')

        if args.only in ['all', 'restart']:
            assert file.exists(), 'Use --catalog-from for a targeted restart phase'
            state = snapshot('03-restart-source.json')
            ids = [t['id'] for t in state['items']]
            assert len(ids) == 2 and all(t['pinned'] for t in state['items'])
            w = make_window('02-restart')
            defaults = track(w)
            assert len(defaults) == 1
            time.sleep(.3)
            base['show_list'](w)
            shot(w, '11-restart-retains-pins-without-autostart.png')
            assert track(w) == defaults and read_json(file) == state
            w.key('Escape')
            activate(w, 'F8', 'Backend · 2')
            backend_pid = list(set(track(w)) - set(defaults))[0]
            assert Path(os.readlink(f'/proc/{backend_pid}/cwd')) == literal
            assert Path(os.readlink(f'/proc/{backend_pid}/exe')).name == 'zsh'
            clean(w, 'RESTORED_BACKEND')
            activate(w, 'F9', 'Admin · 3')
            admin_pid = list(set(track(w)) - set(defaults) - {backend_pid})[0]
            assert Path(os.readlink(f'/proc/{admin_pid}/cwd')) == bash_dir
            assert Path(os.readlink(f'/proc/{admin_pid}/exe')).name == 'bash'
            clean(w, 'RESTORED_ADMIN', bash=True)
            pids = track(w)
            activate(w, 'F8', 'Backend · 2')
            base['show_list'](w)
            w.key('Home')
            w.key('Return')
            wait_for(lambda: ' — Vorlagen — ' not in w.title())
            activate(w, 'F9', 'Admin · 3')
            assert track(w) == pids and read_json(file) == state
            report['restart_no_autostart_pins_applied_on_deliberate_open_exact_shells_directories_and_no_duplicates'] = True
            base['show_list'](w)
            w.key('End')
            w.key('Delete')
            wait_for(lambda: len(read_json(file)['items']) == 1)
            assert read_json(file)['items'][0]['id'] == ids[0]
            w.key('Escape')
            assert 'Admin · 3' in w.title() and track(w) == pids
            shot(w, '12-template-removal-keeps-temporary-pin-and-process.png')
            w.key('ctrl+shift+w')
            assert 'Schließen bestätigen' not in w.title()
            proof = reader(w, 'removed-template-shortcut.bin')
            w.key('F9')
            w.type('!')
            wait_for(proof.exists)
            assert proof.read_bytes() == b'\x1b[20~!'
            wait_for(lambda: 'raw-done' in w.title())
            assert track(w) == pids
            explicit_close(w)
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            w.key('Return')
            wait_for(lambda: start_time(admin_pid) != owned[admin_pid])
            report['removing_template_keeps_pin_and_process_clears_inherited_shortcut'] = True
            activate(w, 'F8', 'Backend · 2')
            before = file.read_bytes()
            lock = file.with_suffix('.lock')
            assert lock.is_file()
            lock.unlink()
            lock.mkdir()  # Deterministic owned write error, without permission assumptions.
            pin(w)
            time.sleep(.4)
            assert file.read_bytes() == before
            assert track(w) == sorted([*defaults, backend_pid])
            shot(w, '13-pin-write-error-visible-old-file-retained.png')
            lock.rmdir()
            click(w, 85, 94)  # Retry in the visible storage-error banner.
            wait_for(lambda: read_json(file)['items'][0]['pinned'] is False)
            assert read_json(file)['items'][0]['id'] == ids[0]
            shot(w, '14-retry-persists-same-template-id.png')
            pin(w)
            wait_for(lambda: read_json(file)['items'][0]['pinned'] is True)
            report['pin_write_error_preserves_old_file_and_retry_saves_pending_state'] = True
            report['restarted_shell_pids'] = pids
            close_window(w)
            phase('restart-explicit-open-unlink-and-pin-write-retry')

        if args.only in ['all', 'errors']:
            # New field validation and old field absence, with separate files.
            original = read_json(file)
            if not original:
                original = {'version': 1, 'next_id': 2, 'items': [{
                    'id': 1, 'identity': {'name': 'Legacy', 'accent': 'green',
                    'shortcut': {'key': 'F8', 'modifiers': 0}},
                    'shell': 'zsh', 'directory': str(literal), 'pinned': True}]}
            for mode in ['invalid-pin', 'legacy']:
                root = output / ('config-' + mode)
                target = root / 'hoori-term/templates-v1.json'
                target.parent.mkdir(parents=True)
                value = json.loads(json.dumps(original))
                for entry in value['items']:
                    if mode == 'invalid-pin':
                        entry['pinned'] = 'true'
                    else:
                        entry.pop('pinned', None)
                target.write_text(json.dumps(value, ensure_ascii=False, indent=2))
                original_bytes = target.read_bytes()
                os.environ['XDG_CONFIG_HOME'] = str(root)
                w = make_window('03-' + mode)
                defaults = track(w)
                assert len(defaults) == 1
                if mode == 'invalid-pin':
                    pin(w)  # Ordinary pinning must not overwrite a rejected file.
                    w.key('ctrl+shift+s')
                    time.sleep(.2)
                    assert 'Als Vorlage speichern' not in w.title()
                    shot(w, '15-invalid-pin-preserved-and-not-silently-saved.png')
                    assert track(w) == defaults
                else:
                    activate(w, 'F8', value['items'][0]['identity']['name'])
                    assert len(track(w)) == 2
                    w.key('ctrl+shift+w')
                    wait_for(lambda: 'Schließen bestätigen' in w.title())
                    shot(w, '16-legacy-template-opens-unpinned-normal-close.png')
                    w.key('Escape')
                    wait_for(lambda: 'Schließen bestätigen' not in w.title())
                assert target.read_bytes() == original_bytes
                close_window(w)
            report['invalid_pin_preserves_file_and_legacy_without_pin_opens_unpinned'] = True
            phase('pin-schema-error-and-legacy-compatibility')

        assert all(start_time(pid) != stamp for pid, stamp in owned.items())
        report['all_owned_children_reaped'] = True
        report['complete'] = True
        (output / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps(report, ensure_ascii=False, indent=2), flush=True)
    except BaseException:
        report['complete'] = False
        (output / 'failure-report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
        for w in windows:
            if w.process.poll() is None:
                w.screenshot(output / 'failure.png')
                break
        raise
    finally:
        for w in windows:
            w.cleanup()
        time.sleep(.2)
        for pid, stamp in owned.items():
            if start_time(pid) == stamp:
                try:
                    os.kill(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
        (output / 'cleanup.json').write_text(json.dumps({
            str(pid): {'start_time': stamp, 'still_running': start_time(pid) == stamp}
            for pid, stamp in owned.items()}, indent=2))
        if old_config is None:
            os.environ.pop('XDG_CONFIG_HOME', None)
        else:
            os.environ['XDG_CONFIG_HOME'] = old_config


if __name__ == '__main__':
    main()
