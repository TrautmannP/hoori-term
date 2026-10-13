#!/usr/bin/env python3
"""Transparent PTY bridge for an explicitly launched CLI review fixture.

Only bounded OSC 0/2/7501 metadata and fixed terminal query replies are recorded.
Terminal contents, user input, credentials and environment values are not logged.
This opt-in test helper is not part of the application or its default logging.
"""
import argparse
import errno
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import sys
import termios
import time
import tty


class Observer:
    def __init__(self, report, direction, save):
        self.report, self.direction, self.save = report, direction, save
        self.state, self.body, self.count = 'ground', bytearray(), 0

    def feed(self, data):
        for byte in data:
            if self.state == 'ground':
                if byte == 27:
                    self.state = 'escape'
            elif self.state == 'escape':
                self.state = 'osc' if byte == 93 else 'escape' if byte == 27 else 'ground'
                self.body.clear()
                self.count = 2
            elif self.state == 'osc':
                self.count += 1
                if byte == 7:
                    self.finish('BEL')
                elif byte == 27:
                    self.state = 'st'
                elif byte in (24, 26):
                    self.state = 'ground'
                elif self.count <= 4096:
                    self.body.append(byte)
            elif self.state == 'st':
                self.count += 1
                if byte == 92:
                    self.finish('ST')
                else:
                    self.body.clear()
                    self.state = 'osc' if byte == 93 else 'escape' if byte == 27 else 'ground'
                    self.count = 2

    def finish(self, terminator):
        if self.count <= 4096 and len(self.report['events']) < 200:
            number, separator, body = bytes(self.body).partition(b';')
            wanted = (number in (b'0', b'2', b'7501') if self.direction == 'output'
                      else number == b'7501' and body.startswith(b'?:states='))
            if separator and wanted:
                self.report['events'].append({'direction': self.direction,
                    'osc': int(number), 'body': body.decode('utf-8', errors='replace'),
                    'terminator': terminator, 'seconds': round(time.monotonic() - self.report['_start'], 3)})
                self.save()
        self.body.clear()
        self.state = 'ground'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('report', type=Path)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    assert command and sys.stdin.isatty() and sys.stdout.isatty()
    path = args.report.resolve()
    assert not path.exists()
    report = {'command': command, 'cwd': os.getcwd(), 'events': [],
              'bridge_pid': os.getpid(), '_start': time.monotonic(),
              'recorded_scope': 'OSC 0/2/7501 only; no terminal transcript or user-input log'}

    def save():
        temporary = path.with_suffix('.tmp')
        temporary.write_text(json.dumps({k: v for k, v in report.items() if k != '_start'},
                                        ensure_ascii=False, indent=2))
        temporary.replace(path)

    original = termios.tcgetattr(0)
    window_size = fcntl.ioctl(0, termios.TIOCGWINSZ, struct.pack('HHHH', 0, 0, 0, 0))
    pid, master = pty.fork()
    if pid == 0:
        fcntl.ioctl(0, termios.TIOCSWINSZ, window_size)
        os.execvp(command[0], command)
    report['cli_pid'] = pid
    save()
    outgoing, incoming = Observer(report, 'output', save), Observer(report, 'input', save)

    def resized(*_):
        try:
            fcntl.ioctl(master, termios.TIOCSWINSZ, fcntl.ioctl(0, termios.TIOCGWINSZ, window_size))
        except OSError:
            pass

    def stopped(signum, _):
        try:
            os.killpg(pid, signum)
        except ProcessLookupError:
            pass

    signal.signal(signal.SIGWINCH, resized)
    signal.signal(signal.SIGHUP, stopped)
    signal.signal(signal.SIGTERM, stopped)
    tty.setraw(0)
    try:
        while True:
            ready, _, _ = select.select([0, master], [], [], .2)
            for source in ready:
                try:
                    data = os.read(source, 16384)
                except OSError as error:
                    if source == master and error.errno == errno.EIO:
                        data = b''
                    else:
                        raise
                if not data:
                    return
                if source == master:
                    outgoing.feed(data)
                    os.write(1, data)
                else:
                    incoming.feed(data)
                    os.write(master, data)
    finally:
        termios.tcsetattr(0, termios.TCSADRAIN, original)
        os.close(master)
        try:
            _, status = os.waitpid(pid, 0)
            report['exit_code'] = os.waitstatus_to_exitcode(status)
        except ChildProcessError:
            pass
        report['finished'] = True
        save()


if __name__ == '__main__':
    main()
