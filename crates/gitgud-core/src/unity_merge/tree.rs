/// A value in a Unity YAML document, keeping the exact text it was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Val {
    Scalar(Scalar),
    Map(Map),
    Seq(Seq),
}

/// A scalar: `head` is the text after `key:` (with its leading space) or after `- `;
/// `cont` are the whole continuation lines of a long wrapped value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scalar {
    pub head: String,
    pub cont: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Map {
    pub indent: usize,
    pub entries: Vec<(String, Val)>,
}

/// A block sequence. Unity writes the dashes at the parent key's indent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Seq {
    pub indent: usize,
    pub items: Vec<Val>,
}

/// One `--- !u!<class> &<id>` document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Doc {
    pub header: String,
    pub id: String,
    pub class_id: u32,
    pub root: String,
    pub body: Map,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct File {
    pub preamble: Vec<String>,
    pub docs: Vec<Doc>,
    pub crlf: bool,
    pub final_newline: bool,
}

impl Map {
    pub fn get(&self, key: &str) -> Option<&Val> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

impl Val {
    /// The value's text on one line, for display.
    pub fn summary(&self) -> String {
        match self {
            Val::Scalar(s) => {
                let mut out = s.head.trim().to_owned();
                for c in &s.cont {
                    out.push(' ');
                    out.push_str(c.trim());
                }
                out
            }
            Val::Map(m) => {
                let parts: Vec<String> = m
                    .entries
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", v.summary()))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            Val::Seq(s) => {
                let n = s.items.len();
                format!("{n} item{}", if n == 1 { "" } else { "s" })
            }
        }
    }
}

/// Parses Unity YAML, refusing anything that wouldn't be written back byte for byte.
pub(crate) fn parse_exact(text: &str) -> Result<File, String> {
    let file = parse(text)?;
    if emit(&file) != text {
        return Err("its layout isn't the one Unity writes".into());
    }
    Ok(file)
}

const CRLF: &str = concat!('\r', '\n');

pub(crate) fn emit(file: &File) -> String {
    let eol = if file.crlf { CRLF } else { "\n" };
    let mut all: Vec<String> = file.preamble.clone();
    for doc in &file.docs {
        all.push(doc.header.clone());
        all.push(doc.root.clone());
        emit_map(&doc.body, false, &mut all);
    }
    let mut out = String::with_capacity(all.iter().map(|l| l.len() + 2).sum());
    for (i, l) in all.iter().enumerate() {
        if i > 0 {
            out.push_str(eol);
        }
        out.push_str(l);
    }
    if file.final_newline {
        out.push_str(eol);
    }
    out
}

/// The lines `val` takes when written under `key`.
pub(crate) fn emit_entry(key: &str, val: &Val, indent: usize, out: &mut Vec<String>) {
    let pad = " ".repeat(indent);
    match val {
        Val::Scalar(s) => {
            out.push(format!("{pad}{key}:{}", s.head));
            out.extend(s.cont.iter().cloned());
        }
        Val::Map(m) => {
            out.push(format!("{pad}{key}:"));
            emit_map(m, false, out);
        }
        Val::Seq(s) => {
            out.push(format!("{pad}{key}:"));
            emit_seq(s, out);
        }
    }
}

fn emit_map(m: &Map, dash_first: bool, out: &mut Vec<String>) {
    for (i, (k, v)) in m.entries.iter().enumerate() {
        if i == 0 && dash_first {
            let mut first = Vec::new();
            emit_entry(k, v, m.indent, &mut first);
            let mut line = first.remove(0);
            line.replace_range(m.indent - 2..m.indent, "- ");
            out.push(line);
            out.extend(first);
        } else {
            emit_entry(k, v, m.indent, out);
        }
    }
}

fn emit_seq(s: &Seq, out: &mut Vec<String>) {
    let pad = " ".repeat(s.indent);
    for item in &s.items {
        match item {
            Val::Scalar(sc) => {
                out.push(format!("{pad}- {}", sc.head));
                out.extend(sc.cont.iter().cloned());
            }
            Val::Map(m) => emit_map(m, true, out),
            Val::Seq(_) => unreachable!("the parser never builds a list directly inside a list"),
        }
    }
}

fn parse(text: &str) -> Result<File, String> {
    let crlf = text.contains("\r\n");
    let final_newline = text.ends_with('\n');
    let mut lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    if final_newline {
        lines.pop();
    }
    let mut p = Parser {
        lines: &lines,
        pos: 0,
    };
    let mut preamble = Vec::new();
    while p.pos < lines.len() && !lines[p.pos].starts_with("--- ") {
        preamble.push(lines[p.pos].to_owned());
        p.pos += 1;
    }
    let mut docs = Vec::new();
    while p.pos < lines.len() {
        let header = lines[p.pos];
        let (class_id, id) = parse_header(header)
            .ok_or_else(|| format!("line {}: not a document header", p.pos + 1))?;
        p.pos += 1;
        let root = *lines
            .get(p.pos)
            .ok_or_else(|| format!("line {}: document without a body", p.pos + 1))?;
        if root.starts_with(' ') || !root.ends_with(':') {
            return Err(format!("line {}: expected the object's type", p.pos + 1));
        }
        p.pos += 1;
        let body = if p.pos < lines.len() && indent(lines[p.pos]) == 2 {
            p.map(2, None)?
        } else {
            Map {
                indent: 2,
                entries: Vec::new(),
            }
        };
        if p.pos < lines.len() && !lines[p.pos].starts_with("--- ") {
            return Err(format!("line {}: unexpected indentation", p.pos + 1));
        }
        docs.push(Doc {
            header: header.to_owned(),
            id,
            class_id,
            root: root.to_owned(),
            body,
        });
    }
    Ok(File {
        preamble,
        docs,
        crlf,
        final_newline,
    })
}

/// `--- !u!<class> &<id>[ stripped]` → (class, id).
pub(crate) fn parse_header(line: &str) -> Option<(u32, String)> {
    let rest = line.strip_prefix("--- !u!")?;
    let (class, rest) = rest.split_once(" &")?;
    let id = rest.split(' ').next()?;
    if id.is_empty() {
        return None;
    }
    Some((class.parse().ok()?, id.to_owned()))
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_dash(content: &str) -> bool {
    content.starts_with("- ") || content == "-"
}

/// `key: value` or `key:` → (key, rest after the colon).
fn split_entry(content: &str) -> Option<(&str, &str)> {
    if let Some(i) = content.find(": ") {
        return Some((&content[..i], &content[i + 1..]));
    }
    content.strip_suffix(':').map(|k| (k, ""))
}

fn looks_like_entry(s: &str) -> bool {
    let Some(first) = s.chars().next() else {
        return false;
    };
    if "{['\"&*!|>%@`".contains(first) {
        return false;
    }
    split_entry(s).is_some_and(|(k, _)| !k.is_empty() && !k.contains(' '))
}

struct Parser<'a> {
    lines: &'a [&'a str],
    pos: usize,
}

