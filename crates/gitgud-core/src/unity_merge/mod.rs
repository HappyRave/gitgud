mod draft;
mod engine;
mod inputs;
mod objects;
#[cfg(test)]
mod tests;
pub(crate) mod tree;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use draft::draft_merge;
use engine::{Decision, Engine, RawRow};
pub use engine::{RowKind, Source};
pub use inputs::{MergeOffer, MissingObject, conflict_merge, conflict_merged, offer};
use objects::{Group, GroupKind, ref_in};

use crate::repo::Error;
use crate::unity_yaml::{
    GuidIndex, UnityIntegrityIssue, UnityScene, check_file_integrity, diff_unity_yaml,
    format_value_display,
};

/// One change in a property merge (R9): a value, an object added or deleted, or a list order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRow {
    pub id: String,
    pub kind: RowKind,
    pub source: Source,
    pub file_id: String,
    pub object_path: String,
    pub component: String,
    pub property: String,
    pub base: Option<String>,
    pub yours: Option<String>,
    pub theirs: Option<String>,
    /// The values as written in the file, for editing (E).
    pub yours_raw: Option<String>,
    pub theirs_raw: Option<String>,
    pub editable: bool,
    /// Object rows: what each side did to it. Order rows: the list read each way.
    pub yours_detail: Vec<String>,
    pub theirs_detail: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyMerge {
    pub path: String,
    /// Clashes first, then every one-sided change in file order.
    pub rows: Vec<MergeRow>,
    pub yours: usize,
    pub theirs: usize,
    pub clashes: usize,
    pub objects: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PickChoice {
    Yours,
    Theirs,
    Custom,
}

/// The user's choice for one row; rows without one keep the side that changed them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowPick {
    pub id: String,
    pub choice: PickChoice,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergedFile {
    pub text: String,
    /// Clashes still without a pick (the text keeps yours for them).
    pub unresolved: usize,
    pub issues: Vec<UnityIntegrityIssue>,
}

/// A merge's state without its text: what's left to pick, what the check found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeCheck {
    pub unresolved: usize,
    pub issues: Vec<UnityIntegrityIssue>,
}

struct Parsed {
    b: tree::File,
    o: tree::File,
    t: tree::File,
}

fn parse(path: &str, base: &str, ours: &str, theirs: &str) -> Result<Parsed, Error> {
    let one = |text: &str, which: &str| {
        tree::parse_exact(text)
            .map_err(|e| Error::Refused(format!("{path}: can't merge {which} by property: {e}")))
    };
    Ok(Parsed {
        b: one(base, "the common version")?,
        o: one(ours, "yours")?,
        t: one(theirs, "theirs")?,
    })
}

/// A value typed into the table, as one YAML scalar Unity reads back as typed:
/// single-quoted (as Unity writes) when it would otherwise read as something
/// else (`Boss: Phase 2`, `#2`, `a, b` inside `{x: …}`). A value typed already
/// quoted stays as it is; numbers like `-1.5` stay plain.
fn yaml_scalar(typed: &str) -> String {
    let quoted = |q: char| {
        typed.len() >= 2
            && typed.starts_with(q)
            && typed.ends_with(q)
            && !tree::still_open(&typed[1..], q, true)
    };
    if quoted('\'') || quoted('"') {
        return typed.to_owned();
    }
    let indicator_then_space = |c: char| {
        typed
            .strip_prefix(c)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    };
    let needs_quotes = typed.contains(": ")
        || typed.contains(" #")
        || typed.ends_with(':')
        || typed.contains(['{', '}', '[', ']', ','])
        || typed.starts_with(['#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`'])
        || ['-', '?', ':'].into_iter().any(indicator_then_space)
        || typed != typed.trim();
    if needs_quotes {
        format!("'{}'", typed.replace('\'', "''"))
    } else {
        typed.to_owned()
    }
}

fn run(p: &Parsed, picks: &HashMap<String, Decision>) -> (tree::File, Vec<RawRow>, usize) {
    let groups = objects::groups(&p.b, &p.o, &p.t);
    let mut engine = Engine::new(picks);
    let merged = engine.merge(&p.b, &p.o, &p.t, groups);
    (merged, engine.rows, engine.unresolved)
}

/// Every change between the common version and each side of a conflicted Unity YAML file.
///
/// # Errors
/// [`Error::Refused`] when a version isn't Unity YAML gitgud can write back exactly.
pub fn analyze(
    path: &str,
    base: &str,
    ours: &str,
    theirs: &str,
    guids: &GuidIndex,
) -> Result<PropertyMerge, Error> {
    let parsed = parse(path, base, ours, theirs)?;
    let (_, raw, _) = run(&parsed, &HashMap::new());
    let names = Names::new(path, base, ours, theirs, guids);
    let mut rows: Vec<MergeRow> = raw.iter().map(|r| names.row(r)).collect();
    rows.sort_by_key(|r| r.source != Source::Clash);
    let count = |s: Source| rows.iter().filter(|r| r.source == s).count();
    Ok(PropertyMerge {
        path: path.to_owned(),
        yours: count(Source::Yours),
        theirs: count(Source::Theirs),
        clashes: count(Source::Clash),
        objects: rows.iter().filter(|r| r.kind == RowKind::Object).count(),
        rows,
    })
}

