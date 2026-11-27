//! Unity's binary serialized files (TerrainData and other assets Unity never writes as
//! YAML), formats 17 to 22, read through the type trees the editor stores in them.

use crate::repo::Error;

/// Unity's shared type and field names, at the offsets type trees refer to with the high bit set.
const COMMON: &str = "AABB\0AnimationClip\0AnimationCurve\0AnimationState\0Array\0Base\0BitField\0bitset\0bool\0char\0ColorRGBA\0Component\0data\0deque\0double\0dynamic_array\0FastPropertyName\0first\0float\0Font\0GameObject\0Generic Mono\0GradientNEW\0GUID\0GUIStyle\0int\0list\0long long\0map\0Matrix4x4f\0MdFour\0MonoBehaviour\0MonoScript\0m_ByteSize\0m_Curve\0m_EditorClassIdentifier\0m_EditorHideFlags\0m_Enabled\0m_ExtensionPtr\0m_GameObject\0m_Index\0m_IsArray\0m_IsStatic\0m_MetaFlag\0m_Name\0m_ObjectHideFlags\0m_PrefabInternal\0m_PrefabParentObject\0m_Script\0m_StaticEditorFlags\0m_Type\0m_Version\0Object\0pair\0PPtr<Component>\0PPtr<GameObject>\0PPtr<Material>\0PPtr<MonoBehaviour>\0PPtr<MonoScript>\0PPtr<Object>\0PPtr<Prefab>\0PPtr<Sprite>\0PPtr<TextAsset>\0PPtr<Texture>\0PPtr<Texture2D>\0PPtr<Transform>\0Prefab\0Quaternionf\0Rectf\0RectInt\0RectOffset\0second\0set\0short\0size\0SInt16\0SInt32\0SInt64\0SInt8\0staticvector\0string\0TextAsset\0TextMesh\0Texture\0Texture2D\0Transform\0TypelessData\0UInt16\0UInt32\0UInt64\0UInt8\0unsigned int\0unsigned long long\0unsigned short\0vector\0Vector2f\0Vector3f\0Vector4f\0m_ScriptingClassIdentifier\0Gradient\0Type*\0int2_storage\0int3_storage\0BoundsInt\0m_CorrespondingSourceObject\0m_PrefabInstance\0m_PrefabAsset\0FileSize\0Hash128\0";

const ALIGN: i32 = 0x4000;

/// A decoded field, by the shapes type trees have.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    /// A vector of a primitive type (heights, colours, indices), kept flat.
    Numbers(Vec<f64>),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

static NULL: Value = Value::Null;

impl Value {
    /// Field `key` of a class, [`Value::Null`] when there is none.
    pub fn get(&self, key: &str) -> &Value {
        match self {
            Value::Map(fields) => fields
                .iter()
                .find(|(k, _)| k == key)
                .map_or(&NULL, |(_, v)| v),
            _ => &NULL,
        }
    }

    pub fn f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Bool(b) => Some(f64::from(u8::from(*b))),
            _ => None,
        }
    }

    pub fn i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            Value::Bool(b) => Some(i64::from(*b)),
            _ => None,
        }
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn numbers(&self) -> &[f64] {
        match self {
            Value::Numbers(n) => n,
            _ => &[],
        }
    }

    pub fn list(&self) -> &[Value] {
        match self {
            Value::List(l) => l,
            _ => &[],
        }
    }

    /// `[x, y, z]` of a `Vector3f`.
    pub fn vec3(&self) -> Option<[f32; 3]> {
        Some([
            self.get("x").f64()? as f32,
            self.get("y").f64()? as f32,
            self.get("z").f64()? as f32,
        ])
    }

    /// A `PPtr`: the external file index (0 for this file) and the object's path id.
    pub fn pptr(&self) -> Option<(usize, i64)> {
        let file = self.get("m_FileID").i64()?;
        let path = self.get("m_PathID").i64()?;
        (path != 0).then_some((usize::try_from(file).ok()?, path))
    }
}

