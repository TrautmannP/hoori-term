#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$repo"
if [[ -n $(git status --porcelain --untracked-files=normal) ]]; then
  echo 'Review builds require clean committed source.' >&2
  exit 1
fi
commit=$(git rev-parse HEAD)
build_root=${HOORI_REVIEW_ROOT:-/home/patrick/Projekte/hoori-term-builds}
destination="$build_root/$commit"
if [[ -e "$destination" ]]; then
  echo "Refusing to overwrite existing review build: $destination" >&2
  exit 1
fi
cargo build --release --locked
if [[ $(git rev-parse HEAD) != "$commit" || -n $(git status --porcelain --untracked-files=normal) ]]; then
  echo 'Source changed while building; refusing to publish.' >&2
  exit 1
fi
binary="${CARGO_TARGET_DIR:-target}/release/hoori-term"
metadata=$("$binary" --version)
if [[ "$metadata" != *"commit: $commit"* || "$metadata" != *'dirty: false'* ]]; then
  echo 'Embedded build identity does not match clean HEAD.' >&2
  exit 1
fi
mkdir -p -- "$build_root"
temporary=$(mktemp -d "$build_root/.build-$commit-XXXXXX")
trap 'rm -rf -- "$temporary"' EXIT
install -m 755 "$binary" "$temporary/hoori-term"
printf '%s\n' "$metadata" > "$temporary/BUILD-METADATA.txt"
uname -sm >> "$temporary/BUILD-METADATA.txt"
. /etc/os-release
printf 'build host: %s\n' "$PRETTY_NAME" >> "$temporary/BUILD-METADATA.txt"
cat > "$temporary/launch" <<'LAUNCH'
#!/usr/bin/env bash
set -euo pipefail
build_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
exec "$build_dir/hoori-term" "$@"
LAUNCH
chmod 755 "$temporary/launch"
cat > "$temporary/hoori-term.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=hoori-term [${commit:0:7}]
Comment=Ubuntu review build $commit
Exec="$destination/launch"
Icon=utilities-terminal
Terminal=false
Categories=System;TerminalEmulator;
DESKTOP
chmod 755 "$temporary/hoori-term.desktop"
cp README.md "$temporary/README.md"
mv -- "$temporary" "$destination"
trap - EXIT
printf 'Review build: %s\nLaunch: %s/launch\n' "$destination" "$destination"
