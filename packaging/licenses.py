#!/usr/bin/env python3
"""Generate notices from dependency sources fetched by Cargo, without network calls."""
import json
from pathlib import Path
import subprocess

metadata = json.loads(subprocess.check_output([
    'cargo', 'metadata', '--locked', '--format-version', '1',
    '--filter-platform', 'x86_64-unknown-linux-gnu',
]))
nodes = {node['id']: node for node in metadata['resolve']['nodes']}
root = metadata['resolve']['root']
reachable = set()
queue = [root]
while queue:
    current = queue.pop()
    if current in reachable:
        continue
    reachable.add(current)
    queue.extend(dependency['pkg'] for dependency in nodes[current]['deps'])
print('Linux Game Bridge — third-party dependency licenses\n')
for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    if package['id'] not in reachable or package['id'] == root:
        continue
    print('\n' + '=' * 72)
    print(f"{package['name']} {package['version']} — {package.get('license') or 'See source license'}")
    print(package.get('repository') or package.get('homepage') or '')
    folder = Path(package['manifest_path']).parent
    notices = [path for path in folder.rglob('*') if path.is_file()
               and path.name.lower().startswith(('license', 'licence', 'copying', 'copyright', 'notice'))
               and path.stat().st_size <= 1024 * 1024]
    if package.get('license_file'):
        explicit = folder / package['license_file']
        if explicit.is_file() and explicit not in notices:
            notices.append(explicit)
    for notice in sorted(notices):
        print(f'\n--- {notice.relative_to(folder)} ---\n')
        print(notice.read_text(errors='replace'))
    if not notices:
        print('This crate publishes its license through the SPDX expression above; see its source repository for full terms.')
