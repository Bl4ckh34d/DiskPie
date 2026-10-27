"""Exercise the repository guard on disposable synthetic Git repositories."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

CHECK = Path(__file__).with_name('check_repository.py').resolve()
IGNORE = CHECK.parent.parent / '.gitignore'

class PrivacyTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='diskpie-privacy-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)

    def stage(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        subprocess.run(['git', '-C', str(self.root), 'add', '-f', name], check=True)

    def check(self):
        return subprocess.run([sys.executable, str(CHECK), '--staged'], cwd=self.root,
                              capture_output=True, text=True)

    def test_source_passes_and_inventory_is_rejected(self):
        self.stage('src/main.rs', b'fn main() {}\n')
        self.assertEqual(self.check().returncode, 0)
        self.stage('report.json', b'{"synthetic":true}')
        self.assertNotEqual(self.check().returncode, 0)

    def test_private_path_inside_allowed_source_is_rejected_without_echo(self):
        private = b'C:' + bytes([92]) + b'Users' + bytes([92]) + b'Example' + bytes([92]) + b'private'
        self.stage('README.md', private)
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn('Example', result.stdout)

    def test_index_is_checked_even_if_working_copy_was_cleaned(self):
        private = b'/' + b'home' + b'/example/private'
        self.stage('README.md', private)
        (self.root / 'README.md').write_text('clean working copy')
        self.assertNotEqual(self.check().returncode, 0)

    def test_unix_username_with_spaces_is_rejected(self):
        self.stage('README.md', b'/' + b'home' + b'/Example Person/private')
        self.assertNotEqual(self.check().returncode, 0)

    def test_default_deny_excludes_runtime_reports_and_arbitrary_new_files(self):
        (self.root / '.gitignore').write_bytes(IGNORE.read_bytes())
        paths = ['runtime/results.sqlite', 'disk-pie.json', 'disk-pie-data.js',
                 'diskpie-open.log', 'dist/DiskPie.exe', 'target/debug/test.exe',
                 'private.txt', 'src/inventory.json', 'tests/local-data.csv',
                 'docs/old-scan.md']
        result = subprocess.run(['git', 'check-ignore', '-z', '--stdin'], cwd=self.root,
                                input='\0'.join(paths).encode(), capture_output=True)
        self.assertEqual(set(result.stdout.decode().strip('\0').split('\0')), set(paths))

if __name__ == '__main__':
    unittest.main()
