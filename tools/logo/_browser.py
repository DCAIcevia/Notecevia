"""用无头浏览器把 SVG 栅格化（只 Windows 需要，Edge 或 Chrome 都行）。"""
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

CANDIDATES = [
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
]


def find_browser():
    for path in CANDIDATES:
        if Path(path).exists():
            return path
    found = shutil.which("msedge") or shutil.which("chrome")
    if not found:
        raise SystemExit("没找到 Edge/Chrome，无法栅格化")
    return found


def render(svg, png, width, height):
    svg, png = Path(svg).resolve(), Path(png).resolve()
    if png.exists():
        png.unlink()
    profile = tempfile.mkdtemp(prefix="notecevia-logo-")
    try:
        subprocess.run(
            [
                find_browser(),
                "--headless=new",
                "--disable-gpu",
                "--hide-scrollbars",
                "--force-device-scale-factor=1",
                f"--window-size={width},{height}",
                f"--user-data-dir={profile}",
                f"--screenshot={png}",
                svg.as_uri(),
            ],
            capture_output=True,
            timeout=90,
            check=False,
        )
        deadline = time.time() + 20
        while not png.exists() and time.time() < deadline:
            time.sleep(0.2)
    finally:
        shutil.rmtree(profile, ignore_errors=True)
    if not png.exists():
        raise SystemExit(f"栅格化失败: {svg.name}")
    return png