#[derive(Debug, Clone)]
struct TypeNode {
    ty: String,
    name: String,
    level: u8,
    meta: i32,
}

#[derive(Debug, Clone)]
struct SerializedType {
    class_id: i32,
    nodes: Vec<TypeNode>,
}

/// An object in the file: its id and where its bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObjectInfo {
    pub path_id: i64,
    pub class_id: i32,
    start: usize,
    size: usize,
    type_ix: usize,
}

/// Another file the objects refer to (`PPtr.m_FileID` - 1 indexes this list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct External {
    /// The asset's GUID as its `.meta` writes it.
    pub guid: String,
    pub path: String,
}

pub(crate) struct SerializedFile<'b> {
    bytes: &'b [u8],
    big: bool,
    types: Vec<SerializedType>,
    pub objects: Vec<ObjectInfo>,
    pub externals: Vec<External>,
}

/// Reads primitives at a position, in the file's byte order.
struct Reader<'b> {
    bytes: &'b [u8],
    pos: usize,
    big: bool,
}

impl<'b> Reader<'b> {
    fn take(&mut self, n: usize) -> Result<&'b [u8], Error> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or_else(|| {
                Error::Read(format!(
                    "unexpected end of a Unity binary file at byte {}",
                    self.pos
                ))
            })?;
        let s = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let mut a = [0; N];
        a.copy_from_slice(self.take(N)?);
        if self.big {
            a.reverse();
        }
        Ok(a)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> Result<i16, Error> {
        Ok(i16::from_le_bytes(self.array()?))
    }
    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn i64(&mut self) -> Result<i64, Error> {
        Ok(i64::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_le_bytes(self.array()?))
    }
    fn f64(&mut self) -> Result<f64, Error> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    fn count(&mut self) -> Result<usize, Error> {
        let n = self.i32()?;
        usize::try_from(n)
            .map_err(|_| Error::Read(format!("negative count {n} in a Unity binary file")))
    }

    fn cstr(&mut self) -> Result<String, Error> {
        let rest = &self.bytes[self.pos.min(self.bytes.len())..];
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| Error::Read("unterminated string in a Unity binary file".into()))?;
        let s = String::from_utf8_lossy(&rest[..len]).into_owned();
        self.pos += len + 1;
        Ok(s)
    }

    fn align(&mut self) {
        self.pos = self.pos.div_ceil(4) * 4;
    }
}

/// A type tree string: in the tree's own buffer, or Unity's common table (high bit set).
fn tree_string(buffer: &[u8], offset: u32) -> String {
    let (table, at) = if offset & 0x8000_0000 == 0 {
        (buffer, offset as usize)
    } else {
        (COMMON.as_bytes(), (offset & 0x7FFF_FFFF) as usize)
    };
    let Some(rest) = table.get(at..) else {
        return (offset & 0x7FFF_FFFF).to_string();
    };
    let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..end]).into_owned()
}

/// Unity writes a GUID's bytes with each byte's two hex digits swapped.
fn guid_text(raw: &[u8]) -> String {
    raw.iter()
        .map(|b| format!("{:x}{:x}", b & 0xF, b >> 4))
        .collect()
}

/// Whether bytes look like a Unity binary serialized file (and not YAML or an LFS pointer).
fn is_serialized(bytes: &[u8]) -> bool {
    if bytes.len() < 48
        || bytes.starts_with(b"%YAML")
        || bytes.starts_with(b"version https://git-lfs")
    {
        return false;
    }
    let version = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    (9..=30).contains(&version) && bytes[16] <= 1
}

