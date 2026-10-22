//! Charts: the project's lines by language and the open file's line lengths, drawn in tabs of
//! their own. Hovering shows the value under the pointer; clicking the language chart switches
//! between bars and a pie, clicking a line length opens the file at that line.
//!
//! Protocol 1.1 adds: the numbers of the last chart in a table rhun keeps in the bottom panel
//! (features `view-tree` and `view-table`), the keyboard in a chart (`view-keys`: Space switches
//! bars and pie, Ctrl+C copies the numbers) and the project's line count in the status bar
//! (`status-items`).

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use rhun_ext::chart::{Chart, ChartKind, Drawn, Series};
use rhun_ext::commands::StatusItem;
use rhun_ext::views::{Column, Node, Patch, State, ViewOpen};
use rhun_ext::{run, Doc, Ext, Feature, Handler, Metrics, Opened, Pos, Range, View, ViewEvent};

/// What a view shows.
enum Kind {
    Languages,
    /// The document the lengths were taken from, and its path (`-` when untitled).
    Lengths { doc: Doc, path: String },
}

struct Tab {
    kind: Kind,
    chart: Chart,
    size: (f32, f32),
    drawn: Drawn,
}

#[derive(Default)]
struct Charts {
    project: Option<PathBuf>,
    metrics: Metrics,
    /// What `opened` said about each document: its path.
    paths: HashMap<Doc, String>,
    tabs: HashMap<View, Tab>,
    /// The table of numbers (the manifest's `[views] numbers`), once rhun opened its instance.
    table: Option<View>,
    /// What it shows: the last chart's numbers.
    numbers: Option<Patch>,
}

impl Charts {
    fn redraw(&mut self, ext: &Ext, view: View) {
        let Some(tab) = self.tabs.get_mut(&view) else { return };
        if tab.size.0 <= 0.0 || tab.size.1 <= 0.0 {
            return; // rhun says the size before the view is shown
        }
        tab.drawn = tab.chart.draw(tab.size.0, tab.size.1, &self.metrics);
        let _ = ext.draw(view, &tab.drawn.picture);
    }

    fn open(&mut self, ext: &Ext, title: &str, kind: Kind, chart: Chart) {
        match ext.view(title) {
            Ok(view) => {
                self.tabs.insert(view, Tab { kind, chart, size: (0.0, 0.0), drawn: Drawn::default() });
            }
            Err(e) => {
                let _ = ext.toast(&format!("Charts: {}", e.text));
            }
        }
    }

