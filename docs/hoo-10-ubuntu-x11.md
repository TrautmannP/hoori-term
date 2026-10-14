# Current Ubuntu/X11 qualification

HOO-10 is a bounded native-platform qualification and review-delivery issue.
Patrick's explicit decision of 14 October 2026, HOO-5 D-40, replaces the current
Ubuntu gate with the available Ubuntu 24.04/X11 desktop and both Bash and zsh.
The two native Wayland rows are deferred in HOO-36. They are not passing results
or current blockers. The Windows/WSL matrix remains its separate HOO-12 gate.

The seven HOO-10 criteria still require real GUI/PTY evidence, the same final
source commit in both shell rows, actual backend/graphics versions, a clean
Ubuntu-24.04-compatible build, and separate product acceptance. This document
defines the reproducible procedure. Actual executions, observations, images,
failures and source identities belong to the versioned build's VALIDATION.md.

## Evidence and privacy boundaries

The ordinary program opens no control socket and logs no terminal contents.
For the qualification, `HOORI_GRAPHICS_INFO=1` explicitly enables one
graphics-only diagnostic. It prints the selected adapter's name, driver and
backend from iced_wgpu's actual compositor initialization. It neither infers
the selected adapter from an installed-device list nor observes PTY data.
Without that exact environment value the logger is not registered. The
Windows input driver remains behind its separate `validation` feature.

The X11 helper launches its own ordinary review processes. It validates
_NET_WM_PID and native focus before input and capture; it never targets an
existing terminal or captures the desktop. Every fixture uses fresh local
configuration directories, known synthetic clipboard values and tracked child
PIDs. Existing clipboard contents are not read. ZDOTDIR points to an empty
fixture directory with an empty .zshrc to bypass zsh-newuser-install; real user
configuration files are not rewritten. Default/menu startup probes verify that
the very first command writes the exact expected file, so a setup dialog
consuming its first byte cannot be hidden by a later marker.

Generated commands enter through the ordinary native paste path; Vim Unicode
input, app shortcuts and raw-byte probes also exercise native keyboard input.
The raw fixtures send real DA queries and consume the exact response from their
own PTY. An inactive fixture completing such a query proves its output was
parsed while a different tab remained active. Its synthetic input-byte files
are explicit test artifacts, not application logging.

## Reproduction

Use the pinned Rust toolchain and the documented local checks. Build and
publish from clean committed source using scripts/build-review.sh. The helper
needs Python 3, xdotool, xprop, xdpyinfo, xrandr, xclip, ImageMagick import,
GNOME settings, zsh, Bash and Vim on the actual X11 desktop. NVIDIA's device
inventory is recorded if nvidia-smi exists; the compositor diagnostic identifies
the actual selected adapter independently.

Run the full finished-stand qualification only once, from another directory,
with the absolute versioned launcher and a fresh evidence destination:

```sh
python3 /path/to/checkout/scripts/gui-ubuntu-qualification.py \
  /home/patrick/Projekte/hoori-term-builds/<full-commit>/launch \
  /path/to/fresh-evidence
```

The helper refuses an existing evidence directory or a dirty build identity.
Use `--only graphics` for the new diagnostic/default-launch path before merge;
it does not certify the functional shell rows. The finished final-Main run uses
the default `--only all`. A concrete failed phase can be continued with
`--only zsh`, `--only bash`, `--only default` or `--only errors`, retaining the
original failed attempt and mapping exact evidence/source versions. Do not
repeat unrelated successful phases solely for a new hash or test-helper fix.
The focused `--only startup` continuation checks both default-launch modes and a
deliberate Bash-to-zsh menu start, without rerunning the functional shell rows.

## Bundled scenarios

Both shell rows use the same ordinary binary and cover:

1. Real interactive UTF-8, ANSI colors and cursor movement. Vim 9.1 is opened,
   receives Unicode input, saves exact UTF-8 file bytes, resizes in its alternate
   screen and returns to the shell. Subsequent stty-size files show actual PTY
   dimensions changing with the native window.
2. Three independent sessions, including explicit shell selection without
   premature process creation. Names, accent colors and direct F8/F9/F10
   bindings remain attached to their tab IDs and original shell variables
   despite dynamic OSC titles. Duplicate and built-in conflicts are shown;
   cancellation preserves the original binding. App/dialog keys never appear
   in the raw fixture's input bytes.
3. Explicit Unicode selection/copy, exact bracketed/plain paste, and rejected
   output-triggered OSC 52 clipboard access. All input reaches only the active
   raw fixture.
4. Six thousand numbered lines from an inactive tab while another receives
   actual input. The timestamps prove that input occurred during the output
   interval. A DA reply reaches the inactive program before it is activated.
   Native resizes, latest output, bounded-history top/page/live navigation are
   shown separately. This is functional evidence, not a performance promise.
5. A window confirmation names all three live sessions; cancellation retains
   every shell/fixture. An individual confirmation terminates only its target.
   The surviving other shell exits with status 7, stays visible, and closes
   without a live-process prompt. Removed bindings are not inherited by the
   new tab. Final window cancellation/confirmation and tracked PID cleanup
   complete the row.

The full run also starts the real no-argument launcher with the graphics
diagnostic both enabled and disabled, and checks a visible missing-shell error
without a live-process confirmation. Normal launches do not emit the selected
adapter or synthetic terminal contents. Metadata, native window identity,
versions, geometry, input bytes, queries, images and phase outcomes are saved.
Title/JSON/PID assertions do not certify rendering: inspect every image.

## Final handoff

Preserve the actual executed helpers and source identity, local check output,
all successful and failed evidence, and the final artifact's checksum list.
Report which checks actually ran at the final commit and which use unchanged
earlier source paths; do not relabel a candidate run as a Main execution.
Build the review executable on Ubuntu 24.04 and record its highest imported
GLIBC version. Install/validate its desktop launcher and check the absolute
launcher, permanent embedded hash and metadata from outside the checkout.

Human Review requires all current HOO-10 shell scenarios at a shared final
source commit. Product acceptance and milestone closure remain Patrick's
separate decisions. No GitHub pipeline, new terminal features, public installer
or additional platform qualification is introduced by this package.
