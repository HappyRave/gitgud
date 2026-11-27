use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::objects::{Group, GroupKind, ref_in};
use super::tree::{Doc, File, Map, Scalar, Seq, Val, parse_header};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Yours,
    Theirs,
    Clash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RowKind {
    Value,
    Object,
    Order,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Yours,
    Theirs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Decision {
    Yours,
    Theirs,
    Custom(String),
}

/// A change as the engine sees it; `super` turns ids and raw values into names.
#[derive(Debug, Clone)]
pub(crate) struct RawRow {
    pub id: String,
    pub kind: RowKind,
    pub source: Source,
    pub file_id: String,
    pub label: String,
    pub base: Option<String>,
    pub yours: Option<String>,
    pub theirs: Option<String>,
    pub editable: bool,
    /// Order rows: the list read yours-first / theirs-first (raw items).
    pub yours_order: Vec<String>,
    pub theirs_order: Vec<String>,
    pub group: Option<Group>,
}

pub(crate) struct Engine<'a> {
    picks: &'a HashMap<String, Decision>,
    pub rows: Vec<RawRow>,
    pub unresolved: usize,
    /// Grouped docs: which side's version is kept (absent = dropped).
    version: HashMap<String, Option<Side>>,
}

const KEYED_ORDERED: [&str; 2] = ["m_Children", "m_Component"];
const KEYED_UNORDERED: [&str; 5] = [
    "m_Modifications",
    "m_RemovedComponents",
    "m_RemovedGameObjects",
    "m_AddedGameObjects",
    "m_AddedComponents",
];

impl<'a> Engine<'a> {
    pub fn new(picks: &'a HashMap<String, Decision>) -> Self {
        Self {
            picks,
            rows: Vec::new(),
            unresolved: 0,
            version: HashMap::new(),
        }
    }

    fn decide(&mut self, row: RawRow) -> Decision {
        let d = match self.picks.get(&row.id) {
            Some(d) => d.clone(),
            None => match row.source {
                Source::Yours => Decision::Yours,
                Source::Theirs => Decision::Theirs,
                Source::Clash => {
                    self.unresolved += 1;
                    Decision::Yours
                }
            },
        };
        self.rows.push(row);
        d
    }

    pub fn merge(&mut self, b: &File, o: &File, t: &File, groups: Vec<Group>) -> File {
        let bd: HashMap<&str, &Doc> = b.docs.iter().map(|d| (d.id.as_str(), d)).collect();
        let td: HashMap<&str, &Doc> = t.docs.iter().map(|d| (d.id.as_str(), d)).collect();
        let od: HashSet<&str> = o.docs.iter().map(|d| d.id.as_str()).collect();
        for g in groups {
            let row = RawRow {
                id: g.id.clone(),
                kind: RowKind::Object,
                source: g.source,
                file_id: g.root.clone(),
                label: "(object)".into(),
                base: None,
                yours: None,
                theirs: None,
                editable: false,
                yours_order: Vec::new(),
                theirs_order: Vec::new(),
                group: Some(g.clone()),
            };
            let kept = match self.decide(row) {
                Decision::Theirs => Side::Theirs,
                _ => Side::Yours,
            };
            for m in &g.members {
                let has = match kept {
                    Side::Yours => od.contains(m.as_str()),
                    Side::Theirs => td.contains_key(m.as_str()),
                };
                self.version.insert(m.clone(), has.then_some(kept));
            }
            if g.kind == GroupKind::Both {
                self.version.insert(g.root.clone(), Some(kept));
            }
        }

        let mut docs: Vec<Doc> = Vec::new();
        for doc in &o.docs {
            match self.version.get(&doc.id) {
                Some(Some(Side::Yours)) => docs.push(doc.clone()),
                Some(Some(Side::Theirs)) => docs.push((*td[doc.id.as_str()]).clone()),
                Some(None) => {}
                None => {
                    if let Some(theirs) = td.get(doc.id.as_str()) {
                        docs.push(self.doc(bd.get(doc.id.as_str()).copied(), doc, theirs));
                    }
                }
            }
        }
        // Docs only theirs has, after the doc they follow there.
        let kept: HashSet<String> = docs.iter().map(|d| d.id.clone()).collect();
        let mut after: HashMap<Option<String>, Vec<Doc>> = HashMap::new();
        let mut prev: Option<String> = None;
        for doc in &t.docs {
            let id = doc.id.as_str();
            if !od.contains(id) && self.version.get(id) == Some(&Some(Side::Theirs)) {
                after.entry(prev.clone()).or_default().push(doc.clone());
                prev = Some(doc.id.clone());
            } else if kept.contains(id) {
                prev = Some(doc.id.clone());
            }
        }
        if !after.is_empty() {
            let mut out =
                Vec::with_capacity(docs.len() + after.values().map(Vec::len).sum::<usize>());
            let first = after.remove(&None).unwrap_or_default();
            for d in first.into_iter().chain(docs) {
                let mut stack = vec![d];
                while let Some(d) = stack.pop() {
                    let follows = after.remove(&Some(d.id.clone()));
                    out.push(d);
                    if let Some(follows) = follows {
                        stack.extend(follows.into_iter().rev());
                    }
                }
            }
            docs = out;
        }

        let preamble = if o.preamble == t.preamble || t.preamble == b.preamble {
            o.preamble.clone()
        } else if o.preamble == b.preamble {
            t.preamble.clone()
        } else {
            let pv = |f: &File| {
                Val::Scalar(Scalar {
                    head: f.preamble.join("\n"),
                    cont: Vec::new(),
                })
            };
            match self.leaf(
                "",
                "(file header)",
                Some(&pv(b)),
                Some(&pv(o)),
                Some(&pv(t)),
            ) {
                Some(Val::Scalar(s)) if s.head == t.preamble.join("\n") => t.preamble.clone(),
                _ => o.preamble.clone(),
            }
        };
        File {
            preamble,
            docs,
            crlf: o.crlf,
            final_newline: o.final_newline,
        }
    }

