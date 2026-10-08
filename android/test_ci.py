"""Exercise CI log capture across emulator root setup and test failures."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parent.parent


class AndroidCiTest(unittest.TestCase):
    def test_log_capture_survives_root_and_preserves_test_exit_status(self):
        for status in (0, 7):
            with self.subTest(status=status), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "android").mkdir()
                shutil.copy2(ROOT / "android/ci-test.sh", root / "android/ci-test.sh")
                tools = root / "bin"
                tools.mkdir()
                scripts = {
                    "adb": """
                        import os, signal, sys, time
                        from pathlib import Path
                        root = Path(os.environ['MV_TEST_ROOT'])
                        command = sys.argv[3:]
                        if command == ['root']:
                            if not (root / 'rooted').exists():
                                if (root / 'logger').exists():
                                    os.kill(int((root / 'logger').read_text()), signal.SIGTERM)
                                (root / 'rooted').touch()
                        elif command == ['logcat', '-v', 'threadtime']:
                            (root / 'logger').write_text(str(os.getpid()))
                            print('device logs', flush=True)
                            time.sleep(30)
                        elif command == ['exec-out', 'screencap', '-p']:
                            sys.stdout.buffer.write(b'PNG')
                    """,
                    "python3": """
                        import os, subprocess, sys, time
                        from pathlib import Path
                        root = Path(os.environ['MV_TEST_ROOT'])
                        deadline = time.monotonic() + 5
                        while not (root / 'logger').exists() and time.monotonic() < deadline:
                            time.sleep(0.01)
                        subprocess.run(['adb', '-s', 'emulator-5554', 'root'], check=True)
                        time.sleep(0.05)
                        os.kill(int((root / 'logger').read_text()), 0)
                        with (root / 'phases').open('a') as phases:
                            phases.write(' '.join(sys.argv[1:]) + '\\n')
                        sys.exit(int(os.environ['MV_TEST_STATUS']))
                    """,
                }
                for name, script in scripts.items():
                    executable = tools / name
                    executable.write_text(f"#!{sys.executable}\n" + textwrap.dedent(script))
                    executable.chmod(0o755)
                result = subprocess.run(
                    ['bash', 'android/ci-test.sh', 'tablet', '1200'], cwd=root,
                    env=dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'],
                             MV_TEST_ROOT=str(root), MV_TEST_STATUS=str(status)),
                    capture_output=True, text=True, timeout=10,
                )
                self.assertEqual(result.returncode, status, result.stderr)
                artifacts = root / 'artifacts/android/tablet'
                self.assertEqual((artifacts / 'logcat.txt').read_text(), 'device logs\n')
                phases = (root / 'phases').read_text().splitlines()
                self.assertEqual(len(phases), 1 if status else 2)
                if status:
                    self.assertEqual((artifacts / 'after-test.png').read_bytes(), b'PNG')
                else:
                    self.assertIn('--layout-only', phases[1])


if __name__ == '__main__':
    unittest.main()
