# HOO-14: Windows saved templates

Windows uses the same explicit configuration and stable template/tab identity
rules as [HOO-13](hoo-13-templates.md). Saving links the existing tab without
starting a process. Opening activates its linked tab, including a failed or
exited tab; close that tab before explicitly opening a replacement. Restart
loads configuration and retains the ordinary default startup, with no template
autostarts. Linked user edits persist; shell output and titles never do.

## Targets and directories

The editor offers Windows PowerShell 5.1 and registered WSL-2 distributions,
each with Bash and zsh. A saved distribution name remains configuration when
that distribution is missing; opening shows an error and editing permits an
explicit replacement. WSL 1 never becomes an eligible template launch target.
No target is installed, converted or selected as a fallback.

Choose an absolute Windows directory for PowerShell and an absolute Linux path
inside the selected distribution for WSL. The initial draft uses the explicit
launch directory or the Windows user directory; for WSL it resolves the
distribution's home on a worker, with a bounded 15-second timeout. An error
leaves a visible request for a manual directory. User input or a changed target
invalidates a late result. The draft does not track a shell's later `cd`.

Native directories are opened and resolved by handle, with delete sharing
disabled until process creation completes. This also converts Windows verbatim
path prefixes to the conventional DOS/UNC form used by PowerShell 5.1.
WSL uses fixed launcher code with the directory and supported executable as
separate positional arguments. Failed `cd` exits before the interactive shell,
so there is no silent HOME fallback or evaluation of path text as shell code.
Changed start data applies only to the next new process.

## Storage and recovery

Windows stores the bounded version-1 JSON catalog at
`%LOCALAPPDATA%\hoori-term\templates-v1.json`. LOCALAPPDATA must be absolute.
Existing Linux version-1 files retain their encoding; the optional
`wsl_distribution` field appears only for Windows WSL targets. PowerShell is
encoded as `powershell_51`. Targets are validated for the current platform.
There are no custom startup command or environment fields.

File I/O runs on a worker. A nonblocking lock and comparison with the last
loaded/written bytes prevent cooperating writers from silently replacing newer
configuration. A same-directory temporary file is flushed and closed before
Windows replacement with `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)`.
Errors, including a destination held without delete sharing, retain the old
file and edited in-memory state. This uses Windows replacement semantics;
it does not claim a POSIX directory-fsync guarantee or hardware power-loss
qualification. Reparse-point files are rejected. Bounds and the visible
load/save/retry/unsaved-exit rules remain those documented for HOO-13.

Removing a template removes its saved/inherited binding and link, preserving
its live process. Template shortcuts survive tab close; ordinary tab bindings
do not. Duplicate and reserved shortcuts are visibly rejected. Recognized app
actions are consumed once before PTY key encoding.

## Validation boundary

Native Windows checks cover replacement/error preservation, target-context
validation, missing native directories and a real PowerShell session in a
Unicode/shell-metacharacter directory. The explicit WSL home check requires
`HOORI_TEST_WSL_DISTRO` and is excluded from an unconfigured default test run:

```powershell
$env:HOORI_TEST_WSL_DISTRO = 'Ubuntu-24.04'
cargo test --test windows_templates configured_wsl_home -- --ignored --nocapture
```

The opt-in `validation` GUI fixture can exercise templates using its own
isolated LOCALAPPDATA, real iced window and newly created PTYs. It never uses
the user's saved templates. Model-driven commands plus native screenshots do
not constitute physical keyboard, layout or IME certification.

Final five-row Windows/WSL evidence, source commit, clean review artifacts and
the two native Ubuntu-24.04/X11 shell rows are recorded in HOO-14/HOO-39. WSLg
does not replace native X11 evidence. HOO-14 stays In Progress until the entire
technical matrix is available; product acceptance remains Patrick's separate
decision. Pinning, symbols, compact presentation, new UI layout and installers
are outside this delivery's qualification scope. No GitHub pipeline is added.

API references: [MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw),
[GetFinalPathNameByHandleW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew).