impl<'b> SerializedFile<'b> {
    /// Reads the header, the type trees, the object table and the externals.
    ///
    /// # Errors
    /// [`Error::Read`] for a file this reader can't follow: a format below 17 or above
    /// 22, no type trees, or a truncated file.
    pub fn parse(bytes: &'b [u8]) -> Result<Self, Error> {
        if !is_serialized(bytes) {
            return Err(Error::Read(
                if bytes.starts_with(b"version https://git-lfs") {
                    "an LFS pointer whose object isn't downloaded, not a Unity binary file".into()
                } else if bytes.starts_with(b"%YAML") {
                    "Unity YAML, not a Unity binary file".into()
                } else {
                    "not a Unity binary file".into()
                },
            ));
        }
        let mut r = Reader {
            bytes,
            pos: 8,
            big: true,
        };
        let version = r.u32()?;
        let mut data_offset = u64::from(r.u32()?);
        if !(17..=22).contains(&version) {
            return Err(Error::Read(format!(
                "Unity binary format {version} isn't read (17 to 22 are)"
            )));
        }
        r.pos = 16;
        let big = r.u8()? != 0;
        r.pos = 20;
        if version >= 22 {
            let _metadata_size = r.u32()?;
            let _file_size = r.i64()?;
            data_offset = u64::try_from(r.i64()?).unwrap_or(0);
            let _unknown = r.i64()?;
        }
        r.big = big;
        let _unity_version = r.cstr()?;
        let _target_platform = r.i32()?;
        if r.u8()? == 0 {
            return Err(Error::Read(
                "this Unity binary file has no type trees (a built player's file, not an editor asset)".into(),
            ));
        }
        let mut types = Vec::new();
        for _ in 0..r.count()? {
            let class_id = r.i32()?;
            let _stripped = r.u8()?;
            let _script_type_index = r.i16()?;
            if class_id == 114 {
                r.take(16)?;
            }
            r.take(16)?;
            let node_count = r.count()?;
            let buffer_size = r.count()?;
            let record = if version >= 19 { 32 } else { 24 };
            let records = r.take(node_count * record)?;
            let buffer = r.take(buffer_size)?;
            let mut nr = Reader {
                bytes: records,
                pos: 0,
                big,
            };
            let mut nodes = Vec::with_capacity(node_count);
            for _ in 0..node_count {
                let _version = nr.i16()?;
                let level = nr.u8()?;
                let _flags = nr.u8()?;
                let ty = nr.u32()?;
                let name = nr.u32()?;
                let _byte_size = nr.i32()?;
                let _index = nr.i32()?;
                let meta = nr.i32()?;
                if version >= 19 {
                    nr.u64()?;
                }
                nodes.push(TypeNode {
                    ty: tree_string(buffer, ty),
                    name: tree_string(buffer, name),
                    level,
                    meta,
                });
            }
            if version >= 21 {
                for _ in 0..r.count()? {
                    r.i32()?;
                }
            }
            types.push(SerializedType { class_id, nodes });
        }
        let mut objects = Vec::new();
        for _ in 0..r.count()? {
            r.align();
            let path_id = r.i64()?;
            let start = if version >= 22 {
                u64::try_from(r.i64()?).unwrap_or(0)
            } else {
                u64::from(r.u32()?)
            };
            let size = r.u32()? as usize;
            let type_ix = r.count()?;
            let class_id = types
                .get(type_ix)
                .ok_or_else(|| {
                    Error::Read(format!(
                        "object {path_id} names type {type_ix}, which isn't listed"
                    ))
                })?
                .class_id;
            objects.push(ObjectInfo {
                path_id,
                class_id,
                start: usize::try_from(data_offset + start).unwrap_or(usize::MAX),
                size,
                type_ix,
            });
        }
        for _ in 0..r.count()? {
            r.i32()?;
            r.align();
            r.i64()?;
        }
        let mut externals = Vec::new();
        for _ in 0..r.count()? {
            r.cstr()?;
            let guid = guid_text(r.take(16)?);
            r.i32()?;
            let path = r.cstr()?;
            externals.push(External { guid, path });
        }
        Ok(Self {
            bytes,
            big,
            types,
            objects,
            externals,
        })
    }

