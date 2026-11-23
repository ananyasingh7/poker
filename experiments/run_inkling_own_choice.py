"""Safe launch/resume wrapper. Without --execute, only prints the offline plan."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from inkling_own_choice import PROTOCOL, ROOT, plan
from inkling_overnight import atomic, utc

FILES = ['experiments/inkling_own_choice.py', 'experiments/inkling_overnight.py',
         'experiments/inkling_own_choice_extra.py',
         'experiments/run_inkling_own_choice.py', 'experiments/test_inkling_own_choice.py',
         'examples/12-inkling-own-choice/protocol.json']


def supervise(directory, output):
    # OS lock survives retries and is released if the supervisor dies.
    with (directory / 'supervisor.lock').open('a') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise SystemExit('This run already has an active supervisor')
        snapshot = directory / 'source'
        manifest_path = directory / 'source-manifest.json'
        if not manifest_path.exists():
            snapshot.mkdir(exist_ok=True)
            manifest = {'prepared_at_launch': utc(), 'files': {}}
            for name in FILES:
                content = (ROOT / name).read_bytes()
                (snapshot / Path(name).name).write_bytes(content)
                manifest['files'][name] = hashlib.sha256(content).hexdigest()
            atomic(manifest_path, manifest)
        manifest = json.loads(manifest_path.read_text())
        for name, digest in manifest['files'].items():
            assert hashlib.sha256((snapshot / Path(name).name).read_bytes()).hexdigest() == digest, 'Snapshot changed'
        command = [sys.executable, '-B', str(snapshot / 'inkling_own_choice.py'),
                   str(directory), str(output), '--protocol', str(snapshot / 'protocol.json'), '--execute']
        atomic(directory / 'supervisor.json', {'pid': os.getpid(), 'started': utc()})
        for attempt in range(4):
            started = directory / 'started.json'
            hours = json.loads((snapshot / 'protocol.json').read_text())['max_hours']
            remaining = hours * 3600
            if started.exists():
                remaining -= time.time() - json.loads(started.read_text())['unix']
            if remaining <= 0:
                break
            with (directory / 'worker.log').open('a') as log:
                process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
                atomic(directory / 'worker-pid.json', {'pid': process.pid, 'attempt': attempt + 1})
                awake = subprocess.Popen(['/usr/bin/caffeinate', '-i', '-w', str(process.pid)],
                                         stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                try:
                    # One in-flight request may finish after the deadline; allow report export.
                    code = process.wait(timeout=remaining + 240)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    try:
                        process.wait(timeout=30)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
                    code = 124
                finally:
                    awake.terminate()
                    awake.wait()
            if code == 0:
                atomic(directory / 'supervisor-complete.json', {'complete': True, 'finished': utc()})
                return
            if attempt < 3:
                time.sleep(10)
        atomic(directory / 'supervisor-stopped.json', {'complete': False, 'finished': utc(),
                                                     'reason': 'Budget or bounded retries exhausted; inspect summary and worker.log'})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', nargs='?', type=Path, default=ROOT / 'runs/inkling-own-choice-v1')
    parser.add_argument('output', nargs='?', type=Path, default=ROOT / 'results/12-inkling-own-choice')
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--supervise', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if not args.execute:
        print(json.dumps(plan(json.loads(PROTOCOL.read_text())), indent=2))
        return
    directory, output = args.directory.resolve(), args.output.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    if args.supervise:
        supervise(directory, output)
        return
    with (directory / 'supervisor.log').open('a') as log:
        process = subprocess.Popen([sys.executable, '-B', str(Path(__file__).resolve()),
                                    str(directory), str(output), '--execute', '--supervise'],
                                   start_new_session=True, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
    print(f'Launch requested; supervisor PID {process.pid}. Logs: {directory}')


if __name__ == '__main__':
    main()
