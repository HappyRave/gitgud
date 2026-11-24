use std::collections::{BTreeSet, HashMap};

use serde::Serialize;
use serde_json::Value;

/// Rows sent for one table; a bigger table is cut with a note.
pub const MAX_ROWS: usize = 20_000;

/// One grid read from a file: CSV / TSV, a JSON list, or a ScriptableObject list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Table {
    /// "rows" for a CSV, the field name for a JSON or ScriptableObject list.
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// One table compared across two versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDiff {
    pub name: String,
    /// Columns of both versions, the newer one's order first.
    pub columns: Vec<String>,
    pub added_columns: Vec<String>,
    pub removed_columns: Vec<String>,
    /// The column rows were matched by, or `None` when they were matched by position
    /// (no usable key: its values repeat).
    pub key: Option<String>,
    pub rows: Vec<RowDiff>,
    pub row_count: usize,
    pub changed: usize,
    pub changed_cells: usize,
    pub added: usize,
    pub removed: usize,
    /// The table had more than [`MAX_ROWS`] rows; only the first were compared.
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RowState {
    Same,
    Changed,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RowDiff {
    pub state: RowState,
    /// One per column: `[old, new]`, `None` where that version has no such cell.
    pub cells: Vec<[Option<String>; 2]>,
}

/// Reads the tables in a CSV, TSV or JSON file.
///
/// # Errors
/// A plain-words reason when the file can't be read as one.
pub fn read_tables(bytes: &[u8], path: &str) -> Result<Vec<Table>, String> {
    let text = decode_text(bytes);
    let ext = path
        .rsplit_once('.')
        .map_or("", |(_, e)| e)
        .to_ascii_lowercase();
    match ext.as_str() {
        "json" => json_tables(&text),
        "tsv" => csv_table(&text, b'\t'),
        _ => csv_table(&text, sniff_delimiter(&text)),
    }
}

/// UTF-8 (BOM or not) or UTF-16 with a BOM, as Excel exports it.
fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |le: bool| {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| {
                if le {
                    u16::from_le_bytes(c)
                } else {
                    u16::from_be_bytes(c)
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, ..] => utf16(true),
        [0xFE, 0xFF, ..] => utf16(false),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// `,`, `;` or tab: whichever splits the header line most often outside quotes.
fn sniff_delimiter(text: &str) -> u8 {
    let first = text.lines().next().unwrap_or("");
    let count = |d: char| {
        let mut quoted = false;
        first
            .chars()
            .filter(|&c| {
                if c == '"' {
                    quoted = !quoted;
                }
                c == d && !quoted
            })
            .count()
    };
    [(',', b','), (';', b';'), ('\t', b'\t')]
        .into_iter()
        .max_by_key(|(c, _)| count(*c))
        .map_or(b',', |(_, d)| d)
}

fn csv_table(text: &str, delimiter: u8) -> Result<Vec<Table>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .has_headers(true)
        .from_reader(text.as_bytes());
    let columns: Vec<String> = reader
        .headers()
        .map_err(|e| format!("not a table this viewer can read ({e})"))?
        .iter()
        .map(str::to_owned)
        .collect();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| format!("not a table this viewer can read ({e})"))?;
        rows.push(record.iter().map(str::to_owned).collect());
    }
    Ok(vec![Table {
        name: "rows".into(),
        columns,
        rows,
    }])
}

/// A top-level list becomes one table; a top-level object gives one table per
/// list of records it holds, and a "fields" table of everything else.
fn json_tables(text: &str) -> Result<Vec<Table>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("not JSON this viewer can read ({e})"))?;
    let mut tables = Vec::new();
    match &value {
        Value::Array(items) => tables.push(records("rows", items.iter().map(Record::Json))),
        Value::Object(map) => {
            let mut fields = Vec::new();
            for (k, v) in map {
                match v {
                    Value::Array(items)
                        if !items.is_empty() && items.iter().all(Value::is_object) =>
                    {
                        tables.push(records(k, items.iter().map(Record::Json)));
                    }
                    _ => flatten_json(k, v, &mut fields),
                }
            }
            if !fields.is_empty() {
                tables.insert(0, key_values(fields));
            }
        }
        other => {
            let mut fields = Vec::new();
            flatten_json("value", other, &mut fields);
            tables.push(key_values(fields));
        }
    }
    Ok(tables)
}