    /// The first object of `class_id`.
    pub fn first_of(&self, class_id: i32) -> Option<ObjectInfo> {
        self.objects
            .iter()
            .copied()
            .find(|o| o.class_id == class_id)
    }

    pub fn by_path_id(&self, path_id: i64) -> Option<ObjectInfo> {
        self.objects.iter().copied().find(|o| o.path_id == path_id)
    }

    /// Decodes `object` through its type tree.
    ///
    /// # Errors
    /// [`Error::Read`] when its bytes don't follow the tree.
    pub fn read(&self, object: ObjectInfo) -> Result<Value, Error> {
        let nodes = &self.types[object.type_ix].nodes;
        if nodes.is_empty() {
            return Err(Error::Read(format!(
                "object {} has an empty type tree",
                object.path_id
            )));
        }
        let end = object
            .start
            .saturating_add(object.size)
            .min(self.bytes.len());
        let mut r = Reader {
            bytes: &self.bytes[..end],
            pos: object.start,
            big: self.big,
        };
        read_node(&mut r, nodes, 0)
    }
}

/// The indices of `nodes[at]`'s direct children.
fn children(nodes: &[TypeNode], at: usize) -> Vec<usize> {
    let level = nodes[at].level;
    let mut out = Vec::new();
    for (i, n) in nodes.iter().enumerate().skip(at + 1) {
        if n.level <= level {
            break;
        }
        if n.level == level + 1 {
            out.push(i);
        }
    }
    out
}

fn is_primitive(ty: &str) -> bool {
    matches!(
        ty,
        "SInt8"
            | "UInt8"
            | "char"
            | "bool"
            | "short"
            | "SInt16"
            | "UInt16"
            | "unsigned short"
            | "int"
            | "SInt32"
            | "UInt32"
            | "unsigned int"
            | "Type*"
            | "long long"
            | "SInt64"
            | "UInt64"
            | "unsigned long long"
            | "FileSize"
            | "float"
            | "double"
    )
}

fn primitive(r: &mut Reader<'_>, ty: &str) -> Result<Value, Error> {
    Ok(match ty {
        "SInt8" => Value::Int(i64::from(r.u8()? as i8)),
        "UInt8" | "char" => Value::Int(i64::from(r.u8()?)),
        "bool" => Value::Bool(r.u8()? != 0),
        "short" | "SInt16" => Value::Int(i64::from(r.i16()?)),
        "UInt16" | "unsigned short" => Value::Int(i64::from(r.u16()?)),
        "int" | "SInt32" => Value::Int(i64::from(r.i32()?)),
        "UInt32" | "unsigned int" | "Type*" => Value::Int(i64::from(r.u32()?)),
        "long long" | "SInt64" => Value::Int(r.i64()?),
        "UInt64" | "unsigned long long" | "FileSize" => Value::Int(r.u64()? as i64),
        "float" => Value::Float(f64::from(r.f32()?)),
        "double" => Value::Float(r.f64()?),
        _ => return Err(Error::Read(format!("{ty} isn't a primitive type"))),
    })
}

/// Most elements an array's capacity is reserved for up front: its count comes from
/// the file, so the bytes run out first when it lies.
const MAX_RESERVE: usize = 1 << 20;

