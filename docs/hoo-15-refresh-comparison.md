# HOO-15: terminal refresh comparison

On the unchanged HOO-7 build, `top` update cadence was comparable with GNOME
Terminal in three 60-second runs per terminal. A reproducible difference appeared
in the synthetic control: its new sequence marker became visible later in
Hoori's X11 drawable. Three finer comparisons have non-overlapping per-run median
visibility bounds. Terminal-process CPU/RSS were also higher for Hoori under
these conditions. Physical monitor latency and the cause of Patrick's original
impression remain unproved.

The bounded next step is a separate notification-scheduling correction proposal,
with before/after evidence. This investigation changes measurement tooling and
documentation; it leaves application behavior and the existing review builds
unchanged.

## Main results: `top` cadence

Measured on 12 October 2026, 20:07:16–20:21:11 UTC, including the excluded attempt
and accepted repeat. Every accepted `top` capture spans at least 60 seconds.
Intervals below are between observed whole-window hash changes, not physical FPS.

| Run | Terminal | Capture span (s) | PTY home markers | Drawable changes | Change interval p50 / p95 / max (ms) |
| --- | --- | ---: | ---: | ---: | ---: |
| 01-hoori | hoori | 60.003 | 299 | 299 | 200.00 / 208.95 / 250.17 |
| 02-gnome-repeat | gnome | 60.002 | 299 | 299 | 200.00 / 203.37 / 262.75 |
| 03-gnome | gnome | 60.002 | 299 | 299 | 200.00 / 202.94 / 250.17 |
| 04-hoori | hoori | 60.002 | 299 | 299 | 200.00 / 205.85 / 250.05 |
| 05-hoori | hoori | 60.002 | 299 | 301 | 200.00 / 200.79 / 250.02 |
| 06-gnome | gnome | 60.002 | 299 | 299 | 200.01 / 210.38 / 250.18 |

The program-output home-event medians were 200.39–200.40 ms in all six windows;
GUI change medians were 200.00–200.01 ms. No observed GUI change gap exceeded
262.75 ms at this sampling resolution. Hoori's third run had two additional hash
changes. Hash changes cannot be mapped one-to-one to completed `top` frames, so
these counts do not prove the absence of every intermediate missed/partial update.
The separately numbered controls provide stronger sequence evidence.

The initial 20 Hz, 30-second controls observed 301 distinct sequence values each,
with no undecodable samples, skipped/backwards transitions or unobserved interior
emissions. Their per-run median visibility bounds were Hoori **19.74–71.58 ms**
and GNOME **0–20.88 ms**. Those bounds overlap, motivating the finer experiment.

## Finer control: first marker visibility

Three 20-second controls per terminal use the same 100 ms generator and unchanged
capture helper, with 100 requested samples/s. Order is GNOME → Hoori,
Hoori → GNOME, GNOME → Hoori. Warmup is three seconds. The one-second idle/`top`
phases in these supplemental runs are setup only; they do not replace the main
60-second observations or supply the main resource comparison.

The following are bounds on each run's unknown visibility-delay quantiles,
not point estimates or statistical confidence intervals. Both the preceding and
new capture plus the generator's write interval enter each bound.

| Fine run | Terminal | Distinct markers | Median delay bounds (ms) | p95 delay bounds (ms) | Sample step p95 / max (ms) |
| --- | --- | ---: | ---: | ---: | ---: |
| 01-gnome | gnome | 201 | 0.00–3.55 | 0.00–4.94 | 13.24 / 26.36 |
| 02-hoori | hoori | 201 | 13.55–25.47 | 13.72–30.91 | 14.33 / 24.96 |
| 03-hoori | hoori | 201 | 14.60–26.75 | 14.65–27.53 | 11.25 / 25.02 |
| 04-gnome | gnome | 201 | 0.00–11.68 | 0.00–19.86 | 10.22 / 23.53 |
| 05-gnome | gnome | 201 | 0.00–8.09 | 0.00–15.75 | 10.19 / 20.47 |
| 06-hoori | hoori | 201 | 18.44–30.35 | 18.55–39.18 | 10.16 / 22.24 |