    fn languages(&mut self, ext: &Ext) {
        let Some(root) = self.project.clone() else {
            let _ = ext.toast("Charts: open a folder to chart its languages");
            return;
        };
        self.status(ext, "$(sync) Counting lines…", "", false);
        // lines and files by language
        let mut counts: HashMap<&'static str, (f64, usize)> = HashMap::new();
        let mut files = 0;
        walk(&root, &mut |path| {
            if let Some(lang) = language(path) {
                if let Some(n) = count_lines(path) {
                    let c = counts.entry(lang).or_default();
                    c.0 += n as f64;
                    c.1 += 1;
                    files += 1;
                }
            }
        });
        let mut rows: Vec<(&str, f64, usize)> = counts.into_iter().map(|(l, (n, f))| (l, n, f)).collect();
        rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(b.0)));
        let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let total: f64 = rows.iter().map(|r| r.1).sum();
        let top: Vec<String> = rows.iter().take(3).map(|r| format!("{} {}", r.0, r.1)).collect();
        self.status(ext, &format!("$(list) {total} lines"), &format!("{name}: {}\nClick to chart them again.", top.join(", ")), true);
        self.numbers(ext, languages_table(&name, &rows, total));
        // eight at most: a chart has eight series colors
        if rows.len() > 8 {
            let rest = rows[7..].iter().fold((0.0, 0), |a, r| (a.0 + r.1, a.1 + r.2));
            rows.truncate(7);
            rows.push(("Other", rest.0, rest.1));
        }
        let labels: Vec<&str> = rows.iter().map(|r| r.0).collect();
        let values: Vec<f64> = rows.iter().map(|r| r.1).collect();
        let title = format!("Lines by language in {name}, {files} files");
        let chart = Chart::bar(&title, &labels, vec![Series::new("lines", values)]).unit(" lines");
        self.open(ext, "Languages", Kind::Languages, chart);
    }

    /// The status bar's item (feature `status-items`), or v1's status text: the line count, a
    /// click charting the languages again (`command`).
    fn status(&self, ext: &Ext, text: &str, tooltip: &str, command: bool) {
        if !ext.has(Feature::StatusItems) {
            let _ = ext.status(if command { "" } else { "Counting lines…" });
            return;
        }
        let item = StatusItem {
            name: "lines".into(),
            align: "left".into(),
            text: text.into(),
            tooltip: tooltip.into(),
            command: if command { "languages".into() } else { String::new() },
            ..StatusItem::default()
        };
        if let Err(e) = ext.set_status_item(&item) {
            eprintln!("charts: status item: {}", e.text);
        }
    }

    /// The table shows these numbers now (and when its instance opens, if it has not yet).
    fn numbers(&mut self, ext: &Ext, patch: Patch) {
        if let Some(view) = self.table {
            if let Err(e) = ext.patch(view, &patch) {
                eprintln!("charts: table: {}", e.text);
            }
        }
        self.numbers = Some(patch);
    }

    fn lengths(&mut self, ext: &Ext, doc: Doc) {
        if doc == 0 {
            let _ = ext.toast("Charts: open a file first");
            return;
        }
        let Ok((text, _)) = ext.text(doc) else { return };
        let text = String::from_utf8_lossy(&text);
        let values: Vec<f64> = text.lines().map(|l| l.chars().count() as f64).collect();
        let numbers: Vec<String> = (1..=values.len()).map(|n| n.to_string()).collect();
        let labels: Vec<&str> = numbers.iter().map(String::as_str).collect();
        let path = self.paths.get(&doc).cloned().unwrap_or_else(|| "-".into());
        let file = Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or("untitled".into());
        let title = format!("Line lengths of {file}, click a point to go to its line");
        self.numbers(ext, lengths_table(&file, &path, doc, &values));
        let chart = Chart::line(&title, &labels, vec![Series::new("characters", values)]).unit(" chars").item("Line");
        self.open(ext, &format!("Lengths: {file}"), Kind::Lengths { doc, path }, chart);
    }

    /// The language chart as a pie, or as bars again.
    fn toggle(&mut self, ext: &Ext, view: View) {
        let Some(tab) = self.tabs.get_mut(&view) else { return };
        if matches!(tab.kind, Kind::Languages) {
            tab.chart.kind = if tab.chart.kind == ChartKind::Bar { ChartKind::Pie } else { ChartKind::Bar };
            self.redraw(ext, view);
        }
    }

    fn clicked(&mut self, ext: &Ext, view: View, x: f32, y: f32) {
        let Some(tab) = self.tabs.get_mut(&view) else { return };
        match &tab.kind {
            Kind::Languages => self.toggle(ext, view),
            Kind::Lengths { doc, path } => {
                let Some(hit) = tab.drawn.hit(x, y) else { return };
                let line = hit.index as u32 + 1;
                if path != "-" {
                    let _ = ext.open(path, Pos::new(line, 1));
                } else {
                    let _ = ext.select(*doc, Range::new(Pos::new(line, 1), Pos::new(line, 1)));
                }
            }
        }
    }
}

impl Handler for Charts {
    fn ready(&mut self, _ext: &Ext, project: Option<&str>) {
        self.project = project.map(PathBuf::from);
    }

    fn event(&mut self, _ext: &Ext, ev: &rhun_ext::Event) {
        if let Some(o) = Opened::parse(ev) {
            self.paths.insert(o.doc, o.path);
        } else if ev.name == "closed" {
            self.paths.remove(&ev.num(0));
        }
    }

