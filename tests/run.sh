#!/bin/sh
# Builds the extension for this machine, lays it out as rhun installs it, and runs tests/basic.txt
# headlessly against a rhun binary. usage: tests/run.sh [RHUN]
set -e
cd "$(dirname "$0")/.."
RHUN=${1:-$(command -v rhun || echo "$HOME/.local/bin/rhun")}
name=$(sed -n 's/^name *= *//p' extension.ini | head -1)
owner=$(sed -n 's/^owner *= *//p' extension.ini | head -1)
case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) target=linux-x86_64 ;;
    Darwin-arm64) target=macos-arm64 ;;
    *) echo "no target for $(uname -s)-$(uname -m)"; exit 77 ;;
esac
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
dir=$work/exts/$owner/$name
mkdir -p "$dir/bin" "$work/proj" "$work/config/rhun" "$work/state"
cargo build --release --quiet
cp "target/release/$name" "$dir/bin/$name-$target"
cp extension.ini icon.png "$dir/"
printf 'fn main() {\n    println!("hello");\n}\n' > "$work/proj/sample.rs"
printf '[files]\nrestore_session = false\nrestore_project = false\n[updates]\ncheck = false\n[git]\nenabled = false\n[ui]\nsidebar = false\nagents_panel = false\n' > "$work/config/rhun/config"
sed "s|proj/|$work/proj/|" tests/basic.txt > "$work/basic.txt"
HOME=$work XDG_CONFIG_HOME=$work/config XDG_STATE_HOME=$work/state \
    "$RHUN" "$work/proj" --headless 1000x600 --scale 1 --extensions "$work/exts" --script "$work/basic.txt" 2>&1 \
    | sed 's/pid=[1-9][0-9]*/pid=PID/' > "$work/out.txt"
if cmp -s "$work/out.txt" tests/basic.expected; then
    echo "ok   basic"
else
    echo "FAIL basic"; diff tests/basic.expected "$work/out.txt" || true; exit 1
fi