/// The merged text for `picks`, with the integrity check run on it.
///
/// # Errors
/// [`Error::Refused`] for a pick that names no row (the file changed), or an edit on a
/// row that can't be edited.
pub fn merge(
    path: &str,
    base: &str,
    ours: &str,
    theirs: &str,
    picks: &[RowPick],
    guids: &GuidIndex,
) -> Result<MergedFile, Error> {
    let parsed = parse(path, base, ours, theirs)?;
    let (_, rows, _) = run(&parsed, &HashMap::new());
    let by_id: HashMap<&str, &RawRow> = rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut decisions = HashMap::new();
    for p in picks {
        let Some(row) = by_id.get(p.id.as_str()) else {
            return Err(Error::Refused(format!(
                "{path} changed since the merge was read: reopen it"
            )));
        };
        let d = match (p.choice, &p.value) {
            (PickChoice::Yours, _) => Decision::Yours,
            (PickChoice::Theirs, _) => Decision::Theirs,
            (PickChoice::Custom, Some(v)) if row.editable && !v.contains('\n') => {
                Decision::Custom(yaml_scalar(v))
            }
            (PickChoice::Custom, _) => {
                return Err(Error::Refused(format!(
                    "{}: this value can't be edited",
                    row.label
                )));
            }
        };
        decisions.insert(p.id.clone(), d);
    }
    let (file, _, unresolved) = run(&parsed, &decisions);
    let text = tree::emit(&file);
    let issues = check_file_integrity(path, &text, guids);
    Ok(MergedFile {
        text,
        unresolved,
        issues,
    })
}

/// Turns the engine's ids and raw values into the names the table shows.
struct Names<'a> {
    path: &'a str,
    texts: [&'a str; 3],
    scenes: [UnityScene; 3],
    guids: &'a GuidIndex,
}

#[derive(Clone, Copy)]
enum At {
    Base = 0,
    Yours = 1,
    Theirs = 2,
}

impl<'a> Names<'a> {
    fn new(
        path: &'a str,
        base: &'a str,
        ours: &'a str,
        theirs: &'a str,
        guids: &'a GuidIndex,
    ) -> Self {
        Self {
            path,
            texts: [base, ours, theirs],
            scenes: [
                UnityScene::parse(base),
                UnityScene::parse(ours),
                UnityScene::parse(theirs),
            ],
            guids,
        }
    }

    fn scene_with(&self, id: &str, prefer: At) -> &UnityScene {
        let order = match prefer {
            At::Base => [0, 1, 2],
            At::Yours => [1, 2, 0],
            At::Theirs => [2, 1, 0],
        };
        order
            .iter()
            .map(|i| &self.scenes[*i])
            .find(|s| s.docs.contains_key(id))
            .unwrap_or(&self.scenes[prefer as usize])
    }

    fn display(&self, raw: Option<&String>, at: At) -> Option<String> {
        raw.map(|v| format_value_display(v, &self.scenes[at as usize], self.guids))
    }

    /// A list item or reference by the name of what it points at.
    fn item(&self, raw: &str) -> String {
        match ref_in(raw) {
            Some(id) if !raw.contains("guid:") => {
                let scene = self.scene_with(&id, At::Yours);
                if scene.docs.contains_key(&id) {
                    let (path, comp) = scene.labels_for(&id, self.guids);
                    if scene.docs[&id].class_id == 1
                        || matches!(comp.as_str(), "Transform" | "RectTransform")
                    {
                        return path;
                    }
                    return format!("{path} ({comp})");
                }
                raw.to_owned()
            }
            _ => format_value_display(raw, &self.scenes[1], self.guids),
        }
    }

