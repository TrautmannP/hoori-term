#!/usr/bin/env python3
"""Bundled HOO-26 X11 changed-flow proof using isolated owned fixtures.

Inspect every PNG independently. IDs/PIDs/order/bytes do not prove rendering.
--only and --catalog-from allow targeted continuation with original records.
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

compact = runpy.run_path(str(Path(__file__).with_name('gui-compact-smoke.py')))
helpers, base = compact['helpers'], compact['base']
Window, command, wait_for, click, start_time, read_json = [base[n] for n in
    ('Window', 'command', 'wait_for', 'click', 'start_time', 'read_json')]
resize, clipboard, geometry, long_name = [helpers[n] for n in
    ('resize', 'clipboard', 'geometry', 'long_name')]


def active(w):
    match = re.search(r' · (\d+)(?: —|$)', w.title())
    assert match, w.title()
    return int(match.group(1))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--only', choices=['all', 'live', 'restart', 'errors'], default='all')
    parser.add_argument('--catalog-from', type=Path)
    parser.add_argument('--resume-live-after-baseline', action='store_true',
                        help='Retain already inspected baseline photos from an earlier attempt')
    a = parser.parse_args()
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
    metadata = command(str(binary), '--version')
    assert 'dirty: false' in metadata
    commit = re.search(r'commit: ([0-9a-f]{40})', metadata).group(1)
    report = {'commit': commit, 'metadata': metadata, 'scope': a.only,
              'baseline_photos_carried_from_previous_attempt': a.resume_live_after_baseline,
              'synthetic_x11_wheel_pixels_per_click': 160,
              'catalog_from': str(a.catalog_from) if a.catalog_from else None,
              'display': os.environ.get('DISPLAY'), 'session_type': os.environ.get('XDG_SESSION_TYPE'),
              'os': Path('/etc/os-release').read_text(), 'logical_sizes': [], 'phases': [],
              'zsh': command('zsh', '--version'), 'bash': command('bash', '--version').splitlines()[0],
              'screenshots': [], 'observed_live_orders': [], 'saved_orders': [],
              'clipboard': 'Generated fixtures only; no previous clipboard contents read.'}
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

    def sized(w, width, height=700):
        report['logical_sizes'].append(resize(w, width, height))

    def make_window(name):
        directory = windows_dir / name
        directory.mkdir()
        w = Window(binary, directory, '/usr/bin/zsh', ['-f', '-i'])
        windows.append(w)
        assert f'hoori-term [{commit[:7]}]' in w.title()
        track(w)
        sized(w, 1280)
        return w

    def shot(w, name, keep_pointer=False):
        if not keep_pointer:
            command('xdotool', 'mousemove', '--window', w.id, '500', '420')
        time.sleep(.15)
        w.screenshot(output / name)
        report['screenshots'].append(name)

    def snapshot(name):
        value = read_json(file)
        assert value
        (snapshots / name).write_text(json.dumps(value, ensure_ascii=False, indent=2))
        return value

    def activate(w, key, tab):
        w.key(key)
        wait_for(lambda: active(w) == tab)
        time.sleep(.08)

    def clean(w, index, bash=False):
        w.key('ctrl+u')
        base['clean_prompt'](w, bash=bash)
        w.run(f"HOO26_STATE=SESSION_{index}; printf '\\033[2J\\033[HOwned HOO-26 session {index}\\n'")
        w.marker(f'HOO26-ready-{index}')

    def close(w, image=None, cancel=False):
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

    def scan_order(w, count):
        original = active(w)
        w.key('ctrl+shift+l')
        w.key('Home')
        w.key('Return')
        time.sleep(.1)
        found = [active(w)]
        for _ in range(count - 1):
            previous = active(w)
            w.key('ctrl+Tab')
            wait_for(lambda: active(w) != previous)
            found.append(active(w))
        assert len(set(found)) == count
        for _ in range(count):
            if active(w) == original:
                break
            previous = active(w)
            w.key('ctrl+Tab')
            wait_for(lambda: active(w) != previous)
        assert active(w) == original
        report['observed_live_orders'].append(found)
        return found

    def focus_id(w, live, tab):
        w.key('ctrl+shift+l')
        w.key('Home')
        for _ in range(live.index(tab)):
            w.key('Right')
        time.sleep(.12)

    def layout(live, props, width):
        span, positions, group = 0, {}, props[live[0]]['pinned']
        for tab in live:
            if props[tab]['pinned'] != group:
                span += 12
                group = props[tab]['pinned']
            size = 104 if props[tab]['identity']['compact'] else 224
            positions[tab] = (span, size)
            span += size
        left = 68 if span > width - 332 else 8
        return positions, left, width - 324 - left, max(0, span - (width - 324 - left))

    def drag(w, live, props, source, target=None, after=False, cancel=False, outside=None, image=None):
        unchanged = active(w)
        focus_id(w, live, source)
        w.key('Escape')
        width = geometry(w)['WIDTH']
        positions, left, viewport, maximum = layout(live, props, width)
        sx, size = positions[source]
        offset = max(0, sx + size - viewport)
        x = left + sx - offset + 28
        command('xdotool', 'mousemove', '--window', w.id, str(x), '28')
        command('xdotool', 'mousedown', '1')
        try:
            time.sleep(.12)
            command('xdotool', 'mousemove', '--sync', '--window', w.id, str(x + 8), '28')
            time.sleep(.1)
            if outside is not None:
                tx, ty = outside
            else:
                tx0, target_width = positions[target]
                wanted = min(max(0, tx0 + target_width / 2 - viewport / 2), maximum)
                delta = wanted - offset
                # The pinned winit X11 backend reports a scroll for both
                # XTEST wheel press and release: 2 * 80 pixels per click.
                # Center the target with the nearest click so either half
                # stays reachable even in the 248px minimum viewport.
                steps = round(abs(delta) / 160)
                if steps:
                    command('xdotool', 'click', '--repeat', str(steps), '--delay', '80', '5' if delta > 0 else '4')
                    offset = min(max(0, offset + (160 * steps if delta > 0 else -160 * steps)), maximum)
                    time.sleep(.15)
                center = left + tx0 - offset + target_width / 2
                tx = min(max(center + (24 if after else -24), left + 4), left + viewport - 4)
                assert (tx >= center) == after
                ty = 40
            command('xdotool', 'mousemove', '--sync', '--window', w.id, str(round(tx)), str(ty))
            time.sleep(.18)
            assert active(w) == unchanged
            if image:
                shot(w, image, keep_pointer=True)
            if cancel:
                # --clearmodifiers also releases mouse buttons. Preserve the
                # held left button until after Escape, as a physical user does.
                command('xdotool', 'key', 'Escape')
                time.sleep(.1)
        finally:
            command('xdotool', 'mouseup', '1')
        time.sleep(.18)
        assert active(w) == unchanged

    def expected_move(live, props, order, source, target, after):
        if source == target or props[source]['pinned'] != props[target]['pinned']:
            return
        old = list(live)
        live.remove(source)
        live.insert(live.index(target) + int(after), source)
        template = props[source]['template']
        if live == old or template is None:
            return
        index = live.index(source)
        linked = lambda tab: props[tab]['template'] if props[tab]['pinned'] == props[source]['pinned'] else None
        following = next((linked(tab) for tab in live[index+1:] if linked(tab)), None)
        previous = next((linked(tab) for tab in reversed(live[:index]) if linked(tab)), None)
        if following or previous:
            order.remove(template)
            order.insert(order.index(following) if following else order.index(previous)+1, template)

    def check(w, live, order, durable=True):
        assert scan_order(w, len(live)) == live
        if durable:
            wait_for(lambda: read_json(file) and read_json(file).get('order', [t['id'] for t in read_json(file)['items']]) == order)
            report['saved_orders'].append(list(order))

    raw = output / 'raw-reader.py'
    raw.write_text(r'''import json,os,signal,sys,termios,tty
from pathlib import Path
old=termios.tcgetattr(0)
def size(*_):
 s=os.get_terminal_size(0)
 Path(sys.argv[2]).write_text(json.dumps({'pid':os.getpid(),'columns':s.columns,'lines':s.lines}))
 os.write(1,b'\x1b[2J\x1b[HHOO26_SELECTION\r\n\x1b[?2004h\x1b]2;HOO26-raw-ready\x07')
signal.signal(signal.SIGWINCH,size)
tty.setraw(0)
size()
data=bytearray()
try:
 while not data.endswith(b'!'): data.extend(os.read(0,4096))
 Path(sys.argv[1]).write_bytes(data)
finally:
 termios.tcsetattr(0,termios.TCSANOW,old)
 os.write(1,b'\x1b[?2004l\x1b]2;HOO26-raw-done\x07')
''')

    def reader(w, name):
        proof, sizes = output / (name + '.bin'), output / (name + '-tty.json')
        w.run('python3 ' + shlex.quote(str(raw)) + ' ' + shlex.quote(str(proof)) + ' ' + shlex.quote(str(sizes)))
        wait_for(lambda: 'HOO26-raw-ready' in w.title() and read_json(sizes))
        pid = read_json(sizes)['pid']
        owned[pid] = start_time(pid)
        return proof, sizes

    try:
        if a.catalog_from:
            source = read_json(a.catalog_from.resolve())
            assert source and source.get('order')
            file.parent.mkdir(parents=True)
            file.write_text(json.dumps(source, ensure_ascii=False, indent=2))
        if a.only in ['all', 'live']:
            catalog = {'version': 1, 'next_id': 14, 'items': []}
            props = {1: {'identity': {'name': long_name(1), 'compact': False, 'symbol': None}, 'pinned': False, 'template': None}}
            saved_for_key = dict(zip(range(2, 13), [1, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13]))
            for index, saved in saved_for_key.items():
                entry = {'id': saved, 'identity': {'name': long_name(index), 'accent': ['blue', 'green', 'orange'][index % 3],
                         'shortcut': {'key': f'F{index}', 'modifiers': 0}, 'symbol': compact['SYMBOLS'][(index-2) % 8],
                         'compact': index % 2 == 0}, 'pinned': index < 6,
                         'shell': 'zsh' if index % 2 == 0 else 'bash',
                         'directory': str(literal if index % 2 == 0 else bash_dir)}
                catalog['items'].append(entry)
                props[index] = {'identity': entry['identity'], 'pinned': entry['pinned'], 'template': saved}
            for saved, pinned in [(2, True), (7, False)]:
                catalog['items'].append({'id': saved, 'identity': {'name': f'Closed neighbor {saved}', 'accent': 'purple',
                    'shortcut': None, 'symbol': 'shield', 'compact': False}, 'pinned': pinned, 'shell': 'bash', 'directory': str(bash_dir)})
            catalog['items'].sort(key=lambda entry: entry['id'])
            file.parent.mkdir(parents=True)
            file.write_text(json.dumps(catalog, ensure_ascii=False, indent=2))
            original = file.read_bytes()
            w = make_window('01-live')
            assert len(track(w)) == 1
            clean(w, 1)
            compact['configure'](w, long_name(1), 311, 'F1')
            pids_by_tab = {1: track(w)[0]}
            for index in range(2, 13):
                before = set(track(w))
                activate(w, f'F{index}', index)
                pids_by_tab[index] = (set(track(w)) - before).pop()
                clean(w, index, bash=index % 2 == 1)
            pids = track(w)
            assert len(pids) == 12 and file.read_bytes() == original
            live, order = [2, 3, 4, 5, 1, 6, 7, 8, 9, 10, 11, 12], list(range(1, 14))
            assert scan_order(w, 12) == live
            report['original_shell_pids_by_tab'] = pids_by_tab
            report['legacy_load_open_does_not_rewrite_file_or_autostart'] = True
            focus_id(w, live, 2)
            w.key('Return')
            if not a.resume_live_after_baseline:
                shot(w, '01-twelve-mixed-tabs-wide-front.png')
            sized(w, 640)
            if not a.resume_live_after_baseline:
                shot(w, '02-twelve-mixed-tabs-narrow-front.png')
            activate(w, 'F12', 12)
            native_before = geometry(w)
            for options in [dict(target=3, after=True, cancel=True, image='03-narrow-valid-preview-escape.png'),
                            dict(target=6, after=False), dict(outside=(200, 100)),
                            dict(outside=(640-296, 26)), dict(outside=(640-196, 26)),
                            dict(outside=(640-28, 26)), dict(target=2, after=True)]:
                drag(w, live, props, 2, **options)
                assert scan_order(w, 12) == live
                assert file.read_bytes() == original and track(w) == pids
                assert geometry(w) == native_before
            report['escape_cross_group_outside_controls_and_noop_leave_order_pin_native_window_unchanged'] = True
            drag(w, live, props, 4, target=2, image='04-narrow-pin-drag-with-overflow-preview.png')
            expected_move(live, props, order, 4, 2, False)
            check(w, live, order)
            assert order == [4,1,2,3,5,6,7,8,9,10,11,12,13]
            focus_id(w, live, 5)
            for _ in range(2):
                target = live[live.index(5)-1]
                w.key('alt+Left')
                expected_move(live, props, order, 5, target, False)
            shot(w, '05-narrow-keyboard-pin-move-keeps-active-id.png')
            assert active(w) == 12
            w.key('Escape')
            check(w, live, order)
            assert order == [4,5,1,2,3,6,7,8,9,10,11,12,13]
            sized(w, 1280)
            activate(w, 'F3', 3)
            w.run("for i in {1..40}; do printf 'inactive-order-%02d\\n' $i; sleep .02; done; printf '\\033]2;HOO26-inactive-done\\007'")
            activate(w, 'F12', 12)
            drag(w, live, props, 8, target=6, image='06-wide-normal-linked-drag-preview.png')
            expected_move(live, props, order, 8, 6, False)
            check(w, live, order)
            assert order == [4,5,1,2,3,9,6,7,8,10,11,12,13]
            old = file.read_bytes()
            drag(w, live, props, 1, target=7, after=True)
            expected_move(live, props, order, 1, 7, True)
            check(w, live, order)
            assert file.read_bytes() == old and props[1]['template'] is None
            activate(w, 'F1', 1)
            before_save = list(live)
            base['save_current'](w, literal)
            wait_for(lambda: len(read_json(file)['items']) == 14)
            saved = read_json(file)['items'][-1]
            assert saved['id'] == 14 and saved['identity']['shortcut'] == {'key': 'F1', 'modifiers': 0}
            props[1]['template'] = 14
            order.insert(order.index(10), 14)
            check(w, live, order)
            assert live == before_save and track(w) == pids
            assert [t['id'] for t in read_json(file)['items']] == list(range(1,15))
            assert read_json(file)['items'][:13] == catalog['items']
            snapshot('01-first-save-between-linked-neighbors.json')
            base['show_list'](w)
            shot(w, '07-saved-order-includes-closed-neighbors-and-first-save.png')
            w.key('Escape')
            phase('twelve-tabs-drag-keyboard-invalid-targets-and-neighbor-order')
            old = file.read_bytes()
            lock = file.with_suffix('.lock')
            if lock.exists():
                assert lock.is_file()
                lock.unlink()
            lock.mkdir()
            drag(w, live, props, 2, target=3, after=True)
            expected_move(live, props, order, 2, 3, True)
            check(w, live, order, durable=False)
            time.sleep(.4)
            assert file.read_bytes() == old
            shot(w, '08-linked-order-write-error-visible-pending-model.png')
            lock.rmdir()
            click(w, 85, 94)
            wait_for(lambda: read_json(file)['order'] == order)
            snapshot('02-retried-order.json')
            assert track(w) == pids
            activate(w, 'F1', 1)
            for pinned in [True, False]:
                click(w, 65, 635)
                live.remove(1)
                props[1]['pinned'] = pinned
                position = sum(props[tab]['pinned'] for tab in live) if pinned else len(live)
                live.insert(position, 1)
                wait_for(lambda: read_json(file)['items'][-1]['pinned'] == pinned)
                check(w, live, order)
            assert read_json(file)['order'] == order
            activate(w, 'F8', 8)
            proof, sizes = reader(w, 'reorder-routing')
            first_size = read_json(sizes)
            drag(w, live, props, 9, target=10, after=True)
            expected_move(live, props, order, 9, 10, True)
            focus_id(w, live, 9)
            w.key('alt+Left')
            expected_move(live, props, order, 9, 10, False)
            w.key('Escape')
            drag(w, live, props, 9, target=6, cancel=True)
            check(w, live, order)
            assert active(w) == 8 and track(w) == pids
            sized(w, 640, 600)
            wait_for(lambda: read_json(sizes)['columns'] < first_size['columns'])
            narrow_size = read_json(sizes)
            shot(w, '09-narrow-moved-raw-terminal-and-persistent-hash.png')
            sized(w, 1280)
            wait_for(lambda: read_json(sizes)['columns'] > narrow_size['columns'])
            report['real_pty_sizes'] = [first_size, narrow_size, read_json(sizes)]
            clipboard(b'OWNED_HOO26_FIXTURE')
            command('xdotool', 'mousemove', '--window', w.id, '12', '72')
            command('xdotool', 'mousedown', '1')
            command('xdotool', 'mousemove', '--sync', '--window', w.id, '158', '72')
            command('xdotool', 'mouseup', '1')
            w.key('ctrl+shift+c')
            time.sleep(.2)
            assert command('xclip', '-selection', 'clipboard', '-out') == 'HOO26_SELECTION'
            shot(w, '10-real-terminal-selection-and-clipboard-after-moves.png')
            clipboard(b'ORDER_ONLY\nsecond')
            w.key('ctrl+shift+v')
            w.type('ok!')
            wait_for(proof.exists)
            assert proof.read_bytes() == b'\x1b[200~ORDER_ONLY\nsecond\x1b[201~ok!'
            wait_for(lambda: 'HOO26-raw-done' in w.title())
            report['exact_reorder_routing_hex'] = proof.read_bytes().hex()
            activate(w, 'F3', 3)
            wait_for(lambda: 'HOO26-inactive-done' in w.title())
            shot(w, '11-retained-inactive-output-after-reordering.png')
            for index in range(1,13):
                activate(w, f'F{index}', index)
                w.run("printf '\\033]2;HOO26-kept-%s\\007' \"$HOO26_STATE\"")
                wait_for(lambda: f'HOO26-kept-SESSION_{index}' in w.title())
            assert track(w) == pids
            report['original_shell_pids_state_inactive_output_resize_clipboard_retained'] = True
            activate(w, 'F2', 2)
            w.key('ctrl+shift+w')
            assert 'Schließen bestätigen' not in w.title() and track(w) == pids
            activate(w, 'F1', 1)
            w.key('ctrl+shift+w')
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            assert ' · 1' in w.title()
            shot(w, '12-targeted-close-after-moves-names-stable-id.png')
            w.key('Escape')
            assert track(w) == pids
            w.key('ctrl+shift+w')
            wait_for(lambda: 'Schließen bestätigen' in w.title())
            w.key('Return')
            wait_for(lambda: start_time(pids_by_tab[1]) != owned[pids_by_tab[1]])
            live.remove(1)
            old = file.read_bytes()
            before = set(track(w))
            activate(w, 'F1', 13)
            reopened = (set(track(w)) - before).pop()
            props[13] = props[1]
            live.insert(live.index(9), 13)  # First open saved successor is template 10 / tab 9.
            assert Path(os.readlink(f'/proc/{reopened}/cwd')) == literal
            assert Path(os.readlink(f'/proc/{reopened}/exe')).name == 'zsh'
            check(w, live, order)
            activate(w, 'F1', 13)
            assert track(w) == sorted(list(before) + [reopened]) and file.read_bytes() == old
            shot(w, '13-explicit-reopen-uses-saved-neighbor-with-new-id.png')
            # Removing template 8 keeps its live compact tab 7 at its position.
            activate(w, 'F7', 7)
            base['show_list'](w)
            w.key('Home')
            for _ in range(order.index(8)):
                w.key('Down')
            w.key('Delete')
            order.remove(8)
            wait_for(lambda: read_json(file)['order'] == order)
            w.key('Escape')
            check(w, live, order)
            focus_id(w, live, 7)
            shot(w, '14-template-removal-preserves-position-compact-symbol-and-pty.png')
            w.key('Escape')
            proof, _ = reader(w, 'unlinked-shortcut')
            w.key('F7')
            w.type('!')
            wait_for(proof.exists)
            assert proof.read_bytes() == b'\x1b[18~!'
            wait_for(lambda: 'HOO26-raw-done' in w.title())
            snapshot('03-before-restart.json')
            close(w, '15-window-close-after-moves-all-names-and-cancel.png', cancel=True)
            report['first_save_failed_write_retry_pin_remove_close_and_explicit_reopen'] = True
            phase('storage-and-real-pty-lifetime-regressions')

        if a.only in ['all', 'restart']:
            state = snapshot('04-restart-source.json')
            assert state.get('order') and all(t['id'] != 8 for t in state['items'])
            w = make_window('02-restart')
            default = track(w)
            assert len(default) == 1 and active(w) == 1
            base['show_list'](w)
            shot(w, '16-restored-saved-order-without-auto-start.png')
            assert track(w) == default and read_json(file) == state
            w.key('Escape')
            live, props, opened, template_pids = [1], {1: {'pinned':False,'template':None}}, {}, {}
            def open_saved(saved):
                before = set(track(w))
                entry = next(t for t in state['items'] if t['id'] == saved)
                shortcut = entry['identity']['shortcut']
                if shortcut:
                    w.key(shortcut['key'])
                else:
                    base['show_list'](w)
                    w.key('Home')
                    for _ in range(state['order'].index(saved)):
                        w.key('Down')
                    w.key('Return')
                tab = len(opened) + 2
                wait_for(lambda: active(w) == tab)
                opened[saved] = tab
                props[tab] = {'pinned':entry['pinned'],'template':saved}
                later = state['order'][state['order'].index(saved)+1:]
                earlier = state['order'][:state['order'].index(saved)]
                next_tab = next((opened[t] for t in later if t in opened and props[opened[t]]['pinned'] == entry['pinned']),None)
                previous = next((opened[t] for t in reversed(earlier) if t in opened and props[opened[t]]['pinned'] == entry['pinned']),None)
                position = live.index(next_tab) if next_tab else live.index(previous)+1 if previous else sum(props[t]['pinned'] for t in live) if entry['pinned'] else len(live)
                live.insert(position,tab)
                pid = (set(track(w)) - before).pop()
                template_pids[saved] = pid
                assert Path(os.readlink(f'/proc/{pid}/cwd')) == Path(entry['directory'])
                assert Path(os.readlink(f'/proc/{pid}/exe')).name == entry['shell']
                assert scan_order(w,len(live)) == live and read_json(file) == state
            for saved in [13,10,14,9,6,1,5,4,2,3,7]:
                open_saved(saved)
            assert len(live) == 12 and len(track(w)) == 12
            pids = track(w)
            w.key('F1')
            wait_for(lambda: active(w) == opened[14])
            assert track(w) == pids and read_json(file) == state
            w.key('ctrl+shift+l')
            w.key('Home')
            w.key('Return')
            shot(w, '17-twelve-deliberately-reopened-tabs-in-saved-groups.png')
            sized(w, 640)
            shot(w, '18-reopened-narrow-overflow-keeps-pin-group.png')
            sized(w, 1280)
            base['show_list'](w)
            w.key('Home')
            for _ in range(state['order'].index(2)):
                w.key('Down')
            w.key('Return')
            wait_for(lambda: active(w) == opened[2])
            clean(w, 'EXITED_PIN', bash=True)
            w.run('exit 7')
            wait_for(lambda: start_time(template_pids[2]) != owned[template_pids[2]])
            before = track(w)
            base['show_list'](w)
            w.key('Home')
            for _ in range(state['order'].index(2)):
                w.key('Down')
            w.key('Return')
            assert active(w) == opened[2] and track(w) == before
            w.key('ctrl+shift+w')
            assert 'Schließen bestätigen' not in w.title()
            shot(w, '19-already-open-exited-pin-activates-without-restart.png')
            assert read_json(file) == state
            report['restart_opened_template_pids'] = template_pids
            report['restart_reverse_open_closed_neighbors_group_order_and_exited_activation_only'] = True
            close(w)
            phase('restart-and-deliberate-opening-with-closed-neighbors')

        if a.only in ['all', 'errors']:
            root = output / 'config-invalid-order'
            target = root / 'hoori-term/templates-v1.json'
            target.parent.mkdir(parents=True)
            broken = {'version':1,'next_id':2,'items':[{'id':1,'identity':{'name':'Invalid order fixture','accent':'green','shortcut':None},'shell':'zsh','directory':str(literal)}],'order':[999]}
            target.write_text(json.dumps(broken,ensure_ascii=False,indent=2))
            before = target.read_bytes()
            os.environ['XDG_CONFIG_HOME'] = str(root)
            w = make_window('03-invalid-order')
            w.key('ctrl+shift+s')
            time.sleep(.25)
            assert 'Als Vorlage speichern' not in w.title()
            shot(w, '20-invalid-order-visible-original-file-preserved.png')
            assert target.read_bytes() == before and len(track(w)) == 1
            close(w)
            report['invalid_unresolved_order_load_visible_and_not_overwritten'] = True
            phase('invalid-id-order-file-preservation')

        assert all(start_time(pid) != stamp for pid,stamp in owned.items())
        report['tracked_processes'] = {str(pid):stamp for pid,stamp in owned.items()}
        report['all_owned_children_reaped'] = True
        report['complete'] = True
        (output / 'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2))
        print(json.dumps({'complete':True,'phases':report['phases'],'images':len(report['screenshots'])}),flush=True)
    except BaseException:
        report['complete'] = False
        (output / 'failure-report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2))
        for w in windows:
            if w.process.poll() is None:
                w.screenshot(output / 'failure.png')
        raise
    finally:
        command('xdotool','mouseup','1',check=False)
        for w in windows:
            w.cleanup()
        for pid,stamp in owned.items():
            if stamp is not None and start_time(pid) == stamp:
                os.kill(pid,signal.SIGKILL)
        if old_config is None:
            os.environ.pop('XDG_CONFIG_HOME',None)
        else:
            os.environ['XDG_CONFIG_HOME'] = old_config


if __name__ == '__main__':
    main()
