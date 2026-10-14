# hoori-term

A local native Rust terminal prototype for Ubuntu and Windows 11 x64 using iced/wgpu,
alacritty_terminal and portable-pty. Multiple independent zsh/Bash tabs,
selection, clipboard and bounded scrollback are available. Tabs can be named,
colored and assigned their own direct shortcuts while their shells keep running.
Explicit local templates preserve selected configurations for later, deliberate
opening. They never resume processes or automatically open extra startup tabs.
Pinned tabs form a front group and resist normal individual close actions.
An optional built-in symbol adds another visual identity cue and is saved with
explicit templates; no icon font or downloaded resource is required.
Each tab can independently use a compact symbol view; pinning stays a separate
property. Saved templates retain the selected presentation.
Tabs can be reordered within their pin/ordinary group. Linked tabs carry the
saved template order; ordinary tabs remain temporary.
Dynamic terminal titles and explicitly reported program status complement the
own tab identity. Their runtime values are never saved in templates.

## Run

On Windows 11 x64, the app discovers existing WSL distributions in the
background and starts Bash in the configured WSL-2 default distribution.
**+** offers the registered distributions and Windows PowerShell 5.1.
WSL 1 remains visible but unavailable. Missing WSL or an unsupported default
opens a notice and explicit selection; PowerShell never starts as a silent
fallback. No distribution is installed, converted or reconfigured. A default
such as `docker-desktop` that has no Bash leaves its error and exit status in
its own tab; use **+** to select an Ubuntu distribution instead.

Ctrl+Shift+T repeats the launch target; Ctrl+Shift+B opens target selection on
Windows. Saved templates remain an Ubuntu feature pending their Windows
integration. The shared tab features do not imply a completed Windows
feature/platform qualification. See [Windows prototype validation](docs/hoo-11-windows.md).
Windows prefers Vulkan/D3D12; an explicit `WGPU_BACKEND` override is preserved.

```powershell
cargo run --locked
cargo run --locked -- --shell C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -- -NoLogo
```

Native Windows development needs the pinned Rust MSVC toolchain and Visual C++
build tools. The review executable itself does not require Rust.

Ubuntu prerequisites: Rust 1.98.1 (pinned by `rust-toolchain.toml`), a C linker,
Ubuntu desktop with X11 or Wayland libraries and a working Vulkan/OpenGL driver,
monospace fonts, and zsh (`/usr/bin/zsh`). The verified environment is Ubuntu
24.04.5 x86_64 with X11, the current Ubuntu qualification baseline. Additional
native Wayland qualification is deferred; Windows has its separate
[WSL/PowerShell matrix](docs/hoo-12-windows-matrix.md).

```sh
cargo run --locked
cargo run --locked -- --shell /bin/bash -- --noprofile --norc -i
```

Type directly into the active terminal. Standard navigation, Ctrl combinations
and unassigned F1–F12 are sent only to that session. Click **+** to choose zsh
or Bash, then select the shell to start one session. Opening or canceling the
choice starts none. Click a tab to activate it. Inactive tabs keep processing output.

| Action | Keyboard / mouse |
| --- | --- |
| New session using the launch shell/arguments | Ctrl+Shift+T |
| New Bash session | Ctrl+Shift+B |
| Focus the tab strip / window controls | Ctrl+Shift+L |
| Inspect full terminal title / reported status | Right-click a tab; or Ctrl+Shift+L, focus a tab, I; **Titel/Status** for the active tab |
| Reorder a tab within its pin/ordinary group | Drag its body; or Ctrl+Shift+L, focus a tab, Alt+Left/Right |
| Move / maximize or restore window | Drag / double-click the dotted build-hash area |
| Resize window | Drag a window edge or corner |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab |
| Close active unpinned tab | Ctrl+Shift+W or its × button |
| Pin / unpin the active tab | **Tab anpinnen** / **Tab entpinnen** |
| Explicitly close a pinned tab | **Angepinnten Tab schließen** |
| Switch active tab normal / compact | **Kompaktansicht** / **Normalansicht** |
| Edit active tab name / accent / direct shortcut / symbol | Ctrl+Shift+E or **Tab bearbeiten** |
| Save active tab as a template / edit its linked template | Ctrl+Shift+S or **Als Vorlage speichern** / **Vorlage bearbeiten** |
| Choose a symbol while editing | F6 or **Auswählen**; arrows/Tab, Enter; Esc cancels the chooser |
| Switch the editor's presentation draft | F7 or **Kompakt anzeigen** / **Normal anzeigen** |
| List saved templates | Ctrl+Shift+P or **Vorlagen** |
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

