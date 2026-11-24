use std::hash::Hasher;
use std::io::Read;

use serde::Serialize;
use ttf_parser::{Face, OutlineBuilder, name_id};
use xxhash_rust::xxh64::Xxh64;

use crate::image::Version;
use crate::repo::{Error, Repo};

/// What the font viewer compares between two versions of a font file.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontInfo {
    /// "TrueType", "OpenType (CFF)", with "WOFF" / "WOFF2" / "collection" when it applies.
    pub format: String,
    /// Faces in the file: more than one for a `.ttc` collection.
    pub faces: u32,
    pub family: Option<String>,
    pub style: Option<String>,
    pub version: Option<String>,
    pub weight: u16,
    pub italic: bool,
    pub units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    pub x_height: Option<i16>,
    pub cap_height: Option<i16>,
    pub glyph_count: u16,
    /// Variation axes of a variable font.
    pub axes: Vec<FontAxis>,
    /// Every Unicode code point the font maps, ascending.
    pub codepoints: Vec<u32>,
    /// A hash of each code point's outline (same order), so redrawn glyphs can be found; 0 for none.
    pub outlines: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontAxis {
    pub tag: String,
    pub name: Option<String>,
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

impl Repo {
    /// Font `path` at `version`, face `face` of a collection.
    ///
    /// # Errors
    /// [`Error::Refused`] when it isn't in that version or can't be read as a font.
    pub fn font_info(&self, path: &str, version: &Version, face: u32) -> Result<FontInfo, Error> {
        let bytes = self.file_bytes(None, path, version, 0)?;
        read_font(&bytes, face).map_err(|e| Error::Refused(format!("{path}: {e}")))
    }
}

/// Reads a TTF / OTF / TTC / WOFF / WOFF2 file.
///
/// # Errors
/// A plain-words reason when the bytes aren't a font gitgud can read.
pub fn read_font(bytes: &[u8], face: u32) -> Result<FontInfo, String> {
    let (sfnt, wrapper) = unwrap_web_font(bytes)?;
    let faces = ttf_parser::fonts_in_collection(&sfnt).unwrap_or(1);
    let f =
        Face::parse(&sfnt, face).map_err(|e| format!("not a font this viewer can read ({e})"))?;
    let name = |id: u16| {
        f.names()
            .into_iter()
            .filter(|n| n.name_id == id && n.is_unicode())
            .find_map(|n| n.to_string())
    };
    let mut format = if f.tables().cff.is_some() || f.tables().cff2.is_some() {
        "OpenType (CFF)".to_owned()
    } else {
        "TrueType".to_owned()
    };
    if let Some(w) = wrapper {
        format = format!("{w} · {format}");
    }
    if faces > 1 {
        format.push_str(" · collection");
    }
    let mut points = Vec::new();
    if let Some(cmap) = f.tables().cmap {
        for sub in cmap.subtables {
            if sub.is_unicode() {
                sub.codepoints(|c| points.push(c));
            }
        }
    }
    points.sort_unstable();
    points.dedup();
    let mut codepoints = Vec::with_capacity(points.len());
    let mut outlines = Vec::with_capacity(points.len());
    for c in points {
        let Some(gid) = char::from_u32(c).and_then(|ch| f.glyph_index(ch)) else {
            continue;
        };
        let mut hash = OutlineHash(Xxh64::new(0));
        let hashed = f
            .outline_glyph(gid, &mut hash)
            .map_or(0, |_| (hash.0.finish() as u32).max(1));
        codepoints.push(c);
        outlines.push(hashed);
    }
    let axes = f
        .variation_axes()
        .into_iter()
        .map(|a| FontAxis {
            tag: a.tag.to_string(),
            name: name(a.name_id),
            min: a.min_value,
            default: a.def_value,
            max: a.max_value,
        })
        .collect();
    Ok(FontInfo {
        format,
        faces,
        family: name(name_id::TYPOGRAPHIC_FAMILY).or_else(|| name(name_id::FAMILY)),
        style: name(name_id::TYPOGRAPHIC_SUBFAMILY).or_else(|| name(name_id::SUBFAMILY)),
        version: name(name_id::VERSION),
        weight: f.weight().to_number(),
        italic: f.is_italic(),
        units_per_em: f.units_per_em(),
        ascender: f.ascender(),
        descender: f.descender(),
        line_gap: f.line_gap(),
        x_height: f.x_height(),
        cap_height: f.capital_height(),
        glyph_count: f.number_of_glyphs(),
        axes,
        codepoints,
        outlines,
    })
}

/// The plain sfnt inside a WOFF or WOFF2 file, and which wrapper it came in.
fn unwrap_web_font(bytes: &[u8]) -> Result<(Vec<u8>, Option<&'static str>), String> {
    if bytes.starts_with(b"wOF2") {
        let sfnt = woff2_patched::decode::convert_woff2_to_ttf(&mut &bytes[..])
            .map_err(|e| format!("this WOFF2 file can't be decoded ({e})"))?;
        return Ok((sfnt, Some("WOFF2")));
    }
    if bytes.starts_with(b"wOFF") {
        return woff1_to_sfnt(bytes).map(|s| (s, Some("WOFF")));
    }
    Ok((bytes.to_vec(), None))
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Rebuilds the sfnt a WOFF 1.0 file wraps: each table is stored zlib-compressed
/// unless compressing didn't shrink it.
fn woff1_to_sfnt(b: &[u8]) -> Result<Vec<u8>, String> {
    let bad = || "this WOFF file is damaged".to_owned();
    let flavor = be32(b, 4).ok_or_else(bad)?;
    let num = usize::from(be16(b, 12).ok_or_else(bad)?);
    let mut tables = Vec::with_capacity(num);
    for i in 0..num {
        let e = 44 + i * 20;
        let tag = be32(b, e).ok_or_else(bad)?;
        let offset = be32(b, e + 4).ok_or_else(bad)? as usize;
        let comp = be32(b, e + 8).ok_or_else(bad)? as usize;
        let orig = be32(b, e + 12).ok_or_else(bad)? as usize;
        let checksum = be32(b, e + 16).ok_or_else(bad)?;
        let raw = b.get(offset..offset + comp).ok_or_else(bad)?;
        let data = if comp < orig {
            let mut out = Vec::with_capacity(orig);
            flate2::read::ZlibDecoder::new(raw)
                .read_to_end(&mut out)
                .map_err(|_| bad())?;
            out
        } else {
            raw.to_vec()
        };
        tables.push((tag, checksum, data));
    }
    tables.sort_by_key(|t| t.0);
    let mut out = Vec::new();
    out.extend_from_slice(&flavor.to_be_bytes());
    let n = u16::try_from(num).map_err(|_| bad())?;
    let pow = 1u16 << (15 - n.max(1).leading_zeros());
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&(pow * 16).to_be_bytes());
    out.extend_from_slice(&(pow.trailing_zeros() as u16).to_be_bytes());
    out.extend_from_slice(&(n * 16 - pow * 16).to_be_bytes());
    let mut offset = 12 + num * 16;
    let mut body = Vec::new();
    for (tag, checksum, data) in &tables {
        out.extend_from_slice(&tag.to_be_bytes());
        out.extend_from_slice(&checksum.to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        body.extend_from_slice(data);
        while body.len() % 4 != 0 {
            body.push(0);
        }
        offset = 12 + num * 16 + body.len();
    }
    out.extend_from_slice(&body);
    Ok(out)
}

/// Feeds a glyph's outline commands into a hash.
struct OutlineHash(Xxh64);

impl OutlineHash {
    fn put(&mut self, op: u8, pts: &[f32]) {
        self.0.write_u8(op);
        for p in pts {
            self.0.write_u32(p.to_bits());
        }
    }
}

impl OutlineBuilder for OutlineHash {
    fn move_to(&mut self, x: f32, y: f32) {
        self.put(b'M', &[x, y]);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.put(b'L', &[x, y]);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.put(b'Q', &[x1, y1, x, y]);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.put(b'C', &[x1, y1, x2, y2, x, y]);
    }
    fn close(&mut self) {
        self.put(b'Z', &[]);
    }
}

/// A minimal TrueType font for tests: glyphs .notdef, "A" (a square) and "B" drawn
/// through `b_points`.
#[cfg(any(test, feature = "test-util"))]
pub mod fixture {
    #[must_use]
    pub fn tiny_ttf(b_points: &[(i16, i16)]) -> Vec<u8> {
        fn table(tag: &[u8; 4], data: Vec<u8>) -> ([u8; 4], Vec<u8>) {
            (*tag, data)
        }
        let glyph = |pts: &[(i16, i16)]| {
            let mut g = Vec::new();
            g.extend_from_slice(&1i16.to_be_bytes());
            for v in [0i16, 0, 500, 500] {
                g.extend_from_slice(&v.to_be_bytes());
            }
            g.extend_from_slice(&((pts.len() - 1) as u16).to_be_bytes());
            g.extend_from_slice(&0u16.to_be_bytes());
            g.extend(std::iter::repeat_n(1u8, pts.len()));
            let (mut px, mut py) = (0i16, 0i16);
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for &(x, y) in pts {
                xs.extend_from_slice(&(x - px).to_be_bytes());
                ys.extend_from_slice(&(y - py).to_be_bytes());
                (px, py) = (x, y);
            }
            g.extend(xs);
            g.extend(ys);
            while g.len() % 4 != 0 {
                g.push(0);
            }
            g
        };
        let a = glyph(&[(0, 0), (0, 500), (500, 500), (500, 0)]);
        let b = glyph(b_points);
        let mut glyf = a.clone();
        glyf.extend(&b);
        let loca: Vec<u8> = [0u32, 0, a.len() as u32, (a.len() + b.len()) as u32]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        let mut head = vec![0u8; 54];
        head[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
        head[18..20].copy_from_slice(&1000u16.to_be_bytes());
        head[50..52].copy_from_slice(&1i16.to_be_bytes());
        let mut hhea = vec![0u8; 36];
        hhea[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        hhea[4..6].copy_from_slice(&800i16.to_be_bytes());
        hhea[6..8].copy_from_slice(&(-200i16).to_be_bytes());
        hhea[34..36].copy_from_slice(&3u16.to_be_bytes());
        let mut maxp = vec![0u8; 6];
        maxp[0..4].copy_from_slice(&0x0000_5000u32.to_be_bytes());
        maxp[4..6].copy_from_slice(&3u16.to_be_bytes());
        let hmtx: Vec<u8> = (0..3).flat_map(|_| [0x02, 0x58, 0, 0]).collect();
        // cmap format 4: 'A' -> 1, 'B' -> 2.
        let mut sub = Vec::new();
        for v in [4u16, 32, 0, 4, 4, 1, 0, 0x42, 0xFFFF, 0, 0x41, 0xFFFF] {
            sub.extend_from_slice(&v.to_be_bytes());
        }
        sub.extend_from_slice(&(-0x40i16).to_be_bytes());
        sub.extend_from_slice(&1i16.to_be_bytes());
        sub.extend_from_slice(&[0, 0, 0, 0]);
        let mut cmap = Vec::new();
        for v in [0u16, 1, 3, 1] {
            cmap.extend_from_slice(&v.to_be_bytes());
        }
        cmap.extend_from_slice(&12u32.to_be_bytes());
        cmap.extend(sub);
        let mut tables = vec![
            table(b"cmap", cmap),
            table(b"glyf", glyf),
            table(b"head", head),
            table(b"hhea", hhea),
            table(b"hmtx", hmtx),
            table(b"loca", loca),
            table(b"maxp", maxp),
        ];
        tables.sort_by_key(|t| t.0);
        let n = tables.len();
        let mut out = Vec::new();
        out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        for v in [n as u16, 64, 2, (n * 16 - 64) as u16] {
            out.extend_from_slice(&v.to_be_bytes());
        }
        let mut offset = 12 + n * 16;
        let mut body = Vec::new();
        for (tag, data) in &tables {
            out.extend_from_slice(tag);
            out.extend_from_slice(&0u32.to_be_bytes());
            out.extend_from_slice(&(offset as u32).to_be_bytes());
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            body.extend(data);
            while body.len() % 4 != 0 {
                body.push(0);
            }
            offset = 12 + n * 16 + body.len();
        }
        out.extend(body);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::tiny_ttf;
    use super::*;

    /// Wraps an sfnt in WOFF 1.0, compressing each table.
    fn to_woff1(sfnt: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let num = usize::from(be16(sfnt, 4).unwrap());
        let mut dir = Vec::new();
        let mut body = Vec::new();
        let base = 44 + num * 20;
        for i in 0..num {
            let e = 12 + i * 16;
            let tag = be32(sfnt, e).unwrap();
            let off = be32(sfnt, e + 8).unwrap() as usize;
            let len = be32(sfnt, e + 12).unwrap() as usize;
            let data = &sfnt[off..off + len];
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
            z.write_all(data).unwrap();
            let comp = z.finish().unwrap();
            let stored = if comp.len() < data.len() {
                comp
            } else {
                data.to_vec()
            };
            for v in [
                tag,
                (base + body.len()) as u32,
                stored.len() as u32,
                len as u32,
                0,
            ] {
                dir.extend_from_slice(&v.to_be_bytes());
            }
            body.extend(stored);
            while body.len() % 4 != 0 {
                body.push(0);
            }
        }
        let mut out = b"wOFF".to_vec();
        out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        out.extend_from_slice(&((base + body.len()) as u32).to_be_bytes());
        out.extend_from_slice(&(num as u16).to_be_bytes());
        out.extend_from_slice(&[0; 2]);
        out.extend_from_slice(&(sfnt.len() as u32).to_be_bytes());
        out.extend_from_slice(&[0; 24]);
        out.extend(dir);
        out.extend(body);
        out
    }

    #[test]
    fn reads_code_points_and_spots_a_redrawn_glyph() {
        let before = read_font(&tiny_ttf(&[(0, 0), (250, 500), (500, 0)]), 0).unwrap();
        let after = read_font(&tiny_ttf(&[(0, 0), (250, 600), (500, 0)]), 0).unwrap();
        assert_eq!(before.codepoints, vec![0x41, 0x42]);
        assert_eq!(before.format, "TrueType");
        assert_eq!((before.units_per_em, before.glyph_count), (1000, 3));
        assert_eq!(
            before.outlines[0], after.outlines[0],
            "A is the same square"
        );
        assert_ne!(before.outlines[1], after.outlines[1], "B's apex moved");
        assert!(before.outlines.iter().all(|&h| h != 0));
    }

    #[test]
    fn unwraps_woff_and_refuses_what_isnt_a_font() {
        let sfnt = tiny_ttf(&[(0, 0), (250, 500), (500, 0)]);
        let woff = read_font(&to_woff1(&sfnt), 0).unwrap();
        assert_eq!(woff.format, "WOFF · TrueType");
        assert_eq!(woff.outlines, read_font(&sfnt, 0).unwrap().outlines);
        assert!(read_font(b"not a font at all", 0).is_err());
        assert!(read_font(b"wOF2 broken", 0).is_err());
    }
}
