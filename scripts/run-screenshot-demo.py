"""Launch the debug demo with an isolated store and no provider keys.

Build first: cargo build -p app --features demo_mode
"""
import argparse
import os
import pathlib
import plistlib
import re
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument(
    "--window-size", default="1600x1000", help="WIDTHxHEIGHT in logical pixels"
)
args = parser.parse_args()
match = re.fullmatch(r"(\d+)x(\d+)", args.window_size)
if not match or int(match[1]) < 640 or int(match[2]) < 480:
    parser.error("--window-size must be WIDTHxHEIGHT, at least 640x480")

repo = pathlib.Path(__file__).resolve().parents[1]
binary = repo / "target/debug/app"
if not binary.is_file():
    parser.error("build the demo first: cargo build -p app --features demo_mode")
root = pathlib.Path(tempfile.mkdtemp(prefix="vortex-screenshot-"))
(root / ".screenshot-demo").touch()
(root / "atlas-notes").mkdir()
env = {
    key: os.environ[key]
    for key in ("HOME", "USER", "LOGNAME", "PATH", "TMPDIR", "SHELL", "LANG")
    if key in os.environ
}
env["VORTEX_DEMO_DATA_DIR"] = str(root)
# Enable the existing debug window-size override; demo storage still uses the marker above.
env["VORTEX_DATA_DIR"] = str(root)
env["VORTEX_TEST_WINDOW_SIZE"] = args.window_size
# Cargo supplies dependency dylib paths during cargo run; reproduce that for a direct launch.
env["DYLD_LIBRARY_PATH"] = ":".join(
    str(p)
    for p in (repo / "target/debug/build").glob(
        "libghostty-vt-sys-*/out/ghostty-install/lib"
    )
)
bundle = root / "Vortex Demo.app"
(bundle / "Contents/MacOS").mkdir(parents=True)
shutil.copy2(binary, bundle / "Contents/MacOS/app")
with (bundle / "Contents/Info.plist").open("wb") as f:
    plistlib.dump(
        {
            "CFBundleExecutable": "app",
            "CFBundleIdentifier": "dev.vortex.screenshot",
            "CFBundleName": "Vortex Demo",
            "CFBundlePackageType": "APPL",
            "NSHighResolutionCapable": True,
        },
        f,
    )
print(f"Demo data directory: {root}", flush=True)
print(f"Demo bundle: {bundle}", flush=True)
subprocess.run(
    [str(bundle / "Contents/MacOS/app")], cwd=root / "atlas-notes", env=env, check=True
)
