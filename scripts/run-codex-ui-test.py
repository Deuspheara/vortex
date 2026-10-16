"""Launch the normal app with an isolated store/workspace for native UI QA.

Build first: cargo build -p app
Default uses the installed Codex CLI and its existing authentication.
--fixture uses the deterministic protocol peer, without inference or credentials.
"""
import argparse
import os
import pathlib
import plistlib
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--fixture", action="store_true")
parser.add_argument("--signed-out", action="store_true")
parser.add_argument("--missing-cli", action="store_true")
args = parser.parse_args()
if args.signed_out and not args.fixture:
    parser.error("--signed-out requires --fixture; real credentials are never changed")
repo = pathlib.Path(__file__).resolve().parents[1]
root = pathlib.Path(tempfile.mkdtemp(prefix="vortex-ui-"))
workspace = root / "workspace"
workspace.mkdir()
env = {key: os.environ[key] for key in ("HOME", "USER", "LOGNAME", "PATH", "TMPDIR", "SHELL", "LANG") if key in os.environ}
env["VORTEX_DATA_DIR"] = str(root / "data")
env["DYLD_LIBRARY_PATH"] = ":".join(str(p) for p in (repo / "target/debug/build").glob("libghostty-vt-sys-*/out/ghostty-install/lib"))
if args.fixture:
    peer = root / "codex"
    shutil.copy2(repo / "crates/agent_codex/tests/fake_server.py", peer)
    peer.chmod(0o700)
    env["VORTEX_CODEX_BIN"] = str(peer)
    if args.signed_out:
        (root / "signed-out").touch()
if args.missing_cli:
    env["VORTEX_CODEX_BIN"] = str(root / "missing-codex")
bundle = root / "Vortex QA.app"
(bundle / "Contents/MacOS").mkdir(parents=True)
shutil.copy2(repo / "target/debug/app", bundle / "Contents/MacOS/app")
with (bundle / "Contents/Info.plist").open("wb") as f:
    plistlib.dump({"CFBundleExecutable": "app", "CFBundleIdentifier": "dev.vortex.qa", "CFBundleName": "Vortex QA", "CFBundlePackageType": "APPL", "NSHighResolutionCapable": True}, f)
print(f"QA directory: {root}", flush=True)
print(f"App bundle: {bundle}", flush=True)
subprocess.run([str(bundle / "Contents/MacOS/app")], cwd=workspace, env=env, check=True)
