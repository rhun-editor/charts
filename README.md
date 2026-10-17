# Charts

What your project and the open file are made of, drawn in a tab of their own in
[rhun](https://rhun.app). A small Rust program built on
[rhun-ext](https://github.com/rhun-editor/extension-protocol/tree/main/rust/rhun-ext), and an
example of rhun's views: an extension draws shapes in theme colors, rhun renders them and sends
back the pointer.

Install it from rhun: the extensions button in the title bar, **Install from GitHub…**, then
`rhun-editor/charts`.

| Command | What it draws |
| --- | --- |
| Chart Lines by Language | the project's lines by language, as bars; click to switch to a pie |
| Chart Line Lengths of This File | the open file's line lengths; click a point to go to its line |

Hover over a bar, a wedge or a point for its value.

## Permissions

It reads the files you have open and the files in your project. It changes nothing, has no
internet and runs no other programs.

## Build

```sh
cargo build --release
tests/run.sh            # headless, against the rhun on your PATH
```

A tag `vX.Y.Z` equal to `version` in `extension.ini` builds Linux, macOS and Windows on their own
runners and publishes what Install from GitHub takes (section 11 of the spec).
