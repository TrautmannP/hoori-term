#!/usr/bin/env python3
"""Owned X11 window/header exercise with isolated synthetic sessions only.

--baseline captures the preceding decorated build's equivalent layouts.
Inspect every image independently of native state, PID and input assertions.
"""
import argparse
import json
import os
from pathlib import Path
import re
import runpy
import shlex
import shutil
import signal
import subprocess
import time

helpers = runpy.run_path(str(Path(__file__).with_name('gui-templates-smoke.py')))
Window, command, wait_for, click, clean_prompt, start_time = [helpers[n] for n in
    ('Window', 'command', 'wait_for', 'click', 'clean_prompt', 'start_time')]
identity = helpers['base']


def geometry(w):
    w.assert_owned()
    values = dict(line.split('=', 1) for line in command(
        'xdotool', 'getwindowgeometry', '--shell', w.id).splitlines())
    return {k: int(values[k]) for k in ('X', 'Y', 'WIDTH', 'HEIGHT')}


def native_state(w):
    w.assert_owned()
    return command('xprop', '-id', w.id, '_NET_WM_STATE')


def maximized(w):
    state = native_state(w)
    return all('MAXIMIZED_' + axis in state for axis in ('HORZ', 'VERT'))


def resize(w, width, height=700):
    w.focus()
    command('xdotool', 'windowsize', '--sync', w.id, str(width), str(height))
    wait_for(lambda: (geometry(w)['WIDTH'], geometry(w)['HEIGHT']) == (width, height))
    time.sleep(.2)
    return geometry(w)


