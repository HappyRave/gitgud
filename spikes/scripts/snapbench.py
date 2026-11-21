"""S1: op-log snapshot cost on a Unity + LFS repo, via the git CLI.

usage: python snapbench.py <source-boat-repo> <work-dir>
Clones the source (no smudge), copies LFS objects, checks out, then times snapshot variants.
"""
import os, shutil, subprocess, sys, time, statistics

SRC, WORK = sys.argv[1], sys.argv[2]
REPO = os.path.join(WORK, "boat")
ENV = {k: v for k, v in os.environ.items() if k != "GIT_CURL_VERBOSE"}


def git(*a, env=None, check=True):
    r = subprocess.run(["git", *a], cwd=REPO, capture_output=True, text=True, env=env or ENV)
    if check and r.returncode:
        sys.exit(f"git {' '.join(a)}: {r.stderr}")
    return r.stdout.strip()


def timed(fn, n=1):
    xs = []
    for _ in range(n):
        t = time.perf_counter()
        out = fn()
        xs.append((time.perf_counter() - t) * 1000)
    return out, xs


def du(path):
    total = 0
    for root, _, files in os.walk(path):
        for f in files:
            try:
                total += os.path.getsize(os.path.join(root, f))
            except OSError:
                pass
    return total


def snapshot_cli(fresh_index=True):
    """Temp index = copy of the real index, add -A, write-tree; plus the real index tree; one commit."""
    gitdir = os.path.join(REPO, ".git")
    tmp = os.path.join(gitdir, "gitgud-snap-index")
    phases = {}
    t = time.perf_counter()
    if fresh_index or not os.path.exists(tmp):
        shutil.copyfile(os.path.join(gitdir, "index"), tmp)
    phases["copy"] = (time.perf_counter() - t) * 1000
    env = dict(ENV, GIT_INDEX_FILE=tmp)
    t = time.perf_counter()
    git("add", "-A", env=env)
    phases["add-A"] = (time.perf_counter() - t) * 1000
    t = time.perf_counter()
    wt = git("write-tree", env=env)
    idx = git("write-tree")
    phases["write-tree x2"] = (time.perf_counter() - t) * 1000
    t = time.perf_counter()
    head = git("rev-parse", "HEAD")
    parent = git("rev-parse", "-q", "--verify", "refs/gitgud/oplog", check=False)
    args = ["commit-tree", wt, "-p", head] + (["-p", parent] if parent else []) + ["-m", f"snap idx={idx}"]
    c = git(*args)
    git("update-ref", "refs/gitgud/oplog", c)
    phases["commit+ref"] = (time.perf_counter() - t) * 1000
    return phases


def snapshot_stash_create():
    return git("stash", "create")


def report(name, runs):
    tot = [sum(p.values()) for p in runs]
    keys = runs[0].keys()
    parts = ", ".join(f"{k} {statistics.median([r[k] for r in runs]):.0f}" for k in keys)
    print(f"{name:42s} median {statistics.median(tot):7.0f} ms  (min {min(tot):.0f}, max {max(tot):.0f})  [{parts}]")