/// Reads `nodes[at]`.
fn read_node(r: &mut Reader<'_>, nodes: &[TypeNode], at: usize) -> Result<Value, Error> {
    let node = &nodes[at];
    let kids = children(nodes, at);
    let mut align = node.meta & ALIGN != 0;
    let value = if node.ty == "string" {
        let n = r.count()?;
        let s = String::from_utf8_lossy(r.take(n)?).into_owned();
        align = true;
        Value::Str(s)
    } else if node.ty == "TypelessData" {
        let n = r.count()?;
        Value::Bytes(r.take(n)?.to_vec())
    } else if kids.is_empty() && is_primitive(&node.ty) {
        primitive(r, &node.ty)?
    } else if let Some(&array) = kids.first().filter(|&&k| nodes[k].ty == "Array") {
        let parts = children(nodes, array);
        let Some(&element) = parts.get(1) else {
            return Err(Error::Read(format!(
                "{} has an Array without an element type",
                node.name
            )));
        };
        align |= nodes[array].meta & ALIGN != 0;
        let n = r.count()?;
        let el = &nodes[element];
        if is_primitive(&el.ty) && children(nodes, element).is_empty() {
            if el.ty == "UInt8" || el.ty == "char" {
                Value::Bytes(r.take(n)?.to_vec())
            } else {
                let mut out = Vec::with_capacity(n.min(MAX_RESERVE));
                for _ in 0..n {
                    out.push(primitive(r, &el.ty)?.f64().unwrap_or(0.0));
                }
                Value::Numbers(out)
            }
        } else {
            let mut out = Vec::with_capacity(n.min(MAX_RESERVE));
            for i in 0..n {
                let before = r.pos;
                out.push(read_node(r, nodes, element)?);
                // An element that reads no bytes would let a lying count spin for billions.
                if i == 0 && r.pos == before && n > 1 {
                    return Err(Error::Read(format!(
                        "{}: an array of {n} elements that take no bytes",
                        node.name
                    )));
                }
            }
            Value::List(out)
        }
    } else {
        let mut fields = Vec::with_capacity(kids.len());
        for k in kids {
            let v = read_node(r, nodes, k)?;
            fields.push((nodes[k].name.clone(), v));
        }
        Value::Map(fields)
    };
    if align {
        r.align();
    }
    Ok(value)
}

/// Writes small binary serialized files for tests: type trees in Unity's blob format.
#[cfg(test)]
pub(crate) mod write {
    /// A type tree node: type, name, level and whether it aligns after.
    pub struct N(pub &'static str, pub &'static str, pub u8, pub bool);

    pub struct Object {
        pub path_id: i64,
        pub type_ix: i32,
        pub data: Vec<u8>,
    }

    /// One type: class id and its nodes in pre-order.
    pub struct Type {
        pub class_id: i32,
        pub nodes: Vec<N>,
    }

