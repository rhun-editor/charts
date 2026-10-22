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

It needs rhun 0.18, and shows these parts of its extension protocol 1.1:

- **A table rhun keeps** (a retained view, features `view-tree` and `view-table`): the
  **Chart Numbers** tab of the bottom panel (Ctrl+J) holds the last chart's numbers, every
  language with its files, lines and share, or every line with its length. rhun draws, sorts and
  scrolls it; a click on a line's row opens the file there.
- **The keyboard in a chart** (`view-keys`): a click gives the chart the keyboard, Space switches
  the language chart between bars and a pie, and Ctrl+C copies its numbers, a label and a value a
  line, ready for a spreadsheet.
- **A status bar item** (`status-items`): the project's line count once counted, its largest
  languages in the tooltip; a click counts and charts them again.

## Permissions

It reads the files you have open and the files in your project. It changes nothing, has no
internet and runs no other programs.

## Build

```sh
cargo build --release
tests/run.sh            # headless, against the rhun on your PATH: tests/basic.txt, tests/numbers.txt
```

A tag `vX.Y.Z` equal to `version` in `extension.ini` builds Linux, macOS and Windows on their own
runners and publishes what Install from GitHub takes (section 11 of the spec).