impl Parser<'_> {
    fn map(&mut self, n: usize, first: Option<&str>) -> Result<Map, String> {
        let mut entries = Vec::new();
        if let Some(content) = first {
            entries.push(self.entry(n, content)?);
        }
        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            if indent(line) != n || line.trim().is_empty() {
                break;
            }
            let content = &line[n..];
            if is_dash(content) {
                break;
            }
            self.pos += 1;
            entries.push(self.entry(n, content)?);
        }
        Ok(Map { indent: n, entries })
    }

    fn entry(&mut self, n: usize, content: &str) -> Result<(String, Val), String> {
        let (key, rest) = split_entry(content)
            .ok_or_else(|| format!("line {}: expected `key: value`", self.pos))?;
        let val = if rest.is_empty() {
            match self.lines.get(self.pos) {
                Some(next) if !next.trim().is_empty() => {
                    let ind = indent(next);
                    let c = &next[ind..];
                    if ind == n && is_dash(c) {
                        Val::Seq(self.seq(n)?)
                    } else if ind > n && is_dash(c) {
                        Val::Seq(self.seq(ind)?)
                    } else if ind > n {
                        Val::Map(self.map(ind, None)?)
                    } else {
                        empty()
                    }
                }
                _ => empty(),
            }
        } else {
            Val::Scalar(Scalar {
                head: rest.to_owned(),
                cont: self.cont(n, rest),
            })
        };
        Ok((key.to_owned(), val))
    }

    fn seq(&mut self, n: usize) -> Result<Seq, String> {
        let mut items = Vec::new();
        while self.pos < self.lines.len() {
            let line = self.lines[self.pos];
            if indent(line) != n || !is_dash(&line[n..]) {
                break;
            }
            let content = &line[n..];
            if content == "-" {
                return Err(format!("line {}: nested list", self.pos + 1));
            }
            self.pos += 1;
            let after = &content[2..];
            if looks_like_entry(after) {
                items.push(Val::Map(self.map(n + 2, Some(after))?));
            } else {
                items.push(Val::Scalar(Scalar {
                    head: after.to_owned(),
                    cont: self.cont(n, after),
                }));
            }
        }
        Ok(Seq { indent: n, items })
    }

    /// Lines of a wrapped scalar: up to its closing quote when `head` opens one, else
    /// deeper than `n`, blank lines included when more follow.
    fn cont(&mut self, n: usize, head: &str) -> Vec<String> {
        let mut out = Vec::new();
        let trimmed = head.trim_start();
        if let Some(q) = trimmed.chars().next().filter(|c| *c == '\'' || *c == '"') {
            let mut open = still_open(&trimmed[1..], q, true);
            while open && self.pos < self.lines.len() {
                let line = self.lines[self.pos];
                out.push(line.to_owned());
                self.pos += 1;
                open = still_open(line, q, true);
            }
            return out;
        }
        let mut look = self.pos;
        while look < self.lines.len() {
            let line = self.lines[look];
            if line.trim().is_empty() {
                look += 1;
                continue;
            }
            if indent(line) <= n {
                break;
            }
            out.extend(self.lines[self.pos..=look].iter().map(|l| (*l).to_owned()));
            look += 1;
            self.pos = look;
        }
        out
    }
}

/// Whether a quoted scalar is still open after `text`, read from inside the quotes.
pub(crate) fn still_open(text: &str, quote: char, inside: bool) -> bool {
    let mut inside = inside;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if !inside {
            continue;
        }
        if quote == '"' && c == '\\' {
            chars.next();
        } else if c == quote {
            if quote == '\'' && chars.peek() == Some(&'\'') {
                chars.next();
            } else {
                inside = false;
            }
        }
    }
    inside
}

fn empty() -> Val {
    Val::Scalar(Scalar {
        head: String::new(),
        cont: Vec::new(),
    })
}
