# hoori-term

A local native Rust terminal prototype for Ubuntu. This first spike tests
iced/wgpu, alacritty_terminal and portable-pty with one real shell session.
It is not yet the planned version with multiple customizable tabs.

## Run

Prerequisites: Rust 1.98.1 (pinned by `rust-toolchain.toml`), a C linker,
Ubuntu desktop with X11 or Wayland libraries and a working Vulkan/OpenGL driver,
monospace fonts, and zsh (`/usr/bin/zsh`). The verified environment is Ubuntu
24.04.5 x86_64 with X11; Wayland and Windows runtime checks remain pending.

```sh
cargo run --locked
cargo run --locked -- --shell /bin/bash -- --noprofile --norc -i
```

Type directly into the terminal. Standard navigation, Ctrl combinations and
F1–F12 are sent to the shell. Closing a running session requires confirmation;
Escape or **Abbrechen** keeps it alive. A terminated shell leaves its output
and exit status visible until the window is closed. A startup error is shown
in the window. The prototype has no selection, clipboard UI or scrollback
navigation yet. No terminal contents or input are logged by the application;
shells may apply their own history/startup configuration.

`--shell PATH` selects an executable, and arguments after `--` are passed
directly to it. With no arguments, zsh starts interactively. This is an
explicit launch choice, never an action triggered by terminal output.
`--version` prints the full embedded commit, dirty state and build target.
The window title retains the short build identity when the shell changes
its title. Development builds show `dirty` when built from modified source.

## Local checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
git diff --check
```

There is deliberately no GitHub CI pipeline. See [the spike evidence](docs/hoo-6-spike.md)
for the GUI validation procedure, actual observations and integration limits.

## Versioned review builds

After merging, check out the final `main` commit with a clean worktree, then:

```sh
./scripts/build-review.sh
```

The script builds with the lockfile, validates the embedded identity and
publishes a binary, `launch`, `.desktop` launcher and `BUILD-METADATA.txt` to
`/home/patrick/Projekte/hoori-term-builds/<full-commit>/`. Existing builds are
preserved. Run the absolute `launch` path printed by the script from any
directory. The `.desktop` file can be opened with `gio launch` or installed
in the local desktop application menu. These are local Ubuntu review
artifacts, not a public installer or a Windows release.