    /// A file in format `version` (17 or 22), little-endian.
    pub fn file(
        version: u32,
        types: &[Type],
        objects: &[Object],
        externals: &[(&[u8; 16], &str)],
    ) -> Vec<u8> {
        let mut meta = Vec::new();
        meta.extend(b"2021.3.39f1\0");
        meta.extend((-2i32).to_le_bytes());
        meta.push(1);
        meta.extend((types.len() as i32).to_le_bytes());
        for t in types {
            meta.extend(t.class_id.to_le_bytes());
            meta.push(0);
            meta.extend((-1i16).to_le_bytes());
            meta.extend([0u8; 16]);
            let mut buffer = Vec::new();
            let mut records = Vec::new();
            for n in &t.nodes {
                let mut offset = |s: &str| {
                    let at = buffer.len() as u32;
                    buffer.extend(s.as_bytes());
                    buffer.push(0);
                    at
                };
                let ty = offset(n.0);
                let name = offset(n.1);
                records.extend(1i16.to_le_bytes());
                records.push(n.2);
                records.push(0);
                records.extend(ty.to_le_bytes());
                records.extend(name.to_le_bytes());
                records.extend((-1i32).to_le_bytes());
                records.extend(0i32.to_le_bytes());
                records.extend((if n.3 { super::ALIGN } else { 0 }).to_le_bytes());
                if version >= 19 {
                    records.extend(0u64.to_le_bytes());
                }
            }
            meta.extend((t.nodes.len() as i32).to_le_bytes());
            meta.extend((buffer.len() as i32).to_le_bytes());
            meta.extend(records);
            meta.extend(buffer);
            if version >= 21 {
                meta.extend(0i32.to_le_bytes());
            }
        }
        let header = if version >= 22 { 48 } else { 20 };
        let mut data = Vec::new();
        let mut table = Vec::new();
        let mut entries = Vec::new();
        for o in objects {
            while !data.len().is_multiple_of(8) {
                data.push(0);
            }
            entries.push((o.path_id, data.len(), o.data.len(), o.type_ix));
            data.extend(&o.data);
        }
        // The object table's alignment depends on its absolute position.
        let table_at = header + meta.len();
        table.extend((objects.len() as i32).to_le_bytes());
        for (path_id, start, size, type_ix) in entries {
            while !(table_at + table.len()).is_multiple_of(4) {
                table.push(0);
            }
            table.extend(path_id.to_le_bytes());
            if version >= 22 {
                table.extend((start as i64).to_le_bytes());
            } else {
                table.extend((start as u32).to_le_bytes());
            }
            table.extend((size as u32).to_le_bytes());
            table.extend(type_ix.to_le_bytes());
        }
        table.extend(0i32.to_le_bytes());
        table.extend((externals.len() as i32).to_le_bytes());
        for (guid, path) in externals {
            table.push(0);
            table.extend(*guid);
            table.extend(0i32.to_le_bytes());
            table.extend(path.as_bytes());
            table.push(0);
        }
        table.extend(0i32.to_le_bytes());
        table.push(0);
        meta.extend(table);
        let data_offset = (header + meta.len()).div_ceil(16) * 16;
        let mut out = vec![0u8; header];
        out[8..12].copy_from_slice(&version.to_be_bytes());
        if version >= 22 {
            out[20..24].copy_from_slice(&(meta.len() as u32).to_be_bytes());
            out[24..32].copy_from_slice(&((data_offset + data.len()) as i64).to_be_bytes());
            out[32..40].copy_from_slice(&(data_offset as i64).to_be_bytes());
        } else {
            out[0..4].copy_from_slice(&(meta.len() as u32).to_be_bytes());
            out[4..8].copy_from_slice(&((data_offset + data.len()) as u32).to_be_bytes());
            out[12..16].copy_from_slice(&(data_offset as u32).to_be_bytes());
        }
        out.extend(meta);
        out.resize(data_offset, 0);
        out.extend(data);
        out
    }

    /// Little-endian bytes of the values in order, with `align()` padding to 4.
    #[derive(Default)]
    pub struct Bytes(pub Vec<u8>);