    fn doc(&mut self, b: Option<&Doc>, o: &Doc, t: &Doc) -> Doc {
        let fid = o.id.as_str();
        let header = self.line(fid, "(header)", b.map(|d| &d.header), &o.header, &t.header);
        let root = self.line(fid, "(type)", b.map(|d| &d.root), &o.root, &t.root);
        let empty = Map {
            indent: 2,
            entries: Vec::new(),
        };
        let body = self.map(
            fid,
            "",
            "",
            Some(b.map_or(&empty, |d| &d.body)),
            &o.body,
            &t.body,
        );
        let class_id = parse_header(&header).map_or(o.class_id, |(c, _)| c);
        Doc {
            header,
            id: o.id.clone(),
            class_id,
            root,
            body,
        }
    }

    fn line(
        &mut self,
        fid: &str,
        what: &str,
        b: Option<&String>,
        o: &String,
        t: &String,
    ) -> String {
        if o == t {
            return o.clone();
        }
        let v = |s: &String| {
            Val::Scalar(Scalar {
                head: s.clone(),
                cont: Vec::new(),
            })
        };
        let picked = self.leaf(fid, what, b.map(v).as_ref(), Some(&v(o)), Some(&v(t)));
        match picked {
            Some(Val::Scalar(s)) if &s.head == t => t.clone(),
            _ => o.clone(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn val(
        &mut self,
        fid: &str,
        path: &str,
        label: &str,
        b: Option<&Val>,
        o: Option<&Val>,
        t: Option<&Val>,
        indent: usize,
    ) -> Option<Val> {
        if o == t {
            return o.cloned();
        }
        match (b, o, t) {
            (None | Some(Val::Map(_)), Some(Val::Map(om)), Some(Val::Map(tm))) => {
                let bm = match b {
                    Some(Val::Map(m)) => Some(m),
                    _ => None,
                };
                return Some(Val::Map(self.map(fid, path, label, bm, om, tm)));
            }
            (_, Some(ov), Some(tv)) if as_seq(ov).is_some() && as_seq(tv).is_some() => {
                let bs = b.and_then(as_seq);
                if b.is_none() || bs.is_some() {
                    let (os, ts) = (
                        as_seq(ov).unwrap_or_default(),
                        as_seq(tv).unwrap_or_default(),
                    );
                    return self.seq(fid, path, label, b, bs, (ov, os), (tv, ts), indent);
                }
            }
            (Some(Val::Scalar(bs)), Some(Val::Scalar(os)), Some(Val::Scalar(ts))) => {
                if let Some(v) = self.flow(fid, path, label, bs, os, ts) {
                    return Some(v);
                }
            }
            _ => {}
        }
        self.leaf2(fid, path, label, b, o, t)
    }

    fn leaf(
        &mut self,
        fid: &str,
        label: &str,
        b: Option<&Val>,
        o: Option<&Val>,
        t: Option<&Val>,
    ) -> Option<Val> {
        self.leaf2(fid, label, label, b, o, t)
    }

    fn leaf2(
        &mut self,
        fid: &str,
        path: &str,
        label: &str,
        b: Option<&Val>,
        o: Option<&Val>,
        t: Option<&Val>,
    ) -> Option<Val> {
        if o == t {
            return o.cloned();
        }
        let source = if o == b {
            Source::Theirs
        } else if t == b {
            Source::Yours
        } else {
            Source::Clash
        };
        let single = |v: Option<&Val>| matches!(v, Some(Val::Scalar(s)) if s.cont.is_empty());
        let row = RawRow {
            id: format!("{fid}/{path}"),
            kind: RowKind::Value,
            source,
            file_id: fid.to_owned(),
            label: label.to_owned(),
            base: b.map(Val::summary),
            yours: o.map(Val::summary),
            theirs: t.map(Val::summary),
            // "(header)", "(type)", "(file header)": a document's frame, picked whole.
            editable: single(o) && single(t) && !label.starts_with('('),
            yours_order: Vec::new(),
            theirs_order: Vec::new(),
            group: None,
        };
        match self.decide(row) {
            Decision::Yours => o.cloned(),
            Decision::Theirs => t.cloned(),
            Decision::Custom(v) => {
                let template = o.or(t);
                let lead = match template {
                    Some(Val::Scalar(s)) if s.head.starts_with(' ') => " ",
                    _ => "",
                };
                Some(Val::Scalar(Scalar {
                    head: format!("{lead}{v}"),
                    cont: Vec::new(),
                }))
            }
        }
    }

    fn map(
        &mut self,
        fid: &str,
        path: &str,
        label: &str,
        b: Option<&Map>,
        o: &Map,
        t: &Map,
    ) -> Map {
        let mut keys: Vec<&str> = o.entries.iter().map(|(k, _)| k.as_str()).collect();
        let mut prev: Option<&str> = None;
        for (k, _) in &t.entries {
            if !keys.contains(&k.as_str()) {
                let at = prev
                    .and_then(|p| keys.iter().position(|x| *x == p))
                    .map_or(0, |i| i + 1);
                keys.insert(at, k);
            }
            prev = Some(k);
        }
        let mut entries = Vec::new();
        for k in keys {
            let p = join(path, k);
            let l = join(label, k);
            let v = self.val(
                fid,
                &p,
                &l,
                b.and_then(|m| m.get(k)),
                o.get(k),
                t.get(k),
                o.indent,
            );
            if let Some(v) = v {
                entries.push((k.to_owned(), v));
            }
        }
        Map {
            indent: o.indent,
            entries,
        }
    }

    /// `{x: 0, y: 1}` merged per key when all three have the same keys.
    fn flow(
        &mut self,
        fid: &str,
        path: &str,
        label: &str,
        b: &Scalar,
        o: &Scalar,
        t: &Scalar,
    ) -> Option<Val> {
        let (bl, bf) = flow_parts(b)?;
        let (_, of) = flow_parts(o)?;
        let (_, tf) = flow_parts(t)?;
        let keys = |f: &[(String, String)]| f.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
        if keys(&bf) != keys(&of) || keys(&of) != keys(&tf) {
            return None;
        }
        let mut parts = Vec::new();
        for (i, (k, _)) in of.iter().enumerate() {
            let sv = |f: &[(String, String)]| {
                Val::Scalar(Scalar {
                    head: f[i].1.clone(),
                    cont: Vec::new(),
                })
            };
            let v = self.leaf2(
                fid,
                &join(path, k),
                &join(label, k),
                Some(&sv(&bf)),
                Some(&sv(&of)),
                Some(&sv(&tf)),
            );
            let text = match v {
                Some(Val::Scalar(s)) => s.head,
                _ => of[i].1.clone(),
            };
            parts.push(format!("{k}: {}", text.trim()));
        }
        Some(Val::Scalar(Scalar {
            head: format!("{bl}{{{}}}", parts.join(", ")),
            cont: Vec::new(),
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn seq(
        &mut self,
        fid: &str,
        path: &str,
        label: &str,
        b_raw: Option<&Val>,
        b: Option<Seq>,
        (o_raw, o): (&Val, Seq),
        (t_raw, t): (&Val, Seq),
        indent: usize,
    ) -> Option<Val> {
        let key = path.rsplit('.').next().unwrap_or(path);
        let keyed = KEYED_ORDERED.contains(&key) || KEYED_UNORDERED.contains(&key);
        let seq_indent = [&o, &t]
            .iter()
            .find(|s| !s.items.is_empty())
            .map_or(indent, |s| s.indent);
        if keyed {
            let empty = Seq {
                indent: seq_indent,
                items: Vec::new(),
            };
            let bs = b.as_ref().unwrap_or(&empty);
            if let Some(v) = self.keyed(
                fid,
                path,
                label,
                KEYED_ORDERED.contains(&key),
                bs,
                &o,
                &t,
                seq_indent,
            ) {
                return v;
            }
        } else if let Some(bs) = &b
            && bs.items.len() == o.items.len()
            && o.items.len() == t.items.len()
            && !o.items.is_empty()
        {
            let mut items = Vec::new();
            for i in 0..o.items.len() {
                let p = format!("{path}[{i}]");
                let l = format!("{label}[{i}]");
                if let Some(v) = self.val(
                    fid,
                    &p,
                    &l,
                    Some(&bs.items[i]),
                    Some(&o.items[i]),
                    Some(&t.items[i]),
                    seq_indent + 2,
                ) {
                    items.push(v);
                }
            }
            return Some(list_val(items, seq_indent));
        }
        self.leaf2(fid, path, label, b_raw, Some(o_raw), Some(t_raw))
    }

    /// Lists whose items are identified by content (references) or by target and
    /// property (prefab overrides). `None` when an item repeats: then it's merged whole.
    #[allow(clippy::too_many_arguments)]
    fn keyed(
        &mut self,
        fid: &str,
        path: &str,
        label: &str,
        ordered: bool,
        b: &Seq,
        o: &Seq,
        t: &Seq,
        indent: usize,
    ) -> Option<Option<Val>> {
        let (bl, bk) = keys_of(b)?;
        let (ol, ok) = keys_of(o)?;
        let (tl, tk) = keys_of(t)?;

        let mut all: Vec<&String> = Vec::new();
        let mut seen = HashSet::new();
        for k in ol.iter().chain(&tl).chain(&bl) {
            if seen.insert(k) {
                all.push(k);
            }
        }
        let mut included: HashMap<String, Val> = HashMap::new();
        for k in all {
            let (bi, oi, ti) = (bk.get(k).copied(), ok.get(k).copied(), tk.get(k).copied());
            if let Some(doc) = item_doc(oi.or(ti).or(bi)).filter(|d| self.version.contains_key(d)) {
                if self.version[&doc].is_some()
                    && let Some(v) = oi.or(ti).or(bi)
                {
                    included.insert(k.clone(), v.clone());
                }
                continue;
            }
            let item_label = item_label(label, oi.or(ti).or(bi));
            let p = format!("{path}[{k}]");
            let v = match (oi, ti) {
                (Some(_), Some(_)) => self.val(fid, &p, &item_label, bi, oi, ti, indent + 2),
                _ => self.leaf2(fid, &p, &item_label, bi, oi, ti),
            };
            if let Some(v) = v {
                included.insert(k.clone(), v);
            }
        }

        let inc = |list: &[String]| {
            list.iter()
                .filter(|k| included.contains_key(*k))
                .cloned()
                .collect::<Vec<_>>()
        };
        let (o_inc, t_inc) = (inc(&ol), inc(&tl));
        let t_set: HashSet<&String> = t_inc.iter().collect();
        let o_set: HashSet<&String> = o_inc.iter().collect();
        let oc: Vec<String> = o_inc
            .iter()
            .filter(|k| t_set.contains(k))
            .cloned()
            .collect();
        let tc: Vec<String> = t_inc
            .iter()
            .filter(|k| o_set.contains(k))
            .cloned()
            .collect();
        let common: HashSet<&String> = oc.iter().collect();
        let bc: Vec<String> = bl.iter().filter(|k| common.contains(k)).cloned().collect();
        let o_add: Vec<String> = o_inc
            .iter()
            .filter(|k| !t_set.contains(k))
            .cloned()
            .collect();
        let t_add: Vec<String> = t_inc
            .iter()
            .filter(|k| !o_set.contains(k))
            .cloned()
            .collect();
        let anchors = |list: &[String], adds: &[String]| -> Vec<(Option<String>, String)> {
            adds.iter()
                .map(|a| {
                    let at = list.iter().position(|x| x == a).unwrap_or(0);
                    let anchor = list[..at]
                        .iter()
                        .rev()
                        .find(|x| common.contains(x))
                        .cloned();
                    (anchor, a.clone())
                })
                .collect()
        };
        let (oa, ta) = (anchors(&ol, &o_add), anchors(&tl, &t_add));
        let order_clash = oc != tc && bc != oc && bc != tc;
        let shared_anchor = oa.iter().any(|(a, _)| ta.iter().any(|(b, _)| a == b));

        let assemble = |first: Side| -> Vec<String> {
            let base_order = if order_clash && first == Side::Theirs {
                &tc
            } else if oc == tc || bc == tc {
                &oc
            } else if bc == oc {
                &tc
            } else {
                &oc
            };
            let (fa, sa) = if first == Side::Yours {
                (&oa, &ta)
            } else {
                (&ta, &oa)
            };
            let at = |anchor: Option<&String>, out: &mut Vec<String>| {
                for list in [fa, sa] {
                    out.extend(
                        list.iter()
                            .filter(|(a, _)| a.as_ref() == anchor)
                            .map(|(_, k)| k.clone()),
                    );
                }
            };
            let mut out = Vec::new();
            at(None, &mut out);
            for c in base_order {
                out.push(c.clone());
                at(Some(c), &mut out);
            }
            out
        };

        let first = if ordered && (order_clash || shared_anchor) {
            let row = RawRow {
                id: format!("{fid}/{path}#order"),
                kind: RowKind::Order,
                source: Source::Clash,
                file_id: fid.to_owned(),
                label: format!("{label} (order)"),
                base: None,
                yours: None,
                theirs: None,
                editable: false,
                yours_order: assemble(Side::Yours),
                theirs_order: assemble(Side::Theirs),
                group: None,
            };
            match self.decide(row) {
                Decision::Theirs => Side::Theirs,
                _ => Side::Yours,
            }
        } else {
            Side::Yours
        };
        let items: Vec<Val> = assemble(first)
            .into_iter()
            .filter_map(|k| included.remove(&k))
            .collect();
        Some(Some(list_val(items, indent)))
    }
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

/// A list, or an empty list written as `[]`.
fn as_seq(v: &Val) -> Option<Seq> {
    match v {
        Val::Seq(s) => Some(s.clone()),
        Val::Scalar(s) if s.head.trim() == "[]" && s.cont.is_empty() => Some(Seq::default()),
        _ => None,
    }
}

fn list_val(items: Vec<Val>, indent: usize) -> Val {
    if items.is_empty() {
        Val::Scalar(Scalar {
            head: " []".into(),
            cont: Vec::new(),
        })
    } else {
        Val::Seq(Seq { indent, items })
    }
}

type Keys<'a> = (Vec<String>, HashMap<String, &'a Val>);

fn keys_of(s: &Seq) -> Option<Keys<'_>> {
    let mut list = Vec::new();
    let mut map = HashMap::new();
    for item in &s.items {
        let k = item_key(item);
        if map.insert(k.clone(), item).is_some() {
            return None;
        }
        list.push(k);
    }
    Some((list, map))
}

fn item_key(item: &Val) -> String {
    if let Val::Map(m) = item
        && let (Some(target), Some(prop)) = (m.get("target"), m.get("propertyPath"))
    {
        return format!("{}|{}", target.summary(), prop.summary());
    }
    item.summary()
}

/// The local object a list item points at: a child, a component, an added object.
fn item_doc(item: Option<&Val>) -> Option<String> {
    match item? {
        Val::Scalar(s) => ref_in(&s.head),
        Val::Map(m) => ["addedObject", "component"]
            .iter()
            .find_map(|k| m.get(k))
            .and_then(|v| match v {
                Val::Scalar(s) => ref_in(&s.head),
                _ => None,
            }),
        Val::Seq(_) => None,
    }
}

/// A prefab override's row reads as the overridden property.
fn item_label(list_label: &str, item: Option<&Val>) -> String {
    if let Some(Val::Map(m)) = item
        && let Some(p) = m.get("propertyPath")
    {
        return p.summary();
    }
    list_label.to_owned()
}

/// `" {x: 0, y: 1}"` → (`" "`, [(x, 0), (y, 1)]); `None` for references and nested values.
fn flow_parts(s: &Scalar) -> Option<(String, Vec<(String, String)>)> {
    if !s.cont.is_empty() {
        return None;
    }
    let trimmed = s.head.trim_start();
    let lead = s.head[..s.head.len() - trimmed.len()].to_owned();
    let inner = trimmed.strip_prefix('{')?.strip_suffix('}')?;
    if inner.contains(['{', '[', '"', '\'']) {
        return None;
    }
    let mut parts = Vec::new();
    for p in inner.split(", ") {
        let (k, v) = p.split_once(": ")?;
        if matches!(k, "fileID" | "guid" | "type") {
            return None;
        }
        parts.push((k.to_owned(), v.to_owned()));
    }
    Some((lead, parts))
}
