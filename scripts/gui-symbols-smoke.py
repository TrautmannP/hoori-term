#!/usr/bin/env python3
"""Bundled HOO-22 X11 exercise, with owned windows and isolated configuration.

All clipboard contents are synthetic fixtures supplied by this script; old
contents are never read. Inspect each PNG independently of PID/byte assertions.
--only and --catalog-from permit targeted continuation of a failed phase.
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

helpers = runpy.run_path(str(Path(__file__).with_name('gui-window-chrome-smoke.py')))
base = helpers['helpers']
Window, command, wait_for, click, start_time, read_json = [base[n] for n in
    ('Window', 'command', 'wait_for', 'click', 'start_time', 'read_json')]
resize, clipboard, configure, geometry, long_name = [helpers[n] for n in
    ('resize', 'clipboard', 'configure', 'geometry', 'long_name')]
SYMBOLS = ['terminal', 'code', 'folder', 'server', 'database', 'branch', 'star', 'shield']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--only', choices=['all', 'live', 'restart', 'layouts', 'errors'], default='all')
    parser.add_argument('--catalog-from', type=Path)
    args = parser.parse_args()
    assert os.environ.get('XDG_SESSION_TYPE') == 'x11', 'This exercise proves X11 only'
    binary, output = args.binary.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    config = output / 'config'
    file = config / 'hoori-term/templates-v1.json'
    snapshots = output / 'snapshots'
    snapshots.mkdir()
    windows_dir = output / 'windows'
    windows_dir.mkdir()
    literal, bash_dir = output / 'space ; literal $(text)', output / 'bash-home'
    literal.mkdir()
    bash_dir.mkdir()
    if args.catalog_from:
        catalog = read_json(args.catalog_from.resolve())
        assert catalog and len(catalog['items']) == 2
        literal, bash_dir = [Path(t['directory']) for t in catalog['items']]
        assert literal.is_dir() and bash_dir.is_dir()
        file.parent.mkdir(parents=True)
        file.write_text(json.dumps(catalog, ensure_ascii=False, indent=2))
    metadata = command(str(binary), '--version')
    commit = re.search(r'commit: ([0-9a-f]{40})', metadata).group(1)
    report = {'commit': commit, 'metadata': metadata, 'scope': args.only,
              'catalog_from': str(args.catalog_from) if args.catalog_from else None,
              'os': Path('/etc/os-release').read_text(), 'display': os.environ.get('DISPLAY'),
              'session_type': os.environ.get('XDG_SESSION_TYPE'),
              'zsh': command('zsh', '--version'), 'bash': command('bash', '--version').splitlines()[0],
              'logical_sizes': [], 'screenshots': [], 'phases': [],
              'clipboard': 'Generated test fixtures only; no old clipboard read.',
              'isolated_config': str(config)}
    old_config = os.environ.get('XDG_CONFIG_HOME')
    os.environ['XDG_CONFIG_HOME'] = str(config)
    windows, owned = [], {}

    def phase(name):
        report['phases'].append(name)
        (output / 'progress.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps({'phase': name}), flush=True)

    def track(w):
        pids = base['children'](w)
        for pid in pids:
            owned.setdefault(pid, start_time(pid))
        return pids

    def make_window(name, shell='/usr/bin/zsh'):
        directory = windows_dir / name
        directory.mkdir()
        w = Window(binary, directory, shell, ['-f', '-i'])
        windows.append(w)
        track(w)
        assert f'hoori-term [{commit[:7]}' in w.title()
        return w

    def shot(w, name):
        command('xdotool', 'mousemove', '--window', w.id, '600', '550')
        time.sleep(.15)
        w.screenshot(output / name)
        report['screenshots'].append(name)

    def sized(w, width, height=700):
        report['logical_sizes'].append(resize(w, width, height))

    def snapshot(name):
        catalog = read_json(file)
        assert catalog
        (snapshots / name).write_text(json.dumps(catalog, ensure_ascii=False, indent=2))
        return catalog

    def activate(w, key, name):
        w.key(key)
        wait_for(lambda: name in w.title())
        time.sleep(.12)

    def clean(w, value, bash=False):
        w.key('ctrl+u')
        base['clean_prompt'](w, bash=bash)
        w.run(f"HOO22_STATE={value}; printf '\\033[2J\\033[HOwned HOO-22 {value}\\n'")
        w.marker(f'HOO-22 {value}-ready')

    def chooser(w):
        w.key('ctrl+shift+e')
        wait_for(lambda: 'Tab bearbeiten' in w.title())
        w.key('F6')
        time.sleep(.15)

    def choose(w, symbol, mouse=False):
        # At 1000x700 every catalog entry is visible; keyboard works at any size.
        chooser(w)
        index = SYMBOLS.index(symbol) + 1 if symbol else 0
        if mouse:
            click(w, [170, 500, 825][index % 3], 160 + (index // 3) * 48)
        else:
            w.key('Home')
            for _ in range(index // 3):
                w.key('Down')
            for _ in range(index % 3):
                w.key('Right')
            w.key('Return')
        w.key('Return')
        wait_for(lambda: 'Tab bearbeiten' not in w.title())

    def close(w, cancel=False, image=None):
        pids = track(w)
        w.request_close()
        assert 'Schließen bestätigen' in w.title()
        if image:
            shot(w, image)
        if cancel:
            w.key('Escape')
            wait_for(lambda: 'Schließen bestätigen' not in w.title())
            assert track(w) == pids
            w.request_close()
        w.key('Return')
        w.wait_exit()
        wait_for(lambda: all(start_time(pid) != owned[pid] for pid in pids))

    raw = output / 'raw-reader.py'
    raw.write_text(r'''import json,os,signal,sys,termios,tty
from pathlib import Path
old=termios.tcgetattr(0)
def size(*_):
 s=os.get_terminal_size(0)
 Path(sys.argv[2]).write_text(json.dumps({'pid':os.getpid(),'columns':s.columns,'lines':s.lines}))
signal.signal(signal.SIGWINCH,size)
tty.setraw(0)
size()
os.write(1,b'\x1b[2J\x1b[HHOO22_SELECTION\r\n\x1b[?2004h\x1b]2;HOO-22 raw-ready\x07')
data=bytearray()
try:
 while not data.endswith(b'!'): data.extend(os.read(0,4096))
 Path(sys.argv[1]).write_bytes(data)
finally:
 termios.tcsetattr(0,termios.TCSANOW,old)
 os.write(1,b'\x1b[?2004l\x1b]2;HOO-22 raw-done\x07')
''')

    def reader(w, name):
        proof, size_file = output / (name + '.bin'), output / (name + '-tty.json')
        w.run('python3 ' + shlex.quote(str(raw)) + ' ' + shlex.quote(str(proof)) + ' ' + shlex.quote(str(size_file)))
        wait_for(lambda: 'HOO-22 raw-ready' in w.title() and read_json(size_file))
        pid = read_json(size_file)['pid']
        owned[pid] = start_time(pid)
        return proof, size_file

    try:
        if args.only in ['all', 'live']:
            w = make_window('01-live')
            backend = track(w)[0]
            clean(w, 'BACKEND')
            configure(w, 'Backend', 311, 'F1')
            chooser(w)
            shot(w, '01-local-catalog-names-and-focus.png')
            w.key('End')
            w.key('Escape')  # Chooser cancel leaves the editor draft untouched.
            w.key('Return')
            assert not file.exists() and track(w) == [backend]
            chooser(w)
            click(w, 825, 160)  # Code, then cancel the entire draft.
            w.key('Escape')
            wait_for(lambda: 'Tab bearbeiten' not in w.title())
            assert not file.exists() and track(w) == [backend]
            choose(w, 'code')
            base['save_current'](w, literal)
            wait_for(lambda: read_json(file) and len(read_json(file)['items']) == 1)
            assert read_json(file)['items'][0]['identity']['symbol'] == 'code'
            assert track(w) == [backend]
            w.key('ctrl+shift+b')
            wait_for(lambda: 'bash · 2' in w.title())
            admin = list(set(track(w)) - {backend})[0]
            clean(w, 'ADMIN', bash=True)
            configure(w, 'Admin', 430, 'F2')
            choose(w, 'server', mouse=True)
            base['save_current'](w, bash_dir, bash=True)
            wait_for(lambda: len(read_json(file)['items']) == 2)
            assert read_json(file)['items'][1]['identity']['symbol'] == 'server'
            click(w, 65, 635)  # Actual pin state remains independent of icons.
            wait_for(lambda: read_json(file)['items'][1]['pinned'] is True)
            w.key('ctrl+shift+t')
            wait_for(lambda: 'zsh · 3' in w.title())
            scratch = list(set(track(w)) - {backend, admin})[0]
            clean(w, 'SCRATCH')
            configure(w, 'Scratch', 256, 'F3')
            choose(w, 'folder')
            pids = track(w)
            state = snapshot('01-three-tabs-two-templates.json')
            ids = [t['id'] for t in state['items']]
            assert len(pids) == 3 and len(state['items']) == 2
            report['original_shell_pids'] = {'Backend': backend, 'Admin': admin, 'Scratch': scratch}
            shot(w, '02-three-symbols-linked-ordinary-and-pin.png')
            activate(w, 'F2', 'Admin')
            ticker = output / 'ticker.py'
            ticker_pid = output / 'ticker-pid.json'
            ticker.write_text("import json,os,sys,time\nfrom pathlib import Path\nPath(sys.argv[1]).write_text(json.dumps({'pid':os.getpid()}))\nfor i in range(40):\n print('HOO22 inactive',i,flush=True); time.sleep(.025)\n")
            w.run('python3 ' + shlex.quote(str(ticker)) + ' ' + shlex.quote(str(ticker_pid)) + "; printf '\\033]2;HOO-22 inactive-done\\007'")
            wait_for(lambda: read_json(ticker_pid))
            owned[read_json(ticker_pid)['pid']] = start_time(read_json(ticker_pid)['pid'])
            activate(w, 'F3', 'Scratch')
            proof, sizes = reader(w, 'symbol-routing')
            choose(w, 'shield')
            assert track(w) == pids and read_json(file) == state
            choose(w, None)
            shot(w, '03-symbol-removed-without-changing-identity.png')
            choose(w, 'branch')
            chooser(w)
            w.key('Home')
            w.key('Escape')
            w.key('Escape')
            assert read_json(file) == state and track(w) == pids
            click(w, 65, 635)  # Temporary pinned tab must still not become a template.
            w.key('ctrl+shift+w')
            assert 'Schließen bestätigen' not in w.title() and track(w) == pids
            click(w, 235, 635)
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            shot(w, '04-symbol-does-not-override-pin-or-close-target.png')
            w.key('Escape')
            wait_for(lambda: 'Schließen bestätigen' not in w.title())
            assert track(w) == pids
            activate(w, 'F2', 'Admin')
            wait_for(lambda: 'inactive-done' in w.title())
            choose(w, 'database')
            wait_for(lambda: read_json(file)['items'][1]['identity']['symbol'] == 'database')
            assert read_json(file)['items'][1]['pinned'] is True
            assert [t['id'] for t in read_json(file)['items']] == ids
            shot(w, '05-inactive-output-and-linked-symbol-change.png')
            activate(w, 'F3', 'Scratch')
            initial_size = read_json(sizes)
            sized(w, 640, 600)
            wait_for(lambda: read_json(sizes) != initial_size)
            narrow_size = read_json(sizes)
            chooser(w)
            w.key('End')
            w.key('Escape')
            w.key('Escape')
            sized(w, 1280)
            wait_for(lambda: read_json(sizes)['columns'] > narrow_size['columns'])
            report['raw_pty_sizes'] = {'initial': initial_size, 'narrow': narrow_size, 'wide': read_json(sizes)}
            clipboard(b'HOO22_COPY_SENTINEL')
            w.focus()
            command('xdotool', 'mousemove', '--window', w.id, '12', '72')
            command('xdotool', 'mousedown', '1')
            time.sleep(.1)
            command('xdotool', 'mousemove', '--window', w.id, '158', '72')
            time.sleep(.15)
            command('xdotool', 'mouseup', '1')
            w.key('ctrl+shift+c')
            wait_for(lambda: command('xclip', '-selection', 'clipboard', '-out') == 'HOO22_SELECTION')
            shot(w, '06-selection-and-resized-symbol-tab.png')
            clipboard(b'PASTE_ONLY\nsecond')
            w.key('ctrl+shift+v')
            w.type('ok!')
            wait_for(proof.exists)
            assert proof.read_bytes() == b'\x1b[200~PASTE_ONLY\nsecond\x1b[201~ok!', proof.read_bytes()
            wait_for(lambda: 'raw-done' in w.title())
            for key, name, marker in [('F1', 'Backend', 'BACKEND'), ('F2', 'Admin', 'ADMIN'), ('F3', 'Scratch', 'SCRATCH')]:
                activate(w, key, name)
                w.run("printf '\\033]2;HOO-22 state-%s\\007' \"$HOO22_STATE\"")
                wait_for(lambda: 'state-' + marker in w.title())
                assert track(w) == pids
            report['choice_cancel_remove_preserves_identity_pin_processes_state_and_exact_pty_routing'] = True
            sized(w, 1000)
            activate(w, 'F2', 'Admin')
            before = file.read_bytes()
            lock = file.with_suffix('.lock')
            assert lock.is_file()
            lock.unlink()
            lock.mkdir()
            choose(w, 'star')
            time.sleep(.4)
            assert file.read_bytes() == before and track(w) == pids
            shot(w, '07-symbol-write-error-is-visible-and-old-file-kept.png')
            lock.rmdir()
            click(w, 85, 94)
            wait_for(lambda: read_json(file)['items'][1]['identity']['symbol'] == 'star')
            assert [t['id'] for t in read_json(file)['items']] == ids
            report['linked_write_error_keeps_file_unsaved_then_retry_preserves_template_id'] = True
            activate(w, 'F1', 'Backend')
            w.key('ctrl+shift+w')
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            w.key('Return')
            wait_for(lambda: start_time(backend) != owned[backend])
            assert track(w) == sorted([admin, scratch])
            assert read_json(file)['items'][0]['identity']['symbol'] == 'code'
            activate(w, 'F1', 'Backend · 4')
            reopened = list(set(track(w)) - {admin, scratch})[0]
            assert Path(os.readlink(f'/proc/{reopened}/cwd')) == literal
            assert Path(os.readlink(f'/proc/{reopened}/exe')).name == 'zsh'
            before = track(w)
            activate(w, 'F1', 'Backend · 4')
            assert track(w) == before
            snapshot('02-before-restart.json')
            close(w, cancel=True, image='08-window-close-names-symbol-tabs-and-pins.png')
            report['explicit_save_no_spawn_linked_auto_save_close_keeps_template_reopen_once'] = True
            phase('live-selection-storage-routing-and-lifecycle')

        if args.only in ['all', 'restart']:
            state = snapshot('03-restart-source.json')
            ids = [t['id'] for t in state['items']]
            assert [t['identity']['symbol'] for t in state['items']] == ['code', 'star']
            w = make_window('02-restart')
            defaults = track(w)
            assert len(defaults) == 1
            base['show_list'](w)
            shot(w, '09-stored-symbols-available-without-autostart.png')
            assert track(w) == defaults and read_json(file) == state
            w.key('Escape')
            activate(w, 'F1', 'Backend · 2')
            backend = list(set(track(w)) - set(defaults))[0]
            activate(w, 'F2', 'Admin · 3')
            admin = list(set(track(w)) - set(defaults) - {backend})[0]
            assert Path(os.readlink(f'/proc/{backend}/cwd')) == literal
            assert Path(os.readlink(f'/proc/{admin}/cwd')) == bash_dir
            assert Path(os.readlink(f'/proc/{backend}/exe')).name == 'zsh'
            assert Path(os.readlink(f'/proc/{admin}/exe')).name == 'bash'
            clean(w, 'RESTORED_ADMIN', bash=True)
            pids = track(w)
            activate(w, 'F1', 'Backend · 2')
            activate(w, 'F2', 'Admin · 3')
            assert track(w) == pids and read_json(file) == state
            base['show_list'](w)
            w.key('End')
            w.key('Delete')
            wait_for(lambda: len(read_json(file)['items']) == 1)
            w.key('Escape')
            assert 'Admin · 3' in w.title() and track(w) == pids
            shot(w, '10-template-removal-keeps-current-user-symbol-and-pin.png')
            proof, _ = reader(w, 'unlinked-shortcut')
            w.key('ctrl+shift+w')
            assert 'Schließen bestätigen' not in w.title()
            w.key('F2')
            w.type('!')
            wait_for(proof.exists)
            assert proof.read_bytes() == b'\x1bOQ!'
            report['restart_default_only_explicit_shells_directories_no_duplicate_and_unlink_keeps_symbol_pin_pty'] = True
            close(w)
            phase('restart-explicit-open-and-unlink')

        if args.only in ['all', 'layouts']:
            root = output / 'config-layouts'
            target = root / 'hoori-term/templates-v1.json'
            target.parent.mkdir(parents=True)
            catalog = {'version': 1, 'next_id': 12, 'items': []}
            for index in range(2, 13):
                catalog['items'].append({'id': index - 1,
                    'identity': {'name': long_name(index), 'accent': ['blue', 'green', 'orange'][(index - 1) % 3],
                                 'shortcut': {'key': f'F{index}', 'modifiers': 0}, 'symbol': SYMBOLS[(index - 1) % 8]},
                    'shell': 'bash' if index % 2 else 'zsh', 'directory': str(bash_dir if index % 2 else literal),
                    'pinned': index in [2, 3]})
            target.write_text(json.dumps(catalog, ensure_ascii=False, indent=2))
            os.environ['XDG_CONFIG_HOME'] = str(root)
            w = make_window('03-layouts')
            configure(w, long_name(1), 311, 'F1')
            choose(w, 'terminal')
            for index in range(2, 13):
                activate(w, f'F{index}', f' · {index}')
            pids = track(w)
            assert len(pids) == 12
            report['layout_shell_pids'] = pids
            report['layout_catalog'] = catalog
            w.key('ctrl+shift+l')
            w.key('Home')
            w.key('Return')
            sized(w, 1280)
            shot(w, '11-twelve-long-names-wide-pinned-and-ordinary-symbols.png')
            sized(w, 640)
            shot(w, '12-twelve-long-names-narrow-overflow.png')
            activate(w, 'F12', ' · 12')
            shot(w, '13-narrow-direct-target-symbol-and-controls.png')
            w.key('ctrl+shift+e')
            wait_for(lambda: 'Tab bearbeiten' in w.title())
            shot(w, '14-full-long-name-inspection.png')
            sized(w, 640, 360)
            w.key('F6')
            w.key('End')
            shot(w, '15-short-window-chooser-scroll-and-action-row.png')
            w.key('Escape')
            w.key('Escape')
            sized(w, 1280)
            clean(w, 'EXIT_LAYOUT')
            w.run('exit 7')
            wait_for(lambda: len(track(w)) == 11)
            time.sleep(.2)
            shot(w, '16-exited-state-is-separate-from-user-symbol.png')
            assert read_json(target) == catalog
            report['twelve_long_names_all_builtin_symbols_pin_groups_overflow_shortcut_inspection_and_exited_state'] = True
            close(w)
            phase('twelve-tab-layouts-and-short-editor')

        if args.only in ['all', 'errors']:
            for mode, bad in [('unknown', 'not-a-symbol'), ('wrong-type', 42), ('legacy', None)]:
                root = output / ('config-' + mode)
                target = root / 'hoori-term/templates-v1.json'
                target.parent.mkdir(parents=True)
                identity = {'name': 'Legacy', 'accent': 'green', 'shortcut': {'key': 'F8', 'modifiers': 0}}
                if mode != 'legacy':
                    identity['symbol'] = bad
                value = {'version': 1, 'next_id': 2, 'items': [{'id': 1, 'identity': identity,
                         'shell': 'zsh', 'directory': str(literal)}]}
                target.write_text(json.dumps(value, ensure_ascii=False, indent=2))
                old = target.read_bytes()
                os.environ['XDG_CONFIG_HOME'] = str(root)
                w = make_window('04-' + mode)
                if mode == 'legacy':
                    activate(w, 'F8', 'Legacy · 2')
                    shot(w, '19-legacy-template-without-symbol-remains-usable.png')
                    assert len(track(w)) == 2
                else:
                    w.key('ctrl+shift+s')
                    time.sleep(.2)
                    assert 'Als Vorlage speichern' not in w.title()
                    shot(w, '17-invalid-symbol-preserved.png' if mode == 'unknown' else '18-wrong-symbol-type-preserved.png')
                    assert len(track(w)) == 1
                assert target.read_bytes() == old
                close(w)
            os.environ['XDG_CONFIG_HOME'] = str(output / 'config-start-error')
            w = make_window('05-start-error', shell='/nonexistent/hoo22-symbol-shell')
            assert not track(w)
            choose(w, 'shield')
            shot(w, '20-user-shield-is-not-a-process-or-security-state.png')
            w.request_close()
            assert 'Schließen bestätigen' not in w.title()
            w.wait_exit()
            report['unknown_and_wrong_type_preserve_file_legacy_loads_without_symbol_and_error_state_stays_explicit'] = True
            phase('schema-legacy-and-error-states')

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
        raise
    finally:
        for w in windows:
            w.cleanup()
        for pid, stamp in owned.items():
            if stamp is not None and start_time(pid) == stamp:
                os.kill(pid, signal.SIGKILL)
        if old_config is None:
            os.environ.pop('XDG_CONFIG_HOME', None)
        else:
            os.environ['XDG_CONFIG_HOME'] = old_config


if __name__ == '__main__':
    main()
