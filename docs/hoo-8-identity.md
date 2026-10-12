# HOO-8: recognizable tabs and fixed direct shortcuts

Scope: [HOO-8](https://linear.app/hoori-term/issue/HOO-8/tab-namen-farben-und-feste-direkt-shortcuts),
approved HOO-5 decisions D-01 through D-08 / slice C, on merged HOO-7 commit
`170ba6f83719ace935e9cf45085cced5b2c85db7`. This adds the remaining tab identity
features of the first Ubuntu intermediate version. HOO-10 retains the full
native Wayland/X11 platform gate; Patrick's product acceptance is separate.

## Behavior and boundaries

Each tab owns its optional custom name, accent and optional shortcut. An edit
targets the existing monotonic `TabId`, not an index or shell OSC title. The
original shell/ID label is retained as the fallback for an empty custom name.
Custom names are trimmed, limited to 40 characters and reject control/bidi
format characters. The application window title includes custom name plus ID,
the dynamic shell title and embedded build identity. Confirmation targets also
include their IDs, so duplicate user-chosen names remain distinguishable.

`Tab bearbeiten` in the footer or Ctrl+Shift+E opens a draft. Name, one of eight
tab accents and a recorded shortcut are applied together only after validation;
cancel discards the whole draft. The terminal process is not restarted. Accent
colors apply only to the tab button, with a bright active outline. Terminal
cell colors continue to come from terminal state. Opening the inline editor
temporarily reduces the terminal viewport and resizes its PTY, just as other
viewport changes do. Existing sessions keep consuming output during editing.

Shortcut capture accepts a logical character/navigation key with Ctrl, Alt or
Super, or F1–F12 with optional modifiers. Modifier-only presses are ignored.
The current keyboard layout's unmodified logical key is normalized for case
and compared with exact modifiers; shortcuts are not physical scancodes.
The visible label shows the configured combination. Desktop-reserved
combinations may never reach the application, and Ctrl+Alt may overlap AltGr
on some layouts. Free Ctrl+Shift combinations or function keys are convenient
choices. This is not a global system hotkey service or a full layout matrix.

One shared built-in dispatch function also supplies conflict validation, so
reserved actions cannot be silently shadowed. A binding already owned by
another open tab is rejected with its target name. Invalid edits do not change
any committed field or existing assignment. A binding lives on its tab rather
than in a separate index: position changes do not retarget it, and removing
the tab removes the binding. New tabs start without bindings. Direct actions
only activate an existing open tab, including a retained exit/error tab; they
never create a process or activate a saved preset.

The event subscription observes widget capture status. Outside the editor,
only unhandled keys enter app/PTY dispatch. Inside it, name-field text is
handled by the text-input widget, Escape/Enter control the draft, and the
recorder receives keys while the name field is disabled. Unrecognized dialog
keys and delayed clipboard results are not sent to the terminal. A recognized
built-in or direct shortcut returns after its app action without also encoding
PTY bytes. Unassigned normal keys retain the terminal path. Window close
discards an open draft and uses the existing named session-close confirmation.

Names, colors and direct bindings last only for this application run. Later
template decisions D-15 onward do not expand this slice. No persistence,
tab-sorting UI, pinning, icons, compact mode, splits or global shortcuts are
introduced here.

## Reproduce validation

Run the README's format, Clippy, test and diff checks. In addition to the
existing terminal/PTY/lifecycle suite, tests cover atomic invalid/conflicting
edits, logical-key normalization and exact modifiers, names independent of
shell titles, target stability after model reordering/removal/reopening, and
custom names in close confirmations. A real raw-mode Python PTY test verifies
that direct shortcuts, editor keys and conflict attempts add no hidden bytes;
ordinary unassigned text still arrives exactly at the active session.

On the actual X11 desktop, with the HOO-6/7 harness prerequisites:

```sh
cargo build --release --locked
python3 scripts/gui-identity-smoke.py target/release/hoori-term /tmp/hoori-term-identity-fresh
```

Use a new evidence directory and keep focus available. The harness checks
window/process ownership before synthetic input. It creates three real
zsh/Bash tabs, assigns distinct names/colors/F8–F10, verifies unchanged shell
PIDs and retained shell variables, and changes shell titles. It tests duplicate
and built-in conflicts, editing/cancellation, a new F11 assignment, removal of
a preceding tab, absence of inherited bindings in a new tab, and named close
confirmation/cleanup. Raw-mode readers verify exact bytes of unassigned
function keys and text after dialog/direct actions. Every screenshot must be
visually inspected; successful PID/title/byte assertions are not rendering
evidence by themselves.

Re-run `scripts/gui-tabs-smoke.py` and `scripts/gui-smoke.py` for the existing
multi-session/clipboard and Vim/resize regressions. Final merged source commit,
clean build identity, absolute launcher and actual GUI results belong in the
versioned artifact's `VALIDATION.md` and Linear delivery. Development runs
marked `dirty` do not replace validation of the final launcher.

## Evidence limits

The available desktop is Ubuntu 24.04.5 x86_64 / X11 on `:1`, in the graphics
environment documented by HOO-6. Native Ubuntu 24.04 Wayland and Ubuntu 26.04
Wayland remain required HOO-10 evidence at the combined final commit. Windows,
PowerShell, ConPTY and WSL runtime checks await the actual VM. The simple cell
renderer's font/fallback/DPI/IME limits remain. No complete keyboard-layout,
terminal-protocol or full-screen application certification is implied.
