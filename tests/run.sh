#!/bin/sh
# Builds the extension for this machine, lays it out as rhun installs it, and runs each
# tests/NAME.txt headlessly against a rhun binary, comparing what it prints with tests/NAME.expected:
#   basic    both charts, a click that switches bars and pie
#   numbers  protocol 1.1: the table of the charts' numbers, Space and Ctrl+C in a chart, the
#            status bar's line count
# usage: tests/run.sh [RHUN [NAME...]]   (UPDATE=1 rewrites the .expected files)
set -e
cd "$(dirname "$0")/.."
RHUN=${1:-$(command -v rhun || echo "$HOME/.local/bin/rhun")}
[ $# -gt 0 ] && shift
names=${*:-basic numbers}
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
mkdir -p "$dir/bin"
cargo build --release --quiet
cp "target/release/$name" "$dir/bin/$name-$target"
cp extension.ini icon.png "$dir/"
fail=0
for test in $names; do
    t=$work/$test
    mkdir -p "$t/proj" "$t/config/rhun" "$t/state"
    printf 'fn main() {\n    println!("hello");\n}\n' > "$t/proj/sample.rs"
    [ "$test" = numbers ] && printf 'one\ntwo\n' > "$t/proj/notes.txt" && printf 'x = 1\n' > "$t/proj/a.py"
    printf '[files]\nrestore_session = false\nrestore_project = false\n[updates]\ncheck = false\n[git]\nenabled = false\n[ui]\nsidebar = false\nagents_panel = false\n' > "$t/config/rhun/config"
    sed "s|proj/|$t/proj/|" "tests/$test.txt" > "$t/$test.txt"
    HOME=$t XDG_CONFIG_HOME=$t/config XDG_STATE_HOME=$t/state \
        "$RHUN" "$t/proj" --headless 1000x600 --scale 1 --extensions "$work/exts" --script "$t/$test.txt" 2>&1 \
        | sed "s/pid=[1-9][0-9]*/pid=PID/; s|$t/||g" > "$t/out.txt"
    [ -n "$UPDATE" ] && cp "$t/out.txt" "tests/$test.expected"
    if cmp -s "$t/out.txt" "tests/$test.expected"; then
        echo "ok   $test"
    else
        echo "FAIL $test"; diff "tests/$test.expected" "$t/out.txt" || true; fail=1
    fi
done
exit $fail
