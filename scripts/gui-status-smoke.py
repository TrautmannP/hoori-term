#!/usr/bin/env python3
"""HOO-29 bundled X11 proof: bounded fixtures, actual CLIs, isolated restart.

Every PNG needs individual visual inspection. JSON/PID/byte assertions alone do
not establish rendering. --only permits focused continuation after a real fault.
Actual CLI phase records only opt-in protocol metadata, never a transcript.
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

compact = runpy.run_path(str(Path(__file__).with_name('gui-compact-smoke.py')))
helpers, base = compact['helpers'], compact['base']
Window, command, wait_for, click, start_time, read_json = [base[n] for n in
    ('Window', 'command', 'wait_for', 'click', 'start_time', 'read_json')]
resize, clipboard, geometry, long_name = [helpers[n] for n in
    ('resize', 'clipboard', 'geometry', 'long_name')]
CAPABILITIES = b'\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\\x1b[?6c'

FIXTURE = r'''
import os,sys,json,tty,termios,select,time,signal,base64
from pathlib import Path
root=Path(sys.argv[1]); number=int(sys.argv[2]); original=termios.tcgetattr(0)
tty.setraw(0); data=bytearray(); sizes=[]; query=bytearray(); seq=0
def save():
    tmp=root/'proof.tmp'
    tmp.write_text(json.dumps({'pid':os.getpid(),'input_hex':data.hex(),'query_hex':query.hex(),
        'sizes':sizes,'sequence':seq}))
    tmp.replace(root/'proof.json')
def draw(*_):
    size=os.get_terminal_size(0); sizes.append([size.columns,size.lines]); save()
    os.write(1,b'\x1b[2J\x1b[HHOO29_SELECTION\r\nUTF-8: Gr\xc3\xbc\xc3\x9fe \xe7\x95\x8c e\xcc\x81\r\n\x1b[31mRED\x1b[32m GREEN\x1b[0m\r\n')
signal.signal(signal.SIGWINCH,draw)
os.write(1,b'\x1b]7501;?\x1b\\\x1b[c')
deadline=time.monotonic()+8
expected=b'\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\\x1b[?6c'
while len(query)<len(expected) and time.monotonic()<deadline:
    if select.select([0],[],[],.1)[0]: query.extend(os.read(0,4096))
if bytes(query)!=expected: save(); raise SystemExit(19)
os.write(1,b'\x1b[?2004h'); draw()
try:
    while True:
        if select.select([0],[],[],.04)[0]:
            block=os.read(0,4096)
            if not block: break
            data.extend(block); save()
        try: update=json.loads((root/'command.json').read_text())
        except (OSError,ValueError): continue
        if update['sequence']<=seq: continue
        os.write(1,base64.b64decode(update['bytes'])); seq=update['sequence']; save()
        if 'exit' in update: raise SystemExit(update['exit'])
finally:
    os.write(1,b'\x1b[?2004l'); termios.tcsetattr(0,termios.TCSADRAIN,original)
'''


def osc(body, number=7501, bell=False):
    return f'\x1b]{number};{body}'.encode() + (b'\x07' if bell else b'\x1b\\')


def encoded(text):
    return base64.b64encode(text.encode()).decode()


def active(w):
    match = re.search(r' · (\d+)(?: —|$)', w.title())
    assert match, w.title()
    return int(match.group(1))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--only', choices=['all', 'fixtures', 'layouts', 'clis', 'restart'], default='all')
    args = parser.parse_args()
    assert os.environ.get('XDG_SESSION_TYPE') == 'x11'
    binary, output = args.binary.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    windows_dir, roots = output / 'windows', output / 'fixtures'
    windows_dir.mkdir()
    roots.mkdir()
    fixture = output / 'fixture.py'
    fixture.write_text(FIXTURE)
    cli_work = output / 'empty-cli-work'
    cli_work.mkdir()
    literal, bash_dir = output / 'space ; literal $(text)', output / 'bash-home'
    literal.mkdir()
    bash_dir.mkdir()
    config = output / 'config'
    file = config / 'hoori-term/templates-v1.json'
    catalog = {'version': 1, 'next_id': 12, 'items': [], 'order': list(range(1, 12))}
    for index in range(2, 13):
        catalog['items'].append({'id': index-1, 'identity': {'name': long_name(index),
            'accent': ['blue', 'green', 'orange'][index % 3],
            'shortcut': {'key': f'F{index}', 'modifiers': 0},
            'symbol': compact['SYMBOLS'][(index-2) % 8], 'compact': index % 2 == 0},
            'pinned': index < 4, 'shell': 'zsh' if index % 2 == 0 else 'bash',
            'directory': str(literal if index % 2 == 0 else bash_dir)})
    file.parent.mkdir(parents=True)
    file.write_text(json.dumps(catalog, ensure_ascii=False, indent=2))
    original = file.read_bytes()
    metadata = command(str(binary), '--version')
    assert 'dirty: false' in metadata
    commit = re.search(r'commit: ([0-9a-f]{40})', metadata).group(1)
    report = {'commit': commit, 'metadata': metadata, 'scope': args.only, 'phases': [],
        'screenshots': [], 'logical_sizes': [], 'os': Path('/etc/os-release').read_text(),
        'display': os.environ.get('DISPLAY'), 'session_type': os.environ.get('XDG_SESSION_TYPE'),
        'zsh': command('zsh', '--version'), 'bash': command('bash', '--version').splitlines()[0],
        'cli_versions': {'codex': command('codex', '--version'), 'claude': command('claude', '--version')},
        'clipboard': 'Only generated fixture contents are read back; previous contents are never read.'}
    old_config = os.environ.get('XDG_CONFIG_HOME')
    os.environ['XDG_CONFIG_HOME'] = str(config)
    windows, owned = [], {}
    sequences = {}

    def phase(name):
        report['phases'].append(name)
        (output / 'progress.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps({'phase': name}), flush=True)

    def track(w):
        pids = base['children'](w)
        for pid in pids:
            owned.setdefault(pid, start_time(pid))
        return pids

    def make_window(name):
        directory = windows_dir / name
        directory.mkdir()
        w = Window(binary, directory, '/usr/bin/zsh', ['-f', '-i'])
        windows.append(w)
        assert f'hoori-term [{commit[:7]}]' in w.title()
        track(w)
        sized(w, 1280)
        return w

    def sized(w, width, height=700):
        report['logical_sizes'].append(resize(w, width, height))

    def shot(w, name):
        command('xdotool', 'mousemove', '--window', w.id, '500', '320')
        time.sleep(.2)
        w.screenshot(output / name)
        report['screenshots'].append(name)

    def send(w, text):
        clipboard(text.encode())
        w.key('ctrl+shift+v')
        w.key('Return')

    def activate(w, index, tab=None):
        w.key(f'F{index}')
        wait_for(lambda: active(w) == (tab or index))
        time.sleep(.1)

    def proof(index):
        return read_json(roots / str(index) / 'proof.json')

    def update(index, data, exit_code=None):
        sequences[index] = sequences.get(index, 0) + 1
        value = {'sequence': sequences[index], 'bytes': base64.b64encode(data).decode()}
        if exit_code is not None:
            value['exit'] = exit_code
        root = roots / str(index)
        temporary = root / 'command.tmp'
        temporary.write_text(json.dumps(value))
        temporary.replace(root / 'command.json')
        wait_for(lambda: (proof(index) or {}).get('sequence') == value['sequence'])
        time.sleep(.15)

    def close(w, image=None):
        pids = track(w)
        w.request_close()
        assert 'Schließen bestätigen' in w.title()
        if image:
            shot(w, image)
        w.key('Escape')
        wait_for(lambda: 'Schließen bestätigen' not in w.title())
        assert track(w) == pids and file.read_bytes() == original
        w.request_close()
        w.key('Return')
        w.wait_exit()
        wait_for(lambda: all(start_time(pid) != owned[pid] for pid in pids))

    def inspect_active(w):
        w.key('ctrl+shift+l')
        w.key('i')
        wait_for(lambda: 'Titel/Status' in w.title())

    try:
        if args.only in ('all', 'fixtures', 'layouts'):
            w = make_window('01-fixtures')
            assert len(track(w)) == 1
            compact['configure'](w, long_name(1), 311, 'F1')
            shell_pids, fixture_pids = {}, {}
            states = ['idle', 'working', 'done', 'blocked', 'error', 'working',
                      'idle', 'blocked', 'error', 'done', 'working', 'idle']
            for index in range(1, 13):
                if index > 1:
                    activate(w, index)
                base['clean_prompt'](w, bash=index > 1 and index % 2 == 1)
                current = track(w)
                assert len(current) == index
                root = roots / str(index)
                root.mkdir()
                send(w, f'python3 {shlex.quote(str(fixture))} {shlex.quote(str(root))} {index}; exit $?')
                value = wait_for(lambda: proof(index))
                assert bytes.fromhex(value['query_hex']) == CAPABILITIES
                fixture_pids[index] = value['pid']
                owned[value['pid']] = start_time(value['pid'])
                title = f'CLI {index}: Grüße 界 ; ' + 'long terminal title ; ' * 12
                status = osc(f'state={states[index-1]}:app=fixture:kind=question:progress=42:msg={encoded(f"Synthetic report for tab {index}")}')
                update(index, osc(title, 2, bell=index % 2 == 0) + status)
                wait_for(lambda: f'CLI {index}:' in w.title())
                shell_pids[index] = next(p for p in current if p not in shell_pids.values())
            pids = track(w)
            assert len(pids) == 12 and file.read_bytes() == original
            report['shell_pids_by_tab'], report['fixture_pids_by_tab'] = shell_pids, fixture_pids
            report['all_twelve_same_source_capability_handshakes'] = True
            activate(w, 2)
            shot(w, '01-twelve-mixed-wide-title-cli-pty.png')
            sized(w, 640)
            shot(w, '02-twelve-mixed-narrow.png')
            activate(w, 12)
            shot(w, '03-direct-target-overflow.png')
            sized(w, 1280)
            activate(w, 6)
            records = osc('state=clear') + osc('state=working:app=fixture:progress=38')
            records += osc(f'id=build:state=blocked:kind=permission:progress=55:title={encoded("Waiting build 界")}:msg={encoded("Explicit permission requested; synthetic fixture only.")}')
            records += osc(f'id=build/child:state=error:msg={encoded("Synthetic child error; no command is executed.")}')
            records += osc('id=result:state=done')
            records += osc(f'id=zz-last:state=idle:msg={encoded("Last record " + "plain Unicode ✓; " * 90)}')
            update(6, records)
            inspect_active(w)
            shot(w, '04-full-title-multirecord-keyboard.png')
            sized(w, 640, 360)
            w.key('End')
            shot(w, '05-minimum-window-last-record.png')
            w.key('Home')
            shot(w, '06-minimum-window-full-own-identity.png')
            if args.only == 'layouts':
                w.key('Escape')
                close(w)
                phase('changed-header-layouts-only-complete')
            else:
                w.key('F12')
                w.key('x')
                w.key('ctrl+shift+t')
                clipboard(b'HIDDEN_STATUS_INPUT')
                w.key('ctrl+shift+v')
                time.sleep(.2)
                assert active(w) == 6 and track(w) == pids
                assert all(proof(i)['input_hex'] == '' for i in range(1, 13))
                w.key('Escape')
                wait_for(lambda: 'Titel/Status' not in w.title())
                sized(w, 1280)
                activate(w, 1)
                # Keyboard focus on inactive tab 6 (order is pin 2,3; ordinary 1,4..12).
                w.key('ctrl+shift+l')
                w.key('Home')
                for _ in range(5):
                    w.key('Right')
                w.key('i')
                wait_for(lambda: 'Titel/Status' in w.title())
                assert active(w) == 1
                update(6, osc('id=build/child:state=done') + osc('id=build:state=blocked:kind=auth'))
                shot(w, '07-inactive-details-live-update.png')
                w.key('Escape')
                activate(w, 2)
                # Direct activation reveals compact pinned tab 2 at the viewport's left.
                command('xdotool', 'mousemove', '--window', w.id, '95', '20')
                command('xdotool', 'click', '3')
                wait_for(lambda: 'Titel/Status' in w.title())
                shot(w, '08-mouse-full-compact-pinned-details.png')
                w.key('Return')
                w.key('ctrl+shift+w')
                assert 'Schließen bestätigen' not in w.title()
                assert track(w) == pids
                activate(w, 6)
                update(6, b'\x1b[?1049h\x1b[!p\x1b[?1049l')
                clipboard(b'STATUS_ONLY\nsecond')
                w.key('ctrl+shift+v')
                w.type('ok!')
                expected = b'\x1b[200~STATUS_ONLY\nsecond\x1b[201~ok!'
                wait_for(lambda: bytes.fromhex(proof(6)['input_hex']) == expected)
                assert all(proof(i)['input_hex'] == '' for i in range(1, 13) if i != 6)
                report['active_input_exact_hex'] = expected.hex()
                report['inspection_direct_shortcut_plus_paste_do_not_leak'] = True
                sized(w, 640, 600)
                sized(w, 1280, 700)
                assert len({tuple(size) for size in proof(6)['sizes']}) > 1
                command('xdotool', 'mousemove', '--window', w.id, '12', '72')
                command('xdotool', 'mousedown', '1')
                command('xdotool', 'mousemove', '--sync', '--window', w.id, '158', '72')
                command('xdotool', 'mouseup', '1')
                w.key('ctrl+shift+c')
                assert command('xclip', '-selection', 'clipboard', '-o') == 'HOO29_SELECTION'
                shot(w, '09-selection-ansi-after-resize.png')
                update(6, b'\x1b]133;A\x1b\\')
                inspect_active(w)
                shot(w, '10-prompt-keeps-only-results.png')
                w.key('Escape')
                update(6, b'\x1bc')
                inspect_active(w)
                shot(w, '11-ris-clears-title-and-reports.png')
                w.key('Escape')
                update(6, osc('state=blocked:kind=question') + osc('id=result:state=error'), exit_code=7)
                wait_for(lambda: start_time(shell_pids[6]) != owned[shell_pids[6]])
                inspect_active(w)
                shot(w, '12-exited-pty-retained-error-result.png')
                w.key('Escape')
                assert file.read_bytes() == original
                assert all(start_time(pid) == owned[pid] for tab, pid in shell_pids.items() if tab != 6)
                report['runtime_metadata_does_not_write_catalog_or_change_other_pids'] = True
                close(w, '13-own-names-window-close-cancel.png')
                phase('twelve-tabs-title-status-details-routing-lifecycle-complete')

        if args.only in ('all', 'clis'):
            w = make_window('02-actual-clis')
            base['clean_prompt'](w)
            compact['configure'](w, 'Codex own identity', 311, 'F1')
            observer = Path(__file__).with_name('cli-protocol-observer.py')
            codex_log, claude_log = output / 'codex-protocol.json', output / 'claude-protocol.json'
            codex_args = ['codex', '--no-daemon', '--sandbox', 'read-only', '-a', 'never', '-C', str(cli_work),
                '-c', 'tui.terminal_title=["app-name","status","project"]']
            send(w, 'cd ' + shlex.quote(str(cli_work)) + '; python3 ' + shlex.quote(str(observer)) + ' ' +
                 shlex.quote(str(codex_log)) + ' -- ' + shlex.join(codex_args))
            # Startup/trust dialogs are captured; only this owned empty directory may be trusted.
            time.sleep(4)
            shot(w, '14-codex-initial-startup.png')
            phase('actual-codex-started-review-startup-image-before-continuing')
            # The operator inspects the startup image and handles only this
            # owned directory's trust dialog before releasing the next step.
            wait_for(lambda: (output / 'continue-codex').exists(), timeout=60)
            wait_for(lambda: any(e['osc'] in (0, 2) and 'codex' in e['body'].lower()
                                 for e in (read_json(codex_log) or {}).get('events', [])), timeout=45)
            for pid in [read_json(codex_log)[key] for key in ('bridge_pid', 'cli_pid')]:
                owned[pid] = start_time(pid)
            shot(w, '15-actual-codex-title.png')
            w.type('HOO29 unsent draft')
            w.key('ctrl+shift+b')
            wait_for(lambda: active(w) == 2)
            compact['configure'](w, 'Claude own identity', 460, 'ctrl+shift+F2')
            base['clean_prompt'](w, bash=True)
            claude_args = ['claude', '--safe-mode', '--strict-mcp-config', '--mcp-config', '{"mcpServers":{}}',
                           '--tools', '', '--disable-slash-commands', '--no-chrome', '--name', 'HOO29 actual Claude']
            send(w, 'cd ' + shlex.quote(str(cli_work)) + '; python3 ' + shlex.quote(str(observer)) + ' ' +
                 shlex.quote(str(claude_log)) + ' -- ' + shlex.join(claude_args))
            time.sleep(4)
            shot(w, '16-claude-initial-startup.png')
            phase('actual-claude-started-review-startup-image-before-continuing')
            wait_for(lambda: (output / 'continue-claude').exists(), timeout=60)
            wait_for(lambda: any(e['direction'] == 'output' and e['osc'] == 7501 and 'state=' in e['body']
                                 for e in (read_json(claude_log) or {}).get('events', [])), timeout=45)
            for pid in [read_json(claude_log)[key] for key in ('bridge_pid', 'cli_pid')]:
                owned[pid] = start_time(pid)
            shot(w, '17-actual-claude-title-status.png')
            inspect_active(w)
            shot(w, '18-actual-claude-full-reported-details.png')
            w.key('Escape')
            w.type('HOO29 unsent Claude draft')
            activate(w, 1)
            shot(w, '19-return-to-actual-codex-unsent-draft.png')
            w.key('ctrl+shift+F2')
            wait_for(lambda: active(w) == 2)
            shot(w, '20-return-to-actual-claude-unsent-draft.png')
            codex_data, claude_data = read_json(codex_log), read_json(claude_log)
            report['codex_actual_output_osc_7501_count'] = sum(e['direction'] == 'output' and e['osc'] == 7501 for e in codex_data['events'])
            report['claude_actual_query_answered'] = any(e['direction'] == 'input' and e['osc'] == 7501 for e in claude_data['events'])
            report['cli_invocations'] = {'codex': codex_args, 'claude': claude_args}
            assert report['claude_actual_query_answered']
            assert file.read_bytes() == original
            close(w, '21-actual-cli-own-names-close.png')
            phase('actual-linux-cli-title-and-claude-reported-status-complete')

        if args.only in ('all', 'restart'):
            w = make_window('03-restart')
            assert len(track(w)) == 1
            base['clean_prompt'](w)
            compact['configure'](w, 'Restart own identity', 311, 'F1')
            inspect_active(w)
            shot(w, '22-restart-has-no-restored-reports.png')
            w.key('Escape')
            activate(w, 2)
            assert len(track(w)) == 2
            inspect_active(w)
            shot(w, '23-deliberate-template-open-no-restored-reports.png')
            w.key('Escape')
            assert file.read_bytes() == original
            close(w)
            phase('restart-deliberate-template-open-runtime-not-persisted')
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
