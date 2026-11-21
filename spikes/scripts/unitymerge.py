import subprocess, sys, os, re

REPO = sys.argv[1]
YAML = r"C:\Program Files\Unity\Hub\Editor\6000.3.21f1\Editor\Data\Tools\UnityYAMLMerge.exe"
P = "Assets/Objects/Levels/Island/ReplayCameras.prefab"
V = "Assets/Objects/UI/prefabs/Utility/BaseButton_Finish.prefab"
env = dict(os.environ)
env.pop("GIT_CURL_VERBOSE", None)


def git(*a, check=True):
    r = subprocess.run(["git", *a], cwd=REPO, capture_output=True, text=True, env=env)
    if check and r.returncode:
        raise SystemExit(f"git {a}: {r.stderr}")
    return r


def edit(path, fn):
    full = os.path.join(REPO, path)
    raw = open(full, encoding="utf-8", newline="").read()
    eol = "\r\n" if "\r\n" in raw else "\n"
    lines = raw.split(eol)
    fn(lines)
    open(full, "w", encoding="utf-8", newline="").write(eol.join(lines))


def go(name, src):
    def new_go(lines, tag, child_list_after):
        go_id, tr_id = f"90000000000000{tag}1", f"90000000000000{tag}2"
        i = lines.index(child_list_after)
        lines.insert(i + 1, f"  - {{fileID: {tr_id}}}")
        while lines and lines[-1] == "":
            lines.pop()
        lines += [
            f"--- !u!1 &{go_id}", "GameObject:", "  m_ObjectHideFlags: 0", "  serializedVersion: 6",
            "  m_Component:", f"  - component: {{fileID: {tr_id}}}", "  m_Layer: 0", f"  m_Name: {name}",
            "  m_TagString: Untagged", "  m_IsActive: 1",
            f"--- !u!4 &{tr_id}", "Transform:", "  m_ObjectHideFlags: 0", f"  m_GameObject: {{fileID: {go_id}}}",
            "  serializedVersion: 2", "  m_LocalRotation: {x: 0, y: 0, z: 0, w: 1}",
            f"  m_LocalPosition: {{x: {src}, y: 0, z: 0}}", "  m_LocalScale: {x: 1, y: 1, z: 1}",
            "  m_Children: []", "  m_Father: {fileID: 4573194999906529360}", "",
        ]
    return new_go


SCENARIOS = {
    "S1 adjacent fields, same Transform": (
        lambda l: l.__setitem__(117, l[117].replace("0.3307133", "0.5")),
        lambda l: l.__setitem__(118, l[118].replace("4.409273", "9.5")),
        P,
    ),
    "S2 same field, different values": (
        lambda l: l.__setitem__(118, l[118].replace("4.409273", "1.0")),
        lambda l: l.__setitem__(118, l[118].replace("4.409273", "2.0")),
        P,
    ),
    "S3 both add a child GameObject": (
        lambda l: go("OursCam", 11)(l, "1", "  - {fileID: 8772121759599154040}"),
        lambda l: go("TheirsCam", 22)(l, "2", "  - {fileID: 8772121759599154040}"),
        P,
    ),
}


def variant_edit(value, prop):
    def f(lines):
        i = next(k for k, s in enumerate(lines) if s.strip() == "m_Modifications:")
        lines[i + 1:i + 1] = [
            "    - target: {fileID: 1000000000000000001, guid: 0000000000000000f000000000000000, type: 0}",
            f"      propertyPath: {prop}", f"      value: {value}", "      objectReference: {fileID: 0}",
        ]
    return f


SCENARIOS["S4 prefab variant: both add an override"] = (variant_edit("1.5", "m_LocalScale.x"), variant_edit("0.5", "m_LocalScale.y"), V)
SCENARIOS["S5 prefab variant: same override, different values"] = (variant_edit("1.5", "m_LocalScale.x"), variant_edit("0.5", "m_LocalScale.x"), V)


def run(driver):
    if driver:
        git("config", "merge.unityyamlmerge.name", "Unity SmartMerge")
        git("config", "merge.unityyamlmerge.driver", f'"{YAML.replace(chr(92), "/")}" merge -h -p --force --fallback none %O %B %A %A')
        with open(os.path.join(REPO, ".git", "info", "attributes"), "w") as f:
            f.write("*.prefab merge=unityyamlmerge\n*.unity merge=unityyamlmerge\n")
    else:
        git("config", "--unset-all", "merge.unityyamlmerge.driver", check=False)
        p = os.path.join(REPO, ".git", "info", "attributes")
        if os.path.exists(p):
            os.remove(p)
    for name, (ours, theirs, path) in SCENARIOS.items():
        git("checkout", "-q", "-f", "spike-base")
        git("checkout", "-q", "-B", "spike-theirs")
        edit(path, theirs)
        git("commit", "-q", "-am", "theirs")
        git("checkout", "-q", "-B", "spike-ours", "spike-base")
        edit(path, ours)
        git("commit", "-q", "-am", "ours")
        r = git("merge", "--no-edit", "spike-theirs", check=False)
        text = open(os.path.join(REPO, path), encoding="utf-8").read()
        markers = text.count("<<<<<<<")
        verdict = "clean" if r.returncode == 0 else "CONFLICT"
        detail = ""
        if r.returncode == 0:
            checks = {
                "S1": ["0.5,", "x: 9.5"], "S3": ["OursCam", "TheirsCam"],
                "S4": ["m_LocalScale.x", "m_LocalScale.y"],
            }.get(name[:2], [])
            detail = "both sides kept" if all(c in text for c in checks) else f"LOST CHANGES (missing {[c for c in checks if c not in text]})"
            if name[:2] == "S3":
                ids = re.findall(r"^--- !u!\d+ &(\d+)", text, re.M)
                detail += f"; duplicate fileIDs: {len(ids) - len(set(ids))}"
            if name[:2] in ("S2", "S5"):
                detail = f"SILENT PICK: value lines {re.findall(r'(?:x: |value: )([12]\.[05])', text)[:3]}"
        print(f"{'driver' if driver else 'plain '} | {name:<48} | {verdict:<8} | markers {markers} | {detail}")
        git("merge", "--abort", check=False)
        git("reset", "-q", "--hard")


git("checkout", "-q", "-B", "spike-base", "HEAD")
run(False)
run(True)
git("checkout", "-q", "-f", "spike-base")
