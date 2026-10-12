# HOO-6: native Ubuntu terminal integration spike

Source: [HOO-6](https://linear.app/hoori-term/issue/HOO-6/technischer-spike-interaktives-ubuntu-terminal-mit-review-build),
approved scope from [HOO-5](https://linear.app/hoori-term/issue/HOO-5/programmkonzept),
D-03 through D-06. This is a one-session feasibility prototype, not the
customizable multi-tab intermediate version or a platform release acceptance.

## Integration

Rust 1.98.1, iced 0.14.0, iced_wgpu 0.14.0 / wgpu 27.0.1,
alacritty_terminal 0.26.0 and portable-pty 0.9.0 are pinned in `Cargo.lock`.
The application uses iced's wgpu renderer; no browser, WebView, account,
network client or telemetry is part of the application.

`session` owns PTY handles, child lifecycle and separate reader/writer/waiter
threads. On Ubuntu the master descriptor is nonblocking, so worker shutdown
can interrupt reads and writes. The UI only receives queued output, sends
input with `try_send` and performs the native window-size ioctl. PTY reads
and writes never wait on the event loop. Shutdown runs on the executor,
closes channels, sends hangup to the foreground job and shell, escalates
for an unresponsive shell, reaps it and joins transport threads.

`terminal` incrementally parses bytes into alacritty_terminal's grid and
terminal modes. `input` encodes text, Ctrl combinations, navigation and
function keys, respecting application cursor mode. `view` maps grid cells,
colors, wide-cell spacing, combining characters and cursor shapes to
cached iced canvas geometry. The canvas is rendered by wgpu; this is a
simple cell renderer, not yet a dedicated terminal glyph atlas.

Output refreshes use coalesced notifications and a 16 ms frame interval.
There is no periodic UI redraw when the terminal is idle. Each update parses
at most 64 KiB, checking a soft 4 ms budget between chunks. A single chunk
can exceed that budget; it is not a hard real-time deadline.

Explicit bounds:

* Output channel: 64 chunks of up to 8 KiB (512 KiB), plus the worker's current
  chunk. Backpressure reaches the child; terminal bytes are not dropped.
* Input channel: 64 entries of up to 4 KiB (256 KiB), plus the in-flight write.
  Saturation returns an immediately visible error rather than blocking the UI.
* Wakeups are coalesced in a bounded notification channel, independent of data.
* Primary scrollback: 2,000 lines. Viewport: up to 500 columns × 200 rows.
  This is a line/cell bound, not a byte quota on all font/core metadata.
* At most 64 forwarded terminal events per parse batch; shell titles are
  limited to 120 characters. Only title changes and terminal protocol replies
  reach the application. Clipboard OSC 52 is disabled at the core and no
  clipboard, hyperlink-opening or host-command handler is installed.

The application does not record terminal output or input. Shell history and
shell startup files remain the shell's own behavior. The GUI harness starts
clean shells and captures only its newly created application windows containing
synthetic fixture data; it verifies process/window ownership before input.

## Reproduce validation

Run the checks in the README. The test suite includes real Bash PTYs for
input, resize, exit status, unavailable executables, lossless backpressure
with 2 MiB of output, and shutdown with saturated input/output and a shell
ignoring hangup. Core tests cover split UTF-8, colors, cursor movement,
alternate screen, bounded history and rejected clipboard requests. Input
tests cover Unicode, space, Ctrl, AltGr and application cursor mode.

For the real X11 GUI exercise (requires `xdotool`, `xprop`, ImageMagick
`import`, zsh, Bash, Vim and Python):

```sh
cargo build --release --locked
python3 scripts/gui-smoke.py target/release/hoori-term /tmp/hoori-term-gui-evidence
```

Use a new output directory each time. Inspect every captured screenshot;
title assertions and successful file writes alone do not prove correct
rendering. The harness exercises:

1. Interactive clean zsh, colors, UTF-8 output (umlauts, CJK, combining marks),
   cursor movement and shell title changes retaining build identity.
2. Vim 9.1 with `-Nu NONE -n`: alternate screen, insertion of `Grüße 界`,
   saving and verifying exact UTF-8 bytes, three resizes and return to zsh.
3. PTY-size reporting after resize; 250,000 numbered output lines (12.5 MB
   before PTY newline translation) with twelve more resizes, then a new command.
4. Close confirmation naming zsh, Escape cancellation preserving the session,
   confirmed close and disappearance of the shell PID.
5. A separate clean Bash window with visible output and exit code 7, closing
   without confirmation; a separate nonexistent-shell error window that closes.

The harness uses 30 ms ASCII key spacing and holds synthetic Unicode keys
for 250 ms so temporary XKB mappings remain available to winit. Very fast or newly mapped X11
test keys were unreliable in early harness runs; these are not throughput
measurements or broad keyboard/IME certification. Ordinary keyboard text and
the exact UTF-8 Vim file are checked independently from parser fixtures.

## Observed environment and result

Validation host: Ubuntu 24.04.5 LTS x86_64, X.Org 21.1.11, X11 session on
`:1`, NVIDIA GeForce GTX 1070 / driver 580.178.04, DejaVu Sans Mono as the
monospace font. Vim 9.1, zsh and Bash versions are recorded with delivery.
This is an actual X11 window test, not a headless compile check. Native
Wayland and Windows/PowerShell/ConPTY/WSL runtime evidence remains pending.

The first complete GUI run passed the functional scenarios. Its 4-second
idle sample consumed 18.25% of one CPU core, with 190.71 MiB application RSS.
That exposed unnecessary periodic UI redraw; the transport was changed to
coalesced notifications before delivery. The repeated run used 0.50% of one
core at idle (188.55 MiB RSS). During the numbered-output/resize sample it
used 25.83% over 3.33 seconds, with RSS from 191.28 to 201.17 MiB; the UI
continued painting output and the changed size. These are application-only
process measurements, excluding shell/fixture processes and GPU memory.
The final measurement and exact
merged build identity are recorded in the Linear delivery comment and the
versioned review artifact's GUI report. RSS/CPU samples describe this machine
and a short interval, not a performance guarantee or a long-duration soak test.

## Recommendation and remaining risks

Continue with iced/wgpu, alacritty_terminal and portable-pty for the bounded
Ubuntu follow-up. Real PTY input, alternate screen, resize and error/lifecycle
handling work together without a framework change. Keep the transport,
terminal model, input router and rendering separated when adding sessions.

The canvas renderer uses per-cell text geometry. Font fallback, shaping,
IME/composition, glyph caching and large-grid performance need further
qualification before treating it as a daily-use terminal. The viewport uses
a fixed 10 × 20 logical-pixel cell size and 16 px monospace text; it has no
font-size preference or DPI matrix yet. Cursor blinking, mouse reporting,
full key-protocol coverage, selection/copy/paste and scrollback navigation
are outside this spike; the next implementation slice adds the agreed
selection/clipboard/scrollback functions. No claim is made that every VT
sequence or terminal program works.

HOO-7 must retain output processing for inactive sessions and the close
confirmation rule. HOO-10 owns the first intermediate version's broader
Ubuntu 24.04 native Wayland/X11 and Ubuntu 26.04 native Wayland matrix.
HOO-9 tracks Windows prerequisites. None of those runtime/acceptance gates
is satisfied by this spike or by a `Human Review` status.
