import os, shutil, subprocess, sys
from PIL import Image

SRC = r"C:\Users\Happy\AppData\Local\Temp\claude\fc3c5e93275cfb3e\48ead373-37a7-4f14-a750-ad033daccf8a\scratchpad\repos\boat"
ROOT = r"C:\Users\Happy\Documents\Projects\gitgud-handson"
REPO = os.path.join(ROOT, "boat")
REMOTE = os.path.join(ROOT, "remote.git")
ENGINE = "Assets/Scripts/Boat/Engine.cs"
PREFAB = "Assets/Objects/Levels/Island/ReplayCameras.prefab"
env = dict(os.environ)
env.pop("GIT_CURL_VERBOSE", None)


def git(*a, cwd=REPO, extra=None):
    e = dict(env, **(extra or {}))
    r = subprocess.run(["git", *a], cwd=cwd, capture_output=True, text=True, env=e)
    if r.returncode:
        sys.exit(f"git {' '.join(a)} failed: {r.stderr}")
    return r.stdout


def edit(path, fn):
    full = os.path.join(REPO, path)
    raw = open(full, encoding="utf-8", newline="").read()
    eol = "\r\n" if "\r\n" in raw else "\n"
    lines = raw.split(eol)
    fn(lines)
    open(full, "w", encoding="utf-8", newline="").write(eol.join(lines))


def commit(msg):
    git("commit", "-q", "-am", msg)


if os.path.exists(ROOT):
    sys.exit(f"{ROOT} exists; delete it first")
os.makedirs(ROOT)
git("clone", "-q", "--bare", "--branch", "master", SRC, REMOTE, cwd=ROOT, extra={"GIT_LFS_SKIP_SMUDGE": "1"})
git("clone", "-q", REMOTE, REPO, cwd=ROOT, extra={"GIT_LFS_SKIP_SMUDGE": "1", "GIT_CLONE_PROTECTION_ACTIVE": "false"})
shutil.copytree(os.path.join(SRC, ".git", "lfs", "objects"), os.path.join(REPO, ".git", "lfs", "objects"), dirs_exist_ok=True)
shutil.copytree(os.path.join(SRC, ".git", "lfs", "objects"), os.path.join(REMOTE, "lfs", "objects"), dirs_exist_ok=True)
git("config", "core.longpaths", "true")
git("lfs", "checkout")
git("config", "user.name", "Hands-on Tester")
git("config", "user.email", "tester@example.invalid")

# Stack for interactive rebase (T4)
git("checkout", "-q", "-b", "feature/stack")
edit(ENGINE, lambda l: l.insert(5, "// Engine: tuned thrust curve"))
commit("WIP engine thrust tweak")
edit(ENGINE, lambda l: l.__setitem__(5, "// Engine: tuned thrust curve (smoothed)"))
commit("fixup! WIP engine thrust tweak")
edit(PREFAB, lambda l: l.__setitem__(118, l[118].replace("4.409273", "5.0")))
commit("Move blimp camera rgiht")

# Cherry-pick source (T7)
git("checkout", "-q", "-b", "hotfix/camera", "master")
edit(PREFAB, lambda l: l.__setitem__(267, l[267].replace("{x: 0, y: 0, z: 0}", "{x: 0, y: 1.5, z: 0}")))
commit("Raise ClearShot camera rig")

# Conflict pair (T5): code conflict + prefab conflicts (one smart-mergeable, one real)
git("checkout", "-q", "-b", "conflict/theirs", "master")
edit(ENGINE, lambda l: l.insert(5, "// theirs: engine uses torque table"))
edit(PREFAB, lambda l: (l.__setitem__(118, l[118].replace("4.409273", "2.0")), l.__setitem__(370, l[370].replace("-0, y: -0", "-0, y: 0.25"))))
commit("Theirs: torque table, move blimp")
git("checkout", "-q", "-b", "conflict/ours", "master")
edit(ENGINE, lambda l: l.insert(5, "// ours: engine uses power curve"))
edit(PREFAB, lambda l: (l.__setitem__(118, l[118].replace("4.409273", "1.0")), l.__setitem__(371, l[371].replace("-38.55", "-40"))))
commit("Ours: power curve, move blimp")

git("push", "-q", "--all", "origin")
git("checkout", "-q", "master")

# Uncommitted work on master (T2 hunk staging, T3 image diff)
edit(ENGINE, lambda l: (l.insert(5, "// hunk A: keep"), l.insert(len(l) - 3, "        // hunk B: leave unstaged")))
png = next(p for p in git("lfs", "ls-files", "-n").splitlines() if p.lower().endswith(".png") and "Icon" not in p)
img = Image.open(os.path.join(REPO, png)).convert("RGBA")
r, g, b, a = img.split()
Image.merge("RGBA", (b, g, r, a)).save(os.path.join(REPO, png))
print("repo ready:", REPO)
print("modified texture:", png)
print(git("log", "--oneline", "--all", "--graph", "-12"))
print(git("status", "--short"))
