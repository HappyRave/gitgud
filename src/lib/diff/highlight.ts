export type TokenKind = "plain" | "kw" | "str" | "num" | "com" | "type" | "fn";

export interface Token {
  text: string;
  kind: TokenKind;
}

export type Lang = "csharp" | "rust" | "ts" | "json" | "yaml" | "shader" | "css" | "xml" | "md" | "py" | "sh";

const EXT_MAP: Record<string, Lang> = {
  cs: "csharp",
  rs: "rust",
  ts: "ts",
  tsx: "ts",
  js: "ts",
  jsx: "ts",
  mjs: "ts",
  cjs: "ts",
  svelte: "ts",
  json: "json",
  yml: "yaml",
  yaml: "yaml",
  unity: "yaml",
  prefab: "yaml",
  asset: "yaml",
  meta: "yaml",
  mat: "yaml",
  anim: "yaml",
  controller: "yaml",
  shader: "shader",
  hlsl: "shader",
  cginc: "shader",
  glsl: "shader",
  compute: "shader",
  css: "css",
  scss: "css",
  html: "xml",
  xml: "xml",
  csproj: "xml",
  svg: "xml",
  md: "md",
  py: "py",
  sh: "sh",
  bash: "sh",
  ps1: "sh",
};

const KEYWORDS: Record<Lang, ReadonlySet<string>> = {
  csharp: new Set([
    "abstract", "as", "async", "await", "base", "bool", "break", "byte", "case", "catch", "char", "checked",
    "class", "const", "continue", "decimal", "default", "delegate", "do", "double", "else", "enum", "event",
    "explicit", "extern", "false", "finally", "fixed", "float", "for", "foreach", "get", "goto", "if",
    "implicit", "in", "int", "interface", "internal", "is", "lock", "long", "namespace", "new", "null", "object",
    "operator", "out", "override", "params", "private", "protected", "public", "readonly", "record", "ref",
    "return", "sbyte", "sealed", "set", "short", "sizeof", "stackalloc", "static", "string", "struct", "switch",
    "this", "throw", "true", "try", "typeof", "uint", "ulong", "unchecked", "unsafe", "ushort", "using", "var",
    "virtual", "void", "volatile", "where", "while", "yield",
  ]),
  rust: new Set([
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "Self", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
    "bool", "char", "str", "u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize", "f32", "f64",
  ]),
  ts: new Set([
    "as", "async", "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default",
    "delete", "do", "else", "enum", "export", "extends", "false", "finally", "for", "from", "function", "get",
    "if", "implements", "import", "in", "instanceof", "interface", "let", "new", "null", "of", "private",
    "protected", "public", "readonly", "return", "set", "static", "super", "switch", "this", "throw", "true",
    "try", "type", "typeof", "undefined", "var", "void", "while", "with", "yield",
  ]),
  json: new Set(["true", "false", "null"]),
  yaml: new Set(["true", "false", "null", "yes", "no", "on", "off"]),
  shader: new Set([
    "Shader", "Properties", "SubShader", "Pass", "Tags", "CGPROGRAM", "ENDCG", "HLSLPROGRAM", "ENDHLSL",
    "struct", "return", "if", "else", "for", "while", "void", "float", "float2", "float3", "float4", "half",
    "half2", "half3", "half4", "fixed", "fixed4", "int", "uint", "bool", "float4x4", "sampler2D", "uniform",
    "in", "out", "inout", "const", "pragma", "include", "true", "false",
  ]),
  css: new Set(["important", "from", "to", "and", "or", "not", "only", "media", "keyframes", "import"]),
  xml: new Set([]),
  md: new Set([]),
  py: new Set([
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else",
    "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "None",
    "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while", "with", "yield", "self",
  ]),
  sh: new Set([
    "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "in", "function",
    "return", "export", "local", "true", "false", "exit", "param", "foreach", "switch",
  ]),
};

const MAX_HIGHLIGHT_LEN = 1000;

export function detectLanguage(path: string): Lang | null {
  const dot = path.lastIndexOf(".");
  if (dot < 0 || dot === path.length - 1) {
    return null;
  }
  return EXT_MAP[path.slice(dot + 1).toLowerCase()] ?? null;
}

function push(out: Token[], kind: TokenKind, text: string) {
  if (!text) {
    return;
  }
  const prev = out[out.length - 1];
  if (prev && prev.kind === kind) {
    prev.text += text;
  } else {
    out.push({ kind, text });
  }
}

