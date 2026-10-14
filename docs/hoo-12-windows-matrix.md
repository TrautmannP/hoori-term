# Windows qualification (HOO-12)

HOO-12 qualifies Windows 11 x64 with WSL 2 / Ubuntu 24.04 and 26.04,
each with Bash and zsh, plus native Windows PowerShell 5.1 / ConPTY.
PowerShell 7 is only the test orchestrator; it does not replace the 5.1 row.
The WSL default distribution is never changed by these tests.

An explicit WSL zsh launch now has a `zsh` fallback tab name. Unknown explicit
WSL executables use `WSL`, rather than claiming that they are Bash. Launch
arguments, distribution selection and default-start policy remain unchanged.

## Evidence and source identity

Use clean committed source. Keep a fresh evidence directory outside the
checkout for each run, with the fixture executable, full embedded metadata,
GPU log, environment versions, assertions, checkpoints and screenshots.
Record candidate and merged commit identities separately. Publish ordinary
review binaries without `validation`, from the clean final merged commit.
The final five runtime rows must identify the same source commit.

The optional `validation` feature contains a native iced window with the real
App, terminal renderer, ConPTY and WSL transports. It creates new synthetic
sessions and calls the same App keyboard/message paths as normal input. It
does not attach to existing sessions or inject operating-system input. This
is actual Windows GUI/runtime evidence, but does not certify physical keyboard
layouts, IME composition or every driver. Inspect screenshots using the normal
Windows computer-use tooling; activate the returned fixture window first.

The fixture logs synthetic terminal grids only when explicitly enabled.
Normal review builds contain neither this input driver nor its synthetic-grid
logger. The separate explicit `HOORI_GRAPHICS_INFO=1` diagnostic can print only
the selected compositor adapter; it is disabled during ordinary launches.
Clipboard copy/paste uses iced's real Windows clipboard backend. The original
text clipboard value is retained in memory and can be restored with the
`restore_clipboard` action; it is never included in snapshots. Non-text clipboard
formats are outside this text test.

## Run the native fixture

Build with `cargo build --release --locked --features validation`. Copy the
binary to a fresh evidence directory and start it with these process-scoped
environment variables:

```powershell
$env:HOORI_GUI_VALIDATION = 'C:\Repos\hoori-term-evidence\hoo12-<commit>-<run>'
$env:HOORI_GUI_MATRIX = '1'
```

Clear both variables in the orchestrator after launching. Save the process ID
in `pid.txt` in that directory. The bounded `step.json` interface is exclusive
to this developer fixture. `scripts/windows-matrix-control.ps1` writes numbered
steps and waits for a fresh native snapshot; assertions must use generated
output markers, not the echoed command text.

`scripts/windows-matrix-scenarios.ps1` provides these phases:

1. For rows 0–4: `row-start`, inspect/save the colors/cursor screenshot;
   for WSL rows 0–3 also run `row-vim`, inspect/save the resized Vim screenshot;
   then `row-finish`. Each row checks interactive Unicode, line editing,
   Ctrl+C, native resize and visible exit code 7. Vim's saved UTF-8 bytes are
   checked independently through the selected distribution's filesystem.
2. `mixed-start`: starts independent foreign WSL work before opening three
   mixed WSL/PowerShell tabs, names/colors/shortcuts them, and removes the five
   already exited row tabs without confirmation.
3. `mixed-shortcuts`: built-in and duplicate conflicts, stable tab identity
   after moving and dynamic titles, positive raw-byte probe for shortcut
   non-delivery to the PTY, selection/copy from WSL and paste into PowerShell.
4. `mixed-load`: 6,000 numbered output lines in an inactive tab, interactive
   PowerShell and twelve native resizes, 2,000-line history bound, top/page/live
   navigation, and observed Windows-app CPU/private/working-set measurements.
   These measurements exclude WSL processes and make no performance guarantee.
5. `mixed-close`: named confirmation, cancellation, owned shell/background-job
   termination, interactive survivor tab, and preservation of the independent
   same-distribution WSL process. Inspect/save the close and window dialogs.
6. `errors`: missing executable and missing distribution remain visible and
   can be closed without confirmation. Separately use fresh fixtures with
   `inventory` actions `missing`, `empty`, and `legacy`: verify deliberate
   PowerShell selection and the WSL-1/default warning. These are isolated
   inventory inputs, not an actual WSL uninstall or conversion.

Before closing the final fixture, restore its saved text clipboard. Terminate
only the explicitly recorded foreign test PID and its Windows helper. Preserve
all evidence, including failed attempts. Do not use global WSL shutdown or
distribution termination as a tab-close substitute.

## Local gates and review artifacts

Run fmt, workspace/all-target Clippy with warnings denied, workspace tests and
`git diff --check`. Include the real WSL transport regression separately for
each distro/shell using `HOORI_TEST_WSL_DISTRO`, `HOORI_TEST_WSL_SHELL` and
`--include-ignored --test-threads=1`. The native test additionally exercises
PowerShell 5.1's interactive PSReadLine buffer with arrow/backspace/end keys.

The final delivery records PR, checked tree, merged commit, exact matrix,
rendering backend, versioned Windows and Ubuntu builds, launch commands and
remaining limits in Linear and the build's validation report. Ubuntu builds
made on 24.04 use its glibc baseline; a 26.04 build is not assumed compatible.
WSLg launcher checks do not claim native Ubuntu/X11 desktop acceptance.
Human Review is technical delivery; Patrick's product acceptance remains
separate. No GitHub pipeline is configured.
