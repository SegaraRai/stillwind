"""Reproduce implementation issues found by the 2026-09-12 Rust comment audit.

Build the CLI first with cargo build -p stillwind-cli, then run this file.
Pass --check to require the corrected behavior after implementing the audit fixes.
Without --check, this script only records observations for before/after comparison.
Each child is bounded by a timeout, including the recursive-placeholder probe.
"""

import json
from pathlib import Path
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[1]
cli = root / 'target' / 'debug' / ('stillwind.exe' if sys.platform == 'win32' else 'stillwind')
cases = [
    ('control', 'x-red', '@utility x-* { color: --value(--color-*); } @theme inline { --color-red: red; }'),
    ('recursive-placeholder', 'x-loop', '@utility x-* { color: --value(--color-*); } @theme inline { --color-loop: --value(--color-*); }'),
    ('quoted-calc', 'x', '@utility x { content: "calc(1 + 1)"; }'),
    ('important-string-semicolon', 'x!', '@utility x { content: "a;b"; }'),
    ('unknown-placeholder', 'x', '@utility x { color: --unknown-placeholder(); }'),
    ('mixed-variant-order', 'state:x', '@utility x { color:red; }\n@variant state { &:hover { @slot; } }\n@custom-variant state (&:focus);\n'),
    ('unused-motion-css', '', '@utility x { opacity:0; }'),
]
observations = []
for name, candidate, css in cases:
    source = f"import {{ style }} from 'stillwind/react'; export const value = style('{candidate}');"
    if name == 'unused-motion-css':
        source = "import { style } from 'stillwind/react'; const unused = style.animation({from:'x'}); export const value = 1;"
    args = [str(cli), 'compile', '--source-text', source, '--source-path', 'probe.tsx',
            '--theme-text', css + ' @stillwind-slot utilities;']
    started = time.monotonic()
    try:
        result = subprocess.run(args, cwd=root, text=True, encoding='utf-8', capture_output=True, timeout=3)
        try:
            output = json.loads(result.stdout)
        except json.JSONDecodeError:
            output = result.stdout
        observations.append(dict(case=name, elapsed=round(time.monotonic()-started, 3),
                                 exit_code=result.returncode, output=output, stderr=result.stderr))
    except subprocess.TimeoutExpired:
        observations.append(dict(case=name, elapsed=round(time.monotonic()-started, 3),
                                 outcome='timeout; child killed and waited for'))
print(json.dumps(observations, ensure_ascii=False, indent=2))

if '--check' in sys.argv:
    results = {item['case']: item for item in observations}
    assert all(item.get('exit_code') == 0 for item in observations), 'probe failed or timed out'
    outputs = {name: item['output'] for name, item in results.items()}
    assert 'red' in outputs['control']['css']
    recursive = outputs['recursive-placeholder']
    assert not recursive['css']
    assert any(item['rule'] == 'unknown-utility' for item in recursive['lint_findings'])
    assert '"calc(1 + 1)"' in outputs['quoted-calc']['css']
    important = outputs['important-string-semicolon']['css']
    assert '"a;b"!important' in important and 'a!important' not in important
    assert ':focus' in outputs['mixed-variant-order']['css']
    assert ':hover' not in outputs['mixed-variant-order']['css']
    assert not outputs['unused-motion-css']['css']
    assert '--unknown-placeholder()' in outputs['unknown-placeholder']['css']
