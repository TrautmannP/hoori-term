#!/usr/bin/env python3
"""Bundled HOO-24 changed-flow proof in owned X11 windows.

Configuration and clipboard fixtures are synthetic and isolated. Inspect each
PNG separately; process/byte assertions do not establish correct rendering.
--only and --catalog-from permit targeted continuation with original evidence.
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
resize, clipboard, configure, long_name = [helpers[n] for n in
    ('resize', 'clipboard', 'configure', 'long_name')]
SYMBOLS = ['terminal', 'code', 'folder', 'server', 'database', 'branch', 'star', 'shield']


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('binary', type=Path)
    p.add_argument('output', type=Path)
    p.add_argument('--only', choices=['all', 'live', 'restart', 'layouts', 'errors'], default='all')
    p.add_argument('--catalog-from', type=Path)
    a = p.parse_args()
    assert os.environ.get('XDG_SESSION_TYPE') == 'x11'
    binary, output = a.binary.resolve(), a.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    windows_dir, snapshots = output / 'windows', output / 'snapshots'
    windows_dir.mkdir()
    snapshots.mkdir()
    config = output / 'config'
    file = config / 'hoori-term/templates-v1.json'
    literal, bash_dir = output / 'space ; literal $(text)', output / 'bash-home'
    literal.mkdir()
    bash_dir.mkdir()
    if a.catalog_from:
        source = read_json(a.catalog_from.resolve())
        assert source and len(source['items']) == 2
        literal, bash_dir = [Path(t['directory']) for t in source['items']]
        assert literal.is_dir() and bash_dir.is_dir()
        file.parent.mkdir(parents=True)
        file.write_text(json.dumps(source, ensure_ascii=False, indent=2))
    metadata = command(str(binary), '--version')
    assert 'dirty: false' in metadata
    commit = re.search(r'commit: ([0-9a-f]{40})', metadata).group(1)
    report = {'commit': commit, 'metadata': metadata, 'scope': a.only,
              'catalog_from': str(a.catalog_from) if a.catalog_from else None,
              'os': Path('/etc/os-release').read_text(), 'display': os.environ.get('DISPLAY'),
              'session_type': os.environ.get('XDG_SESSION_TYPE'),
              'zsh': command('zsh', '--version'), 'bash': command('bash', '--version').splitlines()[0],
              'screenshots': [], 'logical_sizes': [], 'phases': [],
              'clipboard': 'Generated fixtures only; no previous clipboard contents read.',
              'isolated_config': str(config)}
    old_config = os.environ.get('XDG_CONFIG_HOME')
    os.environ['XDG_CONFIG_HOME'] = str(config)
    windows, owned = [], {}

    def phase(name):
        report['phases'].append(name)
        (output / 'progress.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps({'phase': name}), flush=True)

    def track(w):
        ids = base['children'](w)
        for pid in ids:
            owned.setdefault(pid, start_time(pid))
        return ids

    def make_window(name, shell='/usr/bin/zsh'):
        directory = windows_dir / name
        directory.mkdir()
        w = Window(binary, directory, shell, ['-f', '-i'])
        windows.append(w)
        track(w)
        assert f'hoori-term [{commit[:7]}]' in w.title()
        sized(w, 1280)
        return w

    def sized(w, width, height=700):
        report['logical_sizes'].append(resize(w, width, height))

    def shot(w, name, hover=None):
        x, y = hover or (500, 420)
        command('xdotool', 'mousemove', '--window', w.id, str(x), str(y))
        time.sleep(.8 if hover else .15)
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
        w.run(f"HOO24_STATE={state}; printf '\\033[2J\\033[HOwned HOO-24 {state}\\n'")
        w.marker(f'HOO-24 {state}-ready')

    def choose(w, symbol):
        w.key('ctrl+shift+e')
        wait_for(lambda: 'Tab bearbeiten' in w.title())
        w.key('F6')
        w.key('Home')
        for _ in range(SYMBOLS.index(symbol) + 1 if symbol else 0):
            w.key('Right')
        w.key('Return')
        w.key('Return')
        wait_for(lambda: 'Tab bearbeiten' not in w.title())

    def compact(w):
        w.key('ctrl+shift+l')
        w.key('c')
        w.key('Escape')
        time.sleep(.15)

    def draft_compact(w, cancel=False):
        w.key('ctrl+shift+e')
        wait_for(lambda: 'Tab bearbeiten' in w.title())
        w.key('F7')
        w.key('Escape' if cancel else 'Return')
        wait_for(lambda: 'Tab bearbeiten' not in w.title())

    def pin(w):
        click(w, 65, 635)  # The 1280x700 toolbar is kept outside terminal input.
        time.sleep(.2)

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
 os.write(1,b'\x1b[2J\x1b[HHOO24_SELECTION\r\n\x1b[?2004h\x1b]2;HOO-24 raw-ready\x07')
signal.signal(signal.SIGWINCH,size)
tty.setraw(0)
size()
data=bytearray()
try:
 while not data.endswith(b'!'): data.extend(os.read(0,4096))
 Path(sys.argv[1]).write_bytes(data)
finally:
 termios.tcsetattr(0,termios.TCSANOW,old)
 os.write(1,b'\x1b[?2004l\x1b]2;HOO-24 raw-done\x07')
''')

    def reader(w, name):
        proof, sizes = output / (name + '.bin'), output / (name + '-tty.json')
        w.run('python3 ' + shlex.quote(str(raw)) + ' ' + shlex.quote(str(proof)) + ' ' + shlex.quote(str(sizes)))
        wait_for(lambda: 'HOO-24 raw-ready' in w.title() and read_json(sizes))
        pid = read_json(sizes)['pid']
        owned[pid] = start_time(pid)
        return proof, sizes

    try:
        if a.only in ['all', 'live']:
            w = make_window('01-live')
            backend = track(w)[0]
            clean(w, 'BACKEND')
            configure(w, 'Backend', 311, 'F1')
            choose(w, 'star')
            draft_compact(w, cancel=True)
            # Mouse toggle, then actual saved bytes prove that it committed.
            click(w, 375, 635)
            base['save_current'](w, literal)
            wait_for(lambda: read_json(file) and len(read_json(file)['items']) == 1)
            first = snapshot('01-first-save-inherits-compact.json')
            assert first['items'][0]['identity']['compact'] is True
            assert track(w) == [backend]
            compact(w)
            wait_for(lambda: read_json(file)['items'][0]['identity']['compact'] is False)
            w.key('ctrl+shift+b')
            wait_for(lambda: 'bash · 2' in w.title())
            admin = list(set(track(w)) - {backend})[0]
            clean(w, 'ADMIN', bash=True)
            configure(w, 'Admin', 431, 'F2')
            choose(w, 'star')
            draft_compact(w)
            pin(w)
            base['save_current'](w, bash_dir, bash=True)
            wait_for(lambda: len(read_json(file)['items']) == 2)
            state = snapshot('02-two-saved-presentations.json')
            ids = [t['id'] for t in state['items']]
            assert [t['identity']['compact'] for t in state['items']] == [False, True]
            assert [t['pinned'] for t in state['items']] == [False, True]
            assert [t['identity']['symbol'] for t in state['items']] == ['star', 'star']
            w.key('ctrl+shift+b')
            wait_for(lambda: 'bash · 3' in w.title())
            logs = list(set(track(w)) - {backend, admin})[0]
            clean(w, 'LOGS', bash=True)
            configure(w, 'Logs', 550, 'F4')
            choose(w, 'code')
            pin(w)
            w.key('ctrl+shift+t')
            wait_for(lambda: 'zsh · 4' in w.title())
            scratch = list(set(track(w)) - {backend, admin, logs})[0]
            clean(w, 'SCRATCH')
            configure(w, 'Scratch', 256, 'F3')
            compact(w)  # No chosen symbol: neutral fallback, ordinary temporary tab.
            pids = track(w)
            assert len(pids) == 4 and read_json(file) == state
            report['original_shell_pids'] = {'Backend': backend, 'Admin': admin, 'Logs': logs, 'Scratch': scratch}
            shot(w, '01-four-independent-pin-presentation-combinations.png')
            w.key('ctrl+shift+l')
            shot(w, '02-keyboard-details-and-neutral-fallback.png')
            w.key('Escape')
            choose(w, 'star')
            assert read_json(file) == state
            sized(w, 640)
            w.key('ctrl+shift+l')
            w.key('Home')
            assert 'Scratch · 4' in w.title()  # Inspecting Admin does not activate it.
            shot(w, '03-same-symbol-inactive-keyboard-details.png')
            w.key('Escape')
            activate(w, 'F3', 'Scratch · 4')
            shot(w, '04-same-symbol-active-mouse-details.png', hover=(260, 20))
            sized(w, 1280)
            # Pin/unpin keeps Scratch compact; a pinned normal tab can toggle independently.
            pin(w)
            w.key('ctrl+shift+w')
            assert 'Schließen bestätigen' not in w.title()
            pin(w)
            activate(w, 'F4', 'Logs · 3')
            draft_compact(w)
            draft_compact(w)
            assert read_json(file) == state and track(w) == pids
            activate(w, 'F2', 'Admin · 2')
            w.run("python3 -u -c " + shlex.quote("import time; [(print(f'HOO24 inactive {i:02d}',flush=True),time.sleep(.03)) for i in range(40)]") + "; printf '\\033]2;HOO24-inactive-done\\007'")
            activate(w, 'F3', 'Scratch · 4')
            proof, sizes = reader(w, 'compact-routing')
            # Width changes, editor cancellation and guarded close must supply no PTY bytes.
            compact(w)
            compact(w)
            draft_compact(w, cancel=True)
            pin(w)
            w.key('ctrl+shift+w')
            assert 'Schließen bestätigen' not in w.title()
            click(w, 230, 635)
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            assert 'Scratch · 4' in w.title()
            shot(w, '05-compact-pin-explicit-close-cancel.png')
            w.key('Escape')
            pin(w)
            initial_size = read_json(sizes)
            sized(w, 640, 600)
            wait_for(lambda: read_json(sizes)['columns'] != initial_size['columns'])
            narrow_size = read_json(sizes)
            sized(w, 1280)
            wait_for(lambda: read_json(sizes)['columns'] > narrow_size['columns'])
            report['real_pty_sizes'] = [initial_size, narrow_size, read_json(sizes)]
            clipboard(b'OWNED_HOO24_FIXTURE')
            w.focus()
            command('xdotool', 'mousemove', '--window', w.id, '12', '72')
            command('xdotool', 'mousedown', '1')
            command('xdotool', 'mousemove', '--sync', '--window', w.id, '158', '72')
            command('xdotool', 'mouseup', '1')
            w.key('ctrl+shift+c')
            time.sleep(.2)
            assert command('xclip', '-selection', 'clipboard', '-out') == 'HOO24_SELECTION'
            shot(w, '06-retained-raw-terminal-copy-and-resize.png')
            clipboard(b'COMPACT_ONLY\nsecond')
            w.key('ctrl+shift+v')
            w.type('ok!')
            wait_for(proof.exists)
            assert proof.read_bytes() == b'\x1b[200~COMPACT_ONLY\nsecond\x1b[201~ok!'
            wait_for(lambda: 'HOO-24 raw-done' in w.title())
            for key, label, state_name, bash in [('F1', 'Backend · 1', 'BACKEND', False), ('F2', 'Admin · 2', 'ADMIN', True), ('F4', 'Logs · 3', 'LOGS', True), ('F3', 'Scratch · 4', 'SCRATCH', False)]:
                activate(w, key, label)
                if key == 'F2':
                    wait_for(lambda: 'HOO24-inactive-done' in w.title())
                w.run("printf '\\033]2;HOO24-kept-%s\\007' \"$HOO24_STATE\"")
                wait_for(lambda: f'HOO24-kept-{state_name}' in w.title())
            assert track(w) == pids
            report['exact_routing_retained_shell_state_inactive_output_and_real_resize'] = True
            activate(w, 'F2', 'Admin · 2')
            old = file.read_bytes()
            lock = file.with_suffix('.lock')
            if lock.exists():
                assert lock.is_file()
                lock.unlink()
            lock.mkdir()
            compact(w)
            time.sleep(.4)
            assert file.read_bytes() == old
            shot(w, '07-linked-compact-write-error-remains-unsaved.png')
            lock.rmdir()
            click(w, 85, 94)
            wait_for(lambda: read_json(file)['items'][1]['identity']['compact'] is False)
            assert [t['id'] for t in read_json(file)['items']] == ids
            compact(w)
            wait_for(lambda: read_json(file)['items'][1]['identity']['compact'] is True)
            activate(w, 'F1', 'Backend · 1')
            w.key('ctrl+shift+w')
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            assert 'Backend · 1' in w.title()
            shot(w, '08-normal-close-names-exact-target.png')
            w.key('Escape')
            assert track(w) == pids
            w.key('ctrl+shift+w')
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            w.key('Return')
            wait_for(lambda: start_time(backend) != owned[backend])
            assert track(w) == sorted([admin, logs, scratch])
            activate(w, 'F1', 'Backend · 5')
            reopened = list(set(track(w)) - {admin, logs, scratch})[0]
            assert Path(os.readlink(f'/proc/{reopened}/cwd')) == literal
            assert Path(os.readlink(f'/proc/{reopened}/exe')).name == 'zsh'
            active_pids = track(w)
            activate(w, 'F1', 'Backend · 5')
            assert track(w) == active_pids
            snapshot('03-before-restart.json')
            close(w, cancel=True, image='09-window-close-all-names-and-cancel.png')
            report['first_save_auto_save_retry_and_named_target_lifetime'] = True
            phase('live-independent-presentation-storage-routing-and-lifetime')

        if a.only in ['all', 'restart']:
            state = snapshot('04-restart-source.json')
            assert [t['identity']['compact'] for t in state['items']] == [False, True]
            w = make_window('02-restart')
            default = track(w)
            assert len(default) == 1
            base['show_list'](w)
            shot(w, '10-stored-mixed-presentation-without-autostart.png')
            assert track(w) == default and read_json(file) == state
            w.key('Escape')
            activate(w, 'F1', 'Backend · 2')
            backend = list(set(track(w)) - set(default))[0]
            activate(w, 'F2', 'Admin · 3')
            admin = list(set(track(w)) - set(default) - {backend})[0]
            for pid, directory, shell in [(backend, literal, 'zsh'), (admin, bash_dir, 'bash')]:
                assert Path(os.readlink(f'/proc/{pid}/cwd')) == directory
                assert Path(os.readlink(f'/proc/{pid}/exe')).name == shell
            pids = track(w)
            activate(w, 'F2', 'Admin · 3')
            assert track(w) == pids and read_json(file) == state
            base['show_list'](w)
            w.key('End')
            w.key('Delete')
            wait_for(lambda: len(read_json(file)['items']) == 1)
            w.key('Escape')
            sized(w, 640)
            w.key('ctrl+shift+l')
            shot(w, '11-unlinked-tab-keeps-compact-symbol-pin-and-pty.png')
            w.key('Escape')
            w.key('ctrl+shift+w')
            assert 'Schließen bestätigen' not in w.title() and track(w) == pids
            proof, _ = reader(w, 'unlinked-shortcut')
            w.key('F2')
            w.type('!')
            wait_for(proof.exists)
            assert proof.read_bytes() == b'\x1bOQ!'
            close(w)
            report['restart_deliberate_open_no_duplicates_and_unlink_preserves_compact_pin_pty'] = True
            phase('restart-mixed-presentation-and-unlink')

        if a.only in ['all', 'layouts']:
            root = output / 'config-layouts'
            target = root / 'hoori-term/templates-v1.json'
            target.parent.mkdir(parents=True)
            catalog = {'version': 1, 'next_id': 12, 'items': []}
            for index in range(2, 13):
                catalog['items'].append({'id': index - 1,
                    'identity': {'name': long_name(index), 'accent': ['blue', 'green', 'orange'][(index - 1) % 3],
                                 'shortcut': {'key': f'F{index}', 'modifiers': 0},
                                 'symbol': None if index == 6 else 'star', 'compact': index % 2 == 0},
                    'shell': 'bash' if index % 2 else 'zsh', 'directory': str(bash_dir if index % 2 else literal),
                    'pinned': index in [2, 3]})
            target.write_text(json.dumps(catalog, ensure_ascii=False, indent=2))
            original = target.read_bytes()
            os.environ['XDG_CONFIG_HOME'] = str(root)
            w = make_window('03-layouts')
            configure(w, long_name(1), 311, 'F1')
            choose(w, 'star')
            for index in range(2, 13):
                activate(w, f'F{index}', f' · {index}')
            pids = track(w)
            assert len(pids) == 12
            report['layout_shell_pids'] = pids
            report['layout_catalog'] = catalog
            w.key('ctrl+shift+l')
            w.key('Home')
            w.key('Return')
            shot(w, '12-twelve-long-tabs-mixed-wide.png')
            sized(w, 640)
            shot(w, '13-twelve-long-tabs-mixed-narrow-overflow.png')
            activate(w, 'F12', ' · 12')
            w.key('ctrl+shift+l')
            shot(w, '14-full-long-name-and-shortcut-keyboard-details.png')
            w.key('Escape')
            shot(w, '15-full-long-name-and-shortcut-mouse-details.png', hover=(260, 20))
            w.key('ctrl+shift+e')
            wait_for(lambda: 'Tab bearbeiten' in w.title())
            sized(w, 640, 360)
            w.key('F7')
            shot(w, '16-short-editor-draft-normal-and-fixed-actions.png')
            w.key('Escape')
            sized(w, 1280)
            activate(w, 'F2', ' · 2')
            clean(w, 'EXIT_LAYOUT')
            w.run('exit 7')
            wait_for(lambda: len(track(w)) == 11)
            w.key('ctrl+shift+w')
            assert 'Schließen bestätigen' not in w.title()
            time.sleep(.2)
            shot(w, '17-compact-pinned-exited-state-and-close-protection.png')
            assert target.read_bytes() == original
            close(w)
            report['twelve_long_names_same_symbols_all_combinations_overflow_details_and_compact_exit'] = True
            phase('mixed-width-twelve-tab-layout-and-exit')

        if a.only in ['all', 'errors']:
            for mode in ['legacy', 'wrong-type']:
                root = output / ('config-' + mode)
                target = root / 'hoori-term/templates-v1.json'
                target.parent.mkdir(parents=True)
                identity = {'name': 'Legacy', 'accent': 'green', 'shortcut': {'key': 'F8', 'modifiers': 0}, 'symbol': 'code'}
                if mode == 'wrong-type':
                    identity['compact'] = 42
                value = {'version': 1, 'next_id': 2, 'items': [{'id': 1, 'identity': identity,
                         'shell': 'zsh', 'directory': str(literal)}]}
                target.write_text(json.dumps(value, ensure_ascii=False, indent=2))
                before = target.read_bytes()
                os.environ['XDG_CONFIG_HOME'] = str(root)
                w = make_window('04-' + mode)
                if mode == 'legacy':
                    activate(w, 'F8', 'Legacy · 2')
                    shot(w, '18-legacy-template-without-compact-loads-normal.png')
                    assert len(track(w)) == 2
                else:
                    w.key('ctrl+shift+s')
                    time.sleep(.2)
                    assert 'Als Vorlage speichern' not in w.title()
                    shot(w, '19-invalid-compact-type-visible-and-file-preserved.png')
                    assert len(track(w)) == 1
                assert target.read_bytes() == before
                close(w)
            os.environ['XDG_CONFIG_HOME'] = str(output / 'config-start-error')
            w = make_window('05-start-error', shell='/nonexistent/hoo24-compact-shell')
            assert not track(w)
            choose(w, 'shield')
            draft_compact(w)
            shot(w, '20-compact-user-shield-and-explicit-start-error.png')
            w.request_close()
            assert 'Schließen bestätigen' not in w.title()
            w.wait_exit()
            report['legacy_normal_invalid_file_preserved_and_compact_error_explicit'] = True
            phase('compact-schema-legacy-and-error-state')

        assert all(start_time(pid) != stamp for pid, stamp in owned.items())
        report['tracked_processes'] = {str(pid): stamp for pid, stamp in owned.items()}
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
