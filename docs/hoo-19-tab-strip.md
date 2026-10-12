# HOO-19: connected native tab strip

Scope: [HOO-19](https://linear.app/hoori-term/issue/HOO-19/ubuntu-verbundene-tableiste-gestalten),
HOO-5 D-23/D-24 and the eight HOO-16 criteria, on the integrated HOO-8 identity
and session model. This is the bounded Ubuntu implementation. HOO-20 retains
the full Ubuntu/Windows integration and platform matrix; Patrick judges the
appearance and product acceptance on the identified review build.

## Surface and inspection

The strip has a shared dark surface, rounded upper tab corners and a neutral
active surface/border opening into the terminal below. User accents are small
identity markers, not terminal color changes. Hover uses a separate surface;
keyboard focus uses an inner cyan outline and can target an inactive tab
without activating it. A tab's close target is inside its surface, separated
from its name and shortcut lane. Exit and error text supplement color.

Names and shortcut labels have bounded, independently clipped lanes. Long
names use a Unicode-width-aware ellipsis. Hover shows the full name, stable
session ID and full shortcut; the existing editor also retains the full name.
Duplicate names remain valid, while close confirmations identify their session
IDs. Existing fallback shell/ID labels and retained exit/start-error tabs remain.
Blocking menus, the editor and close confirmations remove the tooltip overlay,
including an already visible one. Navigation clears the old hover target so a
moved or removed tab cannot leave stale inspection content behind.

Overflow stays in one horizontal strip. Arrow targets and the mouse wheel move
the tab viewport; the Plus target is outside it and remains reachable. Activation
by a fixed shortcut, cycling or a new session reveals the active target. Moving
keyboard focus also reveals its candidate. Scrolling alone leaves the active
terminal and every process unchanged.

## Deliberate starts and keyboard routing

Plus opens **Neue Sitzung**, offering the existing zsh and Bash start targets.
Opening, Escape or **Abbrechen** creates no session. Choosing a shell creates
one session; stale/repeated choice messages cannot create another. The original
Ctrl+Shift+T launch-shell/arguments action and Ctrl+Shift+B remain available.
The inline choice temporarily reduces the active viewport, as the existing
editor does. PTY output/status processing continues for all tabs.

Ctrl+Shift+L focuses the strip. Left/Right or Home/End moves the candidate among
tabs; Tab/Shift+Tab traverses tab, close and Plus targets. Enter/Space selects;
Delete requests the focused tab's close. Escape returns to terminal input.
Focus movement alone does not activate another tab. Activation returns input
to the terminal; clicking the terminal also leaves strip focus. While the shell
choice is open, arrows/Tab choose zsh/Bash/cancel and Enter/Space confirms it.
Other menu input, app commands and delayed paste results are blocked.

Recognized actions return before PTY encoding. Ordinary unbound input outside
the menu/editor/confirmation/focused strip follows the existing terminal path.
The shared built-in table also reserves Ctrl+Shift+L during shortcut editing.
Existing direct assignments, model IDs, output queues and parsing budgets are
unchanged. Every running close still requires the named D-06 confirmation;
cancel preserves processes, and an exited tab closes without process confirmation.

## Implementation and tests

`src/tab_strip.rs` owns drawing, layout, clipped hit testing, scrolling and focus
targets. Its native iced canvas publishes stable `TabId` targets to the existing
app command path. Names and details are clipped before the close lane, and the
viewport clips all tab geometry/targets before the fixed controls. Background
and clipped foreground geometry use separate ordered frame drafts.
`unicode-width` was already present transitively; it is now a direct pinned
dependency for display truncation. No browser/runtime service is added.

Model tests cover overflow/reveal/hit targets at 640/1280 logical pixels and
identity after reorder. App tests cover no-start/cancel, exactly one deliberate
shell choice, stale actions and modal blocking. Existing real raw-mode PTY
readers additionally verify strip focus/menu cancellation and direct actions add
no hidden bytes while ordinary terminal input still arrives exactly. Existing
identity, bounded transport, inactive output and targeted shutdown tests remain.

Run the README's format, Clippy, workspace tests and whitespace checks. Real X11
GUI evidence is generated with fresh owned windows and synthetic fixtures:

```sh
cargo build --release --locked
python3 scripts/gui-tab-strip-smoke.py /home/patrick/Projekte/hoori-term-builds/8fc02d691b0c819ceecab73681e56f275aeb95b0/launch /tmp/hoo19-before --baseline
python3 scripts/gui-tab-strip-smoke.py target/release/hoori-term /tmp/hoo19-after
python3 scripts/gui-identity-smoke.py target/release/hoori-term /tmp/hoo19-identity
python3 scripts/gui-tabs-smoke.py target/release/hoori-term /tmp/hoo19-tabs
python3 scripts/gui-smoke.py target/release/hoori-term /tmp/hoo19-terminal
```

The comparison driver uses the same three normal names/accents/shortcuts and
twelve 40-character names (including a duplicate pair and Unicode), real shell
state and unchanged display environment. It records actual 640/1280 × 700 X11
window geometry and desktop text scale/DPI. Long names are pasted from an exact
synthetic clipboard fixture; the harness replaces the clipboard and never reads
prior clipboard contents. Other harness prerequisites are listed in HOO-6/7.

The new flow checks hover/focus, menu open/cancel/one Bash start, exact raw input,
all twelve shortcut/PID/state targets, first/last visibility after navigation,
full-name inspection, inactive duplicate-name close/cancel, the surviving equal
name, exit markers and close without confirmation, window cancel/cleanup and an
unavailable-shell error. The baseline mode records the older appearance and
same fixture states, not a pass for the new behavior. Keep every failed attempt
separate; inspect all images rather than accepting JSON flags as visual proof.

After merge, repeat the GUI flows through the absolute versioned launcher from
clean final source. The artifact's `VALIDATION.md`, reports, local-check log and
checksums record the actual results, before/after views, final full hash and
desktop launcher. A dirty development run does not replace this final evidence.

## Boundaries

This slice changes the strip and its actions. It preserves the existing editor,
dialogs, footer tools, clipboard, terminal renderer and session lifecycle;
their complete redesign is not included. No templates, pinning, new icons,
compact mode, sorting, theme settings or notification-scheduling correction is
introduced. HOO-15's measurements and any follow-up remain separate.

Available native runtime evidence is Ubuntu 24.04.5 x86_64/X11 on display :1,
GNOME Shell 46.0, NVIDIA GTX 1070/580.178.04, scale 1, zsh 5.9 and Bash 5.2.21.
The unchanged renderer's actual selected wgpu API/adapter is not logged.
Native Ubuntu 24.04/26.04 Wayland and Windows/WSL/PowerShell still need their
agreed environments and same-final-commit matrix. Font shaping/fallback,
HiDPI/IME and complete layout/protocol certification remain existing limits.
No GitHub pipeline or full-matrix/product acceptance is claimed by this delivery.