All six fine controls have 201 distinct sequence values, zero undecodable samples,
zero skipped/backwards transitions and zero unobserved interior emissions. Every
paired Hoori median interval lies above the corresponding GNOME interval. This is
a repeatable difference at the X11 drawable measurement point under these
conditions. It does not locate the entire delay inside the application, measure
monitor scanout, or establish the exact delay in Patrick's original configuration.
The two rendering stacks may expose drawable pixels at different stages before
physical presentation.

Actual median sampling steps were about 10 ms. The table retains scheduling
outliers; capture calls themselves took about 1.8–1.9 ms at the median, with a
15.59 ms worst capture in this fine set. The observer used **28.86–31.30% of one
core** in fine controls and is outside terminal-process CPU. This is an intrusive
measurement, with common overhead and a live desktop, not a presentation trace.

## Main resources and interaction

| Run | Terminal | Idle CPU (% one core) | Idle RSS (MiB) | `top` CPU (% one core) | `top` RSS (MiB) |
| --- | --- | ---: | ---: | ---: | ---: |
| 01-hoori | hoori | 0.40 | 189.23–189.23 | 1.23 | 192.04–192.14 |
| 02-gnome-repeat | gnome | 0.00 | 51.34–51.41 | 0.22 | 42.71–44.20 |
| 03-gnome | gnome | 0.00 | 51.51–51.51 | 0.22 | 51.51–51.51 |
| 04-hoori | hoori | 0.50 | 185.07–185.07 | 1.17 | 187.79–187.89 |
| 05-hoori | hoori | 0.60 | 187.32–187.32 | 1.10 | 190.28–190.33 |
| 06-gnome | gnome | 0.00 | 51.49–51.49 | 0.18 | 51.49–51.49 |

Hoori's terminal process used 1.10–1.23% of one core during `top`; the private GNOME
Terminal server used 0.18–0.22%. Their sampled `top` RSS ranges were
187.79–192.14 MiB and 42.71–51.51 MiB respectively. This repeated process-cost
difference is separate from update cadence and is not an absolute performance SLA.

During the same main `top` windows, the **observer** used 5.90–6.60% of one core,
**top** about 0.083%, **proxy** 0.033–0.050%, **shell** no measured tick increment,
and **fixture** 29.97–30.02%. They are separate resource labels in the raw JSON;
none is added to the terminal value. Idle values are ten-second samples with the
fixture still running. RSS variation is retained, including GNOME's first-run
reduction; these are sampled process footprints, not total system/GPU memory.

All six main runs passed the qualitative new-tab/return/help/quit checks and
owned-process cleanup checks. The original `top` PID survived the inactive-tab
interval, the secondary shell had a distinct PID, and targeted exit reaped it.
All **60 main PNGs and 60 fine-control PNGs** were visually inspected, using
contact sheets and representative full-resolution views. Their terminal data are
synthetic commands, controlled counters and the selected fixture process.

## Source evidence and bounded follow-up

The fixed baseline's [notification stream](https://github.com/TrautmannP/hoori-term/blob/170ba6f83719ace935e9cf45085cced5b2c85db7/src/notifications.rs)
unconditionally awaits 16 ms after receiving a wakeup before yielding it to the
UI. This is a concrete application-side delay candidate consistent with later
marker visibility. It is not a profile proving that this wait accounts for all
observed delay. PTY polling, parsing, canvas work, wgpu and presentation have not
been individually timed. The notification file is identical in HOO-6 commit
`d316aee900d2b7731154a0ba5c3ae96c2f007e36` and HOO-7, so this source behavior is
not newly introduced by HOO-7.

Recommended handoff to PM: plan a separate bounded engineering issue for the
notification path. Emit a first wake promptly after idle, rate-limit subsequent
emissions relative to the previous refresh, and retain bounded coalescing and
idle quietness. Keep PTY polling, parsing budgets and renderer architecture out
of that first correction unless new evidence requires them. No fixed SLA or
renderer rewrite is proposed.

