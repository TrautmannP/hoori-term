# HOO-11: Windows 11 x64 WSL prototype

Scope: [HOO-11](https://linear.app/hoori-term/issue/HOO-11), approved HOO-5
D-03 through D-06 and D-09 through D-14, on the merged Ubuntu prototype.
This is the bounded W-A prototype. HOO-12 retains the complete five-row
Windows matrix; HOO-14 and later issues retain their own feature matrices.

## Launch and lifecycle

Windows startup discovers the existing WSL inventory on a blocking worker,
with bounded output and five-second command deadlines. It matches the names
from `wsl --list --quiet` to `--list --verbose`, including UTF-16LE output,
localized state words, names with spaces and the default marker. It neither
installs distributions nor changes their default or WSL version. These are
the [documented Microsoft inventory/launch commands](https://learn.microsoft.com/en-us/windows/wsl/basic-commands).

The WSL-2 default starts Bash in its home directory. The Plus menu lists all
registered targets and the system Windows PowerShell 5.1 executable. WSL 1
is visible but disabled. Without a supported default, no replacement shell
starts automatically; the user deliberately chooses a target. Names remain
individual argument data, never interpolated shell commands. A removed or
unstartable target leaves its error/exit state in its own tab. Windows
Ctrl+Shift+B opens this target chooser; Ctrl+Shift+T repeats the launch target.
Saved templates remain unavailable on Windows until their separate integration.

The existing portable-pty backend uses ConPTY. Each session has its own
bounded transport and terminal model; inactive output is still parsed.
During Windows shutdown the output reader drains until ConPTY closes, since
ClosePseudoConsole can wait for its output. The writer's owned thread receives
CancelSynchronousIo to release blocked writes. Channels close, the owned child
terminates, ConPTY closes and all transport threads join away from the UI thread.
There is no global WSL shutdown or distribution-wide termination command.

Shell/distribution labels, custom identities and direct bindings remain tied
to TabId. Existing named close confirmations, cancel, exit retention and
clipboard restrictions are unchanged. The embedded full commit/dirty flag is
available with `--version`; both the window title and custom header retain
the short identity after shell title changes. Release Windows builds use the
GUI subsystem, so the review shortcut does not create an extra console window.

## Renderer qualification

The available VM's default adapter selection chose VMware/Mesa OpenGL.
Real window captures exposed missing clipped geometry: tab accent dots,
active surfaces and menu backgrounds were omitted. This is a measured
configuration-specific failure, not a passing render result.

Windows now defaults to Vulkan/D3D12 when WGPU_BACKEND is unset. An explicit
user override remains intact. This VM selects the Microsoft Basic Render
Driver on D3D12 (software rendering); the targeted repeat shows correct menu
backgrounds, colored dots, active outlines and terminal rendering. No hardware
GPU or general driver-matrix claim follows. The process-local environment
default is set before GUI/worker startup and is Windows-only; Rust documents
[environment mutation as sound on Windows](https://doc.rust-lang.org/std/env/fn.set_var.html).

## Reproduce the checks

Run the README Rust gates on Windows. The synthetic core, identity, selection
and launch-inventory tests run natively. The tests using Ubuntu paths, Unix
signals or Unix template semantics are explicitly Unix-only and still run
under Ubuntu; they are not reported as Windows evidence.

```powershell
$env:HOORI_TEST_WSL_DISTRO = 'Ubuntu-26.04'
cargo test --workspace --locked -- --include-ignored --test-threads=1
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

The opt-in WSL test requires the named existing distribution. It verifies
three independent Bash/ConPTY sessions, exact UTF-8 output, resize, Linux
shell/background-job disappearance, surviving other tabs, and a separately
started foreign WSL process. The foreign fixture is cleaned up by its own PID.
Native PowerShell tests cover actual 5.1, UTF-8, resize, exit 7 and shutdown
with saturated input. Missing/WSL-1 defaults and stale chooser results have
separate model tests. No real distribution is removed or converted for tests.

For native GUI fixtures, build with the test-only `validation` feature, copy
the binary out of Cargo's target directory, set HOORI_GUI_VALIDATION to a
fresh existing evidence directory and set HOORI_TEST_WSL_DISTRO explicitly.
The normal release does not contain this driver or its graphics logger.
Writing stage.txt requests one fixture step; each step records a JSON state.
The driver exercises our input/action model inside the real iced window,
ConPTY/WSL processes and native resize. Window captures are read-only.
This is distinct from a manual Windows keyboard/layout/IME qualification.

Stages: inventory; missing (after startup shell exit); powershell;
powershell_exit; wsl; show1/show2/show3; vim; vimtext; resize; savevim; size;
close; cancel; close; confirm; inspect; exit7; close_exited; window_close;
cancel; missing_target; finish. Wait for each resulting state and inspect
the actual rendered image; do not count file creation alone as GUI success.
The fixture saves only its synthetic Vim text, under a PID-specific /tmp path.

## Environment and delivery boundary

The observed Windows host is Windows 11 Pro x64, build 26300.9711,
VMware SVGA 3D driver 9.17.11.3. WSL is 3.0.3.0 with kernel
6.18.54.1-microsoft-standard-WSL2 and WSLg 1.0.79. The existing supported
distribution is Ubuntu-26.04, Ubuntu 26.04.1, Bash 5.3.9 and Vim 9.1
(patches 1-948, 950-2141). The delivered system PowerShell is 5.1.26100.9711.
The configured WSL default remains docker-desktop; its missing Bash is an
observed startup-error case, and Ubuntu is selected deliberately.

Final commit, exact checks, fixture attempts/repeats, screenshots and review
launchers are recorded in Linear and each versioned build's VALIDATION.md.
Interrupted/invalid fixture attempts are preserved with their original dirty
source identity and are not relabeled as clean final-commit GUI runs.

Ubuntu source regression checks also run on Ubuntu 26.04.1 inside WSL 2.
This is not HOO-10's native Ubuntu desktop/Wayland qualification. The Windows
Ubuntu-24.04 row, zsh rows, complete PowerShell interaction/clipboard/load
matrix and later feature gates remain open for their separate issues.
Patrick's product acceptance remains separate from technical delivery.

After merge use scripts/build-review.ps1 on clean final Windows source and
scripts/build-review.sh on clean Ubuntu source. Both preserve previous builds,
check the embedded full commit/dirty flag and publish convenient launchers.
No GitHub pipeline or public installer is added.
