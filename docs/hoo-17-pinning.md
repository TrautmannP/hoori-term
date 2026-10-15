# HOO-17: explicit pinning on Ubuntu

This bounded P-A delivery implements HOO-5 D-20–D-22 on Ubuntu. Pinning is
separate from tab identity, a saved template, active state and accent color.
Native Wayland, Ubuntu 26.04 and Windows qualification remain in HOO-18 and
the existing platform issues. Technical delivery does not imply product acceptance.

## Behavior and controls

The lower toolbar has **Tab anpinnen** / **Tab entpinnen** for the active tab.
Pinning moves that existing tab to the end of the pinned group, before ordinary
tabs. Unpinning moves it to the end of the ordinary group. Other tabs keep their
relative order within their group. Repeating the same property is a no-op.

Pinned tabs have a neutral tinted surface and a textual **PIN** marker alongside
the shortcut/process state. A gap and separator divide pinned and ordinary
areas. The active connected surface/outline and colored accent remain separate.
Scrolling and keyboard navigation use the same geometry, including this gap;
activating a direct shortcut reveals the same tab at the new position. No compact
mode, icon collection, drag-and-drop ordering or automatic template creation is
included.

The normal ×, Ctrl+Shift+W and Delete with strip focus preserve a pinned tab,
including one whose process already exited. A named message explains the
protection. These handled app actions do not also enter the PTY. Unpinning
restores the existing normal close behavior.

**Angepinnten Tab schließen** explicitly overrides individual protection. A
running target still requires its named confirmation. Escape / **Abbrechen**
retains its tab, pin and process. Confirmation removes only the identified tab;
an already exited/failed target needs no process confirmation. Window close
continues to name all running targets, including pinned ones. Cancel preserves
all of them; confirmed window close requires no prior unpinning.

Pinning never spawns, restarts or ends a process. Tab ID, template ID, name,
accent, binding, terminal content and inactive-output processing are preserved.
The extra action row uses viewport space; normal terminal resizing applies.

## Stored field and recovery

Each template has a bounded boolean `pinned` field. The existing version-1 JSON
format and `templates-v1.json` path remain; absence of the field is read as
`false`. Wrong types and unknown fields are rejected. Loading an old valid file
alone does not rewrite it. New writes include the field; older review binaries
that do not know it reject the file and preserve it rather than interpreting its
pin semantics.

Saving an already pinned ordinary tab captures its actual state without another
PTY. Pin changes on a linked tab update the same template ID and use the existing
background, coalesced, bounded, atomic file writer. Failed writes keep the old
file and display the pending unsaved state; **Erneut speichern** retries. Corrupt
or unsupported loads stay visible and cannot be silently overwritten. Existing
64-template and 256-KiB limits, conflict checks and temporary-file rules apply.

Startup still opens only its ordinary default session. A template's stored pin
is applied when the user deliberately opens it. A second list/shortcut request
activates its existing tab, including an exited/error tab, without another PTY.
Explicit tab close preserves the template and its pin for the next opening.
Removing a template preserves the open tab's pin and process while clearing its
link and inherited binding. Such a tab becomes temporary; pinning alone does not
recreate the removed configuration.

The editor displays the current pin state and preserves it during name/color/
shortcut or future-start edits. Use the corresponding open tab's explicit pin
button to change it.
