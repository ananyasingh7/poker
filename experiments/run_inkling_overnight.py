"""Detached overnight supervisor. Writes its PID, keeps the Mac awake, and resumes failures."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    directory, output = args.directory.resolve(), args.output.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    files = ['experiments/inkling_overnight.py', 'experiments/test_inkling_overnight.py',
             'experiments/run_inkling_overnight.py', 'examples/11-inkling-overnight/protocol.json']
    manifest = {'launched': datetime.now(timezone.utc).isoformat(),
                'files': {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in files},
                'purpose': 'Frozen worker source and protocol for repeatable resume'}
    if not (directory / 'source-manifest.json').exists():
        (directory / 'source-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    # Preserve the exact source so a later workspace edit cannot change a resume.
    snapshot = directory / 'source'
    snapshot.mkdir(exist_ok=True)
    for name in files:
        target = snapshot / Path(name).name
        if not target.exists():
            target.write_bytes((ROOT / name).read_bytes())
    command = [sys.executable, '-B', str(snapshot / 'inkling_overnight.py'), str(directory), str(output),
               '--protocol', str(snapshot / 'protocol.json')]
    for attempt in range(4):
        with (directory / 'worker.log').open('a') as log:
            log.write(f'\nWorker attempt {attempt + 1} at {datetime.now(timezone.utc).isoformat()}\n')
            log.flush()
            process = subprocess.Popen(command, cwd=ROOT, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            (directory / 'worker-pid.json').write_text(json.dumps({'pid': process.pid, 'attempt': attempt + 1}) + '\n')
            awake = subprocess.Popen(['/usr/bin/caffeinate', '-i', '-w', str(process.pid)],
                                     stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            code = process.wait()
            try:
                awake.wait(timeout=15)
            except subprocess.TimeoutExpired:
                awake.terminate()
        if code == 0:
            (directory / 'supervisor-complete.json').write_text(json.dumps({'complete': True, 'finished': datetime.now(timezone.utc).isoformat()}) + '\n')
            return
        if (directory / 'started.json').exists():
            started = json.loads((directory / 'started.json').read_text())['unix']
            hours = json.loads((snapshot / 'protocol.json').read_text())['max_hours']
            if time.time() >= started + hours * 3600:
                break
        if attempt < 3:
            time.sleep(30)
    raise SystemExit('Overnight worker stopped after bounded retries; see summary.md and worker.log')


if __name__ == '__main__':
    main()
