# hoori-term

A local native Rust terminal prototype for Ubuntu using iced/wgpu,
alacritty_terminal and portable-pty. Multiple independent zsh/Bash tabs,
selection, clipboard and bounded scrollback are available. Tabs can be named,
colored and assigned their own direct shortcuts while their shells keep running.

## Run

Prerequisites: Rust 1.98.1 (pinned by `rust-toolchain.toml`), a C linker,
Ubuntu desktop with X11 or Wayland libraries and a working Vulkan/OpenGL driver,
monospace fonts, and zsh (`/usr/bin/zsh`). The verified environment is Ubuntu
24.04.5 x86_64 with X11; Wayland and Windows runtime checks remain pending.

```sh
cargo run --locked
cargo run --locked -- --shell /bin/bash -- --noprofile --norc -i
```

Type directly into the active terminal. Standard navigation, Ctrl combinations
and F1–F12 are sent only to that session. Click **+ zsh** / **+ Bash** to open
sessions and a tab to activate it. Inactive tabs keep processing output.

| Action | Keyboard / mouse |
| --- | --- |
| New session using the launch shell/arguments | Ctrl+Shift+T |
| New Bash session | Ctrl+Shift+B |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab |
| Close active tab | Ctrl+Shift+W or its × button |
| Edit active tab name / accent / direct shortcut | Ctrl+Shift+E or **Tab bearbeiten** |
| Select / copy / paste | Drag left mouse / Ctrl+Shift+C / Ctrl+Shift+V |
| Scroll history | Mouse wheel, Shift+PageUp / Shift+PageDown |
| Oldest history / live output | Ctrl+Shift+Home / Ctrl+Shift+End or **↓ Live** |

The reserved shortcuts are app actions and are not also sent to the shell.
OS-intercepted combinations may not reach the application. Copy/paste buttons
also appear in the footer. Clipboard access requires these explicit actions;
selecting text alone does not change either clipboard. OSC 52 is disabled.
Paste respects bracketed-paste mode, removes escape/control bytes except
tab/newlines, and normalizes newlines to carriage returns outside that mode.
Pastes over 65,524 UTF-8 bytes are rejected visibly before sending any data;
queue saturation also rejects a whole entry rather than a partial bracket.

**Tab bearbeiten** opens a draft with a name (up to 40 characters), eight
accent choices and a shortcut recorder. Click **Aufnehmen**, press a combination,
then **Übernehmen** to apply all changes together. Use Ctrl, Alt or Super with
a character/navigation key, or F1–F12 with optional modifiers. Duplicate tab
bindings and reserved app actions are rejected visibly. **Entfernen** clears a
binding; an empty name restores the original shell/ID label. Escape stops a
recording, then cancels the draft; Enter applies a draft outside recording.
Name-field input stays in that field, and other app/terminal actions are paused
while editing. Every session continues processing output.

Names remain visible when a shell changes its title. Accents affect tab buttons,
with a bright outline for the active tab; terminal colors stay controlled by
the terminal program. Direct shortcuts target the same open tab ID after
renaming, color changes or removal of preceding tabs. A closed tab's binding
is removed and never inherited by a new tab. Shortcuts activate existing tabs,
including their retained exit/error states. They are local to the focused
window and current run. Names/colors/bindings are not saved across app restarts.
Shortcut keys use the current layout's unmodified logical key and exact
modifiers. Desktop-reserved chords may not be capturable; Ctrl+Alt may overlap
AltGr on some layouts. Prefer free Ctrl+Shift combinations or function keys.

Running sessions require confirmation naming the affected tabs; Escape or
**Abbrechen** keeps them alive, Enter or **Beenden** confirms. Window close
names all running targets. A terminated shell leaves its output and exit
status visible until its tab is closed, without another confirmation.
A startup error affects its own tab. No terminal contents or input are logged
by the application; shells may apply their own history/startup configuration.

Limits: 16 open tabs, 2,000 history lines per tab, up to 500 × 200 viewport
cells, 512 KiB output and 256 KiB queued/in-flight input per session. Input
entries are at most 64 KiB. Each tab parses at most 32 KiB per refresh with
a soft 2 ms budget between chunks; backpressure keeps output lossless.
Inactive tabs retain their last PTY size until activation resizes them to
the current viewport. Stable internal tab IDs are independent of their
position and dynamic shell title. These identities are local to this run.

`--shell PATH` selects an executable, and arguments after `--` are passed
directly to it. With no arguments, zsh starts interactively. This is an
explicit launch choice, never an action triggered by terminal output.
`--version` prints the full embedded commit, dirty state and build target.
The window title retains the short build identity when the shell changes
its title. Development builds show `dirty` when built from modified source.

## Local checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
git diff --check
```

There is deliberately no GitHub CI pipeline. See [the spike evidence](docs/hoo-6-spike.md)
for the GUI validation procedure, actual observations and integration limits.
The [multi-session validation](docs/hoo-7-sessions.md) covers the follow-up
tests and X11 GUI/clipboard exercise. The [tab-identity validation](docs/hoo-8-identity.md)
covers editing, conflicts and direct shortcut routing. Native Wayland and Windows runtime
evidence remain separate gates.

## Versioned review builds

After merging, check out the final `main` commit with a clean worktree, then:

```sh
./scripts/build-review.sh
```

The script builds with the lockfile, validates the embedded identity and
publishes a binary, `launch`, `.desktop` launcher and `BUILD-METADATA.txt` to
`/home/patrick/Projekte/hoori-term-builds/<full-commit>/`. Existing builds are
preserved. Run the absolute `launch` path printed by the script from any
directory. The `.desktop` file can be opened with `gio launch` or installed
in the local desktop application menu. These are local Ubuntu review
artifacts, not a public installer or a Windows release.
