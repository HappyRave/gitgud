import { describe, expect, it } from "vitest";
import { detectLanguage, highlightLine, mergeInline } from "./highlight";

describe("detectLanguage", () => {
  it("recognizes Unity, code, and shader extensions", () => {
    expect(detectLanguage("Assets/Scripts/Engine.cs")).toBe("csharp");
    expect(detectLanguage("Assets/Prefabs/Rig.prefab")).toBe("yaml");
    expect(detectLanguage("crates/core/src/lib.rs")).toBe("rust");
    expect(detectLanguage("Shaders/Water.shader")).toBe("shader");
    expect(detectLanguage("README")).toBeNull();
  });
});

describe("highlightLine", () => {
  it("preserves exact text while tagging keywords, strings, comments, numbers, and calls", () => {
    const line = 'public float speed = 12.5f; // update "fast"';
    const tokens = highlightLine(line, "csharp");
    expect(tokens.map((t) => t.text).join("")).toBe(line);
    expect(tokens.find((t) => t.text === "public")?.kind).toBe("kw");
    expect(tokens.find((t) => t.text === "12.5f")?.kind).toBe("num");
    expect(tokens.find((t) => t.kind === "com")?.text).toContain('// update "fast"');
  });

  it("highlights YAML/Unity keys and JSON property names", () => {
    const yaml = "  m_LocalPosition: {x: 1.5, y: 2}";
    const yTokens = highlightLine(yaml, "yaml");
    expect(yTokens.map((t) => t.text).join("")).toBe(yaml);
    expect(yTokens.find((t) => t.text === "m_LocalPosition")?.kind).toBe("type");

    const json = '  "name": "gitgud",';
    const jTokens = highlightLine(json, "json");
    expect(jTokens.find((t) => t.text === '"name"')?.kind).toBe("type");
    expect(jTokens.find((t) => t.text === '"gitgud"')?.kind).toBe("str");
  });

  it("skips highlighting oversized lines safely", () => {
    const huge = "var x = 1; ".repeat(150);
    expect(highlightLine(huge, "ts")).toEqual([{ text: huge, kind: "plain" }]);
  });

  it("merges syntax tokens with intra-line change spans in lockstep", () => {
    const tokens = highlightLine("let value = 42;", "rust");
    const merged = mergeInline(tokens, [
      { text: "let value = ", changed: false },
      { text: "42", changed: true },
      { text: ";", changed: false },
    ]);
    expect(merged.map((s) => s.text).join("")).toBe("let value = 42;");
    expect(merged.find((s) => s.text === "let")?.kind).toBe("kw");
    expect(merged.find((s) => s.text === "42")?.changed).toBe(true);
    expect(merged.find((s) => s.text === "42")?.kind).toBe("num");
  });
});
