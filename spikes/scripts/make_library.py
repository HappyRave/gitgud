"""S2 fixture: N synthetic repos (300 files, 100 commits each) with upstream tracking, some ahead/behind, some dirty.

usage: python make_library.py <out-dir> [n=150]
Writes <out-dir>/repos.txt with one path per line.
"""
import os, shutil, subprocess, sys

OUT = sys.argv[1]
N = int(sys.argv[2]) if len(sys.argv) > 2 else 150
ENV = {k: v for k, v in os.environ.items() if k != "GIT_CURL_VERBOSE"}


def git(cwd, *a, data=None):
    r = subprocess.run(["git", *a], cwd=cwd, input=data, capture_output=True, env=ENV)
    if r.returncode:
        sys.exit(f"git {' '.join(a)}: {r.stderr.decode()}")
    return r.stdout.decode().strip()


def template(path):
    os.makedirs(path)
    git(path, "init", "-q", "-b", "main")
    lines = []
    mark = 0
    for c in range(100):
        mark += 1
        files = range(300) if c == 0 else [(c * 7 + k) % 300 for k in range(5)]
        lines.append(f"commit refs/heads/main\nmark :{mark}\ncommitter bench <bench@example.invalid> {1700000000 + c * 3600} +0000\n")
        msg = f"commit {c}\n"
        lines.append(f"data {len(msg)}\n{msg}")
        if c:
            lines.append(f"from :{mark - 1}\n")
        for f in files:
            body = f"// file {f} rev {c}\n" + "x" * 2000 + "\n"
            lines.append(f"M 100644 inline src/dir{f % 12}/file{f}.cs\ndata {len(body)}\n{body}")
        lines.append("\n")
    git(path, "fast-import", "--quiet", data="".join(lines).encode())
    git(path, "checkout", "-q", "-f", "main")
    git(path, "remote", "add", "origin", "https://example.invalid/repo.git")
    git(path, "config", "branch.main.remote", "origin")
    git(path, "config", "branch.main.merge", "refs/heads/main")


def main():
    if os.path.exists(OUT):
        shutil.rmtree(OUT, ignore_errors=True)
    os.makedirs(OUT)
    tpl = os.path.join(OUT, "_template")
    template(tpl)
    heads = git(tpl, "rev-list", "main").split()  # newest first
    paths = []
    for i in range(N):
        p = os.path.join(OUT, f"repo{i:03d}")
        shutil.copytree(tpl, p)
        # upstream: equal, behind by k, or ahead by k (fake remote ref pointing at an older commit)
        k = i % 7
        git(p, "update-ref", "refs/remotes/origin/main", heads[k] if i % 3 else heads[0])
        if i % 10 == 0:
            with open(os.path.join(p, "src", "dir0", "file0.cs"), "a") as f:
                f.write("// dirty\n")
        git(p, "status", "--porcelain")  # settle the index
        paths.append(p)
    with open(os.path.join(OUT, "repos.txt"), "w") as f:
        f.write("\n".join(paths) + "\n")
    print(f"{N} repos in {OUT}")


main()
