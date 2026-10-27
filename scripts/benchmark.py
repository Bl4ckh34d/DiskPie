"""Benchmark only a generated temporary fixture; print aggregate data only."""
import argparse
from pathlib import Path
import statistics
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('executable', type=Path)
args = parser.parse_args()
executable = args.executable.resolve()
with tempfile.TemporaryDirectory(prefix='diskpie-synthetic-bench-') as temporary:
    root = Path(temporary)
    expected = 0
    for group in range(100):
        folder = root / f'group-{group:03}'
        folder.mkdir()
        for file in range(200):
            size = (group * 200 + file) % 1024
            (folder / f'sample-{file:04}.bin').write_bytes(bytes(size))
            expected += size
    print(f'Synthetic fixture: files=20000 folders=101 bytes={expected}')
    for workers in [1, 4, 8]:
        samples = []
        for _ in range(3):
            result = subprocess.check_output([str(executable), '--benchmark', str(root), str(workers)], text=True)
            counts = dict(part.split('=', 1) for part in result.split())
            assert int(counts['files']) == 20_000 and int(counts['folders']) == 101
            assert int(counts['bytes']) == expected and int(counts['errors']) == 0
            samples.append(float(counts['seconds']))
        print(f'workers={workers} median_seconds={statistics.median(samples):.6f}')
