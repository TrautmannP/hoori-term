#!/usr/bin/env python3
"""Focused owned X11 launcher proof; --only-focus checks the final paint change.

Inspect all PNGs separately. This is not a repeat of the full feature GUI suite.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import re
import runpy
import shlex
import time

s = runpy.run_path(str(Path(__file__).with_name('gui-status-smoke.py')))
Window, command, wait_for, start_time = (s[k] for k in ('Window', 'command', 'wait_for', 'start_time'))
base, helpers = s['base'], s['helpers']


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('binary', type=Path)
    p.add_argument('output', type=Path)
    p.add_argument('--only-focus', action='store_true')
    a = p.parse_args()
    binary, output = a.binary.resolve(), a.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    assert os.environ.get('XDG_SESSION_TYPE') == 'x11'
    metadata = command(str(binary), '--version')
    assert 'dirty: false' in metadata
    commit = re.search(r'commit: ([0-9a-f]{40})', metadata).group(1)
    report = {'metadata': metadata, 'commit': commit, 'focus_only': a.only_focus,
              'display': os.environ.get('DISPLAY'), 'session_type': os.environ.get('XDG_SESSION_TYPE'),
              'screenshots': [], 'logical_sizes': [], 'pids': {}, 'input_hex': {}}
    config = output / 'config'
    file = config / 'hoori-term/templates-v1.json'
    literal, bash_dir = output / 'space ; literal $(text)', output / 'bash-dir'
    literal.mkdir()
    bash_dir.mkdir()
    catalog = {'version': 1, 'next_id': 3, 'order': [2, 1], 'items': [
        {'id': 1, 'identity': {'name': 'Own pinned zsh', 'accent': 'green',
            'shortcut': {'key': 'F2', 'modifiers': 5}, 'symbol': 'star', 'compact': True},
            'shell': 'zsh', 'directory': str(literal), 'pinned': True},
        {'id': 2, 'identity': {'name': 'Own named Bash', 'accent': 'orange',
            'shortcut': {'key': 'F3', 'modifiers': 5}, 'symbol': 'code', 'compact': False},
            'shell': 'bash', 'directory': str(bash_dir), 'pinned': False}]}
    file.parent.mkdir(parents=True)
    file.write_text(json.dumps(catalog, indent=2))
    original = file.read_bytes()
    fixture = output / 'fixture.py'
    fixture.write_text(s['FIXTURE'])
    old_config = os.environ.get('XDG_CONFIG_HOME')
    os.environ['XDG_CONFIG_HOME'] = str(config)
    windows, owned, roots, sequences = [], {}, {}, {}

    def children(w):
        pids = base['children'](w)
        for pid in pids:
            owned.setdefault(pid, start_time(pid))
        return pids

    def sized(w, width):
        report['logical_sizes'].append(helpers['resize'](w, width, 700))

    def make(name):
        directory = output / name
        directory.mkdir()
        w = Window(binary, directory, '/usr/bin/zsh', ['-f', '-i'])
        windows.append(w)
        assert f'hoori-term [{commit[:7]}]' in w.title()
        assert len(children(w)) == 1
        sized(w, 1280)
        return w

    def shot(w, name):
        command('xdotool', 'mousemove', '--window', w.id, '500', '350')
        time.sleep(.15)
        w.screenshot(output / name)
        report['screenshots'].append(name)

    def activate(w, key, tab):
        w.key(key)
        wait_for(lambda: s['active'](w) == tab)

    def proof(index):
        return base['read_json'](roots[index] / 'proof.json')

    def update(index, data):
        sequences[index] = sequences.get(index, 0) + 1
        value = {'sequence': sequences[index], 'bytes': base64.b64encode(data).decode()}
        temporary = roots[index] / 'command.tmp'
        temporary.write_text(json.dumps(value))
        temporary.replace(roots[index] / 'command.json')
        wait_for(lambda: proof(index)['sequence'] == value['sequence'])
        time.sleep(.1)

    def inspect(w):
        w.key('ctrl+shift+l')
        w.key('i')
        wait_for(lambda: 'Titel/Status' in w.title())

    def close(w, image=None):
        pids = children(w)
        w.request_close()
        assert 'Schließen bestätigen' in w.title()
        if image:
            shot(w, image)
        w.key('Escape')
        wait_for(lambda: 'Schließen bestätigen' not in w.title())
        assert children(w) == pids and file.read_bytes() == original
        w.request_close()
        w.key('Return')
        w.wait_exit()
        wait_for(lambda: all(start_time(pid) != owned[pid] for pid in pids))

    try:
        w = make('01-live')
        if not a.only_focus:
            base['show_list'](w)
            shot(w, '01-final-startup-no-auto-start.png')
            w.key('Escape')
        for key, tab, shell, cwd in [('ctrl+shift+F3', 2, 'bash', bash_dir),
                                     ('ctrl+shift+F2', 3, 'zsh', literal)]:
            before = set(children(w))
            activate(w, key, tab)
            pid = (set(children(w)) - before).pop()
            assert Path(f'/proc/{pid}/cwd').resolve() == cwd
            assert Path(f'/proc/{pid}/exe').resolve() == Path('/usr/bin/' + shell).resolve()
            report['pids'][str(tab)] = pid
            activate(w, key, tab)
            assert set(children(w)) == before | {pid}
            base['clean_prompt'](w, bash=shell == 'bash')
            root = output / f'raw-{tab}'
            root.mkdir()
            roots[tab] = root
            helpers['clipboard'](('python3 ' + shlex.quote(str(fixture)) + ' ' + shlex.quote(str(root)) + ' ' + str(tab)).encode())
            w.key('ctrl+shift+v')
            w.key('Return')
            value = wait_for(lambda: proof(tab))
            assert bytes.fromhex(value['query_hex']) == s['CAPABILITIES']
            owned[value['pid']] = start_time(value['pid'])
            update(tab, s['osc'](f'Final CLI title for tab {tab}; Grüße 界', number=2) +
                   s['osc']('state=blocked:kind=permission:app=fixture') +
                   s['osc']('id=child:state=error'))
            wait_for(lambda: f'Final CLI title for tab {tab}' in w.title())
        assert len(children(w)) == 3 and file.read_bytes() == original
        activate(w, 'ctrl+shift+F3', 2)
        w.key('ctrl+shift+l')
        shot(w, '02-normal-focused-long-shortcut.png')
        w.key('Escape')
        sized(w, 640)
        activate(w, 'ctrl+shift+F2', 3)
        w.key('ctrl+shift+l')
        shot(w, '03-compact-focused-narrow.png')
        w.key('Escape')
        if not a.only_focus:
            inspect(w)
            shot(w, '04-full-status-details-final.png')
            w.key('Escape')
            w.type('HOO29')
            wait_for(lambda: bytes.fromhex(proof(3)['input_hex']) == b'HOO29')
            inspect(w)
            w.key('Return')
            w.type('!')
            wait_for(lambda: bytes.fromhex(proof(3)['input_hex']) == b'HOO29!')
            assert proof(2)['input_hex'] == ''
            report['input_hex'] = {str(i): proof(i)['input_hex'] for i in (2, 3)}
            assert file.read_bytes() == original
            sized(w, 1280)
        close(w, None if a.only_focus else '05-final-own-names-close-cancel.png')
        if not a.only_focus:
            w = make('02-restart')
            inspect(w)
            shot(w, '06-restart-no-restored-title-or-status.png')
            w.key('Escape')
            activate(w, 'ctrl+shift+F2', 2)
            assert len(children(w)) == 2
            inspect(w)
            shot(w, '07-deliberate-template-no-restored-status.png')
            w.key('Escape')
            assert file.read_bytes() == original
            close(w)
        wait_for(lambda: all(start_time(pid) != stamp for pid, stamp in owned.items()))
        report['owned_pids_reaped'], report['complete'] = sorted(owned), True
        (output / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps({'complete': True, 'report': str(output / 'report.json')}), flush=True)
    except BaseException as error:
        report['complete'], report['error'] = False, repr(error)
        for w in windows:
            if w.process.poll() is None:
                w.screenshot(output / 'failure.png')
        (output / 'failure-report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
        raise
    finally:
        for w in windows:
            w.cleanup()
        if old_config is None:
            os.environ.pop('XDG_CONFIG_HOME', None)
        else:
            os.environ['XDG_CONFIG_HOME'] = old_config


if __name__ == '__main__':
    main()
