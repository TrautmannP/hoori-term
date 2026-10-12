# HOO-13: explicit local Ubuntu templates

This slice implements HOO-5 D-15 through D-19 for Ubuntu Bash/zsh. It builds on
the integrated HOO-8 identity/shortcut model and HOO-19 connected strip. Native
Wayland and Windows/WSL qualification remain separate HOO-10/HOO-14 gates.
Technical delivery and Patrick's product acceptance remain separate.

## Configuration and lifecycle

Use **Als Vorlage speichern** or Ctrl+Shift+S on an ordinary tab. Its draft
contains a name, accent, optional shortcut, next-start shell and absolute start
directory. Saving links this existing tab to one new stable `TemplateId`; it
does not create a PTY. The live `TabId` and saved ID have separate lifetimes.
The draft initially shows an explicit launch directory, or HOME for an ordinary
tab. It does not track shell `cd` or startup-script directory changes. Set the
intended future directory explicitly. Ordinary startup retains the previous
PTY library default directory behavior.

Open **Vorlagen** with Ctrl+Shift+P. Click a name or use arrows/Home/End and
Enter. An existing linked tab is activated; otherwise one new session is
created at the stored shell/directory. An exited or failed tab is still an
existing target. Close it before explicitly opening a replacement. There is
at most one open tab per saved template.

The ordinary default startup session remains unchanged. Loading templates
does not open more tabs. Restart restores configuration only: no process,
terminal output, scrollback, environment or runtime title is restored.

Name, accent and shortcut changes made with **Tab bearbeiten** automatically
update a linked template under its original ID. Ordinary tabs remain temporary.
An empty linked-tab name restores and saves its original shell/ID label;
creating/editing a saved template requires a nonempty name. Shell output and
OSC titles never update saved configuration. The list's **Bearbeiten** action
(or E) also edits future shell/directory. Those changes do not restart or
relocate a running process.

The list's **Entfernen** action (or Delete) removes the saved entry, its link
and inherited shortcut. Its open tab retains name, accent, terminal and PTY.
It is now temporary and can receive a new independent shortcut. IDs are not
reused within a catalog; its next-ID counter survives saving an empty catalog.
Running tab/window close still uses D-06 named confirmation and cancellation.

## Routing and modal surfaces

The shared reserved action table now includes Ctrl+Shift+P (templates) and
Ctrl+Shift+S (save/edit template). Template shortcuts remain after their tab is
closed; ordinary tab shortcuts disappear with their tab. All bindings apply
only within the focused application window. Conflicts across app actions,
saved entries (including closed targets), and open ordinary tabs are rejected
before changing settings. An ordinary binding assigned during a slow initial
load is preserved; a conflicting load fails visibly until that conflict is
resolved and retried. A template and its own linked tab share one target.

Recognized actions return before terminal encoding. Draft/list input and
deferred paste cannot bypass the modal surface. Transport/status processing
continues for inactive tabs. The inline surfaces temporarily reduce the
active viewport; activation/resize still targets the correct PTY. The toolbar
is below the terminal; the connected strip, Plus choice and existing editor
geometry remain in place. Header tooltips are removed while a modal is open.

## Storage and recovery

The file is JSON version 1 at:

```text
$XDG_CONFIG_HOME/hoori-term/templates-v1.json
```

An absent or relative `XDG_CONFIG_HOME` falls back to the absolute
`$HOME/.config` directory. Failure to obtain an absolute location is visible.
The schema allows only saved identity, the Bash/zsh target and directory, plus
version/next-ID metadata. It does not accept custom startup commands or
environment entries. Direct file editing/import/export is not a supported UI.

Bounds are 64 templates, 256 KiB encoded file, 40 Unicode scalar values per
name and 4,096 UTF-8 bytes per absolute directory. Invalid control/direction
characters, IDs, shortcuts, duplicate bindings, unknown fields, unsupported
versions and malformed/incomplete JSON are rejected. Aggregate byte/count
limits are checked before mutating the catalog or linked tab.