export function highlightLine(line: string, lang: Lang | null): Token[] {
  if (!line || !lang || line.length > MAX_HIGHLIGHT_LEN) {
    return line ? [{ text: line, kind: "plain" }] : [];
  }
  if (lang === "md") {
    if (/^\s{0,3}#{1,6}\s/.test(line)) {
      return [{ text: line, kind: "kw" }];
    }
    if (/^\s*```/.test(line)) {
      return [{ text: line, kind: "com" }];
    }
  }
  if (lang === "yaml" && line.startsWith("--- ")) {
    return [{ text: line, kind: "kw" }];
  }

  const out: Token[] = [];
  const kws = KEYWORDS[lang];
  const hashComment = lang === "yaml" || lang === "py" || lang === "sh";
  const slashComment = lang === "csharp" || lang === "rust" || lang === "ts" || lang === "shader" || lang === "css";
  let i = 0;

  while (i < line.length) {
    const ch = line[i];

    if (slashComment && ch === "/" && line[i + 1] === "/") {
      push(out, "com", line.slice(i));
      break;
    }
    if (slashComment && ch === "/" && line[i + 1] === "*") {
      const close = line.indexOf("*/", i + 2);
      const end = close >= 0 ? close + 2 : line.length;
      push(out, "com", line.slice(i, end));
      i = end;
      continue;
    }
    if (hashComment && ch === "#") {
      push(out, "com", line.slice(i));
      break;
    }
    if (lang === "xml" && line.startsWith("<!--", i)) {
      const close = line.indexOf("-->", i + 4);
      const end = close >= 0 ? close + 3 : line.length;
      push(out, "com", line.slice(i, end));
      i = end;
      continue;
    }

    if (ch === '"' || ch === "'" || (ch === "`" && lang === "ts")) {
      let j = i + 1;
      while (j < line.length) {
        if (line[j] === "\\" && ch !== "`") {
          j += 2;
          continue;
        }
        if (line[j] === ch) {
          j++;
          break;
        }
        j++;
      }
      const str = line.slice(i, j);
      if (lang === "json" && /^\s*:/.test(line.slice(j))) {
        push(out, "type", str);
      } else {
        push(out, "str", str);
      }
      i = j;
      continue;
    }

    if ((ch >= "0" && ch <= "9") || (ch === "." && line[i + 1] >= "0" && line[i + 1] <= "9")) {
      const rest = line.slice(i);
      const m = rest.match(/^(?:0x[0-9a-fA-F_]+|0b[01_]+|[0-9][0-9_]*(?:\.[0-9_]+)?(?:[eE][+-]?[0-9]+)?[fFdDmMuUlL]*)/);
      if (m) {
        push(out, "num", m[0]);
        i += m[0].length;
        continue;
      }
    }

    if ((ch >= "a" && ch <= "z") || (ch >= "A" && ch <= "Z") || ch === "_" || ch === "$" || ch === "@") {
      const rest = line.slice(i);
      const m = rest.match(/^[@$A-Za-z_][A-Za-z0-9_-]*/);
      if (m) {
        const word = m[0];
        const nextIdx = i + word.length;
        let kind: TokenKind = "plain";
        if (lang === "yaml" && line[nextIdx] === ":") {
          kind = "type";
        } else if (kws.has(word)) {
          kind = "kw";
        } else if (line[nextIdx] === "(") {
          kind = "fn";
        } else if (/^[A-Z][A-Za-z0-9_]*$/.test(word)) {
          kind = "type";
        } else if (lang === "css" && word.startsWith("@")) {
          kind = "kw";
        }
        push(out, kind, word);
        i = nextIdx;
        continue;
      }
    }

    if (lang === "xml" && ch === "<") {
      const m = line.slice(i).match(/^<\/?[A-Za-z0-9:_-]+/);
      if (m) {
        push(out, "kw", m[0]);
        i += m[0].length;
        continue;
      }
    }

    push(out, "plain", ch);
    i++;
  }

  return out;
}

export interface StyledSegment {
  text: string;
  kind: TokenKind;
  changed: boolean;
}

/** Merges syntax tokens and optional intra-line diff spans into disjoint styled segments. */
export function mergeInline(
  tokens: Token[],
  inline?: { text: string; changed: boolean }[],
): StyledSegment[] {
  if (!inline || inline.length === 0) {
    return tokens.map((t) => ({ ...t, changed: false }));
  }
  const out: StyledSegment[] = [];
  let ti = 0;
  let to = 0;
  let ii = 0;
  let io = 0;
  while (ti < tokens.length && ii < inline.length) {
    const t = tokens[ti];
    const span = inline[ii];
    const remT = t.text.length - to;
    const remI = span.text.length - io;
    const take = Math.min(remT, remI);
    if (take > 0) {
      const piece = t.text.slice(to, to + take);
      const prev = out[out.length - 1];
      if (prev && prev.kind === t.kind && prev.changed === span.changed) {
        prev.text += piece;
      } else {
        out.push({ text: piece, kind: t.kind, changed: span.changed });
      }
    }
    to += take;
    io += take;
    if (to >= t.text.length) {
      ti++;
      to = 0;
    }
    if (io >= span.text.length) {
      ii++;
      io = 0;
    }
  }
  return out;
}

