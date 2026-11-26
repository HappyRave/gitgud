use std::collections::{BTreeMap, HashMap, HashSet};

use super::engine::{Side, Source};
use super::tree::{File, Map, Val};

/// Docs added or deleted on one side, gathered per GameObject (or prefab instance) so
/// an object and its components, children and parent link come or go together.
#[derive(Debug, Clone)]
pub(crate) struct Group {
    pub id: String,
    pub root: String,
    pub source: Source,
    pub kind: GroupKind,
    /// The side that deleted (Deleted) or added (Added) it; Yours for Both.
    pub side: Side,
    pub members: Vec<String>,
    /// Members the other side changed or added: what a deletion would drop.
    pub other_changes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GroupKind {
    Added,
    Deleted,
    /// Added on both sides under the same fileID, differently.
    Both,
}

/// `{fileID: 123, ...}` → `123`; `None` for `{fileID: 0}` and anything else.
pub(crate) fn ref_id(v: Option<&Val>) -> Option<String> {
    let Some(Val::Scalar(s)) = v else {
        return None;
    };
    ref_in(&s.head)
}

pub(crate) fn ref_in(text: &str) -> Option<String> {
    let at = text.find("fileID: ")? + "fileID: ".len();
    let id: String = text[at..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    (!id.is_empty() && id != "0").then_some(id)
}

/// Which object node (GameObject or prefab instance) each doc belongs to, and each node's parent.
pub(crate) struct Hier {
    owner: HashMap<String, String>,
    parent: HashMap<String, String>,
}

impl Hier {
    pub fn build(file: &File) -> Self {
        let mut owner = HashMap::new();
        let mut go_transform = HashMap::new();
        for doc in &file.docs {
            let node = match doc.class_id {
                1 | 1001 => Some(doc.id.clone()),
                _ => ref_id(doc.body.get("m_GameObject"))
                    .or_else(|| ref_id(doc.body.get("m_PrefabInstance"))),
            };
            owner.insert(doc.id.clone(), node.unwrap_or_else(|| doc.id.clone()));
            if matches!(doc.class_id, 4 | 224)
                && let Some(go) = ref_id(doc.body.get("m_GameObject"))
            {
                go_transform.insert(go, doc.id.clone());
            }
        }
        let by_id: HashMap<&str, &Map> =
            file.docs.iter().map(|d| (d.id.as_str(), &d.body)).collect();
        let mut parent = HashMap::new();
        for doc in &file.docs {
            let father = match doc.class_id {
                1 => go_transform
                    .get(&doc.id)
                    .and_then(|tr| by_id.get(tr.as_str()))
                    .and_then(|tr| ref_id(tr.get("m_Father"))),
                1001 => match doc.body.get("m_Modification") {
                    Some(Val::Map(m)) => ref_id(m.get("m_TransformParent")),
                    _ => None,
                },
                _ => None,
            };
            if let Some(f) = father.and_then(|f| owner.get(&f).cloned()) {
                parent.insert(doc.id.clone(), f);
            }
        }
        Self { owner, parent }
    }

    fn owner(&self, doc: &str) -> String {
        self.owner
            .get(doc)
            .cloned()
            .unwrap_or_else(|| doc.to_owned())
    }

    /// The topmost ancestor of `node` (itself included) whose doc is in `set`.
    fn top_in(&self, node: &str, set: &HashSet<&str>) -> String {
        let mut cur = node.to_owned();
        let mut seen = HashSet::new();
        while let Some(p) = self.parent.get(&cur) {
            if !set.contains(p.as_str()) || !seen.insert(p.clone()) {
                break;
            }
            cur = p.clone();
        }
        cur
    }
}

struct Sides<'a> {
    b: Docs<'a>,
    o: Docs<'a>,
    t: Docs<'a>,
}

type Docs<'a> = HashMap<&'a str, &'a Map>;

/// Docs `mine` deleted that `other` still has.
fn deleted<'a>(b: &Docs<'a>, mine: &Docs<'a>, other: &Docs<'a>) -> HashSet<&'a str> {
    b.keys()
        .filter(|id| !mine.contains_key(*id) && other.contains_key(*id))
        .copied()
        .collect()
}