    fn command(&mut self, ext: &Ext, name: &str, doc: Doc) {
        if !ext.has(Feature::Views) {
            let _ = ext.toast("Charts needs a rhun with views (0.18.0 or later)");
            return;
        }
        match name {
            "languages" => self.languages(ext),
            "lengths" => self.lengths(ext, doc),
            _ => {}
        }
    }

    fn view(&mut self, ext: &Ext, view: View, ev: ViewEvent) {
        match ev {
            ViewEvent::Size { width, height } => {
                if let Some(t) = self.tabs.get_mut(&view) {
                    t.size = (width, height);
                }
                self.redraw(ext, view);
            }
            ViewEvent::Metrics(m) => {
                self.metrics = m;
                self.redraw(ext, view);
            }
            ViewEvent::Hover { x, y } => {
                if let Some(t) = self.tabs.get_mut(&view) {
                    // the tooltip follows the pointer over an item; elsewhere only a change redraws
                    let before = t.drawn.hovered;
                    t.chart.pointer = Some((x, y));
                    if before.is_none() && t.drawn.hit(x, y).is_none() {
                        return;
                    }
                }
                self.redraw(ext, view);
            }
            ViewEvent::Leave => {
                if let Some(t) = self.tabs.get_mut(&view) {
                    t.chart.pointer = None;
                }
                self.redraw(ext, view);
            }
            ViewEvent::Click { x, y } => self.clicked(ext, view, x, y),
            // with view-keys the chart takes the keyboard when clicked: Space switches bars and pie
            ViewEvent::Key { mods, name } if mods == "-" && name == "Space" => self.toggle(ext, view),
            ViewEvent::Closed => {
                self.tabs.remove(&view);
            }
            _ => {}
        }
    }

    /// The table's instance: what it shows (an empty state until a chart is drawn).
    fn view_open(&mut self, ext: &Ext, open: &ViewOpen) {
        if open.name != "numbers" {
            return;
        }
        self.table = Some(open.view);
        let patch = self.numbers.clone().unwrap_or_else(|| {
            let mut p = Patch::new();
            p.state(State::Empty("Chart something to see its numbers here".into(), vec![]));
            p
        });
        let _ = ext.patch(open.view, &patch);
    }

    /// Ctrl+C in a chart (view-keys): its numbers, a line each with a tab between label and value,
    /// ready to paste into a spreadsheet.
    fn view_copy(&mut self, _ext: &Ext, view: View, _cut: bool) -> Option<String> {
        let tab = self.tabs.get(&view)?;
        let values = &tab.chart.series.first()?.values;
        let mut out = String::new();
        for (label, value) in tab.chart.labels.iter().zip(values) {
            out.push_str(&format!("{label}\t{value}\n"));
        }
        Some(out)
    }
}

/// The table of the language chart: every language (not only the eight the chart shows), sorted
/// by lines; rhun sorts again by a click on a header.
fn languages_table(name: &str, rows: &[(&str, f64, usize)], total: f64) -> Patch {
    let mut p = Patch::new();
    p.title(&format!("Languages in {name}"));
    p.columns(&[
        Column::new("Language").sortable(),
        Column::new("Files").right().min(6).sortable(),
        Column::new("Lines").right().min(8).sortable(),
        Column::new("Share").right().min(7).sortable(),
    ]);
    let nodes: Vec<Node> = rows
        .iter()
        .map(|(lang, lines, files)| {
            let share = if total > 0.0 { lines * 100.0 / total } else { 0.0 };
            Node::new(lang, lang).cell(&files.to_string()).cell(&lines.to_string()).cell(&format!("{share:.1}%"))
        })
        .collect();
    p.kids(b"", &nodes, false);
    p.message(&format!("{total} lines"));
    p
}

