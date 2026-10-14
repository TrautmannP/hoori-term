#!/usr/bin/env python3
"""Owned native X11 checks for the HOO-37 layout and existing operation paths.

Only synthetic data enters newly spawned, PID/focus-verified windows. OCR locates
known controls in those windows; it is never used as visual acceptance evidence.
Inspect every retained PNG independently of state/PID/byte assertions.
"""
import argparse
import base64
import csv
import io
import json
import os
from pathlib import Path
import re
import runpy
import shlex
import shutil
import subprocess
import time
import unicodedata

helpers = runpy.run_path(str(Path(__file__).with_name('gui-templates-smoke.py')))
chrome = runpy.run_path(str(Path(__file__).with_name('gui-window-chrome-smoke.py')))
qualification = runpy.run_path(str(Path(__file__).with_name('gui-ubuntu-qualification.py')))
Window, command, wait_for, click, start_time, children = [helpers[k] for k in
    ('Window', 'command', 'wait_for', 'click', 'start_time', 'children')]
geometry, resize, drag, native_state, maximized = [chrome[k] for k in
    ('geometry', 'resize', 'drag', 'native_state', 'maximized')]

INTERACTIVE = False
MANUAL = []

RAW = qualification['FIXTURE'].replace('HOO10', 'HOO37')
RAW = RAW.replace('COPY: Grüße 界\\nOwned HOO-10 clipboard fixture\\n',
                  'HOO37_SELECTION\\r\\nUTF-8: Grüße 界 e\\u0301\\r\\n')
RAW = RAW.replace('paint(); save()\ntry:', '''needs_resize=True
def resized(*_):
    global needs_resize
    needs_resize=True
signal.signal(signal.SIGWINCH,resized)
paint(); save()
try:''')
RAW = RAW.replace('    while True:\n        try: instruction', '''    while True:
        if needs_resize:
            needs_resize=False
            size=os.get_terminal_size(0)
            state.setdefault("sizes",[]).append({"columns":size.columns,"rows":size.lines})
            save()
        try: instruction''')


