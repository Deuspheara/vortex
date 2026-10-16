"""Launch the real Debug demo binary with a fresh isolated store and no provider keys."""
import os, pathlib, subprocess, tempfile, plistlib, shutil
repo=pathlib.Path(__file__).resolve().parents[1]
root=pathlib.Path(tempfile.mkdtemp(prefix='vortex-portfolio-'))
(root/'.portfolio-demo').touch()
(root/'atlas-notes').mkdir()
env={key:os.environ[key] for key in ('HOME','USER','LOGNAME','PATH','TMPDIR','SHELL','LANG') if key in os.environ}
env['VORTEX_DEMO_DATA_DIR']=str(root)
# Cargo supplies dependency dylib paths during cargo run; reproduce that for a direct launch.
env['DYLD_LIBRARY_PATH']=':'.join(str(p) for p in (repo/'target/debug/build').glob('libghostty-vt-sys-*/out/ghostty-install/lib'))
bundle=root/'Vortex Demo.app'
(bundle/'Contents/MacOS').mkdir(parents=True)
shutil.copy2(repo/'target/debug/app',bundle/'Contents/MacOS/app')
with (bundle/'Contents/Info.plist').open('wb') as f:
    plistlib.dump({'CFBundleExecutable':'app','CFBundleIdentifier':'dev.vortex.portfolio','CFBundleName':'Vortex Demo','CFBundlePackageType':'APPL','NSHighResolutionCapable':True},f)
print(f'Demo data directory: {root}',flush=True)
print(f'Demo bundle: {bundle}',flush=True)
subprocess.run([str(bundle/'Contents/MacOS/app')],cwd=root/'atlas-notes',env=env,check=True)
