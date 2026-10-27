"""Check only Git's staged/tracked blobs, never scan personal directories."""
import argparse
import re
import subprocess
import sys

EXACT = {'.gitignore', 'AGENTS.md', 'Cargo.toml', 'Cargo.lock',
         'rust-toolchain.toml', 'README.md', 'DiskPie.cmd', '.cargo/config.toml',
         '.github/workflows/build.yml', 'docs/native-design.md',
         'docs/native-plan.md', 'docs/native-verification.md'}
SOURCE = re.compile(r'^(src/[^/]+\.rs|tests/[^/]+\.rs|scripts/[^/]+\.(py|ps1))$')
# Detect personal absolute paths, local inventory formats and common credentials.
PATTERNS = [
    rb'(?i)[a-z]:[\\/]+Users[\\/]+[^\\/\s"\']+',
    rb'(?i)/(?:Users|home)/[^/\r\n"\'<>]+/',
    rb'(?i)(?:ghp_|github_pat_)[a-z0-9_]{20,}',
    rb'-----BEGIN (?:RSA |OPENSSH |EC )?PRIVATE KEY-----',
    rb'SQLite' + rb' format 3',
]

def git(*args):
    return subprocess.check_output(['git', *args])

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--staged', action='store_true')
    args = parser.parse_args()
    names = git('ls-files', '-z').split(b'\0')
    failed = False
    count = 0
    for raw in filter(None, names):
        name = raw.decode('utf-8')
        if name not in EXACT and not SOURCE.fullmatch(name):
            print(f'BLOCKED: unapproved repository file: {name}')
            failed = True
            continue
        # Both modes inspect index blobs, including existing tracked content.
        content = git('show', ':' + name)
        count += 1
        if len(content) > 1_000_000 or b'\0' in content:
            print(f'BLOCKED: binary or oversized source: {name}')
            failed = True
        if any(re.search(pattern, content) for pattern in PATTERNS):
            print(f'BLOCKED: possible private content in {name} (content withheld)')
            failed = True
    print(f'Privacy check: {count} source files; ' + ('FAILED' if failed else 'passed'))
    return int(failed)

if __name__ == '__main__':
    sys.exit(main())