def read_json(path):
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def clipboard(value):
    subprocess.run(['xclip', '-selection', 'clipboard', '-in'], input=value.encode(),
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def field_value(w, value):
    # Native text-input paste avoids xdotool's temporary Unicode XKB mappings.
    # The generated value is the only clipboard content read by this action.
    clipboard(str(value))
    key(w, 'ctrl+v')


def send(w, value):
    clipboard(value)
    key(w, 'ctrl+shift+v')
    time.sleep(.12)
    key(w, 'Return')


def key(w, value):
    w.key(value)
    # xdotool returns before the native event/frame has completed. Give modal
    # replacement and iced focus/scroll operations a frame before the next key.
    time.sleep(.2)


def wheel(w, direction, count=6, panel=True):
    w.focus()
    g = geometry(w)
    command('xdotool', 'mousemove', '--window', w.id,
            str(g['WIDTH'] - 100 if panel else 120), str(g['HEIGHT'] // 2),
            'click', '--repeat', str(count), '--delay', '25', str(direction))
    time.sleep(.15)


def words(w):
    w.focus()
    png = subprocess.run(['import', '-window', w.id, 'png:-'], check=True,
                         capture_output=True).stdout
    # Separate light labels from dark/blue button fills before OCR's automatic
    # binarization can merge them. This ephemeral analysis copy is enlarged;
    # retained PNGs remain unmodified native captures.
    png = subprocess.run(['convert', 'png:-', '-colorspace', 'Gray', '-threshold', '60%', '-negate', '-resize', '200%', 'png:-'], input=png,
                         check=True, capture_output=True).stdout
    result = subprocess.run(['tesseract', 'stdin', 'stdout', '-l', 'eng',
                             '--psm', '11', 'tsv'], input=png, check=True,
                            capture_output=True).stdout.decode()
    items = list(csv.DictReader(io.StringIO(result), delimiter='\t'))
    for item in items:
        for field in ('left', 'top', 'width', 'height'): item[field] = str(int(item[field]) // 2)
    return items


def normalized(value):
    return ''.join(c for c in unicodedata.normalize('NFKD', value.casefold()) if not unicodedata.combining(c)).strip(' -.:/›>+•')


def find_control(w, word, panel=False):
    left = geometry(w)['WIDTH'] - 320 if panel else 0
    for item in words(w):
        if normalized(item['text']) == normalized(word) and int(item['left']) >= left:
            return int(item['left']) + int(item['width']) // 2, int(item['top']) + int(item['height']) // 2
    return None


def press(w, word, panel=False, scroll=False):
    for attempt in range(14 if scroll else 2):
        point = find_control(w, word, panel)
        if point:
            click(w, *point)
            time.sleep(.15)
            return
        if scroll:
            wheel(w, 5, 1, panel)
        else:
            time.sleep(.2)
    if INTERACTIVE:
        wheel(w, 4, 30, panel)
        while True:
            path = w.output / f"manual-{len(MANUAL):02}-{normalized(word)}.png"
            w.focus(); w.screenshot(path)
            print(json.dumps({'manual_control': word, 'image': str(path), 'window': w.id, 'pid': w.process.pid,
                              'commands': 'click X Y | up N | down N'}), flush=True)
            instruction = input().strip().split()
            if len(instruction) == 3 and instruction[0] == 'click':
                x, y = map(int, instruction[1:])
                bounds = geometry(w)
                assert 0 <= x < bounds['WIDTH'] and 0 <= y < bounds['HEIGHT']
                MANUAL.append({'control': word, 'action': ['click', x, y], 'image': str(path), 'window': w.id, 'pid': w.process.pid})
                click(w, x, y); time.sleep(.2); return
            if len(instruction) == 2 and instruction[0] in ['up', 'down']:
                count = int(instruction[1]); assert 1 <= count <= 30
                MANUAL.append({'control': word, 'action': instruction, 'image': str(path), 'window': w.id, 'pid': w.process.pid})
                wheel(w, 4 if instruction[0] == 'up' else 5, count, panel)
                continue
            raise AssertionError('Invalid manual control instruction')
    raise AssertionError(f'Own visible control not found: {word}')


class Study:
    def __init__(self, binary, output):
        self.binary, self.output = binary, output
        self.windows, self.owned, self.fixtures = [], {}, []
        self.report = {
            'metadata': command(str(binary), '--version'), 'complete': False,
            'display': os.environ.get('DISPLAY'), 'session_type': os.environ.get('XDG_SESSION_TYPE'),
            'os': Path('/etc/os-release').read_text(), 'kernel': command('uname', '-r'),
            'desktop': command('gnome-shell', '--version'), 'monitors': command('xrandr', '--current'),
            'text_scale': command('gsettings', 'get', 'org.gnome.desktop.interface', 'text-scaling-factor'),
            'scale': command('gsettings', 'get', 'org.gnome.desktop.interface', 'scaling-factor'),
            'zsh': command('zsh', '--version'), 'bash': command('bash', '--version').splitlines()[0],
            'phases': [], 'screenshots': [], 'results': {},
            'policy': 'Fresh PID/_NET_WM_PID/focus ownership; isolated config; synthetic clipboard only; inspect every PNG.',
        }
        assert 'dirty: false' in self.report['metadata']
        self.commit = re.search(r'commit: ([0-9a-f]{40})', self.report['metadata']).group(1)
        self.report['commit'] = self.commit
        self.raw_script = output / 'raw-fixture.py'
        self.raw_script.write_text(RAW)
        zsh = output / 'isolated-zsh'
        zsh.mkdir(); (zsh / '.zshrc').write_text('')
        os.environ['ZDOTDIR'] = str(zsh)

    def phase(self, value):
        self.report['phases'].append(value)
        (self.output / 'progress.json').write_text(json.dumps(self.report, indent=2, ensure_ascii=False))
        print(json.dumps({'phase': value}), flush=True)

    def track(self, w):
        pids = children(w)
        for pid in pids:
            self.owned.setdefault(pid, start_time(pid))
        return pids

    def shot(self, w, name):
        w.focus()
        time.sleep(.2)
        w.screenshot(self.output / name)
        self.report['screenshots'].append(name)
        assert self.commit[:7] in w.title()

    def create(self, name, catalog=None, shell='/usr/bin/zsh', default=False):
        folder = self.output / name
        folder.mkdir()
        config = folder / 'config'
        file = config / 'hoori-term/templates-v1.json'
        if catalog is not None:
            file.parent.mkdir(parents=True)
            file.write_text(catalog if isinstance(catalog, str) else json.dumps(catalog, ensure_ascii=False, indent=2))
        os.environ['XDG_CONFIG_HOME'] = str(config)
        w = (qualification['DefaultWindow'](self.binary, folder) if default else
             Window(self.binary, folder, shell, ['-f', '-i'] if shell == '/usr/bin/zsh' else ['--noprofile', '--norc', '-i'] if shell == '/bin/bash' else []))
        self.windows.append(w)
        self.track(w)
        time.sleep(.3)
        return w, file

    def activate(self, w, shortcut, label):
        key(w, shortcut)
        wait_for(lambda: label in w.title())

    def edit_name(self, w, label):
        key(w, 'ctrl+shift+e')
        wait_for(lambda: 'Tab bearbeiten' in w.title())
        key(w, 'ctrl+a'); field_value(w, label)

    def capture(self, w, shortcut):
        wheel(w, 4, 30)
        press(w, 'Aufnehmen', panel=True, scroll=True)
        key(w, shortcut)

    def save(self, w, file, name, directory, shortcut, symbol_index, compact=False):
        before = self.track(w)
        key(w, 'ctrl+shift+s'); wait_for(lambda: 'Als Vorlage speichern' in w.title())
        key(w, 'ctrl+a'); field_value(w, name)
        key(w, 'Tab'); key(w, 'ctrl+a'); field_value(w, directory)
        if shortcut == 'F8':
            wheel(w, 4, 30)
            press(w, 'zsh', panel=True, scroll=True); press(w, 'Bash', panel=True)
            press(w, 'Bash', panel=True); press(w, 'zsh', panel=True)
            assert 'Als Vorlage speichern' in w.title() and self.track(w) == before
        self.choose_symbol(w, symbol_index)
        self.capture(w, shortcut)
        if compact: key(w, 'F7')
        self.shot(w, f'save-{shortcut.lower()}-draft.png')
        press(w, 'Übernehmen', panel=True)
        wait_for(lambda: 'Als Vorlage speichern' not in w.title())
        wait_for(lambda: read_json(file) and any(t['identity']['name'] == name for t in read_json(file)['items']))
        assert self.track(w) == before
        template = next(t for t in read_json(file)['items'] if t['identity']['name'] == name)
        assert template['directory'] == str(directory)
        assert template['identity']['shortcut']['key'] == shortcut
        return template['id']

    def choose_symbol(self, w, index):
        key(w, 'F6')
        wait_for(lambda: find_control(w, 'Tab-Symbol', panel=True))
        key(w, 'Home')
        for _ in range(index): key(w, 'Right')
        key(w, 'Return')
        wait_for(lambda: find_control(w, 'Übernehmen', panel=True))

    def details(self, w):
        key(w, 'ctrl+shift+l'); key(w, 'i')
        wait_for(lambda: 'Titel/Status' in w.title())

    def raw(self, w, folder, tab):
        root = self.output / folder
        root.mkdir()
        send(w, 'unset HISTFILE; python3 ' + shlex.quote(str(self.raw_script)) + ' ' + shlex.quote(str(root)) + ' ' + str(tab))
        wait_for(lambda: read_json(root / 'proof.json') and f'HOO37 raw-{tab}' in w.title())
        proof = read_json(root / 'proof.json')
        self.owned[proof['pid']] = start_time(proof['pid'])
        self.fixtures.append(root)
        return root

    def control(self, root, operation, **values):
        proof = read_json(root / 'proof.json')
        sequence = proof['sequence'] + 1
        instruction = {'sequence': sequence, 'op': operation, **values}
        (root / 'command.json').write_text(json.dumps(instruction))
        wait_for(lambda: read_json(root / 'proof.json')['sequence'] == sequence, timeout=30)
        return read_json(root / 'proof.json')

    def close(self, w):
        self.track(w)
        w.request_close()
        if w.process.poll() is None:
            key(w, 'Return')
        w.wait_exit()

    def main(self, resume=None, after_window=None, before_clipboard=None):
        if before_clipboard:
            return self.resume_window(before_clipboard, clipboard_only=True)
        if after_window:
            return self.resume_window(after_window)
        self.phase('main-native-shell-starts-and-explicit-save')
        if resume:
            seed = read_json(resume)
            assert seed and len(seed['items']) == 12
            admin_template = next(t for t in seed['items'] if t['identity']['shortcut'] and t['identity']['shortcut']['key'] == 'F9')
            backend_template = next(t for t in seed['items'] if t['identity']['shortcut'] and t['identity']['shortcut']['key'] == 'F8')
            admin, backend = admin_template['id'], backend_template['id']
            admin_dir = Path(admin_template['directory'])
            backend_dir = self.output / 'zsh Grüße ; literal $(data)'
            backend_dir.mkdir()
            w, file = self.create('main-window', seed)
            first = self.track(w)[0]
            assert len(self.track(w)) == 1
            self.activate(w, 'F9', 'Admin')
            bash_pid = next(p for p in self.track(w) if p != first)
            assert os.readlink(f'/proc/{bash_pid}/cwd') == str(admin_dir)
            before = self.track(w)
            key(w, 'ctrl+shift+p'); key(w, 'End'); key(w, 'e')
            wait_for(lambda: 'Vorlage bearbeiten' in w.title())
            key(w, 'Tab'); key(w, 'ctrl+a'); field_value(w, backend_dir)
            press(w, 'Übernehmen', panel=True)
            wait_for(lambda: next(t for t in read_json(file)['items'] if t['id'] == backend)['directory'] == str(backend_dir))
            key(w, 'Escape')
            assert self.track(w) == before
            self.activate(w, 'F8', 'Backend')
            zsh_pid = next(p for p in self.track(w) if p not in [first, bash_pid])
            assert os.readlink(f'/proc/{zsh_pid}/cwd') == str(backend_dir)
            self.shot(w, '01b-utf8-directory-corrected-and-opened.png')
            self.report['results']['remaining_after_saves'] = {
                'source_catalog': str(resume), 'scope': 'Native backend path edit via text-input paste, deliberate reopen, then remaining main scenarios. Initial saves belong to the named earlier report.',
                'corrected_backend_directory': str(backend_dir), 'saved_ids_retained': [admin, backend],
                'actual_shell_cwds': {str(bash_pid): str(admin_dir), str(zsh_pid): str(backend_dir)},
            }
        else:
            extras = [dict(id=i, identity=dict(name=('Long duplicate review session' if i < 3 else f'Long saved template {i:02}').ljust(40, 'x'),
                       accent='blue', shortcut=None, symbol='folder', compact=False), shell='zsh', directory='/tmp', pinned=False) for i in range(1, 11)]
            seed = dict(version=1, next_id=11, order=list(range(1, 11)), items=extras)
            w, file = self.create('main-window', seed)
            assert len(self.track(w)) == 1
            self.shot(w, '01-working-1000.png')
            first = self.track(w)[0]
            click(w, 95, 214); wait_for(lambda: 'bash · 2' in w.title())
            bash_pid = next(p for p in self.track(w) if p != first)
            bash_before = os.readlink(f'/proc/{bash_pid}/cwd')
            admin_dir = self.output / 'Bash space ; literal $(data)'
            backend_dir = self.output / 'zsh Grüße ; literal $(data)'
            admin_dir.mkdir(); backend_dir.mkdir()
            admin = self.save(w, file, 'Admin', admin_dir, 'F9', 4, True)
            self.details(w); press(w, 'Anpinnen', panel=True, scroll=True)
            wait_for(lambda: next(t for t in read_json(file)['items'] if t['id'] == admin)['pinned'])
            assert os.readlink(f'/proc/{bash_pid}/cwd') == bash_before
            click(w, 95, 178); wait_for(lambda: 'zsh · 3' in w.title())
            zsh_pid = next(p for p in self.track(w) if p not in [first, bash_pid])
            zsh_before = os.readlink(f'/proc/{zsh_pid}/cwd')
            backend = self.save(w, file, 'Backend', backend_dir, 'F8', 2)
            assert os.readlink(f'/proc/{zsh_pid}/cwd') == zsh_before
        self.activate(w, 'F8', 'Backend'); self.activate(w, 'F9', 'Admin')
        assert len(self.track(w)) == 3
        key(w, 'ctrl+Tab')
        wait_for(lambda: 'zsh · 1' in w.title())
        self.edit_name(w, 'Scratch'); self.capture(w, 'F10'); key(w, 'Return')
        wait_for(lambda: 'Tab bearbeiten' not in w.title() and 'Scratch' in w.title())
        assert len(read_json(file)['items']) == 12
        self.activate(w, 'F8', 'Backend'); backend_raw = self.raw(w, 'raw-backend', 3)
        self.activate(w, 'F9', 'Admin'); admin_raw = self.raw(w, 'raw-admin', 2)
        self.activate(w, 'F10', 'Scratch'); active_raw = self.raw(w, 'raw-scratch', 1)
        runtime = ("\x1b]2;HOO37 long CLI;" + "界-title;" * 30 + "\x07"
                   "\x1b]7501;state=blocked:kind=permission:app=fixture:progress=42\x07"
                   "\x1b]7501;state=error:id=child:msg=bGF5b3V0IGZpeHR1cmU=\x07").encode()
        self.control(backend_raw, 'emit', bytes=base64.b64encode(runtime).decode())
        self.shot(w, '02-mixed-tabs-1000.png')
        self.phase('inactive-id-editor-conflicts-and-responsive-panels')
        key(w, 'ctrl+shift+l'); key(w, 'End'); key(w, 'i')
        wait_for(lambda: 'Titel/Status' in w.title() and 'Scratch' in w.title())
        self.shot(w, '03-inactive-target-details.png')
        key(w, 'End'); self.shot(w, '03b-full-cli-details.png'); key(w, 'Home')
        press(w, 'Bearbeiten', panel=True, scroll=True)
        wait_for(lambda: 'Tab bearbeiten' in w.title())
        key(w, 'ctrl+a'); field_value(w, 'Discarded draft')
        self.capture(w, 'F9'); self.shot(w, '04-duplicate-shortcut-visible.png')
        key(w, 'ctrl+shift+t'); self.shot(w, '05-reserved-shortcut-visible.png')
        key(w, 'Escape'); assert 'Tab bearbeiten' in w.title()
        key(w, 'F6'); key(w, 'End'); resize(w, 640, 360)
        self.shot(w, '06-symbol-choice-640.png')
        key(w, 'Escape'); assert 'Tab bearbeiten' in w.title()
        self.shot(w, '07-draft-640.png')
        key(w, 'Escape'); assert 'Tab bearbeiten' not in w.title()
        assert next(t for t in read_json(file)['items'] if t['id'] == backend)['identity']['name'] == 'Backend'
        assert self.track(w) == [first, bash_pid, zsh_pid] or set(self.track(w)) == {first, bash_pid, zsh_pid}
        for root in [backend_raw, admin_raw, active_raw]: assert read_json(root / 'proof.json')['input_hex'] == ''
        resize(w, 1280, 700)
        key(w, 'ctrl+shift+l'); key(w, 'End'); key(w, 'i')
        press(w, 'Bearbeiten', panel=True, scroll=True)
        wait_for(lambda: 'Tab bearbeiten' in w.title() and 'Scratch' in w.title())
        key(w, 'ctrl+a'); field_value(w, 'Backend Grüße long stable identity'.ljust(40, 'x'))
        self.choose_symbol(w, 8); key(w, 'F7')
        self.shot(w, '08-inline-editor-1280.png')
        sizes_inline = read_json(active_raw / 'proof.json')['sizes']
        assert sizes_inline[-1]['columns'] == (1280 - 196 - 320 - 20) // 10
        self.control(admin_raw, 'load', count=3000)
        self.shot(w, '09-inactive-output-during-editor.png')
        press(w, 'Übernehmen', panel=True)
        wait_for(lambda: 'Tab bearbeiten' not in w.title())
        wait_for(lambda: next(t for t in read_json(file)['items'] if t['id'] == backend)['identity']['symbol'] == 'shield')
        assert next(t for t in read_json(file)['items'] if t['id'] == backend)['identity']['compact']
        self.report['results']['inline_and_overlay_sizes'] = sizes_inline
        resize(w, 640, 360)
        self.shot(w, '10-working-640.png')
        self.activate(w, 'F10', 'Scratch')
        key(w, 'ctrl+shift+p'); key(w, 'End'); self.shot(w, '11-many-templates-640.png')
        key(w, 'Home'); key(w, 'Delete'); wait_for(lambda: len(read_json(file)['items']) == 11)
        key(w, 'Escape')
        press(w, 'Navigation'); press(w, 'Neue')
        wait_for(lambda: 'Neue Sitzung' in w.title())
        self.shot(w, '12-shell-chooser-640.png'); key(w, 'Escape')
        assert len(self.track(w)) == 3
        self.activate(w, 'F9', 'Admin'); key(w, 'ctrl+shift+w')
        self.shot(w, '13-pin-close-protected-640.png')
        assert len(self.track(w)) == 3 and 'Schließen bestätigen' not in w.title()
        self.activate(w, 'F10', 'Scratch')
        key(w, 'ctrl+shift+l'); key(w, 'Right'); key(w, 'alt+Left'); key(w, 'Escape')
        self.finish_main(w, file, backend_raw, admin_raw, active_raw, first, bash_pid, zsh_pid, admin, backend, admin_dir, backend_dir)

    def resume_window(self, source, clipboard_only=False):
        self.phase('remaining-native-window-clipboard-history-close')
        seed = read_json(source)
        admin_template = next(t for t in seed['items'] if t['identity']['shortcut'] and t['identity']['shortcut']['key'] == 'F9')
        backend_template = next(t for t in seed['items'] if t['identity']['shortcut'] and t['identity']['shortcut']['key'] == 'F8')
        admin, backend = admin_template['id'], backend_template['id']
        admin_dir, backend_dir = Path(admin_template['directory']), Path(backend_template['directory'])
        w, file = self.create('main-window', seed)
        first = self.track(w)[0]
        assert len(self.track(w)) == 1
        self.edit_name(w, 'Scratch'); self.capture(w, 'F10'); key(w, 'Return')
        self.activate(w, 'F9', 'Admin')
        bash_pid = next(p for p in self.track(w) if p != first)
        assert os.readlink(f'/proc/{bash_pid}/cwd') == str(admin_dir)
        admin_raw = self.raw(w, 'raw-admin', 2)
        self.activate(w, 'F8', backend_template['identity']['name'])
        zsh_pid = next(p for p in self.track(w) if p not in [first, bash_pid])
        assert os.readlink(f'/proc/{zsh_pid}/cwd') == str(backend_dir)
        backend_raw = self.raw(w, 'raw-backend', 3)
        self.activate(w, 'F10', 'Scratch'); active_raw = self.raw(w, 'raw-scratch', 1)
        self.control(admin_raw, 'load', count=3000)
        self.report['results']['remaining_after_window'] = {
            'source_catalog': str(source),
            'scope': ('Remaining clipboard/history/window-close/restart/error cases only; earlier native window operations are inherited.' if clipboard_only else 'Fresh owned sessions for the remaining minimize/return, close, clipboard, history and restart/error cases. Earlier editing/inactive-output/drag/resize/maximize assertions belong to their original report.'),
        }
        self.finish_main(w, file, backend_raw, admin_raw, active_raw, first, bash_pid, zsh_pid, admin, backend, admin_dir, backend_dir, native_prefix=False, clipboard_only=clipboard_only)

    def finish_main(self, w, file, backend_raw, admin_raw, active_raw, first, bash_pid, zsh_pid, admin, backend, admin_dir, backend_dir, native_prefix=True, clipboard_only=False):
        if not clipboard_only:
            self.phase('native-window-operations-and-real-pty-sizes')
            resize(w, 1000, 700)
            if native_prefix:
                before, after = drag(w, 804, 26, 35, 20)
                assert (after['X'], after['Y']) != (before['X'], before['Y'])
                before, after = drag(w, 998, 350, -40, 0)
                assert after['WIDTH'] < before['WIDTH']
                self.report['results']['native_drag_resize'] = [before, after]
            resize(w, 640, 360)
            self.activate(w, 'F9', 'Admin'); self.details(w)
            press(w, 'bewusst', panel=True, scroll=True)
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            self.shot(w, '13b-explicit-pinned-close-target-640.png')
            key(w, 'Escape'); assert len(self.track(w)) == 3
            self.activate(w, 'F8', 'Backend'); key(w, 'ctrl+shift+w')
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            self.shot(w, '13c-ordinary-close-target-640.png')
            key(w, 'Escape'); assert len(self.track(w)) == 3
            self.activate(w, 'F10', 'Scratch'); resize(w, 1000, 700)
            self.report['results']['named_individual_pin_and_ordinary_cancel_preserve_all_sessions'] = True
            self.edit_name(w, 'Window draft')
            if native_prefix:
                click(w, geometry(w)['WIDTH'] - 62, 26); wait_for(lambda: maximized(w))
                click(w, geometry(w)['WIDTH'] - 62, 26); wait_for(lambda: not maximized(w))
                self.report['results']['native_maximize_restore'] = True
            click(w, geometry(w)['WIDTH'] - 98, 26); wait_for(lambda: 'HIDDEN' in native_state(w))
            command('xdotool', 'windowactivate', '--sync', w.id); wait_for(lambda: 'HIDDEN' not in native_state(w)); w.focus()
            assert 'Tab bearbeiten' in w.title()
            w.request_close(); assert 'Schließen bestätigen' in w.title(); key(w, 'Escape')
            assert 'Tab bearbeiten' not in w.title()
            assert self.track(w) == sorted([first, bash_pid, zsh_pid])
            self.report['results']['native_minimize_os_return_and_modal_close_cancel'] = True
        resize(w, 640, 600)
        self.control(active_raw, 'paint')
        clipboard('HOO37 synthetic pre-copy fixture')
        w.focus(); command('xdotool', 'mousemove', '--window', w.id, '12', '117', 'mousedown', '1')
        command('xdotool', 'mousemove', '--window', w.id, '158', '117', 'mouseup', '1')
        self.shot(w, '14-selection-640.png')
        press(w, 'Kopieren')
        wait_for(lambda: command('xclip', '-selection', 'clipboard', '-out') == 'HOO37_SELECTION')
        clipboard('LAYOUT Grüße 界\nzweite'); press(w, 'Einfügen'); w.type('ok!')
        expected = '\x1b[200~LAYOUT Grüße 界\nzweite\x1b[201~ok!'.encode()
        wait_for(lambda: read_json(active_raw / 'proof.json')['input_hex'] == expected.hex())
        assert read_json(backend_raw / 'proof.json')['input_hex'] == ''
        assert read_json(admin_raw / 'proof.json')['input_hex'] == ''
        self.report['results']['exact_pty_input_hex'] = expected.hex()
        self.activate(w, 'F9', 'Admin'); key(w, 'ctrl+shift+Home')
        self.shot(w, '15-scrollback-640.png'); press(w, 'Live')
        for _ in range(3): key(w, 'ctrl+shift+t')
        wait_for(lambda: len(self.track(w)) == 6)
        resize(w, 640, 360)
        self.activate(w, 'F8', 'Backend')
        self.shot(w, '16-many-tabs-640.png')
        w.request_close(); self.shot(w, '17-named-close-640.png')
        wheel(w, 5, 10); self.shot(w, '18-named-close-bottom-640.png')
        press(w, 'Abbrechen'); assert len(self.track(w)) == 6
        current = file.read_bytes()
        self.close(w)
        assert file.read_bytes() == current
        saved = self.output / 'main-catalog.json'; saved.write_bytes(current)
        self.report['results'].update({'saved_ids': [admin, backend], 'main_catalog': str(saved),
            'main_real_shell_pids': [first, bash_pid, zsh_pid], 'main_shell_executables': ['/usr/bin/zsh', '/bin/bash', '/usr/bin/zsh'],
            'backend_directory': str(backend_dir), 'admin_directory': str(admin_dir),
            'main_dialog_input_consumed_and_original_sessions_retained': True})

    def restart(self, source):
        self.phase('restart-no-autostart-real-directories-and-removal')
        catalog = read_json(source)
        assert catalog
        admin = next(t for t in catalog['items'] if t['identity']['shortcut'] and t['identity']['shortcut']['key'] == 'F9')
        backend = next(t for t in catalog['items'] if t['identity']['shortcut'] and t['identity']['shortcut']['key'] == 'F8')
        w, file = self.create('restart-window', catalog)
        assert len(self.track(w)) == 1
        self.shot(w, '19-restart-default-only.png')
        pids = self.track(w)
        for shortcut, target, executable in [('F8', backend, '/usr/bin/zsh'), ('F9', admin, '/usr/bin/bash')]:
            self.activate(w, shortcut, target['identity']['name'])
            pid = next(p for p in self.track(w) if p not in pids)
            pids.append(pid)
            assert os.readlink(f'/proc/{pid}/cwd') == target['directory']
            assert os.path.realpath(f'/proc/{pid}/exe') == executable
            key(w, shortcut); assert self.track(w) == sorted(pids)
        self.shot(w, '20-reopened-real-shells.png')
        key(w, 'ctrl+shift+p'); key(w, 'Home')
        for _ in range(read_json(file)['order'].index(admin['id'])): key(w, 'Down')
        key(w, 'Delete')
        wait_for(lambda: all(t['id'] != admin['id'] for t in read_json(file)['items']))
        key(w, 'Escape')
        assert set(self.track(w)) == set(pids)
        for _ in range(3):
            if 'Admin' in w.title(): break
            key(w, 'ctrl+Tab')
        wait_for(lambda: 'Admin' in w.title())
        raw = self.raw(w, 'raw-unlinked-admin', 3)
        key(w, 'F9'); w.type('!')
        expected = b'\x1b[20~!'
        wait_for(lambda: read_json(raw / 'proof.json')['input_hex'] == expected.hex())
        self.shot(w, '21-removed-template-live-pin-retained.png')
        self.report['results']['unlinked_f9_input_hex'] = expected.hex()
        self.close(w)

    def errors(self, write_only=False):
        valid = dict(version=1, next_id=1, order=[], items=[])
        if not write_only:
            self.phase('corrupt-load-retry-at-minimum-size')
            corrupt = '{"version":99,"items":[]}'
            w, file = self.create('corrupt-window', corrupt)
            resize(w, 640, 360)
            wait_for(lambda: find_control(w, 'laden'))
            self.shot(w, '22-load-error-640.png'); assert file.read_text() == corrupt
            file.write_text(json.dumps(valid))
            press(w, 'laden'); time.sleep(.4)
            press(w, 'Navigation'); self.shot(w, '23-load-retry-preserves-file-640.png'); key(w, 'Escape')
            assert read_json(file) == valid and len(self.track(w)) == 1
            self.close(w)
        self.phase('write-error-retry-while-editing-and-explicit-unsaved-exit')
        seed = dict(version=1, next_id=2, order=[1], items=[dict(id=1, identity=dict(name='Error target', accent='green',
            shortcut=dict(key='F8', modifiers=0), symbol='code', compact=False), shell='zsh', directory='/tmp', pinned=False)])
        w, file = self.create('write-error-window', seed)
        self.activate(w, 'F8', 'Error target')
        before = file.read_bytes(); pids = self.track(w)
        lock = file.with_suffix('.lock')
        if lock.exists(): lock.unlink()
        lock.mkdir()
        self.edit_name(w, 'Retry target'); key(w, 'Return')
        wait_for(lambda: 'Retry target' in w.title())
        assert file.read_bytes() == before
        resize(w, 640, 360)
        self.edit_name(w, 'Discarded unsaved draft')
        self.shot(w, '24-write-error-editor-640.png')
        lock.rmdir(); press(w, 'speichern')
        wait_for(lambda: read_json(file)['items'][0]['identity']['name'] == 'Retry target')
        assert 'Tab bearbeiten' in w.title() and self.track(w) == pids
        self.shot(w, '25-retry-with-editor-actions-640.png'); key(w, 'Escape')
        if lock.exists(): lock.unlink()
        lock.mkdir()
        self.edit_name(w, 'Unsaved target'); key(w, 'Return')
        wait_for(lambda: 'Unsaved target' in w.title())
        w.request_close(); key(w, 'Return')
        wait_for(lambda: not self.track(w))
        self.shot(w, '26-explicit-unsaved-exit-640.png')
        assert read_json(file)['items'][0]['identity']['name'] == 'Retry target'
        press(w, 'Ungespeichert'); w.wait_exit()
        self.phase('visible-start-error-with-no-fallback')
        w, _ = self.create('start-error-window', valid, shell='/nonexistent/hoo37-start-target')
        resize(w, 640, 360); assert not self.track(w)
        self.shot(w, '27-start-error-640.png')
        w.request_close(); w.wait_exit()
        result = 'write_retry_unsaved_exit_start_error' if write_only else 'load_write_retry_unsaved_exit_start_error'
        self.report['results'][result] = True

    def launcher(self):
        self.phase('focused-final-no-argument-launcher')
        seed = dict(version=1, next_id=3, order=[1, 2], items=[dict(id=i,
            identity=dict(name=name, accent='green' if i == 1 else 'orange', shortcut=dict(key=key, modifiers=0),
                          symbol='code' if i == 1 else 'star', compact=i == 2), shell=shell,
            directory='/tmp', pinned=i == 2) for i, name, key, shell in
            [(1, 'Backend review', 'F8', 'zsh'), (2, 'Admin review', 'F9', 'bash')]])
        w, file = self.create('final-window', seed, default=True)
        before = file.read_bytes(); pids = self.track(w)
        assert len(pids) == 1
        self.shot(w, 'final-01-normal-launch.png')
        for shortcut, name in [('F8', 'Backend review'), ('F9', 'Admin review')]:
            self.activate(w, shortcut, name)
            pid = next(p for p in self.track(w) if p not in pids); pids.append(pid)
            assert os.readlink(f'/proc/{pid}/cwd') == '/tmp'
            key(w, shortcut); assert set(self.track(w)) == set(pids)
        self.edit_name(w, 'Final cancelled draft')
        press(w, 'Blau', panel=True)
        self.capture(w, 'ctrl+shift+t')
        self.shot(w, 'final-02a-readable-disabled-draft.png')
        key(w, 'Escape')
        resize(w, 640, 360)
        self.shot(w, 'final-02-narrow-editor.png'); key(w, 'Escape')
        w.request_close(); self.shot(w, 'final-03-named-close.png'); key(w, 'Escape')
        assert set(self.track(w)) == set(pids) and file.read_bytes() == before
        self.close(w)
        self.report['results']['final_launcher_focus_only'] = True

    def cleanup(self):
        for w in self.windows:
            w.cleanup()
        wait_for(lambda: all(start_time(pid) != stamp for pid, stamp in self.owned.items()), timeout=15)
        self.report['owned_pid_stamps'] = self.owned
        self.report['all_owned_processes_reaped'] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--only', choices=['all', 'main', 'restart', 'errors', 'launcher'], default='all')
    parser.add_argument('--catalog-from', type=Path)
    parser.add_argument('--resume-after-saves', type=Path)
    parser.add_argument('--resume-after-window', type=Path)
    parser.add_argument('--resume-before-clipboard', type=Path)
    parser.add_argument('--resume-write-errors', action='store_true', help='Check only the remaining write/retry/unsaved-exit and start-error cases')
    parser.add_argument('--interactive', action='store_true', help='Allow owned native control input through stdin after OCR cannot locate a control')
    args = parser.parse_args()
    global INTERACTIVE
    INTERACTIVE = args.interactive
    assert os.environ.get('XDG_SESSION_TYPE') == 'x11'
    assert shutil.which('tesseract'), 'Control location uses Tesseract, not an existing desktop window'
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    study = Study(args.binary.resolve(), output)
    try:
        if args.only in ('all', 'main'): study.main(args.resume_after_saves, args.resume_after_window, args.resume_before_clipboard)
        if args.only in ('all', 'restart'): study.restart(args.catalog_from or output / 'main-catalog.json')
        if args.only in ('all', 'errors'): study.errors(args.resume_write_errors)
        if args.only == 'launcher': study.launcher()
        study.cleanup(); study.report['complete'] = True
        study.report['manual_native_controls'] = MANUAL
        (output / 'report.json').write_text(json.dumps(study.report, ensure_ascii=False, indent=2))
        print(json.dumps({'complete': True, 'images': len(study.report['screenshots'])}), flush=True)
    except BaseException as error:
        study.report['error'] = repr(error)
        study.report['manual_native_controls'] = MANUAL
        for index, w in enumerate(study.windows):
            if w.process.poll() is None:
                try: study.shot(w, f'failure-{index}.png')
                except BaseException: pass
        study.cleanup()
        (output / 'failure-report.json').write_text(json.dumps(study.report, ensure_ascii=False, indent=2))
        raise


if __name__ == '__main__':
    main()