Names remain visible when a shell changes its title. Connected tabs use colored
accents and a neutral active outline/surface; terminal colors stay controlled by
the terminal program. Long names are shortened in the strip; hover, keyboard
focus details or the editor show the full name. Compact tabs show the chosen
symbol, accent, process state and PIN separately. With no chosen symbol, a neutral
Terminal drawing is used only for display; no symbol setting is added. Compact
presentation never follows overflow or pinning automatically.
Normal tabs add a title preview and separate `CLI:`/`PTY` labels. Compact tabs
retain their width and separate status lines. Only explicit OSC 7501 reports
produce CLI states; a title alone does not. Full details scroll with the mouse,
arrows, PageUp/PageDown or Home/End; Escape/Enter returns to terminal input.
See [the bounded protocol and lifecycle rules](docs/hoo-29-title-status.md).
Scroll arrows and the mouse wheel move overflowing tabs;
the Plus target stays available. Direct navigation reveals its target.
With strip focus, arrows/Home/End move focus, Tab/Shift+Tab traverse tab/close/Plus
and window-control targets. Full name, unique tab ID and shortcut appear in the
focus details. C switches the focused tab's normal/compact view and E edits it;
Enter/Space selects, Delete requests the focused tab's close, and Escape
returns to terminal input. Focus movement alone preserves the active terminal.
The system title bar is replaced by minimize, maximize/restore and close buttons
in the same row. A reserved dotted area moves the window and keeps the build hash
visible. The minimum window size is 640 × 360 logical pixels. Return from minimize
through the desktop task switcher, taskbar or overview; sessions keep running.
Direct shortcuts target the same open tab ID after
renaming, color changes or removal of preceding tabs. Ordinary closed tabs lose
their binding. A saved template keeps its opening/activation shortcut after its
tab is closed. Shortcuts are local to the focused window.
Shortcut keys use the current layout's unmodified logical key and exact
modifiers. Desktop-reserved chords may not be capturable; Ctrl+Alt may overlap
AltGr on some layouts. Prefer free Ctrl+Shift combinations or function keys.

**Als Vorlage speichern** links the current tab to a new saved configuration
without starting another process. Choose a name, color, optional shortcut,
zsh/Bash and an absolute start directory. **Vorlagen** lists saved entries;
opening activates their existing tab, including an exited/error tab, or starts
one new session. Name/color/shortcut/symbol/presentation edits to a linked tab save automatically.
Ordinary tabs remain temporary. The list supports arrows/Home/End, Enter to
open, E to edit, Delete to remove, and Escape to return. Removing a template
clears its link and inherited shortcut, while its open tab keeps running and
retains its current pin state and presentation. Saving a pinned tab captures that state; linked
pin changes save automatically. Pinning ordinary tabs alone saves no template.

Templates load from `$XDG_CONFIG_HOME/hoori-term/templates-v1.json`, or
`$HOME/.config/hoori-term/templates-v1.json`. Startup still opens only the
ordinary default session. Storage runs in the background; load/write errors
are visible and preserve existing files. Pending changes are not described as
saved. See [local templates](docs/hoo-13-templates.md) for limits, recovery,
start-directory semantics and the targeted validation procedure.

Running sessions require confirmation naming the affected tabs; Escape or
**Abbrechen** keeps them alive, Enter or **Beenden** confirms. Window close
names all running targets. A terminated shell leaves its output and exit
status visible until its tab is closed, without another process confirmation.
Pinned tabs keep normal close protection after exit. Unpin them or explicitly
choose **Angepinnten Tab schließen**. The **PIN** marker and separated front
group distinguish pinning from the active tab and its accent color. Window close
still includes all running pinned targets in its named confirmation.
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
covers editing, conflicts and direct shortcut routing. The current
[Ubuntu/X11 qualification](docs/hoo-10-ubuntu-x11.md) covers both Bash and zsh
on the same source commit. Native Wayland is deferred; Windows runtime evidence
is tracked separately.
The [refresh comparison](docs/hoo-15-refresh-comparison.md) records the bounded
HOO-15 `top`/GNOME investigation, raw evidence, timing limits and follow-up proposal.
The [connected tab-strip validation](docs/hoo-19-tab-strip.md) describes the
HOO-19 layout, input routing and real comparison/regression procedure.
The [template validation](docs/hoo-13-templates.md) covers HOO-13 configuration,
stable links, asynchronous storage and the bundled X11 restart/error exercise.
The [window-header validation](docs/hoo-32-window-header.md) covers native window
operations, focus, reserved geometry and the targeted X11 evidence procedure.
The [pinning validation](docs/hoo-17-pinning.md) covers stable groups, protected
close, the stored pin field and the bundled X11 restart/write-error exercise.
The [symbol validation](docs/hoo-22-symbols.md) covers the original native catalog,
keyboard/mouse selection, optional stable IDs and targeted GUI/storage evidence.
The [compact-view validation](docs/hoo-24-compact.md) covers independent pin/view
combinations, mixed-width geometry, full details and stored presentation.
The [tab-order validation](docs/hoo-26-order.md) defines deliberate drag/keyboard
reordering, stable saved neighbors, reopening, legacy data and targeted evidence.

## Versioned review builds

After merging, check out the final `main` commit with a clean worktree, then:

```sh
./scripts/build-review.sh
```

The script builds with the lockfile, validates the embedded identity and
publishes a binary, `launch`, `.desktop` launcher, README/documentation and `BUILD-METADATA.txt` to
`/home/patrick/Projekte/hoori-term-builds/<full-commit>/`. Existing builds are
preserved. Run the absolute `launch` path printed by the script from any
directory. The `.desktop` file can be opened with `gio launch` or installed
in the local desktop application menu. These are local Ubuntu review
artifacts, not a public installer or a Windows release.

For a clean merged Windows x64 source checkout, `./scripts/build-review.ps1`
creates `C:\Repos\hoori-term-builds\<full-commit>\` with the executable,
`launch.ps1`, a `hoori-term.lnk` shortcut and full embedded build metadata.
Existing versioned directories are preserved. The Windows preview requires
Windows 11 x64 and, for WSL sessions, an existing WSL-2 distribution with Bash.
The separate Ubuntu build remains available for cross-platform review.