    fn row(&self, r: &RawRow) -> MergeRow {
        let mut row = MergeRow {
            id: r.id.clone(),
            kind: r.kind,
            source: r.source,
            file_id: r.file_id.clone(),
            object_path: String::new(),
            component: String::new(),
            property: r.label.clone(),
            base: None,
            yours: None,
            theirs: None,
            yours_raw: None,
            theirs_raw: None,
            editable: r.editable,
            yours_detail: Vec::new(),
            theirs_detail: Vec::new(),
        };
        let prefer = match &r.group {
            Some(g) if g.kind == GroupKind::Deleted => At::Base,
            Some(g) if g.side == engine::Side::Theirs => At::Theirs,
            _ => At::Yours,
        };
        if r.file_id.is_empty() {
            row.object_path = "(file)".into();
        } else {
            let (path, comp) = self
                .scene_with(&r.file_id, prefer)
                .labels_for(&r.file_id, self.guids);
            row.object_path = path;
            row.component = comp;
        }
        match r.kind {
            RowKind::Value => {
                if r.id.contains("m_Modifications[") {
                    for field in [".value", ".objectReference"] {
                        if let Some(p) = row.property.strip_suffix(field) {
                            row.property = p.to_owned();
                        }
                    }
                }
                let is_ref_list = r.id.contains("[{");
                if is_ref_list {
                    row.base = r.base.as_deref().map(|v| self.item(v));
                    row.yours = r.yours.as_deref().map(|v| self.item(v));
                    row.theirs = r.theirs.as_deref().map(|v| self.item(v));
                } else {
                    row.base = self.display(r.base.as_ref(), At::Base);
                    row.yours = self.display(r.yours.as_ref(), At::Yours);
                    row.theirs = self.display(r.theirs.as_ref(), At::Theirs);
                }
                row.yours_raw.clone_from(&r.yours);
                row.theirs_raw.clone_from(&r.theirs);
            }
            RowKind::Order => {
                row.yours = Some("yours first".into());
                row.theirs = Some("theirs first".into());
                row.yours_detail = r.yours_order.iter().map(|v| self.item(v)).collect();
                row.theirs_detail = r.theirs_order.iter().map(|v| self.item(v)).collect();
            }
            RowKind::Object => {
                if let Some(g) = &r.group {
                    self.object(g, &mut row);
                }
            }
        }
        row
    }

    fn object(&self, g: &Group, row: &mut MergeRow) {
        let at = match g.side {
            engine::Side::Yours => At::Yours,
            engine::Side::Theirs => At::Theirs,
        };
        let counted = |scene: &UnityScene| {
            let objects = g
                .members
                .iter()
                .filter(|m| {
                    scene
                        .docs
                        .get(*m)
                        .is_some_and(|d| matches!(d.class_id, 1 | 1001))
                })
                .count();
            let comps = g
                .members
                .iter()
                .filter(|m| scene.docs.contains_key(*m))
                .count()
                - objects;
            let plural = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
            match (objects, comps) {
                (0, c) => plural(c, "component"),
                (o, 0) => plural(o, "object"),
                (o, c) => format!("{}, {}", plural(o, "object"), plural(c, "component")),
            }
        };
        let (mine, other) = match g.side {
            engine::Side::Yours => (&mut row.yours, &mut row.theirs),
            engine::Side::Theirs => (&mut row.theirs, &mut row.yours),
        };
        match g.kind {
            GroupKind::Added => {
                *mine = Some(format!("added: {}", counted(&self.scenes[at as usize])));
            }
            GroupKind::Deleted => {
                row.base = Some(counted(&self.scenes[0]));
                *mine = Some("deleted".into());
                *other = Some(if g.other_changes == 0 {
                    "unchanged".into()
                } else {
                    format!(
                        "kept, {} change{}",
                        g.other_changes,
                        if g.other_changes == 1 { "" } else { "s" }
                    )
                });
                if g.other_changes > 0 {
                    let other_at = match g.side {
                        engine::Side::Yours => At::Theirs,
                        engine::Side::Theirs => At::Yours,
                    };
                    let detail = self.changes(g, other_at);
                    match g.side {
                        engine::Side::Yours => row.theirs_detail = detail,
                        engine::Side::Theirs => row.yours_detail = detail,
                    }
                }
            }
            GroupKind::Both => {
                row.yours = Some("added".into());
                row.theirs = Some("added differently".into());
            }
        }
    }

    /// What side `at` changed or added among a deleted object's docs.
    fn changes(&self, g: &Group, at: At) -> Vec<String> {
        let diff = diff_unity_yaml(
            self.path,
            Some(self.texts[0]),
            Some(self.texts[at as usize]),
            self.guids,
        );
        let mut out = Vec::new();
        for obj in diff
            .objects
            .iter()
            .filter(|o| g.members.contains(&o.file_id))
        {
            match obj.kind.as_str() {
                "added" if obj.component == "GameObject" => {
                    out.push(format!("+ {}", obj.object_path))
                }
                "added" => out.push(format!("+ {} › {}", obj.object_path, obj.component)),
                "modified" => {
                    for p in &obj.properties {
                        out.push(format!(
                            "{} › {}: {} → {}",
                            obj.component,
                            p.property,
                            p.before_display.as_deref().unwrap_or("—"),
                            p.after_display.as_deref().unwrap_or("—"),
                        ));
                    }
                }
                _ => {}
            }
        }
        out
    }
}