    impl Bytes {
        pub fn i32(mut self, v: i32) -> Self {
            self.0.extend(v.to_le_bytes());
            self
        }
        pub fn i64(mut self, v: i64) -> Self {
            self.0.extend(v.to_le_bytes());
            self
        }
        pub fn f32(mut self, v: f32) -> Self {
            self.0.extend(v.to_le_bytes());
            self
        }
        pub fn i16s(mut self, v: &[i16]) -> Self {
            self.0.extend((v.len() as i32).to_le_bytes());
            for x in v {
                self.0.extend(x.to_le_bytes());
            }
            self
        }
        pub fn raw(mut self, v: &[u8]) -> Self {
            self.0.extend(v);
            self
        }
        pub fn string(self, s: &str) -> Self {
            self.i32(s.len() as i32).raw(s.as_bytes()).align()
        }
        pub fn align(mut self) -> Self {
            while !self.0.len().is_multiple_of(4) {
                self.0.push(0);
            }
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::write::{Bytes, N, Object, Type, file};
    use super::*;

    fn thing() -> Type {
        Type {
            class_id: 156,
            nodes: vec![
                N("Thing", "Base", 0, false),
                N("string", "m_Name", 1, true),
                N("Array", "Array", 2, false),
                N("int", "size", 3, false),
                N("char", "data", 3, false),
                N("vector", "m_Heights", 1, true),
                N("Array", "Array", 2, false),
                N("int", "size", 3, false),
                N("SInt16", "data", 3, false),
                N("Vector3f", "m_Scale", 1, false),
                N("float", "x", 2, false),
                N("float", "y", 2, false),
                N("float", "z", 2, false),
                N("vector", "m_Layers", 1, false),
                N("Array", "Array", 2, false),
                N("int", "size", 3, false),
                N("PPtr<TerrainLayer>", "data", 3, false),
                N("int", "m_FileID", 4, false),
                N("SInt64", "m_PathID", 4, false),
                N("TypelessData", "image data", 1, true),
                N("bool", "m_Flag", 1, true),
            ],
        }
    }

    fn data() -> Vec<u8> {
        Bytes::default()
            .string("Island")
            .i16s(&[0, 16383, 32766])
            .align()
            .f32(2.0)
            .f32(600.0)
            .f32(3.0)
            .i32(2)
            .i32(1)
            .i64(11400000)
            .i32(0)
            .i64(-7)
            .i32(3)
            .raw(&[1, 2, 3])
            .align()
            .raw(&[1])
            .align()
            .0
    }

    #[test]
    fn reads_objects_through_their_type_trees_in_both_formats() {
        let guid = [
            0x12u8, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0, 0, 0, 0, 0, 0, 0, 0x01,
        ];
        for version in [17, 22] {
            let bytes = file(
                version,
                &[thing()],
                &[Object {
                    path_id: 15600000,
                    type_ix: 0,
                    data: data(),
                }],
                &[(&guid, "")],
            );
            assert!(is_serialized(&bytes), "format {version}");
            let f = SerializedFile::parse(&bytes).unwrap();
            assert_eq!(f.externals[0].guid, "21436587a9cbed0f0000000000000010");
            let o = f.first_of(156).unwrap();
            assert_eq!(o.path_id, 15600000);
            let v = f.read(o).unwrap();
            assert_eq!(v.get("m_Name").str(), Some("Island"));
            assert_eq!(v.get("m_Heights").numbers(), [0.0, 16383.0, 32766.0]);
            assert_eq!(v.get("m_Scale").vec3(), Some([2.0, 600.0, 3.0]));
            let layers = v.get("m_Layers").list();
            assert_eq!(layers[0].pptr(), Some((1, 11400000)));
            assert_eq!(layers[1].pptr(), Some((0, -7)));
            assert_eq!(v.get("image data").bytes(), Some(&[1u8, 2, 3][..]));
            assert_eq!(v.get("m_Flag"), &Value::Bool(true));
            assert_eq!(v.get("nothing"), &Value::Null);
        }
    }

    #[test]
    fn an_array_of_empty_elements_with_a_huge_count_is_refused() {
        let empty = Type {
            class_id: 156,
            nodes: vec![
                N("Thing", "Base", 0, false),
                N("vector", "m_Nothing", 1, false),
                N("Array", "Array", 2, false),
                N("int", "size", 3, false),
                N("Empty", "data", 3, false),
            ],
        };
        let bytes = file(
            22,
            &[empty],
            &[Object {
                path_id: 1,
                type_ix: 0,
                data: Bytes::default().i32(i32::MAX).0,
            }],
            &[],
        );
        let f = SerializedFile::parse(&bytes).unwrap();
        let started = std::time::Instant::now();
        assert!(f.read(f.first_of(156).unwrap()).is_err());
        assert!(started.elapsed().as_secs() < 1);
    }

    #[test]
    fn resolves_common_strings_and_refuses_what_it_cannot_read() {
        assert_eq!(tree_string(b"", 0x8000_0000 | 49), "Array");
        assert_eq!(tree_string(b"", 0x8000_0000 | 1161), "Hash128");
        assert_eq!(tree_string(b"m_Heights\0", 0), "m_Heights");
        assert!(!is_serialized(
            b"%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!156 &1\n"
        ));
        assert!(!is_serialized(
            b"version https://git-lfs.github.com/spec/v1\noid sha256:00\nsize 9\n"
        ));
        let mut old = file(22, &[thing()], &[], &[]);
        old[8..12].copy_from_slice(&15u32.to_be_bytes());
        assert!(SerializedFile::parse(&old).is_err());
        assert!(SerializedFile::parse(&[0; 10]).is_err());
    }
}
