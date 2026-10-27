"""Package upstream dependency licenses without including local manifest paths."""
import argparse
import json
from pathlib import Path
import subprocess
import re
import urllib.request
import urllib.error

parser = argparse.ArgumentParser()
parser.add_argument('--target', required=True)
parser.add_argument('--output', required=True)
parser.add_argument('--cargo', default='cargo')
args = parser.parse_args()
metadata = json.loads(subprocess.check_output([
    args.cargo, 'metadata', '--locked', '--format-version', '1', '--filter-platform', args.target]))
resolved = {node['id'] for node in metadata['resolve']['nodes']}
sections = ['DiskPie third-party notices\nUpstream license texts for resolved build dependencies.\n']
missing = []
for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    if package['id'] not in resolved or package['name'] == 'diskpie':
        continue
    root = Path(package['manifest_path']).parent
    files = set()
    for pattern in ('LICENSE*', 'LICENCE*', 'COPYING*', 'NOTICE*', 'license*', '*OFL*', '*UFL*'):
        files.update(p for p in root.rglob(pattern) if p.is_file())
    if package.get('license_file'):
        path = root / package['license_file']
        if path.is_file(): files.add(path)
    if package['name'] == 'epaint_default_fonts':
        files.update(root.rglob('*.txt'))
    heading = f"{package['name']} {package['version']} — {package.get('license') or 'See license text'}"
    sections.append('\n' + '=' * 72 + '\n' + heading + '\n')
    # Some workspace crates omit their repository-level license in crates.io archives.
    # Fetch only pinned upstream license texts, never local source/inventory data.
    if not files or package['name'] == 'epaint_default_fonts':
        vcs = root / '.cargo_vcs_info.json'
        sha = json.loads(vcs.read_text())['git']['sha1'] if vcs.is_file() else ''
        match = re.match(r'https://github.com/([^/]+/[^/]+)', package.get('repository') or '')
        found = False
        if match and re.fullmatch('[a-f0-9]{40}', sha):
            repository = match[1].removesuffix('.git')
            for name in ('LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE', 'LICENSE.txt', 'LICENSE.md', 'COPYING'):
                cache = Path('target/license-cache') / repository / sha / name
                absent = cache.with_name(cache.name + '.absent')
                if absent.is_file(): continue
                if not cache.is_file():
                    url = f'https://raw.githubusercontent.com/{repository}/{sha}/{name}'
                    try:
                        with urllib.request.urlopen(url, timeout=20) as response:
                            content = response.read(100_000)
                        cache.parent.mkdir(parents=True, exist_ok=True)
                        cache.write_bytes(content)
                    except urllib.error.HTTPError as error:
                        if error.code == 404:
                            absent.parent.mkdir(parents=True, exist_ok=True)
                            absent.touch()
                            continue
                        raise
                sections.append(name + '\n' + cache.read_text(encoding='utf-8', errors='replace'))
                found = True
        if not files and not found and package.get('license') == 'CC0-1.0':
            cache = Path('target/license-cache/CC0-1.0.txt')
            if not cache.is_file():
                with urllib.request.urlopen('https://raw.githubusercontent.com/spdx/license-list-data/31ba1a50e5397e00a304dbadc76531740e89ee48/text/CC0-1.0.txt', timeout=20) as response:
                    content = response.read(100_000)
                cache.parent.mkdir(parents=True, exist_ok=True)
                cache.write_bytes(content)
            sections.append(cache.read_text(encoding='utf-8'))
            found = True
        if not files and not found:
            missing.append(package['name'])
    for file in sorted(files):
        sections.append(file.name + '\n' + file.read_text(encoding='utf-8', errors='replace'))
if missing:
    raise SystemExit('Missing license texts: ' + ', '.join(missing))
destination = Path(args.output)
destination.parent.mkdir(parents=True, exist_ok=True)
destination.write_text('\n'.join(sections), encoding='utf-8')
print(f'Dependency license notices written ({len(resolved) - 1} packages)')