/// The table of the line-length chart: a row per line (the first 20000), and a click on one opens
/// the file there.
fn lengths_table(file: &str, path: &str, doc: Doc, values: &[f64]) -> Patch {
    const MAX: usize = 20_000;
    let mut p = Patch::new();
    p.title(&format!("Line lengths of {file}"));
    p.columns(&[Column::new("Line").right().min(6).sortable(), Column::new("Characters").right().min(10).sortable()]);
    let target = if path == "-" { format!("#{doc}") } else { path.to_string() };
    let nodes: Vec<Node> = values
        .iter()
        .take(MAX)
        .enumerate()
        .map(|(i, v)| {
            let line = (i + 1).to_string();
            Node::new(&line, &line).cell(&v.to_string()).open(&target, i as u32 + 1, 1)
        })
        .collect();
    p.kids(b"", &nodes, false);
    p.message(&if values.len() > MAX { format!("The first {MAX} of {} lines", values.len()) } else { String::new() });
    p
}

/// Every file under `root`, leaving out hidden folders and the usual build output.
fn walk(root: &Path, f: &mut dyn FnMut(&Path)) {
    let mut stack = vec![root.to_path_buf()];
    let mut seen = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || matches!(name.as_ref(), "target" | "node_modules" | "build" | "dist" | "vendor" | "out") {
                continue;
            }
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                stack.push(e.path());
            } else if kind.is_file() {
                seen += 1;
                if seen > 50_000 {
                    return; // a folder this large is not a project
                }
                f(&e.path());
            }
        }
    }
}

/// Lines in a text file (one without a NUL in its first 8 KB), up to 4 MB.
fn count_lines(path: &Path) -> Option<usize> {
    let meta = fs::metadata(path).ok()?;
    if meta.len() > 4 << 20 {
        return None;
    }
    let mut data = Vec::new();
    fs::File::open(path).ok()?.read_to_end(&mut data).ok()?;
    if data.iter().take(8192).any(|&b| b == 0) {
        return None;
    }
    let n = data.iter().filter(|&&b| b == b'\n').count();
    Some(if data.last().is_some_and(|&b| b != b'\n') { n + 1 } else { n })
}

fn language(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "rs" => "Rust",
        "go" => "Go",
        "c" | "h" => "C",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "C++",
        "zig" => "Zig",
        "s" | "asm" | "inc" => "Assembly",
        "py" => "Python",
        "js" | "mjs" | "cjs" | "jsx" => "JavaScript",
        "ts" | "tsx" => "TypeScript",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "swift" => "Swift",
        "rb" => "Ruby",
        "php" => "PHP",
        "lua" => "Lua",
        "cs" => "C#",
        "sh" | "bash" | "zsh" => "Shell",
        "ps1" => "PowerShell",
        "md" | "markdown" => "Markdown",
        "html" | "htm" => "HTML",
        "css" | "scss" => "CSS",
        "json" => "JSON",
        "toml" => "TOML",
        "yml" | "yaml" => "YAML",
        "ini" => "INI",
        "sql" => "SQL",
        "txt" => "Text",
        _ => return None,
    })
}

fn main() {
    if let Err(e) = run(&[Feature::Views, Feature::ViewTree, Feature::ViewTable, Feature::ViewKeys, Feature::StatusItems], Charts::default()) {
        eprintln!("charts: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_by_extension() {
        assert_eq!(language(Path::new("a/b.RS")), Some("Rust"));
        assert_eq!(language(Path::new("x.hpp")), Some("C++"));
        assert_eq!(language(Path::new("Makefile")), None);
    }

    #[test]
    fn lines_are_counted_with_or_without_a_last_newline() {
        let dir = std::env::temp_dir().join(format!("charts-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.rs"), "a\nb\n").unwrap();
        fs::write(dir.join("b.rs"), "a\nb").unwrap();
        fs::write(dir.join("c.bin"), b"\0\0\n").unwrap();
        assert_eq!(count_lines(&dir.join("a.rs")), Some(2));
        assert_eq!(count_lines(&dir.join("b.rs")), Some(2));
        assert_eq!(count_lines(&dir.join("c.bin")), None);
        let mut seen = Vec::new();
        walk(&dir, &mut |p| seen.push(p.file_name().unwrap().to_string_lossy().into_owned()));
        seen.sort();
        assert_eq!(seen, ["a.rs", "b.rs", "c.bin"]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