Load/save filesystem operations run through `spawn_blocking`. One write can
be in flight; later changes coalesce into the newest bounded catalog snapshot.
Revision receipts prevent an old success from marking newer changes saved.
The toolbar distinguishes loading, writing and unsaved changes.

Writes acquire a nonblocking advisory lock, compare the existing file with
the loaded/last-written bytes, write a same-directory new temporary file with
mode 0600, flush it, rename it atomically, and flush the directory. A failure
before replacement preserves the old file. A directory-flush failure after
replacement retains the new stamp but reports uncertain durability and leaves
the revision unsaved until a successful retry. Interrupted temporary files
are ignored; unrelated files are not removed.

Corrupt, oversized, nonregular/symlink or unsupported files remain untouched.
The visible load error disables template mutations; ordinary sessions remain
usable. Correct the file outside the app and use **Erneut laden**, or retain it
for diagnosis and restart after relocating it yourself.

Write errors retain the edited in-memory configuration and show **Erneut
speichern**. There is no success message for a failed automatic save. Another
hoori-term writer is serialized by the lock; stale loaded bytes cause a
visible conflict rather than overwriting its file. No automatic merge or
filesystem watching is provided. Direct external writers need not honor the
advisory lock.

Closing waits for an in-flight/follow-up save after shutting down confirmed
sessions. A failed final save keeps the application visible with retry and
an explicit **Ungespeichert beenden** choice. That choice discards pending
configuration; the ordinary D-06 process confirmation is still required.

## Start targets

Targets are fixed `/usr/bin/zsh -i` and `/bin/bash -i`; shell startup/history
configuration belongs to those shells. Arbitrary `--shell` arguments are not
stored as template commands. The save draft exposes the next-start target;
it can deliberately differ from the currently running program.

Paths/arguments are data passed to the PTY command builder, never concatenated
shell code. Spaces, semicolons and literal `$()` in a directory are supported.
A missing target or invalid/unreachable directory produces a retained failed
tab and visible error; the template remains editable.

`portable-pty` 0.9.0 otherwise substitutes HOME for a missing directory. The
Ubuntu transport opens an `O_PATH | O_DIRECTORY` descriptor and holds its
`/proc/self/fd` alias until spawn, so path replacement/removal cannot trigger
that library fallback. A usable Linux `/proc` is required; failure is visible.
This Linux implementation is not evidence for native Windows/WSL path handling.

No terminal contents, input or status text are logged by the application.
There is no account, network storage, telemetry, automatic template startup,
process persistence, pinning, icons, compact mode, workspace or daemon in this
slice. No GitHub pipeline is added.

## Validation

Run the documented local gates:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
git diff --check
```

`tests/templates.rs` exercises links and temporary tabs, atomic conflicts
against closed templates/app/live tabs, ordinary-versus-saved binding
lifetime, nonreused IDs, encoded/count limits, strict format roundtrips,
literal-directory real-PTY reopening, exited targets, future start-data edits,
invalid directories, atomic store failure/conflict preservation, interrupted
temporary files and symlinks. App tests cover loading without startup sessions,
coalesced receipts, visible save failure and shutdown waiting. The real raw-PTY
routing test includes template/list actions and rejected deferred paste.

The bundled `scripts/gui-templates-smoke.py` uses newly owned X11 windows,
synthetic shell/input fixtures and an isolated `XDG_CONFIG_HOME`.
It records separate process/state assertions and screenshots for multiple
saved entries plus ordinary tabs, repeated opening, automatic edits, restart
without autostart, exact Bash/zsh directories, retained exited targets,
removal without process loss, shortcut routing, save retry and bad-file/start
errors. Inspect every screenshot; JSON flags alone do not prove rendering.
It changes only its own generated configuration.

```sh
python3 scripts/gui-templates-smoke.py /absolute/review-build/launch /absolute/new-evidence-directory
```

Use an unused evidence directory. Record actual Ubuntu/display/shell versions
and embedded commit identity. Per-delivery `VALIDATION.md`, local-check logs,
GUI reports and checksums accompany the clean merged review build outside
the repository. Existing valid terminal/Vim/strip evidence retains its actual
older commit; unchanged full GUI suites are not repeated on intermediate builds.
