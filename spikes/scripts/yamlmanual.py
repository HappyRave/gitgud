import subprocess, os, sys

S = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPO = os.path.join(S, "repos", "boat")
D = os.path.join(S, "ym")
YAML = r"C:\Program Files\Unity\Hub\Editor\6000.3.21f1\Editor\Data\Tools\UnityYAMLMerge.exe"
F = "Assets/Objects/Levels/Island/ReplayCameras.prefab"
os.makedirs(D, exist_ok=True)
base = subprocess.run(["git", "show", f"spike-base:{F}"], cwd=REPO, capture_output=True).stdout
eol = b"\r\n" if b"\r\n" in base else b"\n"


def variant(name, ln, a, b):
    lines = base.split(eol)
    lines[ln] = lines[ln].replace(a, b)
    p = os.path.join(D, name)
    open(p, "wb").write(eol.join(lines))
    return p


b = os.path.join(D, "base.prefab")
open(b, "wb").write(base)
ours = variant("ours.prefab", 117, b"0.3307133", b"0.5")
theirs = variant("theirs.prefab", 118, b"4.409273", b"9.5")

if len(sys.argv) > 1:
    ours = variant("ours2.prefab", 118, b"4.409273", b"1.0")
    theirs = variant("theirs2.prefab", 118, b"4.409273", b"2.0")
    out, conf = os.path.join(D, "out4.prefab"), os.path.join(D, "conflicts.txt")
    r = subprocess.run([YAML, "merge", "-h", "--force", "--fallback", "none", "-o", conf, "--describe", b, theirs, ours, out], capture_output=True, text=True)
    print("exit", r.returncode, "|", r.stdout.strip()[:300])
    print(open(conf, encoding="utf-8", errors="replace").read()[:1500] if os.path.exists(conf) else "no conflict file")
    sys.exit()

for label, args, dest in [
    ("-h -p, dest=ours (driver style)", ["-h", "-p", "--fallback", "none"], "inplace"),
    ("-h, separate dest", ["-h", "--fallback", "none"], "out2.prefab"),
    ("-h -p, separate dest", ["-h", "-p", "--fallback", "none"], "out3.prefab"),
]:
    if dest == "inplace":
        out = os.path.join(D, "out1.prefab")
        open(out, "wb").write(open(ours, "rb").read())
        cmd = [YAML, "merge", *args, b, theirs, out, out]
    else:
        out = os.path.join(D, dest)
        cmd = [YAML, "merge", *args, b, theirs, ours, out]
    r = subprocess.run(cmd, capture_output=True, text=True)
    text = open(out, encoding="utf-8").read() if os.path.exists(out) else ""
    print(f"{label:<34} exit {r.returncode}  ours-kept {'0.5,' in text}  theirs-kept {'x: 9.5' in text}  out: {r.stdout.strip()[:200]} {r.stderr.strip()[:200]}")