Verification for that follow-up: deterministic timing/coalescing tests covering
idle-to-output, sustained/backlogged output and shutdown; retain per-tab parsing
budgets, inactive consumption and bounded queues; run the documented Rust gates
and terminal/tab/clipboard regressions. Repeat the paired cadence and fine-marker
experiments before/after with actual sampling bounds and observer costs, and
publish the changed application from a clean final merged commit with a versioned
review build. Source-level plausibility and these baseline measurements do not
replace that future correction's evidence or Patrick's product acceptance.

## Scope and fixed baseline

This is the bounded investigation in [HOO-15](https://linear.app/hoori-term/issue/HOO-15/feedback-prufen-top-wirkt-gegenuber-ubuntu-terminal-langsamer).
Patrick reported that `top` screen updates felt slower, including before HOO-7.
His exact feedback build, comparator, settings and perceived delay remain
unconfirmed. The experiment does not establish a newly introduced HOO-7
regression or invalidate that observation.

The baseline is the existing, unchanged HOO-7 Ubuntu release artifact:
`170ba6f83719ace935e9cf45085cced5b2c85db7`, `dirty: false`,
`x86_64-unknown-linux-gnu`. Its absolute launcher is
[/home/patrick/Projekte/hoori-term-builds/170ba6f83719ace935e9cf45085cced5b2c85db7/launch](/home/patrick/Projekte/hoori-term-builds/170ba6f83719ace935e9cf45085cced5b2c85db7/launch).
The full build metadata is checked before every run, and the persistent title
identifier `[170ba6f]` is checked before/after dynamic title changes. The original
binary, launcher and build metadata hashes are recorded in `environment.json`.
No application source or old artifact was changed by this investigation.

## Environment and comparability

| Item | Recorded setup |
| --- | --- |
| OS / desktop | Ubuntu 24.04.5 x86_64; GNOME Shell 46.0; X11 `:1` |
| X server | X.Org 1.21.1.11; reported package `2:21.1.12-1ubuntu1.8` |
| GPU / installed driver | NVIDIA GeForce GTX 1070 / 580.178.04 |
| CPU | Intel Core i5-11600KF; 12 logical CPUs available to the driver; no affinity pinning |
| Displays / scale | Multi-monitor desktop near 60 Hz; 96 DPI; GNOME text scale 1.0 |
| Hoori renderer stack | iced/iced_wgpu 0.14.0, wgpu 27.0.1; actual selected API/adapter not logged by the unchanged binary |
| Terminal/parser/PTY libraries | alacritty_terminal 0.26.0; portable-pty 0.9.0 |
| Comparator | Installed GNOME Terminal 3.52.0, VTE 0.76.0; not confirmed as Patrick's comparator |
| Shell / locale | `/usr/bin/zsh -f -i`, zsh 5.9, `C.UTF-8`, both terminals |
| `top` | Same `/usr/bin/top`, procps-ng 4.0.4, binary SHA-256 recorded |
| Terminal grid | 98 columns × 29 rows, verified from the actual PTY |
| Font | Hoori requests 16 px monospace (system match DejaVu Sans Mono); GNOME explicitly uses DejaVu Sans Mono 12 pt at 96 DPI; measured cell width 10 px in both |
| Line height | Hoori 20 px, GNOME 19 px; logical grid matches, pixel height does not |
| Main measurement | One active tab; `top -d 0.2 -p <own-fixture-pid> -w 98` |

`/etc/toprc`, `~/.toprc` and `~/.config/procps/toprc` were absent. Each run has a
new private `XDG_CONFIG_HOME`; `HOME` is retained. GNOME Terminal uses a private
D-Bus session with service autoactivation disabled, a directly started and
PID-verified server, and a keyfile GSettings backend. Its explicit profile uses
the font above, 2,000 scrollback lines, no cursor blinking or scrollbar, and a
clean zsh command for new tabs. Existing terminal servers/profiles are untouched.
The comparator is forced to X11 with scale 1. This setup and the normal desktop
background load are limits on generalization.

The worker has the synthetic process name `hoo15-fixture` and spends about
30 ms busy per 100 ms. It is the only process selected by `top`. The desktop's
other work is neither stopped nor enumerated; aggregate headers/load can vary.

## Measurement method

[scripts/gui-refresh-compare.py](../scripts/gui-refresh-compare.py) launches only
new owned windows, verifies their process ownership, and checks keyboard focus
before capture/input. GTK focus may belong to a child X window, so capture checks
its ancestry. A focus loss aborts capture. During `top`, unexpected PTY input is counted,
discarded, and invalidates the run; its contents are never logged or forwarded.

The main order is Hoori → GNOME, GNOME → Hoori, Hoori → GNOME. Each terminal has
three independent 60-second active-`top` windows after five seconds of warmup,
and a ten-second idle sample after the same warmup. The fixture runs during idle
too. There are no simultaneous comparison terminals. One 30-second synthetic
control per terminal follows another warmup; it emits a numbered, bounded
full-screen update every 100 ms. A colored binary marker identifies its sequence.

An identical nested PTY proxy forwards `top` output to either terminal. It logs
monotonic timestamps, byte counts and `ESC[H` home markers, never output contents.
A home marker identifies a program-output update event, not a finished GUI frame.
This additional PTY is part of the test setup for both terminals.

The observer uses XGetImage on the owned client window at 20 requested samples/s.
It records actual capture start/end, whole-window image hash, changed/unchanged
state, and the decoded control sequence when applicable. PNG encoding occurs
after sampling; three snapshots per phase support visual review. The raw CSV
records scheduling gaps and capture cost rather than assuming exact 50 ms steps.

A hash change is an observed drawable change, not a completed or physically
presented frame. Multiple partial updates can change the hash while retaining
the same sequence marker. Conversely a stable image can be redrawn unchanged.
For `top`, compare GUI change cadence with independently recorded output cadence.
For the control, compare decoded sequence progression with generator timestamps.

First marker visibility is interval-censored: the preceding capture still showed
the previous sequence; the next capture showed the new one. Bounds also account
for the generator's write interval and capture interval. The first frame already
visible at measurement start is omitted from delay bounds. These are client
drawable visibility bounds, not physical display latency, full-frame completion,
or exact midpoint estimates. Compositor presentation and monitor scanout are not
measured. A finer control study is specified separately with its actual cadence.
The two rendering stacks can expose pixels at different points before physical
presentation; this experiment compares the X11 drawable measurement point.

## Resource accounting and interaction

`/proc/<pid>/stat` user+system CPU time is sampled once/s, with a final endpoint.
Percentages use one CPU core as 100% and the actual resource time span. RSS comes
from `statm`; reported ranges are sampled minima/maxima, not peaks or PSS.
The local CPU accounting clock has 100 ticks/s; a reported zero means no tick
increment in the sample, not proof of literally zero CPU work.

| Label | Included process |
| --- | --- |
| `terminal` | Hoori app process/all its threads, or private GNOME Terminal server/all its threads |
| `top` | The actual procps process in the nested PTY |
| `proxy` | The common forwarding/timing helper |
| `shell` | The terminal's clean zsh |
| `fixture` | The synthetic CPU worker selected by `top` |
| `observer` | The Python capture/analysis driver, measured separately |
| `generator` | The synthetic control writer, in that phase only |

GPU execution/memory, compositor/Xorg costs, shared-memory accounting and any
other desktop processes are outside terminal CPU/RSS. GNOME's waiting CLI is
outside its server sample. Ratios between these process footprints must not be
presented as total system-memory or GPU-efficiency ratios.

After each main measurement, a second clean shell tab is opened and identified
by its distinct PID and marker. Returning to the original tab shows `top` still
running; `h` shows help, Escape resumes, and `q` returns to the shell. The second
shell is then exited and reaped. Screenshots and PID checks support these
qualitative observations; their scripted waits are not response-time measurements.
Successful runs verify that their own terminal, shell, top, proxy, generator and
fixture processes have been reaped. Incidental private configuration/cookies are
removed after the run; the explicit settings remain in metadata.

## Reproduction and raw evidence

Requires the existing baseline artifact, an active Ubuntu X11 desktop, Python 3
with Pillow, libX11, xdotool, ImageMagick `import`, GNOME Terminal, D-Bus tools,
gsettings, zsh and procps. Keep test windows focused during observation. Use a
fresh output directory; successful and invalid attempts must not be overwritten.

```sh
cd /home/patrick/Projekte/hoori-term
python3 -m py_compile scripts/gui-refresh-compare.py
python3 scripts/gui-refresh-compare.py suite /tmp/hoo15-new-comparison
python3 scripts/gui-refresh-compare.py analyze /tmp/hoo15-new-comparison
```

The default suite has three 60-second `top` runs per terminal and one 30-second
control each. `run hoori|gnome <new-directory>` exposes the same individual-run
parameters; `analyze <directory>` reconstructs its summary from raw files.
The full build identity is also available independently of the checkout:

```sh
/home/patrick/Projekte/hoori-term-builds/170ba6f83719ace935e9cf45085cced5b2c85db7/launch --version
```

The local study root is
[/home/patrick/Projekte/hoori-term-measurements/hoo15-2026-10-12T2008Z](/home/patrick/Projekte/hoori-term-measurements/hoo15-2026-10-12T2008Z).
Each accepted run contains `metadata.json`, `*-samples.csv`,
`*-resources.json`, `*-summary.json`, timing JSONL, interaction/PID evidence,
PNG snapshots, `COMPLETE.json`, and `analysis.json`. The root manifest records
the effective order and excluded attempts; `environment.json` records commands,
versions and artifact hashes. Local validation and checksums accompany delivery.

The initial `02-gnome` attempt received nine unexpected one-byte inputs during
observation and is excluded. Their content/source is unknown. Its data/log are
retained, and `02-gnome-repeat` is the accepted replacement. The unaffected first
Hoori run is retained. Earlier short pilots corrected test-launch/isolation,
UTF-8 locale, GTK child-focus and tab-selection/logging problems; they are not
long-run evidence and establish no application failure.

## Remaining limits

- This is one Ubuntu 24.04.5/X11 desktop and one explicitly configured comparator.
  Native Wayland, Ubuntu 26.04, Windows/WSL, different GPUs/fonts/scales, many tabs,
  and Patrick's original unknown configuration are unverified here.
- `top` filters one controlled process at a fixed 0.2-second interval. It does not
  reproduce every usual process list, program interval, terminal setting or load.
- Observer and proxy overhead are measured/reported, but may affect scheduling.
  The background desktop is live; repeated order reduces rather than removes
  that uncertainty.
- The actual selected wgpu API/adapter and physical presentation timing are not
  proved. Neither PTY throughput nor process CPU alone establishes GUI latency.
- Screenshots are snapshots, and no human perception study was performed. The
  reported user impression remains a separate observation.
- No absolute SLA, unconditional performance equality, total-memory claim,
  HOO-7 regression claim, renderer rewrite or product acceptance follows from
  this experiment. Application corrections need their own bounded delivery and
  regression evidence; the existing platform gates remain open.

## Local validation for this delivery

The measured helper content SHA-256 is
`a03eac6a3c7857151c9abe4cf494113f69c8dadc06cd936219a97c9fc4bb167c`.
The exact helper and its `gui-smoke.py` dependency are retained under the study's
`measured-helper/`; the fine-run commands and orchestrator are retained under
`fine-control/`. Both datasets reconstruct through the helper's `analyze` action.

Python syntax compilation, the real X11 measurements/interaction/cleanup checks,
raw-analysis reconstruction and repository whitespace checks cover this
measurement/documentation delivery. Rust application source is unchanged. The
study's `VALIDATION.md`, `LOCAL-CHECKS.txt` and `SHA256SUMS` record the final local
evidence and artifact verification. Human Review is a delivery handoff; product
acceptance and the existing platform gates remain separate.
