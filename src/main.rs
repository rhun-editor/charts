//! Charts: the project's lines by language and the open file's line lengths, drawn in tabs of
//! their own. Hovering shows the value under the pointer; clicking the language chart switches
//! between bars and a pie, clicking a line length opens the file at that line.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use rhun_ext::chart::{Chart, ChartKind, Drawn, Series};
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
        let _ = ext.status("Counting lines…");
        let mut counts: HashMap<&'static str, f64> = HashMap::new();
        let mut files = 0;
        walk(&root, &mut |path| {
            if let Some(lang) = language(path) {
                if let Some(n) = count_lines(path) {
                    *counts.entry(lang).or_default() += n as f64;
                    files += 1;
                }
            }
        });
        let _ = ext.status("");
        let mut rows: Vec<(&str, f64)> = counts.into_iter().collect();
        rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(b.0)));
        // eight at most: a chart has eight series colors
        if rows.len() > 8 {
            let rest: f64 = rows[7..].iter().map(|r| r.1).sum();
            rows.truncate(7);
            rows.push(("Other", rest));
        }
        let labels: Vec<&str> = rows.iter().map(|r| r.0).collect();
        let values: Vec<f64> = rows.iter().map(|r| r.1).collect();
        let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let title = format!("Lines by language in {name}, {files} files");
        let chart = Chart::bar(&title, &labels, vec![Series::new("lines", values)]).unit(" lines");
        self.open(ext, "Languages", Kind::Languages, chart);
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
        let chart = Chart::line(&title, &labels, vec![Series::new("characters", values)]).unit(" chars").item("Line");
        self.open(ext, &format!("Lengths: {file}"), Kind::Lengths { doc, path }, chart);
    }

    fn clicked(&mut self, ext: &Ext, view: View, x: f32, y: f32) {
        let Some(tab) = self.tabs.get_mut(&view) else { return };
        match &tab.kind {
            Kind::Languages => {
                tab.chart.kind = if tab.chart.kind == ChartKind::Bar { ChartKind::Pie } else { ChartKind::Bar };
                self.redraw(ext, view);
            }
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
            ViewEvent::Closed => {
                self.tabs.remove(&view);
            }
            _ => {}
        }
    }
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
    if let Err(e) = run(&[Feature::Views], Charts::default()) {
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