def main():
    if os.path.exists(WORK):
        shutil.rmtree(WORK)
    os.makedirs(WORK)
    env = dict(ENV, GIT_LFS_SKIP_SMUDGE="1")
    subprocess.run(["git", "clone", "-q", "--local", SRC, REPO], env=env, check=True, capture_output=True)
    shutil.copytree(os.path.join(SRC, ".git", "lfs", "objects"), os.path.join(REPO, ".git", "lfs", "objects"), dirs_exist_ok=True)
    git("lfs", "checkout")
    git("config", "user.name", "bench")
    git("config", "user.email", "bench@example.invalid")
    git("status", "--porcelain")  # settle the index once
    print("git", git("--version"), "| lfs", git("lfs", "version").split()[0])

    # spawn cost
    _, xs = timed(lambda: git("--version"), 20)
    print(f"{'spawn: git --version':42s} median {statistics.median(xs):7.1f} ms")
    _, xs = timed(lambda: git("rev-parse", "HEAD"), 20)
    print(f"{'spawn: git rev-parse HEAD':42s} median {statistics.median(xs):7.1f} ms")

    lfs = [l.split(" ", 2)[2] for l in git("lfs", "ls-files").splitlines()]
    lfs_sizes = sorted(((os.path.getsize(os.path.join(REPO, p)), p) for p in lfs), reverse=True)
    cs = [p for p in git("ls-files", "*.cs").splitlines()][:3]

    # stage-hunk cost: append a line to a .cs, stage via apply --cached of a one-hunk patch, reset
    target = os.path.join(REPO, cs[0])
    orig = open(target, "rb").read()
    open(target, "ab").write(b"\n// bench line\n")
    patch = subprocess.run(["git", "diff", "--", cs[0]], cwd=REPO, env=ENV, capture_output=True, check=True).stdout
    xs = []
    for _ in range(10):
        t = time.perf_counter()
        subprocess.run(["git", "apply", "--cached", "-"], cwd=REPO, input=patch, env=ENV, check=True, capture_output=True)
        xs.append((time.perf_counter() - t) * 1000)
        git("reset", "-q", "--", cs[0])
    print(f"{'stage hunk: git apply --cached':42s} median {statistics.median(xs):7.1f} ms")
    open(target, "wb").write(orig)
    git("status", "--porcelain")

    sizes0 = (du(os.path.join(REPO, ".git", "objects")), du(os.path.join(REPO, ".git", "lfs", "objects")))

    report("snapshot, clean", [snapshot_cli() for _ in range(5)])
    _, xs = timed(snapshot_stash_create, 5)
    print(f"{'stash create, clean (reference)':42s} median {statistics.median(xs):7.0f} ms")

    for p in cs:
        open(os.path.join(REPO, p), "ab").write(b"\n// dirty\n")
    report("snapshot, 3 dirty .cs", [snapshot_cli() for _ in range(5)])

    big = [p for _, p in lfs_sizes[:1]]
    for p in big:
        open(os.path.join(REPO, p), "ab").write(b"\0")
    print(f"  (dirty LFS file: {lfs_sizes[0][0] / 1e6:.1f} MB)")
    report("snapshot, +1 changed LFS file", [snapshot_cli() for _ in range(3)])

    twenty = [p for _, p in lfs_sizes[:20]]
    mb = sum(s for s, _ in lfs_sizes[:20]) / 1e6
    for p in twenty[1:]:
        open(os.path.join(REPO, p), "ab").write(b"\0")
    report(f"snapshot, 20 changed LFS files ({mb:.0f} MB)", [snapshot_cli() for _ in range(3)])

    sizes1 = (du(os.path.join(REPO, ".git", "objects")), du(os.path.join(REPO, ".git", "lfs", "objects")))
    print(f"  .git/objects +{(sizes1[0] - sizes0[0]) / 1e6:.1f} MB, .git/lfs/objects +{(sizes1[1] - sizes0[1]) / 1e6:.1f} MB after the snapshots above")

    # restore content, then mass touch (Unity reimport: same content, new mtime)
    git("checkout", "--", ".")
    git("status", "--porcelain")
    now = time.time()
    for p in lfs:
        os.utime(os.path.join(REPO, p), (now, now))
    report("snapshot after mass touch, 1st", [snapshot_cli()])
    report("snapshot after mass touch, 2nd (fresh copy)", [snapshot_cli()])
    report("snapshot after mass touch, kept snap index", [snapshot_cli(fresh_index=False)])
    _, xs = timed(lambda: git("update-index", "-q", "--refresh", check=False))
    print(f"{'update-index --refresh (real index)':42s}        {xs[0]:7.0f} ms")
    report("snapshot after refresh", [snapshot_cli() for _ in range(3)])


main()
