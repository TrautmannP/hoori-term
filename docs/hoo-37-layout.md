# HOO-37: native Ubuntu layout

This is the early Ubuntu UI-A slice of the confirmed HOO-35 / HOO-5
D-38–D-41 contract. The current qualification target is native Ubuntu 24.04/X11
with Bash and zsh. HOO-38 owns the later combined Ubuntu/Windows qualification;
this delivery does not satisfy the separate feature or Daily Driver gates.

## Working and editing

The left navigation groups deliberate shell starts and saved templates. A
name opens or activates the same template; **⋯** selects it for **Bearbeiten**
or **Entfernen**. **Liste** / Ctrl+Shift+P enters keyboard selection with the
existing arrows, Home/End, Enter, E, Delete and Escape. **+ Neue Sitzung** and
the tab-strip **+** open the same explicit shell chooser. The current tab can
be deliberately saved from the navigation or Ctrl+Shift+S.

The terminal retains a small identity/action header and a copy/paste/live
footer. **Bearbeiten** / Ctrl+Shift+E opens the right draft editor. Name,
accent, symbol, native shortcut recording, compact presentation and existing
start-directory rules keep their meanings. **Übernehmen** applies to the
original stable tab/template ID; **Abbrechen** discards the draft. Escape
first leaves symbol selection or shortcut recording, then cancels editing.
Active and inactive PTYs continue consuming output while all editor input
stays outside the PTY. Template editing returns to template selection;
ordinary tab editing returns to terminal input.

**Details / Aktionen**, right-click on a tab, or strip-focus I opens full
identity, title/status and build information. Explicit actions edit/save,
pin/unpin, switch compact presentation, move within the same pin group and
close the selected stable tab. Boundary moves and unavailable save actions
are disabled. A delayed panel click cannot act again after that panel closes.
Normal close still protects pins; the explicit pinned-close action and window
close retain the named, cancellable running-session confirmation.

## Responsive composition and recovery

The single opaque blue-grey palette is shared by widgets and canvas chrome.
Selection has an outline/marker as well as color; state labels distinguish
user identity, a program's reported status and PTY process state.

At widths below 880 logical pixels, **Navigation** / Ctrl+Shift+P opens the
left navigation over the terminal. The right panel is 320 pixels wide and
also overlays the terminal below 1200 pixels; wider windows allocate a real
column. These are view breakpoints, not a new native minimum. The existing
minimum remains 640 × 360. Form fields, long errors and details scroll inside
their own areas; apply/cancel/back and close-confirmation actions stay outside
those scroll regions. The top window controls/hash and native resize border
remain independent of modal content.

The global storage-error banner retains Retry even with a panel open. Full
errors remain in navigation/details/editor as appropriate. Failed persistence
is never described as saved, existing files are preserved, and the shutdown
view retains retry and explicit unsaved exit. The catalog/file/launch/session
models and their persistence rules are unchanged by this issue.
