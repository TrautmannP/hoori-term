# HOO-24: independent compact tab presentation on Ubuntu

This implements the bounded C-A slice of HOO-5 D-26. The complete native
Ubuntu/Windows matrix remains HOO-25; the early Ubuntu/X11 review does not
replace that gate or Patrick's product acceptance.

## Controls and independent properties

Use **Kompaktansicht** / **Normalansicht** on the active tab. For keyboard
inspection, press Ctrl+Shift+L and select a tab with arrows/Home/End or traverse
tab/close targets with Tab/Shift+Tab. The visible details include the full name,
unique run-local tab ID, assigned shortcut (or its absence), process state,
pin protection, presentation and chosen symbol. C switches that focused tab's
view without activating it; E edits it. Enter/Space activates, Delete requests
normal close, and Escape returns to terminal input. Mouse hover gives the same
identity details, including for inactive compact tabs with identical symbols.

The tab/save/template editor has an **Ansicht** row. F7 or its button toggles
only the draft; Apply commits and Cancel discards it. F7 remains recordable
while the shortcut recorder is active. Symbol selection retains its own F6
scope and cancellation. These app actions do not also enter the terminal.

Normal tabs retain the connected 224-pixel presentation. Compact tabs use 104
logical pixels with the user's native vector drawing, a separate accent dot,
close target, active surface/outline, keyboard focus and textual process/PIN
cues. Without a chosen symbol, the original neutral Terminal drawing is used
only for presentation. `identity.symbol` remains absent/null. A chosen warning
or pin-like drawing never replaces the actual state or protection cue.

Normal/compact and pin/unpin are independent. All four combinations work;
changing presentation does not move a tab to another group or change its
identity, binding, symbol, process or terminal. Neither new tabs nor overflow
automatically chooses compact mode. Pin changes retain presentation.

The layout computes cumulative widths for the actual mixed tab sequence plus
the existing pin separator. Drawing, reveal, hit testing and close geometry
share those widths. Plus/shell choice, free window-drag space, window controls
and the persistent build hash retain reserved space at the 640-pixel minimum.
Direct navigation reveals its exact existing target, including after widths
change. Compact close buttons and full details stay separate from activation.

## Storage and lifetime

The bounded version-1 template JSON adds `identity.compact: bool`, defaulting
to false when absent. Wrong types, including null, reject the load visibly
and preserve the original file. Loading a legacy catalog does not rewrite it.
New writes contain the field; older builds with strict unknown-field checks
reject that newer file rather than silently losing presentation. Keep isolated
configuration when comparing old and new review versions.

An explicit Save draft starts with the current presentation and links the same
tab without spawning another PTY. Changes to a linked tab update the existing
template ID using the bounded asynchronous atomic writer. Failed writes remain
visibly unsaved and retain old disk bytes; retry saves the pending model.
Name/shortcut validation and catalog limits apply before either model changes.

Startup still opens only the default ordinary normal tab. A saved compact
presentation is applied only on deliberate template opening, which activates
the existing associated live/exited/error tab when one exists. Closing a tab
keeps its template. Removing a template leaves the current presentation, symbol,
pin and PTY intact, while clearing the link/inherited binding. Ordinary compact
tabs remain temporary. No processes or terminal contents are persisted.
