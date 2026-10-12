# HOO-7: independent terminal sessions and user-controlled clipboard

Scope: [HOO-7](https://linear.app/hoori-term/issue/HOO-7/mehrere-nutzbare-ubuntu-terminal-sitzungen-und-tab-lebenszyklus),
approved HOO-5 decisions D-01 through D-08, on the merged HOO-6 integration.
This is the multi-session part of the Ubuntu intermediate version. Custom
names/colors/direct shortcuts belong to HOO-8; HOO-10 qualifies the complete
native Wayland/X11 platform matrix after that slice.

## Implementation and bounds

`tabs` owns stable monotonic IDs, independent terminal models, PTY transports
and per-session errors. Tab labels identify the shell and ID even before
custom names exist. Shell OSC titles affect the window title, retaining both
the tab identity and embedded build identity. Removing or adding another tab
does not change an existing ID. IDs and processes are not persisted.

Every session has an output subscription, including inactive sessions.
Coalesced wakeups allow up to one refresh per 16 ms per session. A refresh
parses at most 32 KiB, checking a soft 2 ms budget between chunks; one chunk
may exceed that budget. Each output channel holds at most 512 KiB plus the
reader's current 8 KiB chunk. Backpressure reaches the child instead of
discarding terminal bytes. Only active output invalidates the shared canvas
cache. Inactive terminals retain their last PTY size; activation sends the
current viewport size to that exact tab's PTY. Delayed resize/selection
messages for an inactive or closed ID are ignored.

The application permits 16 open tabs. Each terminal retains 2,000 history
lines and up to 500 × 200 viewport cells. Wheel and keyboard scrolling use
the core display offset. Selection anchors use grid coordinates accounting
for that offset, including wide characters and existing core selection
rotation when new output arrives. Selection never implicitly writes the
standard or primary clipboard. Mouse reporting to terminal programs and
automatic selection autoscroll are not added in this slice.

Clipboard reads/writes are explicit button or keyboard actions only. OSC 52
remains disabled and terminal output cannot invoke app commands. Clipboard
read results carry the original tab ID; switching tabs, closing the target
or opening a close dialog prevents a delayed result from being redirected.
Pastes respect the terminal's current bracketed-paste mode. ESC and other
control characters except tab/newline are removed, CRLF/CR normalized, and
plain paste newlines encoded as CR. The maximum accepted source text is
65,524 UTF-8 bytes, leaving room for bracket markers in a 64 KiB input entry.
Oversized pastes are rejected before sending any bytes. The input transport
reserves an entire entry atomically against a 256 KiB queued/in-flight byte
budget and a 64-entry channel, so queue saturation cannot accept only the
start of a bracketed paste. An I/O failure after transmission starts remains
an I/O failure and is shown; atomic queue acceptance does not guarantee that
a terminated child consumes every accepted byte.

Close requests inspect actual child states. A running target opens a named
confirmation; cancel leaves all sessions intact. Window confirmation names
all running tabs. Transport updates continue while commands are frozen in
the dialog. Confirm removes only the selected targets, then terminates and
reaps their transports on a blocking executor worker. Other tabs remain
usable. Exited/error tabs retain their state until closed without a prompt.
Closing the last tab also closes the window after pending shutdowns finish.

## Reproduce validation

Run the README's format, Clippy, test and diff checks. The tests cover three
real zsh/Bash PTYs, inactive output, targeted resize, retained exit status,
failed starts, surviving sessions after targeted shutdown, monotonic identity,
tab-count limits, dialog cancellation and frozen commands, delayed paste and
resize routing, core history eviction/selection, bracketed/plain paste,
64 KiB transport entries and saturated queues.

For the actual X11 window exercise, install the HOO-6 harness prerequisites
and `xclip`, build, then use a fresh evidence directory:

```sh
cargo build --release --locked
python3 scripts/gui-tabs-smoke.py target/release/hoori-term /tmp/hoori-term-tabs-evidence
```

The harness verifies process/window ownership and focus before synthetic
input. It starts three genuine zsh/Bash tabs through keyboard and UI,
switches between distinguishable output, drives 250,000 lines in an inactive
tab with repeated resize/switching, navigates bounded history, selects and
copies `Grüße 界`, and verifies exact bytes of bracketed/plain clipboard paste
using a synthetic raw-mode reader. It writes a synthetic clipboard fixture;
no prior clipboard contents are read or saved. OSC 52 cannot replace that
fixture. Cancel/confirm of one tab and the window, disappearance of targeted
shell PIDs, retained Bash exit 7, and new monotonic tab IDs are checked.
Every captured screenshot must be inspected; process/title assertions alone
do not establish correct rendering. Very fast synthetic mouse drags can be
coalesced; the harness separates press/move/release events and waits for the
visible confirmation title before confirming.

`scripts/gui-smoke.py` retains the original UTF-8/color/cursor/Vim/resize,
output-load, Bash-exit and unavailable-shell regression exercise. The close
dialog now also identifies its state in the window title while retaining
the commit identity.

## Evidence boundary

The available actual desktop is Ubuntu 24.04.5 x86_64 / X11 on `:1`, using the
HOO-6 graphics environment. Exact final merged commit, launcher, screenshot
review, checks and short process-only CPU/RSS observations are recorded in
the Linear delivery and versioned review artifact. Short samples exclude
child processes and GPU memory and are not performance guarantees.

Native Ubuntu 24.04 Wayland and Ubuntu 26.04 Wayland are still separate,
required HOO-10 evidence. Windows/ConPTY/PowerShell/WSL runtime evidence is
also pending. The simple cell renderer's font/DPI/IME/glyph-cache limitations
from HOO-6 remain; these tests do not certify every terminal protocol or
full-screen program. Technical delivery and Patrick's product acceptance
are separate.
