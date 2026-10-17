"""Verify the exact requested release inventory and bind package hashes to the CI commit."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import tarfile

parser = argparse.ArgumentParser()
parser.add_argument('--tools', required=True)
parser.add_argument('--targets', required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
catalogue = json.loads((root / 'registry/index.json').read_text())['tools']
apps = json.loads((root / 'registry/apps.json').read_text())['apps']
versions = {entry['tool']['id']: entry['tool']['version'] for entry in catalogue}
requested = set(versions) if args.tools == 'all' else set(args.tools.split())
if not requested or requested - versions.keys():
    raise SystemExit('Unknown or empty tool selection')
expected = requested | {'cli'}
for entry in apps:
    app = entry['app']
    if app['status'] in ['beta', 'stable']:
        expected.add(app['id'])
        versions[app['id']] = app['version']
for target in args.targets.split():
    packages = {}
    for path in sorted((root / 'dist/release').glob(f'*-{target}.tar.gz')):
        with tarfile.open(path, 'r:gz') as archive:
            members = archive.getmembers()
            if len(members) != 2 or any(not m.isfile() for m in members):
                raise SystemExit(f'Invalid package entries: {path.name}')
            manifest = json.load(archive.extractfile('manifest.json'))
            if set(m.name for m in members) != {'manifest.json', manifest['binary']}:
                raise SystemExit(f'Unexpected files: {path.name}')
            binary = archive.extractfile(manifest['binary']).read()
        item = manifest['id']
        if item in packages or item not in expected or manifest['target'] != target:
            raise SystemExit(f'Unexpected package: {path.name}')
        if item != 'cli' and manifest['version'] != versions[item]:
            raise SystemExit(f'Stale version: {path.name}')
        if hashlib.sha256(binary).hexdigest() != manifest['sha256_binary']:
            raise SystemExit(f'Binary integrity failed: {path.name}')
        packages[item] = {'file': path.name, 'version': manifest['version'],
                          'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                          'sha256_binary': manifest['sha256_binary']}
    if set(packages) != expected:
        raise SystemExit(f'Incomplete {target}: missing {sorted(expected - packages.keys())}')
    proof = {'schema': 1, 'commit': os.environ.get('GITHUB_SHA'),
             'run_id': os.environ.get('GITHUB_RUN_ID'), 'target': target,
             'tool_count': len(requested), 'packages': packages,
             'profile': {'lto': os.environ.get('CARGO_PROFILE_RELEASE_LTO', 'workspace'),
                         'codegen_units': os.environ.get('CARGO_PROFILE_RELEASE_CODEGEN_UNITS', 'workspace')}}
    (root / f'dist/release/build-proof-{target}.json').write_text(json.dumps(proof, indent=2) + '\n')
    print(f'Verified {target}: {len(requested)} tools, {len(expected) - len(requested) - 1} apps, CLI')
