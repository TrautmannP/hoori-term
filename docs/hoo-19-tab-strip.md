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

## Implementation

`src/tab_strip.rs` owns drawing, layout, clipped hit testing, scrolling and focus
targets. Its native iced canvas publishes stable `TabId` targets to the existing
app command path. Names and details are clipped before the close lane, and the
viewport clips all tab geometry/targets before the fixed controls. Background
and clipped foreground geometry use separate ordered frame drafts.
`unicode-width` was already present transitively; it is now a direct pinned
dependency for display truncation. No browser/runtime service is added.

## Boundaries

This slice changes the strip and its actions. It preserves the existing editor,
dialogs, footer tools, clipboard, terminal renderer and session lifecycle;
their complete redesign is not included. No templates, pinning, new icons,
compact mode, sorting, theme settings or notification-scheduling correction is
introduced. HOO-15's measurements and any follow-up remain separate.