/// A record of a list: its fields flattened to `a.b` / `a[2].c` paths.
pub(crate) enum Record<'a> {
    Json(&'a Value),
    Flat(Vec<(String, String)>),
}

/// A table with the union of the records' columns, in first-seen order.
pub(crate) fn records<'a>(name: &str, items: impl Iterator<Item = Record<'a>>) -> Table {
    let mut columns: Vec<String> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut flat_rows = Vec::new();
    for item in items {
        let fields = match item {
            Record::Json(Value::Object(map)) => {
                let mut f = Vec::new();
                for (k, v) in map {
                    flatten_json(k, v, &mut f);
                }
                f
            }
            Record::Json(v) => {
                let mut f = Vec::new();
                flatten_json("value", v, &mut f);
                f
            }
            Record::Flat(f) => f,
        };
        for (k, _) in &fields {
            if !index.contains_key(k) {
                index.insert(k.clone(), columns.len());
                columns.push(k.clone());
            }
        }
        flat_rows.push(fields);
    }
    let rows = flat_rows
        .into_iter()
        .map(|fields| {
            let mut row = vec![String::new(); columns.len()];
            for (k, v) in fields {
                row[index[&k]] = v;
            }
            row
        })
        .collect();
    Table {
        name: name.to_owned(),
        columns,
        rows,
    }
}

pub(crate) fn key_values(fields: Vec<(String, String)>) -> Table {
    Table {
        name: "fields".into(),
        columns: vec!["field".into(), "value".into()],
        rows: fields.into_iter().map(|(k, v)| vec![k, v]).collect(),
    }
}

