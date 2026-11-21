import subprocess, sys, os, tempfile
from PIL import Image

EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
OUT = r"C:\Users\Happy\Documents\Projects\gitgud\docs\research\img"

# usage: python shot.py <url> <name> [width=1440] [height=1000] [crop_top=0] [crop_bottom=height]
url, name = sys.argv[1], sys.argv[2]
w = int(sys.argv[3]) if len(sys.argv) > 3 else 1440
h = int(sys.argv[4]) if len(sys.argv) > 4 else 1000
top = int(sys.argv[5]) if len(sys.argv) > 5 else 0
bottom = int(sys.argv[6]) if len(sys.argv) > 6 else h

tmp = tempfile.mkdtemp()
png = os.path.join(tmp, "s.png")
subprocess.run([EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--mute-audio",
                f"--window-size={w},{h}", "--virtual-time-budget=10000",
                f"--user-data-dir={os.path.join(tmp, 'prof')}", f"--screenshot={png}", url],
               capture_output=True, timeout=90)
if not os.path.exists(png):
    sys.exit("screenshot failed")
img = Image.open(png).convert("RGB").crop((0, top, w, bottom))
if img.width > 1440:
    img = img.resize((1440, int(img.height * 1440 / img.width)))
os.makedirs(OUT, exist_ok=True)
dst = os.path.join(OUT, name + ".webp")
img.save(dst, "WEBP", quality=78)
print(dst, img.size, os.path.getsize(dst))
