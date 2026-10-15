# HOO-32: native window controls in the connected tab strip

This Ubuntu slice removes the system title bar and puts window controls in the
existing 52 logical pixel tab strip. It preserves the terminal, tab identities,
local templates and close confirmation. Native Wayland and Windows evidence
belongs to HOO-33; an X11 result does not qualify those backends.

## Controls and layout

The right side reserves a 144 pixel drag area containing the short embedded
build commit, followed by minimize, maximize/restore and close controls. The
Plus and overflow controls remain separate. The native minimum window size is
640 by 360 logical pixels. At 640 pixels, an overflowing strip still reveals
one complete 224 pixel tab. At larger widths, more tabs fit without changing
their identity or position. The hash stays visible with menus and confirmations,
while the OS title also retains it for task switching and inspection.

Drag the dotted/hash area with the primary mouse button to request native
window movement after four pixels of deliberate motion. A click alone does not
start a native grab, preserving double-click and focus-loss cancellation.
Double-click it to toggle maximization. A six pixel border
requests native resizing in all eight directions; corner targets extend along
the border for sixteen pixels. Resizing is disabled while maximized or closing.
Selections started inside terminal content remain selections when they reach
the edge. Tabs, their close targets and Plus do not start a window drag.

Ctrl+Shift+L focuses the header. Tab/Shift+Tab traverse tabs, their close targets,
Plus and the three window controls; Shift+Tab from the first tab reaches window
close, then maximize/restore, then minimize. Enter/Space performs the focused
action and returns input routing to the active terminal. Escape returns without
an action. The cyan focus outline is distinct from the active-tab appearance.
Hover gives each control feedback and a description. No header-navigation or
window-action keys are also encoded as terminal input.

Minimize keeps sessions and output processing alive. Return through the desktop
task switcher, taskbar or overview. There is no separate application unminimize
control. Maximize/restore uses the window manager's actual state, and the icon
and tooltip update after observing it. Restore geometry is managed by the window
manager rather than an application coordinate cache.

Own close and native close requests use the same named confirmation for all
running sessions. Repeated requests do not confirm it. Cancel preserves the
processes and configuration; confirm shuts them down before exiting. A failed
or exited session alone can close without a running-session confirmation and
without starting a replacement session. Window controls remain usable on edit,
launch and confirmation surfaces; requesting close discards an uncommitted edit
before showing the confirmation.

## Native integration

`window_chrome` owns control labels, native state observations and border hit
testing. `tab_strip` owns the reserved layout and mouse/keyboard targets. `App`
dispatches native tasks before modal terminal filters. PTY I/O remains on the
existing bounded asynchronous path.

The implementation uses iced 0.14's native minimize, toggle-maximize, drag and
drag-resize tasks. Drag starts with movement while the primary button remains
held; resize starts on the border press. Neither uses a global positioning
assumption. Focus, resize and scale events query actual
maximization. A one second query while focused catches external state changes
that produce no size change; it is absent while unfocused/minimized. Only the
latest observation revision updates the displayed state.

iced's winit adapter currently discards the result of native drag/resize calls.
The application therefore cannot expose that backend error as a typed result.
Real geometry, PTY dimensions and screenshots must establish whether the tested
desktop honored each request. Unsupported or untested compositor behavior must
remain an explicit validation gap. No new global shortcuts, fullscreen mode,
snap menu, geometry persistence, multiwindow feature or CI pipeline is added.