fn flatten_json(prefix: &str, v: &Value, out: &mut Vec<(String, String)>) {
    match v {
        Value::Object(map) if !map.is_empty() => {
            for (k, v) in map {
                flatten_json(&format!("{prefix}.{k}"), v, out);
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (i, v) in items.iter().enumerate() {
                flatten_json(&format!("{prefix}[{i}]"), v, out);
            }
        }
        Value::String(s) => out.push((prefix.to_owned(), s.clone())),
        Value::Null => out.push((prefix.to_owned(), "null".into())),
        other => out.push((prefix.to_owned(), other.to_string())),
    }
}

/// The column rows are matched by: `id`, else `key`, else `name`, else the first;
/// `None` when its values repeat in either version.
fn key_column(columns: &[String], sides: &[&Table]) -> Option<usize> {
    let pick = ["id", "key", "name", "field"]
        .iter()
        .find_map(|want| columns.iter().position(|c| c.eq_ignore_ascii_case(want)))
        .or((!columns.is_empty()).then_some(0))?;
    let name = &columns[pick];
    for t in sides {
        let at = t.columns.iter().position(|c| c == name)?;
        let mut seen = BTreeSet::new();
        if !t
            .rows
            .iter()
            .all(|r| seen.insert(r.get(at).map_or("", String::as_str)))
        {
            return None;
        }
    }
    Some(pick)
}

/// Compares the tables of two versions by name; a lone version compares as all "same".
#[must_use]
pub fn diff_tables(before: Option<&[Table]>, after: &[Table]) -> Vec<TableDiff> {
    let empty = Table {
        name: String::new(),
        columns: Vec::new(),
        rows: Vec::new(),
    };
    let mut out: Vec<TableDiff> = after
        .iter()
        .map(|a| {
            let b = before.map(|b| b.iter().find(|t| t.name == a.name).unwrap_or(&empty));
            diff_table(b, a)
        })
        .collect();
    for b in before.unwrap_or_default() {
        if !after.iter().any(|a| a.name == b.name) {
            let mut d = diff_table(
                Some(b),
                &Table {
                    name: b.name.clone(),
                    ..empty.clone()
                },
            );
            d.name = b.name.clone();
            out.push(d);
        }
    }
    out
}

fn diff_table(before: Option<&Table>, after: &Table) -> TableDiff {
    let truncated = after.rows.len() > MAX_ROWS || before.is_some_and(|b| b.rows.len() > MAX_ROWS);
    let cut = |t: &Table| Table {
        name: t.name.clone(),
        columns: t.columns.clone(),
        rows: t.rows.iter().take(MAX_ROWS).cloned().collect(),
    };
    let after = cut(after);
    let before = before.map(cut);
    let mut columns = after.columns.clone();
    let mut removed_columns = Vec::new();
    let mut added_columns = Vec::new();
    if let Some(b) = &before {
        for c in &b.columns {
            if !columns.contains(c) {
                columns.push(c.clone());
                removed_columns.push(c.clone());
            }
        }
        added_columns = after
            .columns
            .iter()
            .filter(|c| !b.columns.contains(c) && !b.columns.is_empty())
            .cloned()
            .collect();
    }
    let cell = |t: &Table, row: &[String], col: &str| {
        t.columns
            .iter()
            .position(|c| c == col)
            .map(|i| row.get(i).cloned().unwrap_or_default())
    };
    let Some(b) = before else {
        let rows = after
            .rows
            .iter()
            .map(|r| RowDiff {
                state: RowState::Same,
                cells: columns.iter().map(|c| [None, cell(&after, r, c)]).collect(),
            })
            .collect();
        return TableDiff {
            name: after.name.clone(),
            row_count: after.rows.len(),
            columns,
            added_columns,
            removed_columns,
            key: None,
            rows,
            changed: 0,
            changed_cells: 0,
            added: 0,
            removed: 0,
            truncated,
        };
    };
    let key = key_column(&columns, &[&b, &after]);
    let key_of = |t: &Table, r: &[String]| key.and_then(|k| cell(t, r, &columns[k]));
    let mut b_index: HashMap<String, usize> = HashMap::new();
    if key.is_some() {
        for (i, r) in b.rows.iter().enumerate() {
            b_index.insert(key_of(&b, r).unwrap_or_default(), i);
        }
    }
    let mut used = vec![false; b.rows.len()];
    let mut rows = Vec::new();
    let (mut changed, mut changed_cells, mut added) = (0, 0, 0);
    for (i, r) in after.rows.iter().enumerate() {
        let matched = if key.is_some() {
            b_index.get(&key_of(&after, r).unwrap_or_default()).copied()
        } else {
            (i < b.rows.len()).then_some(i)
        };
        let Some(bi) = matched else {
            added += 1;
            rows.push(RowDiff {
                state: RowState::Added,
                cells: columns.iter().map(|c| [None, cell(&after, r, c)]).collect(),
            });
            continue;
        };
        used[bi] = true;
        let cells: Vec<[Option<String>; 2]> = columns
            .iter()
            .map(|c| [cell(&b, &b.rows[bi], c), cell(&after, r, c)])
            .collect();
        let diffs = cells
            .iter()
            .filter(|[o, n]| o.is_some() && n.is_some() && o != n)
            .count();
        if diffs > 0 {
            changed += 1;
            changed_cells += diffs;
        }
        rows.push(RowDiff {
            state: if diffs > 0 {
                RowState::Changed
            } else {
                RowState::Same
            },
            cells,
        });
    }
    let mut removed = 0;
    for (bi, r) in b.rows.iter().enumerate() {
        if !used[bi] {
            removed += 1;
            rows.push(RowDiff {
                state: RowState::Removed,
                cells: columns.iter().map(|c| [cell(&b, r, c), None]).collect(),
            });
        }
    }
    TableDiff {
        name: after.name.clone(),
        row_count: after.rows.len(),
        key: key.map(|k| columns[k].clone()),
        columns,
        added_columns,
        removed_columns,
        rows,
        changed,
        changed_cells,
        added,
        removed,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv(text: &str) -> Vec<Table> {
        read_tables(text.as_bytes(), "a.csv").unwrap()
    }

    #[test]
    fn matches_rows_by_id_so_a_reorder_is_no_change() {
        let before = csv("id,name,speed\nskiff,Skiff,12\ntug,Tug,7\nraft,Raft,4\n");
        let after = csv("id,name,speed\ntug,Tug,7\nskiff,Skiff,13.5\nfoil,Hydrofoil,22\n");
        let d = &diff_tables(Some(&before), &after)[0];
        assert_eq!(d.key.as_deref(), Some("id"));
        let states: Vec<_> = d.rows.iter().map(|r| r.state).collect();
        assert_eq!(
            states,
            [
                RowState::Same,
                RowState::Changed,
                RowState::Added,
                RowState::Removed
            ]
        );
        assert_eq!(d.rows[1].cells[2], [Some("12".into()), Some("13.5".into())]);
        assert_eq!(
            (d.changed, d.changed_cells, d.added, d.removed),
            (1, 1, 1, 1)
        );
    }

    #[test]
    fn falls_back_to_position_when_the_key_repeats_and_tracks_columns() {
        let before = csv("kind,speed\nboat,1\nboat,2\n");
        let after = csv("kind,speed,cost\nboat,1,5\nboat,3,6\n");
        let d = &diff_tables(Some(&before), &after)[0];
        assert_eq!(d.key, None);
        assert_eq!(d.added_columns, ["cost"]);
        assert_eq!(d.rows[1].cells[1], [Some("2".into()), Some("3".into())]);
        assert_eq!(d.rows[1].cells[2], [None, Some("6".into())]);
        assert_eq!(d.changed_cells, 1);
    }

    #[test]
    fn reads_messy_csv_semicolons_quotes_and_utf16() {
        let t = &csv("id;note\n1;\"a; b\"\n2;\"two\nlines\"\n")[0];
        assert_eq!(t.columns, ["id", "note"]);
        assert_eq!(t.rows[1], ["2", "two\nlines"]);
        let mut utf16 = vec![0xFF, 0xFE];
        for u in "id\tx\n1\tä\n".encode_utf16() {
            utf16.extend_from_slice(&u.to_le_bytes());
        }
        let t = &read_tables(&utf16, "a.tsv").unwrap()[0];
        assert_eq!(t.rows[0], ["1", "ä"]);
        assert_eq!(
            read_tables(b"\xEF\xBB\xBFa,b\n1,2\n", "x.csv").unwrap()[0].columns,
            ["a", "b"]
        );
    }

    #[test]
    fn reads_json_lists_and_flattens_nested_fields() {
        let t = read_tables(
            br#"{"version": 2, "boats": [{"id": "skiff", "stats": {"speed": 12}, "tags": ["a"]}, {"id": "tug", "cost": null}]}"#,
            "b.json",
        )
        .unwrap();
        assert_eq!(t[0].name, "fields");
        assert_eq!(t[0].rows, [["version", "2"]]);
        assert_eq!(t[1].name, "boats");
        assert_eq!(t[1].columns, ["id", "stats.speed", "tags[0]", "cost"]);
        assert_eq!(t[1].rows[1], ["tug", "", "", "null"]);
        let ordered = read_tables(br#"[{"zeta": 1, "alpha": 2}]"#, "o.json").unwrap();
        assert_eq!(
            ordered[0].columns,
            ["zeta", "alpha"],
            "file order, not sorted"
        );
        assert!(read_tables(b"{", "b.json").is_err());
        let top = read_tables(br#"[{"a":1},{"a":2,"b":true}]"#, "c.json").unwrap();
        assert_eq!(top[0].rows, [["1", ""], ["2", "true"]]);
    }

    #[test]
    fn a_lone_version_and_a_dropped_table() {
        let after = csv("id,x\n1,2\n");
        let d = &diff_tables(None, &after)[0];
        assert_eq!(d.rows[0].state, RowState::Same);
        assert_eq!(d.rows[0].cells[1], [None, Some("2".into())]);
        let before = read_tables(br#"{"a":[{"id":1}],"b":[{"id":2}]}"#, "x.json").unwrap();
        let after = read_tables(br#"{"a":[{"id":1}]}"#, "x.json").unwrap();
        let d = diff_tables(Some(&before), &after);
        assert_eq!(d.len(), 2);
        assert_eq!((d[1].name.as_str(), d[1].removed), ("b", 1));
    }
}
