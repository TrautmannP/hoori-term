# HOO-29: local terminal titles and reported program status

This Ubuntu slice adds transient runtime metadata. A tab's own name, shortcut,
accent, pin state, symbol, presentation and saved template remain independent.
It does not identify a program by its output, process name, title or spinner.
No protocol message starts, focuses, stops or authorizes anything.

## Display and inspection

Normal tabs show the own name, a short terminal-title preview, the actual PTY
state and a separate `CLI:` report. Compact tabs keep their fixed symbol width
and display separate CLI and PTY lines. A missing report displays `CLI: —`.
Reported states are `bereit`, `arbeitet`, `wartet`, `fertig` and `Fehler`.

Right-click a tab, including an inactive one, to open its full title/status
details without activating it. Alternatively use Ctrl+Shift+L, focus a tab and
press I. The **Titel/Status** button inspects the active tab. The scroll panel
shows its full own name/unique ID, full accepted terminal title, actual PTY state
and all reported records. Mouse wheel, arrows, PageUp/PageDown and Home/End scroll;
Escape, Enter or **Zurück zum Terminal** closes it. Input and other tab actions
are paused during inspection; PTY output and runtime metadata continue updating.

The tab overview uses a deterministic attention order: error, blocked, working,
done, idle. Equal states select the most recently updated record. When both
errors and waits exist, the overview names both; details show counts of waiting
and error records. Details sort by record ID and show reported kind, progress,
title/message and the nearest inherited program identifier when provided.
This is a summary of program claims, not an agent browser or process monitor.

The OS window title retains the embedded source hash, and the reserved header
area always shows the short hash. Close dialogs continue using own tab names
and IDs. Runtime titles and reports never update the template catalog and are
absent after restarting the app.

## Bounded protocol support

Standard OSC 0 and OSC 2 titles accept BEL and ST terminators, UTF-8 and
fragmented PTY reads. Empty titles and RIS clear the title. Accepted full titles
are available in details; the OS title uses a shorter sanitized preview.

Local OSC 7501 support follows [Program Status Protocol revision 0.4](https://www.superlogical.com/rex/docs/build/program-status), dated 2026-10-10.
The fixed reply to `OSC 7501 ; ? ST` (BEL is also accepted) is:

```text
ESC ] 7501 ; ?:states=idle,working,done,blocked,error:kinds=permission,question,auth ESC \
```

The reply goes only to the source PTY, in order with core terminal replies.
No message fields are echoed. Supported records have `state`, optional `id`,
`app`, `kind`, `progress`, and base64 UTF-8 `title`/`msg`. A report replaces its
whole record; omitted optional fields do not retain stale values. Child records
inherit the nearest ancestor's `app` dynamically. A subtree clear affects that
ID and its descendants; a root clear removes all records.

Limits apply before mutation: 4096 bytes per complete OSC (including framing),
64 records per terminal, ID 128 bytes/eight nonempty segments of at most 32
bytes, app 32 bytes, key 16 bytes, title 256 encoded/192 decoded bytes, message
2732 encoded/2048 decoded bytes. Base64 padding is optional; invalid UTF-8,
control characters, malformed supplied IDs and over-limit reports are rejected
atomically. Unknown states are rejected; unknown fields and malformed pairs
follow the reference's ignore rules. Unsupported/invalid optional kind,
progress or app values are absent. Duplicate valid fields use the last value.
Progress is 0–100 and only applies to working/blocked; kind only to blocked.
At capacity, the least recently updated record is evicted.

The OSC gate bounds buffering before vte's generic OSC parser, including unknown
OSC strings. Incomplete strings remain bounded; oversized strings are discarded
through their terminator. CAN/SUB or a new escape cancels a partial string.
All display text is plain text with controls and bidi/invisible formatting
removed. No markup, clipboard, file or network action is derived from metadata.
OSC 52 remains disabled. Title events coalesce to the last legal update rather
than being dropped behind a reply flood. Replies are capped at 64 per feed;
status storage and accepted last updates remain independent of that cap.

OSC 133 A (shell prompt) and attached PTY exit remove idle/working/blocked
reports. Done/error results survive until clear, replacement or RIS. Primary/
alternate screen changes and DECSTR do not erase reports. RIS clears them all.
There is no inactivity timeout or inferred progress. A terminal whose process
has exited always shows that actual PTY state, even with a retained result.

## Validation boundaries

`tests/program_status.rs` checks fragmented controls, ANSI/Unicode rendering,
query/DA reply ordering, invalid/oversized input, replacement/hierarchy/eviction,
flood coalescing, prompt/reset/screen/exit lifetimes and two real independent raw
PTYs which must read their own exact capability replies. App tests check identity,
catalog isolation, inactive updates, navigation, inspection input routing and
close cancellation. These synthetic fixtures do not prove an actual CLI's
protocol support.

The bundled `scripts/gui-status-smoke.py` uses isolated owned configurations and
synthetic clipboard/PTY fixtures. Its optional CLI phase runs installed Codex and
Claude in an empty test directory through `cli-protocol-observer.py`. That opt-in
bridge records only bounded OSC 0/2/7501 metadata and fixed capability responses,
not a terminal transcript or user input. Exact installed versions, invocation
arguments and actual observed signals belong in the versioned build's report.
Synthetic CLI status fixtures must never be reported as actual Codex/Claude
integration. Codex title configuration is not evidence of OSC 7501 support.

References: [xterm title controls](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html),
[official OpenAI Docs: Codex /title](https://learn.chatgpt.com/docs/developer-commands#configure-terminal-title-items-with-title),
[Claude terminal status](https://code.claude.com/docs/en/terminal-config#see-session-status-in-your-terminal).
Native Wayland, Ubuntu 26.04, Windows 11/ConPTY/PowerShell and all four WSL shell
combinations remain separate HOO-31 qualification work. The early X11 review
build does not satisfy that matrix or Patrick's product acceptance.