/// Docs only `mine` has.
fn added<'a>(b: &Docs<'a>, mine: &Docs<'a>, other: &Docs<'a>) -> HashSet<&'a str> {
    mine.keys()
        .filter(|id| !b.contains_key(*id) && !other.contains_key(*id))
        .copied()
        .collect()
}

fn index(file: &File) -> HashMap<&str, &Map> {
    file.docs.iter().map(|d| (d.id.as_str(), &d.body)).collect()
}

pub(crate) fn groups(b: &File, o: &File, t: &File) -> Vec<Group> {
    let s = Sides {
        b: index(b),
        o: index(o),
        t: index(t),
    };
    let hiers = [Hier::build(b), Hier::build(o), Hier::build(t)];
    let (hb, ho, ht) = (&hiers[0], &hiers[1], &hiers[2]);

    let del = [deleted(&s.b, &s.o, &s.t), deleted(&s.b, &s.t, &s.o)];
    let add = [added(&s.b, &s.o, &s.t), added(&s.b, &s.t, &s.o)];

    // (side, kind, root) → members
    let mut found: BTreeMap<(usize, u8, String), Vec<String>> = BTreeMap::new();
    let mut joined: HashMap<(usize, String), usize> = HashMap::new();
    let del_root = |side: usize, doc: &str| -> String {
        let node = hb.owner(doc);
        if del[side].contains(node.as_str()) {
            hb.top_in(&node, &del[side])
        } else {
            doc.to_owned()
        }
    };
    for (side, deleted) in del.iter().enumerate() {
        for doc in deleted {
            found
                .entry((side, 0, del_root(side, doc)))
                .or_default()
                .push((*doc).to_owned());
        }
    }
    for (side, added) in add.iter().enumerate() {
        let other = 1 - side;
        let h = if side == 0 { ho } else { ht };
        for doc in added {
            let node = h.owner(doc);
            let anchor = if added.contains(node.as_str()) {
                let top = h.top_in(&node, added);
                match h.parent.get(&top) {
                    Some(p) if del[other].contains(p.as_str()) => Err(p.clone()),
                    _ => Ok(top),
                }
            } else if del[other].contains(node.as_str()) {
                Err(node)
            } else {
                Ok((*doc).to_owned())
            };
            match anchor {
                Ok(root) => found
                    .entry((side, 1, root))
                    .or_default()
                    .push((*doc).to_owned()),
                Err(deleted_node) => {
                    let root = del_root(other, &deleted_node);
                    found
                        .entry((other, 0, root.clone()))
                        .or_default()
                        .push((*doc).to_owned());
                    *joined.entry((other, root)).or_default() += 1;
                }
            }
        }
    }

    let mut out = Vec::new();
    for ((side, kind, root), mut members) in found {
        members.sort();
        let sd = if side == 0 { Side::Yours } else { Side::Theirs };
        let other_side = if side == 0 { &s.t } else { &s.o };
        let (kind, source, other_changes) = if kind == 0 {
            let edited = members
                .iter()
                .filter(|m| {
                    s.b.get(m.as_str())
                        .is_some_and(|bm| other_side.get(m.as_str()) != Some(bm))
                })
                .count();
            let changes = edited + joined.get(&(side, root.clone())).copied().unwrap_or(0);
            let source = if changes > 0 {
                Source::Clash
            } else if side == 0 {
                Source::Yours
            } else {
                Source::Theirs
            };
            (GroupKind::Deleted, source, changes)
        } else {
            let source = if side == 0 {
                Source::Yours
            } else {
                Source::Theirs
            };
            (GroupKind::Added, source, 0)
        };
        out.push(Group {
            id: format!("obj:{}:{root}", if side == 0 { "yours" } else { "theirs" }),
            root,
            source,
            kind,
            side: sd,
            members,
            other_changes,
        });
    }
    let mut both: Vec<&str> =
        s.o.iter()
            .filter(|(id, m)| !s.b.contains_key(*id) && s.t.get(*id).is_some_and(|tm| tm != *m))
            .map(|(id, _)| *id)
            .collect();
    both.sort_unstable();
    for id in both {
        out.push(Group {
            id: format!("obj:both:{id}"),
            root: id.to_owned(),
            source: Source::Clash,
            kind: GroupKind::Both,
            side: Side::Yours,
            members: vec![id.to_owned()],
            other_changes: 1,
        });
    }
    out
}
