# HOO-26: deliberate tab and template order on Ubuntu

This implements HOO-5 D-27 through D-29's bounded O-A slice. HOO-27 retains
the complete native Ubuntu/Windows qualification gate. The early X11 build
and technical merge remain separate from Patrick's product acceptance.

## Controls and stable identities

Drag a tab body to the left/right half of another tab in the same pin group.
A cyan line previews insertion before/after that target. A six-logical-pixel
threshold distinguishes a click from a drag. Clicking activates as before;
reordering leaves the active tab unchanged, including when moving an inactive
tab. Close buttons, plus, overflow arrows, window drag and window controls keep
their separate targets. Terminal selection never starts a tab gesture.

Escape cancels. Releasing outside a valid target, across the pin boundary,
or at an unchanged position leaves both orders and pin states unchanged.
Window focus loss, resizing/scaling and modal commands cancel the gesture.
During a held drag, use the header's mouse wheel to reveal overflow; move over
a target again after scrolling before releasing. Source and cached target IDs
are revalidated, so closing either cannot substitute its positional neighbor.

For keyboard use, Ctrl+Shift+L focuses the header; arrows/Home/End choose an
identity without activating it. **Alt+Left/Right** moves that focused tab one
place within its group and keeps focus on it. Endpoints do not wrap or pin.
This action has priority in header focus; outside that scope existing direct
bindings and terminal input retain their behavior. Escape returns to terminal
input. Mouse and keyboard share the same model operation and consume the app
action once, without sending it to a PTY.

Reordering preserves every tab's ID, template link, name, accent, shortcut,
symbol, normal/compact presentation, pin state, shell, terminal and process.
Pin/unpin remains a separate deliberate action. It moves only the changed tab
to its destination group's end and preserves the saved global template order.
The two live groups are never globally sorted together.

## Saved neighbors and deliberate reopening

The bounded version-1 catalog adds `order`, a permutation of stable template
IDs independent of the item storage array. On a linked tab move, remove only
that template's ID and insert it before the next visible linked neighbor in
the same group, otherwise after the previous linked neighbor. Without another
linked neighbor, keep saved order unchanged. All other entries, including
closed templates, retain their relative order. Ordinary tabs are not saved
position anchors; moving them does not write or create templates.

Explicit first Save applies the same neighbor rule to the new template without
reordering existing live tabs or spawning another process. Removing a template
removes its order ID/link/inherited binding; its open tab retains its position,
other identity fields, pin and PTY. Closing a tab retains its template/order.

Opening a closed template inserts its one new tab before the first already
open linked successor in saved order within its pin group, otherwise after
the last open linked predecessor, otherwise at that group's end. Existing tabs
retain their relative order. An already associated tab is only activated,
including after shell exit. Loading never opens saved templates automatically;
ordinary tab positions and processes are not persisted.

Legacy files without `order` use their existing item listing as the starting
order and are not rewritten by loading. Explicit null, wrong types, omitted,
duplicate and unresolved IDs in a present order reject the load visibly.
The original file is preserved. Catalog count/file limits still apply. New
writes include `order`; old strict builds reject it, so use isolated config
when comparing versions. The existing asynchronous atomic writer stores the
whole validated catalog. Failure retains old disk bytes and a visibly unsaved
pending model; Retry saves the pending order. No file I/O runs on the UI thread.

## Proportionate validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 -m py_compile scripts/gui-order-smoke.py
git diff --check
```

Model/storage tests exercise both groups, stale/cross-group/no-op targets,
active identity/binding/property preservation, mouse/keyboard equivalence,
mixed temporary/linked tabs, closed neighbors, first save, deliberate reverse
opening, pin/remove/close lifetime, legacy/invalid data and failed atomic write
with retry. Canvas tests cover threshold, reserved geometry, overflow, outside
targets and focus cancellation at 640/1280. A real raw-PTY application test
checks both action routes and Escape without extra input bytes.

Run one bundled new GUI exercise on a clean committed candidate:

```sh
python3 /home/patrick/Projekte/hoori-term/scripts/gui-order-smoke.py \
  /absolute/clean-candidate/launch /absolute/new-evidence-directory
```

The driver uses only owned X11 windows, isolated configuration, real Bash/zsh
processes and generated clipboard data. It records at least twelve mixed tabs
at 640/1280, drag/keyboard order, invalid/aborted/unchanged targets, overflow,
direct navigation, closed stored neighbors, first save, write failure/retry,
restart/open/remove/close, inactive output, shell/PID retention, actual PTY
sizes and exact clipboard/input bytes. Inspect every PNG individually;
process/order assertions are distinct from rendering proof. `--only` and
`--catalog-from` support targeted continuation, preserving original failures.
The presence of a driver is not evidence that a run passed.

After merge, build clean final source outside disposable worktrees, compare
candidate/final runtime sources and check the actual absolute final launcher
from another working directory. Preserve candidate hashes/reports, exact
executed drivers, final metadata, local checks and visual scope in the build's
`VALIDATION.md`. AV-01 carries unchanged broader terminal/native-window evidence
with its original commit instead of rerunning full suites at each intermediate
version. Native Wayland, Ubuntu 26.04 and Windows/WSL remain open until their
actual environments are available. No GitHub pipeline is added.