def clipboard(value):
    # Replace the clipboard with this own fixture; never read old contents.
    subprocess.run(['xclip', '-selection', 'clipboard', '-in'], input=value,
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def configure(w, label, color, key):
    identity['open_editor'](w)
    click(w, 240, 114)
    w.key('ctrl+a')
    clipboard(label.encode())
    w.key('ctrl+v')
    time.sleep(.25)
    click(w, color, 183)
    identity['capture'](w, key)
    w.key('Return')
    wait_for(lambda: 'Tab bearbeiten' not in w.title() and label in w.title())


def drag(w, x, y, dx, dy):
    w.focus()
    before = geometry(w)
    command('xdotool', 'mousemove', '--window', w.id, str(x), str(y))
    command('xdotool', 'mousedown', '1')
    try:
        time.sleep(.15)
        # Cross the deliberate-motion threshold while held, then continue the
        # gesture after the native manager has received the request.
        command('xdotool', 'mousemove', '--sync',
                str(before['X'] + x + (4 if dx > 0 else -4 if dx else 0)),
                str(before['Y'] + y + (4 if dy > 0 else -4 if dy else 0)))
        time.sleep(.1)
        command('xdotool', 'mousemove', '--sync',
                str(before['X'] + x + dx), str(before['Y'] + y + dy))
        time.sleep(.2)
    finally:
        command('xdotool', 'mouseup', '1')
    time.sleep(.2)
    w.assert_owned()
    assert command('xdotool', 'getactivewindow') == w.id, 'Focus changed during gesture'
    return before, geometry(w)


def double_drag_area(w):
    w.focus()
    g = geometry(w)
    command('xdotool', 'mousemove', '--window', w.id, str(g['WIDTH'] - 196), '26')
    command('xdotool', 'click', '--repeat', '2', '--delay', '100', '1')


def read_json(path):
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def long_name(index):
    if index in (1, 2):
        return 'Long duplicate session for review'.ljust(40, 'x')
    if index == 6:
        return 'Grüße 界 Unicode session for review'.ljust(40, 'x')
    return f'Long session {index:02d} for review'.ljust(40, 'x')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--baseline', action='store_true')
    parser.add_argument('--only', choices=['all', 'window', 'layouts', 'empty'], default='all')
    args = parser.parse_args()
    assert os.environ.get('XDG_SESSION_TYPE') == 'x11', 'This exercise proves X11 only'
    required = ['xdotool', 'xprop', 'xwininfo', 'xrandr', 'xclip', 'import']
    if not args.baseline:
        required.append('wmctrl')
    missing = [name for name in required if shutil.which(name) is None]
    assert not missing, f'Missing X11 test tools: {", ".join(missing)}'
    binary, output = args.binary.resolve(), args.output.resolve()
    output.mkdir(exist_ok=False, parents=True)
    config = output / 'config'
    file = config / 'hoori-term/templates-v1.json'
    file.parent.mkdir(parents=True)
    catalog = {'version': 1, 'next_id': 12, 'items': []}
    for index in range(2, 13):
        directory = output / (f'cwd-{index:02d}' if index != 2 else 'space ; literal $(text)')
        directory.mkdir()
        catalog['items'].append({
            'id': index - 1,
            'identity': {'name': {2: 'Frontend', 3: 'Deploy'}.get(index, long_name(index)),
                         'accent': ['blue', 'green', 'orange'][(index - 1) % 3],
                         'shortcut': {'key': f'F{index}', 'modifiers': 0}},
            'shell': 'bash' if index % 3 == 0 else 'zsh', 'directory': str(directory)})
    file.write_text(json.dumps(catalog, ensure_ascii=False, indent=2))
    os.environ['XDG_CONFIG_HOME'] = str(config)
    metadata = command(str(binary), '--version')
    commit = re.search(r'commit: ([0-9a-f]{40})', metadata).group(1)
    report = {'metadata': metadata, 'commit': commit, 'baseline_only': args.baseline,
              'scope': args.only, 'session_type': os.environ['XDG_SESSION_TYPE'],
              'display': os.environ.get('DISPLAY'), 'os': Path('/etc/os-release').read_text(),
              'desktop': command('gnome-shell', '--version'),
              'text_scale': command('gsettings', 'get', 'org.gnome.desktop.interface', 'text-scaling-factor'),
              'clipboard': 'Only generated name/copy/paste fixtures replace the clipboard; no old clipboard read.',
              'screenshots': [], 'phases': [], 'geometry': []}
    windows, owned = [], {}

    def track(w):
        w.assert_owned()
        ids = sorted(int(p) for p in Path(
            f'/proc/{w.process.pid}/task/{w.process.pid}/children').read_text().split())
        for pid in ids:
            owned[pid] = start_time(pid)
        return ids

    def phase(name):
        report['phases'].append(name)
        (output / 'progress.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps({'phase': name}), flush=True)

    def shot(w, name):
        # Include only the WM frame that directly contains this owned client.
        # Never capture the root/desktop or another application's window.
        w.assert_owned()
        tree = command('xwininfo', '-id', w.id, '-tree')
        parent = re.search(r'Parent window id: (0x[0-9a-fA-F]+)', tree).group(1)
        root = re.search(r'Root window id: (0x[0-9a-fA-F]+)', tree).group(1)
        capture_id = w.id
        if int(parent, 16) != int(root, 16):
            frame = dict(line.split('=', 1) for line in command(
                'xdotool', 'getwindowgeometry', '--shell', parent).splitlines())
            client = geometry(w)
            assert client['WIDTH'] <= int(frame['WIDTH']) <= client['WIDTH'] + 100
            assert client['HEIGHT'] <= int(frame['HEIGHT']) <= client['HEIGHT'] + 100
            capture_id = parent
        command('import', '-window', capture_id, str(output / name))
        report['screenshots'].append(name)

    def create(name, shell='/usr/bin/zsh', shell_args=None):
        folder = output / name
        folder.mkdir()
        w = Window(binary, folder, shell, ['-f', '-i'] if shell_args is None else shell_args)
        windows.append(w)
        track(w)
        assert f'hoori-term [{commit[:7]}' in w.title()
        # Deterministic X11 study placement on the actual primary monitor.
        monitors = command('xrandr', '--current')
        primary = re.search(r'\S+ connected primary (\d+)x(\d+)([+-]\d+)([+-]\d+)', monitors)
        assert primary and int(primary.group(1)) >= 1440 and int(primary.group(2)) >= 900
        command('xdotool', 'windowmove', '--sync', w.id,
                str(int(primary.group(3)) + 80), str(int(primary.group(4)) + 80))
        return w

    def activate(w, key, label):
        w.key(key)
        wait_for(lambda: label in w.title())
        time.sleep(.12)

    def close(w):
        ids = track(w)
        w.request_close()
        assert 'Schließen bestätigen' in w.title()
        w.key('Return')
        w.wait_exit()
        wait_for(lambda: all(start_time(p) != owned[p] for p in ids))

    def window_flow(w, pids):
        activate(w, 'F3', 'Deploy')
        ticker = output / 'ticker.json'
        ticker_script = output / 'ticker.py'
        ticker_script.write_text("""import json,os,sys,time
from pathlib import Path
for i in range(3000):
 Path(sys.argv[1]).write_text(json.dumps({'pid':os.getpid(),'count':i}))
 print('HOO32 inactive tick',i,flush=True)
 time.sleep(.1)
""")
        w.run('python3 ' + shlex.quote(str(ticker_script)) + ' ' + shlex.quote(str(ticker)))
        wait_for(lambda: read_json(ticker))
        ticker_pid = read_json(ticker)['pid']
        owned[ticker_pid] = start_time(ticker_pid)
        activate(w, 'F1', 'Backend')
        proof, sizes = output / 'window-routing.bin', output / 'raw-tty-size.json'
        reader = output / 'raw-reader.py'
        reader.write_text(r'''import json,os,signal,sys,termios,tty
from pathlib import Path
old=termios.tcgetattr(0)
def size(*_):
 s=os.get_terminal_size(0)
 Path(sys.argv[2]).write_text(json.dumps({'pid':os.getpid(),'columns':s.columns,'lines':s.lines}))
signal.signal(signal.SIGWINCH,size)
tty.setraw(0)
size()
os.write(1,b'\x1b[2J\x1b[HHOO32_SELECTION\r\n\x1b[?2004h\x1b]2;HOO-32 raw-ready\x07')
data=bytearray()
try:
 while not data.endswith(b'!'): data.extend(os.read(0,4096))
 Path(sys.argv[1]).write_bytes(data)
finally:
 termios.tcsetattr(0,termios.TCSANOW,old)
 os.write(1,b'\x1b[?2004l\x1b]2;HOO-32 raw-done\x07')
''')
        w.run('python3 ' + shlex.quote(str(reader)) + ' ' + shlex.quote(str(proof)) + ' ' + shlex.quote(str(sizes)))
        wait_for(lambda: 'HOO-32 raw-ready' in w.title() and read_json(sizes))
        owned[read_json(sizes)['pid']] = start_time(read_json(sizes)['pid'])
        original_config = file.read_bytes()
        before, after = drag(w, 1280 - 196, 26, 50, 30)
        assert (after['WIDTH'], after['HEIGHT']) == (before['WIDTH'], before['HEIGHT'])
        assert abs(after['X'] - before['X']) + abs(after['Y'] - before['Y']) >= 20
        report['native_drag'] = {'before': before, 'after': after}
        normal = after
        clipboard(b'HOO32_COPY_SENTINEL')
        # Selection starts inside the terminal, then reaches the border without
        # being converted into resize. Separate press/move/release events.
        w.focus()
        command('xdotool', 'mousemove', '--window', w.id, '12', '72')
        command('xdotool', 'mousedown', '1')
        time.sleep(.1)
        command('xdotool', 'mousemove', '--window', w.id, '158', '72')
        time.sleep(.15)
        command('xdotool', 'mouseup', '1')
        w.key('ctrl+shift+c')
        wait_for(lambda: command('xclip', '-selection', 'clipboard', '-out') == 'HOO32_SELECTION')
        before, after = drag(w, 12, 72, 1266, 0)
        assert before == after
        command('xdotool', 'mousemove', '--window', w.id, '500', '400')
        shot(w, '03-terminal-selection-at-border.png')
        # Keyboard focus reaches the maximize control without terminal bytes.
        w.key('ctrl+shift+l')
        w.key('Home')
        w.key('shift+Tab')
        w.key('shift+Tab')
        shot(w, '04-window-control-keyboard-focus.png')
        w.key('Return')
        wait_for(lambda: maximized(w))
        time.sleep(1.2)  # Inspect the observed state, not an optimistic icon.
        shot(w, '05-native-maximized.png')
        command('wmctrl', '-ir', w.id, '-b', 'remove,maximized_vert,maximized_horz')
        wait_for(lambda: not maximized(w))
        wait_for(lambda: geometry(w) == normal)
        time.sleep(1.2)
        shot(w, '06-external-restore-observed.png')
        double_drag_area(w)
        wait_for(lambda: maximized(w))
        double_drag_area(w)
        wait_for(lambda: not maximized(w))
        wait_for(lambda: geometry(w) == normal)
        report['keyboard_maximize_external_restore_and_double_click'] = True
        # Own mouse control also uses the native confirmed state.
        click(w, geometry(w)['WIDTH'] - 62, 26)
        wait_for(lambda: maximized(w))
        click(w, geometry(w)['WIDTH'] - 62, 26)
        wait_for(lambda: not maximized(w))
        wait_for(lambda: geometry(w) == normal)
        gestures = [('east',-30,0),('west',30,0),('north',0,25),('south',0,-25),
                    ('northwest',20,20),('northeast',-20,20),('southwest',20,-20),('southeast',-20,-20)]
        report['native_resizes'] = []
        for edge, dx, dy in gestures:
            g = geometry(w)
            x = 2 if 'west' in edge else g['WIDTH']-2 if 'east' in edge else g['WIDTH']//2
            y = 2 if 'north' in edge else g['HEIGHT']-2 if 'south' in edge else g['HEIGHT']//2
            old_size = read_json(sizes)
            before, after = drag(w,x,y,dx,dy)
            assert (after['WIDTH'] < before['WIDTH']) == bool(dx)
            assert (after['HEIGHT'] < before['HEIGHT']) == bool(dy)
            wait_for(lambda: read_json(sizes)['columns'] == (after['WIDTH'] - 20)//10)
            if dy: wait_for(lambda: read_json(sizes)['lines'] < old_size['lines'])
            report['native_resizes'].append({'edge':edge,'before':before,'after':after,'pty_size':read_json(sizes)})
        shot(w, '07-eight-native-border-resizes.png')
        count = read_json(ticker)['count']
        click(w, geometry(w)['WIDTH'] - 98, 26)
        wait_for(lambda: 'HIDDEN' in native_state(w))
        time.sleep(.5)
        # OS window-manager activation, the task-switch/taskbar return path;
        # never request application unminimize or type in a foreign window.
        command('wmctrl', '-ia', w.id)
        wait_for(lambda: 'HIDDEN' not in native_state(w))
        w.focus()
        wait_for(lambda: read_json(ticker)['count'] > count)
        assert track(w) == pids
        report['minimize_return'] = {'return_method':'OS _NET_ACTIVE_WINDOW via wmctrl',
                                     'ticker_before':count,'ticker_after':read_json(ticker)['count']}
        shot(w, '08-minimize-os-return-retains-session.png')
        resize(w,1280)
        click(w,1280-296,26)
        wait_for(lambda: 'Neue Sitzung' in w.title())
        assert track(w) == pids
        shot(w, '09-plus-menu-with-visible-build.png')
        w.key('Escape')
        identity['open_editor'](w)
        w.type('discarded window draft')
        click(w,1280-26,26)
        wait_for(lambda: 'Schließen bestätigen' in w.title())
        assert 'Tab bearbeiten' not in w.title()
        shot(w, '10-own-close-modal-with-visible-build.png')
        click(w,1280-26,26)  # Repetition is not confirmation.
        w.key('alt+F4')
        wait_for(lambda: 'Schließen bestätigen' in w.title())
        assert track(w) == pids
        w.key('Escape')
        wait_for(lambda: 'Schließen bestätigen' not in w.title())
        assert track(w) == pids and file.read_bytes() == original_config
        clipboard(b'PASTE_ONLY\nsecond')
        w.key('ctrl+shift+v')
        w.type('ok!')
        wait_for(proof.exists)
        assert proof.read_bytes() == b'\x1b[200~PASTE_ONLY\nsecond\x1b[201~ok!', proof.read_bytes()
        wait_for(lambda: 'HOO-32 raw-done' in w.title())
        activate(w,'F3','Deploy')
        w.key('ctrl+c')
        wait_for(lambda: start_time(ticker_pid) != owned[ticker_pid])
        w.marker('HOO-32 inactive output retained')
        assert track(w) == pids
        report['selection_copy_paste_window_routing_close_cancel_and_inactive_output'] = True
        phase('native-window-paths-complete')

    try:
        if args.only != 'empty':
            w = create('01-main')
            assert len(track(w)) == 1
            time.sleep(.3)
            assert len(track(w)) == 1  # Saved fixtures never auto-start.
            w.key('ctrl+u')
            clean_prompt(w,state='ONE')
            w.marker('HOO-32 first ready')
            configure(w,'Backend',256,'F1')
            for index,label in [(2,'Frontend'),(3,'Deploy')]:
                activate(w,f'F{index}',label)
                w.key('ctrl+u')
                clean_prompt(w,bash=index==3,state=f'SESSION_{index}')
                w.marker(f'HOO-32 session-{index} ready')
            pids = track(w)
            assert len(pids) == 3
            report['three_shell_pids'] = pids
            report['frame_extents'] = command('xprop','-id',w.id,'_NET_FRAME_EXTENTS','_MOTIF_WM_HINTS')
            activate(w,'F1','Backend')
            for index,width in [(1,640),(2,1280)]:
                report['geometry'].append(resize(w,width))
                command('xdotool','mousemove','--window',w.id,'300','400')
                shot(w,f'{index:02d}-three-tabs-{width}.png')
            phase('three-tab-layouts')
            if not args.baseline and args.only in ('all','window'):
                window_flow(w,pids)
            if args.baseline or args.only in ('all','layouts'):
                for index,color in [(1,256),(2,311),(3,430)]:
                    activate(w,f'F{index}',{1:'Backend',2:'Frontend',3:'Deploy'}[index])
                    configure(w,long_name(index),color,f'F{index}')
                for index in range(4,13):
                    activate(w,f'F{index}',long_name(index))
                all_pids = track(w)
                assert len(all_pids) == 12 and set(pids).issubset(all_pids)
                report['twelve_shell_pids'] = all_pids
                for width in [640,1280]:
                    resize(w,width)
                    activate(w,'F1',long_name(1))
                    command('xdotool','mousemove','--window',w.id,'300','400')
                    shot(w,f'11-twelve-long-tabs-{width}.png')
                resize(w,640)
                activate(w,'F12',long_name(12))
                shot(w,'12-twelve-direct-target-visible-640.png')
                assert track(w) == all_pids
                current = read_json(file)
                assert len(current['items']) == 11  # Ordinary Backend remains temporary.
                assert current['items'][0]['identity']['name'] == long_name(2)
                assert [t['id'] for t in current['items']] == list(range(1,12))
                report['long_layout_identity_and_integrated_templates'] = True
                if not args.baseline:
                    w.run('exit 7')
                    wait_for(lambda: len(track(w)) == 11)
                    wait_for(lambda: 'beendet' in w.title())
                    shot(w,'13-exited-tab-retained.png')
                    click(w,640-26,26)
                    wait_for(lambda: 'Schließen bestätigen' in w.title())
                    shot(w,'14-twelve-named-window-close-640.png')
                    before = track(w)
                    w.key('alt+F4')
                    assert track(w) == before
                    w.key('Escape')
                    wait_for(lambda: 'Schließen bestätigen' not in w.title())
                    assert track(w) == before
                phase('twelve-tab-layouts-and-close-cancellation')
            close(w)
        if not args.baseline and args.only in ('all','empty'):
            w = create('02-failed-default','/nonexistent/hoo32-owned-missing-shell',[])
            assert not track(w)
            shot(w,'15-failed-session-window-header.png')
            click(w,geometry(w)['WIDTH']-26,26)
            w.wait_exit()  # No running-session confirmation, no replacement PTY.
            report['failed_session_own_close_exits_without_start_or_confirmation'] = True
        report['all_owned_processes_reaped'] = all(start_time(p) != stamp for p,stamp in owned.items())
        assert report['all_owned_processes_reaped']
        report['complete'] = True
        phase('complete')
        (output/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2))
        print(json.dumps(report,ensure_ascii=False,indent=2),flush=True)
    except Exception as error:
        report.update(complete=False,failure=f'{type(error).__name__}: {error}')
        if windows and windows[-1].process.poll() is None:
            try: shot(windows[-1],'failure.png')
            except Exception as shot_error: report['failure_screenshot_error'] = repr(shot_error)
        (output/'failure-report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2))
        raise
    finally:
        for w in windows:
            if w.process.poll() is None: track(w)
            w.cleanup()
        for pid,stamp in owned.items():
            if stamp and start_time(pid) == stamp:
                try: os.kill(pid,signal.SIGTERM)
                except ProcessLookupError: pass


if __name__ == '__main__':
    main()
