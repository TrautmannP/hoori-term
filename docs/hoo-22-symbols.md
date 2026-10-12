# HOO-22: built-in user symbols on Ubuntu

This is the bounded I-A implementation of HOO-5 D-25. The complete native
Ubuntu/Windows platform qualification remains HOO-23, with its actual code and
environment prerequisites. A technical Ubuntu-X11 delivery is not that gate or
Patrick's product acceptance.

## Selection and rendering

Open **Tab bearbeiten** with Ctrl+Shift+E, then **Auswählen (F6)**. The local
catalog contains Terminal, Code, Ordner, Server, Datenbank, Branch, Stern and
Schild, plus **Kein Symbol**. Click a named tile, or use arrows / Tab / Shift+Tab,
Home / End and Enter / Space. Escape cancels the chooser without changing the
draft. Choosing a tile updates only the editor draft; **Übernehmen** / Enter
commits it, and **Abbrechen** / Escape discards the whole draft. Existing name,
accent and shortcut editing and shortcut recording retain their own scope; F6
can still be recorded when the shortcut recorder is active. Handled editor and
chooser input never also reaches a shell.

The same chooser is available when saving or editing a template. The fields
scroll on short windows; the apply/cancel row stays reachable. Opening a chooser
never launches a process, changes a live tab, or creates a template.

The eight drawings are original geometric paths in `src/symbols_view.rs`, shared
by the catalog, editor preview and connected strip. They need no asset files,
icon font, resource downloads or network calls. No third-party icon license or
product-license decision is introduced. Serialized symbol IDs are defined in
`src/symbols.rs`: `terminal`, `code`, `folder`, `server`, `database`, `branch`,
`star`, `shield`. Their IDs do not depend on the localized label or tile order.

An optional 16px user symbol precedes the name in the normal tab. Its name lane
is independently clipped before the close button. Accent, active connected
surface/outline, keyboard focus, textual PIN, process exit/error and binding
remain independent cues. A shield is a user decoration, not a security check.
The catalog explicitly says that symbols are not status messages. Full symbol
names are available in the editor, template list and tab inspection. Long names
are deliberately inspectable through the editor even when the strip truncates
them; shortcuts retain their stable tab/template targets through overflow and
pin groups. This does not add compact mode, free ordering or a general redesign.

## Identity and storage

The optional `identity.symbol` field is part of the existing bounded version-1
template JSON. Missing or `null` means no user symbol. Unknown IDs, wrong types
and unknown properties reject the load visibly and preserve the original file.
Loading a legacy file alone does not rewrite it. New writes include the field;
older builds that do not understand it reject the new file rather than silently
discarding it. Keep versioned review builds/configuration boundaries in mind
when comparing old binaries.

Saving explicitly starts with the active tab's symbol in the draft and links
that same tab without another PTY. Deliberate changes on a linked tab update the
same template ID using the existing background, coalesced atomic writer.
64-template / 256-KiB limits and conflict/temporary-file protections still apply.
Write failures retain the last on-disk configuration and show **NICHT
gespeichert**; **Erneut speichern** retries the pending change. Symbol changes
on an ordinary tab stay temporary.

Startup still creates only the default ordinary session. Stored symbols are
available in the list and applied only when a template is deliberately opened.
Repeated opening activates the existing associated tab, including an exited or
failed tab, without another process. Closing a tab retains its template.
Removing a template leaves the running temporary tab's current symbol, pin,
name, accent and PTY intact while removing the link and inherited shortcut.
Shell output and dynamic titles never write user identity. No process or
terminal content is persisted.

## Proportionate validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
git diff --check
python3 -m py_compile scripts/gui-symbols-smoke.py
```

Model/storage checks cover all symbol IDs, deliberate selection/removal,
chooser/editor cancellation, atomic invalid edits, legacy absence, unknown IDs
and wrong types, linked identity/bindings/pins, saved reopening/unlinking and
write failure/retry. The existing application test with real raw-mode PTYs also
exercises the chooser's keyboard and mouse-message routes; unintended bytes
would change its exact observed input.

Run the bundled changed-path exercise once on a clean committed candidate:

```sh
python3 /home/patrick/Projekte/hoori-term/scripts/gui-symbols-smoke.py \
  /absolute/clean-candidate/launch /absolute/new-evidence-directory
```

It uses only owned X11 windows, isolated configuration and generated clipboard
fixtures. It records process start identities, shell executables/directories,
retained variables, configuration snapshots, real PTY dimensions, exact input
bytes and screenshots. The live flow covers mouse/keyboard/cancel/removal,
multiple ordinary/linked symbols, save without spawn, linked auto-save,
independent pin/close protection, inactive output, copy/paste/resize and visible
write failure/retry. Restart proves no template autostart and deliberate
opening/unlinking. Separate fixtures cover twelve long names at 640/1280 logical
pixels, all catalog drawings, overflow/inspection, 640x360 chooser scrolling,
explicit exit/error state, invalid files and legacy data.

Inspect every PNG separately; metadata, PID and byte checks cannot prove the
drawing or a visible error. Preserve failed attempts and their actual source
identity. `--only live|restart|layouts|errors` and an appropriate two-template
`--catalog-from` snapshot support targeted continuation without repeating
unchanged phases. No measured result is implied by the harness's existence.

After merge, publish clean final source with `scripts/build-review.sh`. Verify
the actual final absolute launcher, metadata and a focused symbol/template
opening/close check. Keep candidate execution attached to its actual commit,
prove final runtime-source equivalence, and record both hashes, scaling/backend,
all checks and limits in the final artifact's `VALIDATION.md`. Per AV-01, broad
unchanged terminal/Vim/window suites retain their previous evidence rather than
being repeated for each intermediate version. Native Wayland and Windows/WSL
runtime behavior remain unverified until their environments are supplied.
