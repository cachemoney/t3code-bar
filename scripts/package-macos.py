#!/usr/bin/env python3
"""Build isolated Tauri macOS packages; never install or publish them."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import struct
import sys
import tarfile
import tempfile
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]
DESKTOP = ROOT / 'desktop'
CONFIG = DESKTOP / 'src-tauri/tauri.conf.json'
APP = 'T3 Code Bar Tauri.app'


def run(*args, **kwargs):
    subprocess.run(args, check=True, **kwargs)


def required(name):
    value = os.environ.get(name, '').strip()
    if not value:
        raise ValueError(f'{name} is required')
    return value


def https_url(value, suffix=None):
    url = urlsplit(value)
    if (any(c.isspace() for c in value) or url.scheme != 'https' or not url.hostname or url.username is not None
            or url.password is not None or url.query or url.fragment
            or (suffix and not url.path.endswith(suffix))):
        raise ValueError('expected an HTTPS URL without credentials, query, or fragment' + (f' ending in {suffix}' if suffix else ''))
    return value


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()



def write_checksum(archive):
    sidecar = archive.with_name(archive.name + '.sha256')
    sidecar.write_text(f'{digest(archive)}  {archive.name}\n')
    return sidecar


def check_signature(archive, signature, public_key):
    with tempfile.TemporaryDirectory(prefix='bar-signature-') as tmp:
        key = Path(tmp) / 'public.key'
        sig = Path(tmp) / 'archive.sig'
        key.write_bytes(base64.b64decode(public_key, validate=True))
        sig.write_bytes(base64.b64decode(signature, validate=True))
        run('minisign', '-Vm', str(archive), '-p', str(key), '-x', str(sig))


def verify_bundle_archive(archive, version, target):
    expected_cpu = {'darwin-aarch64': 0x0100000c, 'darwin-x86_64': 0x01000007}[target]
    with tarfile.open(archive, 'r:gz') as bundle:
        members = bundle.getmembers()
        names = [member.name.rstrip('/') for member in members]
        if len(names) != len(set(names)) or any(
                not name or Path(name).is_absolute() or '..' in Path(name).parts or Path(name).parts[0] != APP
                for name in names):
            raise ValueError('invalid updater archive layout')
        info = bundle.getmember(APP + '/Contents/Info.plist')
        if not info.isfile() or info.size > 1024 * 1024:
            raise ValueError('invalid bundled Info.plist')
        with bundle.extractfile(info) as source:
            plist = plistlib.load(source)
        executable = plist.get('CFBundleExecutable', '')
        if (plist.get('CFBundleShortVersionString') != version
                or plist.get('CFBundleIdentifier') != 'com.t3code.bar.tauri'
                or not executable or Path(executable).name != executable):
            raise ValueError('bundled app identity or version mismatch')
        binary = bundle.getmember(APP + '/Contents/MacOS/' + executable)
        if not binary.isfile():
            raise ValueError('invalid bundled executable')
        with bundle.extractfile(binary) as source:
            header = source.read(4096)
        magic = header[:4]
        if magic in (b'\xcf\xfa\xed\xfe', b'\xce\xfa\xed\xfe', b'\xfe\xed\xfa\xcf', b'\xfe\xed\xfa\xce'):
            endian = '<' if magic[:1] in (b'\xcf', b'\xce') else '>'
            cpus = [struct.unpack_from(endian + 'I', header, 4)[0]]
        elif magic in (b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca', b'\xca\xfe\xba\xbf', b'\xbf\xba\xfe\xca'):
            endian = '>' if magic[:1] == b'\xca' else '<'
            count = struct.unpack_from(endian + 'I', header, 4)[0]
            stride = 32 if magic in (b'\xca\xfe\xba\xbf', b'\xbf\xba\xfe\xca') else 20
            if not 0 < count <= (len(header) - 8) // stride:
                raise ValueError('invalid universal executable header')
            cpus = [struct.unpack_from(endian + 'I', header, 8 + index * stride)[0] for index in range(count)]
        else:
            raise ValueError('bundled executable is not Mach-O')
        if expected_cpu not in cpus:
            raise ValueError('bundled executable target mismatch')


def verify(directory, public_key):
    metadata = json.loads((directory / 'metadata.json').read_text())
    manifest_path = directory / 'manifest.json'
    manifest = json.loads(manifest_path.read_text())
    target = metadata['target']
    if target not in ('darwin-aarch64', 'darwin-x86_64'):
        raise ValueError('unsupported updater target')
    version = metadata['version']
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?', version):
        raise ValueError('unsupported version')
    name = f'T3-Code-Bar-{version}-{target}.app.tar.gz'
    archive = directory / name
    signature_path = directory / (name + '.sig')
    if metadata['archive'] != name or manifest['version'] != version or manifest['tag_name'] != 'v' + version:
        raise ValueError('version or archive metadata mismatch')
    platform = manifest['platforms'][target]
    if set(manifest['platforms']) != {target}:
        raise ValueError('manifest target mismatch')
    https_url(platform['url'], '/' + name)
    signature = signature_path.read_text().strip()
    if not signature or platform['signature'] != signature:
        raise ValueError('manifest signature mismatch')
    for path, expected in ((archive, metadata['archive_sha256']), (signature_path, metadata['signature_sha256']), (manifest_path, metadata['manifest_sha256'])):
        if digest(path) != expected:
            raise ValueError(f'hash mismatch: {path.name}')
    check_signature(archive, signature, public_key)
    lines = base64.b64decode(signature, validate=True).decode('utf-8').splitlines()
    comment = lines[2] if len(lines) == 4 and lines[2].startswith('trusted comment: ') else ''
    if (not re.search(r'(?:^|\s)version:' + re.escape(version) + r'(?:\s|$)', comment)
            or not re.search(r'(?:^|\s)file:' + re.escape(name) + r'(?:\s|$)', comment)):
        raise ValueError('signed artifact version or filename mismatch')
    verify_bundle_archive(archive, version, target)
    print(f'Updater metadata and signature verified for {version} {target}.')


def merge_packages(directories, output, public_key):
    if output.exists():
        raise ValueError('output path already exists')
    if len(directories) != 2:
        raise ValueError('both macOS architecture packages are required')
    platforms = {}
    inputs = []
    version = None
    for directory in directories:
        verify(directory, public_key)
        metadata = json.loads((directory / 'metadata.json').read_text())
        manifest = json.loads((directory / 'manifest.json').read_text())
        if version is not None and version != metadata['version']:
            raise ValueError('package versions differ')
        version = metadata['version']
        target = metadata['target']
        if target in platforms:
            raise ValueError('duplicate package target')
        platforms[target] = manifest['platforms'][target]
        inputs.append(metadata)
    if set(platforms) != {'darwin-aarch64', 'darwin-x86_64'}:
        raise ValueError('both macOS architectures are required')
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.bar-manifest-', dir=output.parent) as temp:
        staging = Path(temp) / 'result'
        staging.mkdir()
        manifest_path = staging / 'manifest.json'
        manifest_path.write_text(json.dumps({'version': version, 'tag_name': 'v' + version,
                                             'platforms': platforms}, indent=2) + '\n')
        (staging / 'metadata.json').write_text(json.dumps({
            'version': version, 'manifest_sha256': digest(manifest_path),
            'packages': inputs, 'published': False,
        }, indent=2) + '\n')
        staging.rename(output)
    print(f'Verified two-architecture manifest written to {output}; nothing published.')


def package(mode, target, output, cli):
    if sys.platform != 'darwin':
        raise ValueError('packaging requires macOS')
    if output.exists():
        raise ValueError('output path already exists')
    triple = {'darwin-aarch64': 'aarch64-apple-darwin', 'darwin-x86_64': 'x86_64-apple-darwin'}[target]
    config = json.loads(CONFIG.read_text())
    version = config['version']
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?', version):
        raise ValueError('unsupported version')
    identity = '-'
    if mode == 'release':
        identity = required('BAR_MACOS_SIGNING_IDENTITY')
        if not re.fullmatch(r'[0-9A-Fa-f]{40}', identity):
            raise ValueError('BAR_MACOS_SIGNING_IDENTITY must be the exact 40-character certificate SHA-1')
        identities = subprocess.check_output(['security', 'find-identity', '-v', '-p', 'codesigning'], text=True)
        if not re.search(r'\b' + re.escape(identity) + r'\s+"Developer ID Application: [^"\n]+"', identities, re.I):
            raise ValueError('the exact Developer ID Application identity is not valid in the keychain')
        profile = required('BAR_NOTARY_KEYCHAIN_PROFILE')
        endpoint = https_url(required('BAR_UPDATER_ENDPOINT'))
        public_key = required('BAR_UPDATER_PUBLIC_KEY')
        base64.b64decode(public_key, validate=True)
        key_path = Path(required('TAURI_SIGNING_PRIVATE_KEY_PATH')).expanduser().resolve(strict=True)
        name = f'T3-Code-Bar-{version}-{target}.app.tar.gz'
        url = https_url(required('BAR_UPDATER_ARTIFACT_URL'), '/' + name)
        if not shutil.which('minisign'):
            raise ValueError('minisign is required to verify the updater signature')
    output = output.absolute()
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.bar-package-', dir=output.parent) as temp:
        temp = Path(temp)
        staging = temp / 'result'
        staging.mkdir()
        overlay = temp / 'release-config.json'
        overlay.write_text(json.dumps({'bundle': {'createUpdaterArtifacts': False, 'macOS': {'signingIdentity': identity, 'hardenedRuntime': True}}}))
        env = os.environ.copy()
        for key in ('APPLE_SIGNING_IDENTITY', 'APPLE_ID', 'APPLE_PASSWORD', 'APPLE_TEAM_ID', 'APPLE_API_KEY', 'APPLE_API_ISSUER', 'APPLE_API_KEY_PATH', 'TAURI_SIGNING_PRIVATE_KEY'):
            env.pop(key, None)
        env['CARGO_TARGET_DIR'] = str(temp / 'target')
        if mode == 'release':
            env.update(BAR_UPDATER_ENDPOINT=endpoint, BAR_UPDATER_PUBLIC_KEY=public_key, TAURI_SIGNING_PRIVATE_KEY_PATH=str(key_path))
        else:
            env.pop('BAR_UPDATER_ENDPOINT', None)
            env.pop('BAR_UPDATER_PUBLIC_KEY', None)
        run(str(cli), 'build', '--ci', '--target', triple, '--bundles', 'app', '--config', str(overlay), cwd=DESKTOP, env=env)
        built = temp / 'target' / triple / 'release/bundle/macos' / APP
        app = staging / APP
        run('ditto', str(built), str(app))
        with (app / 'Contents/Info.plist').open('rb') as f:
            if plistlib.load(f)['CFBundleShortVersionString'] != version:
                raise ValueError('app version differs from configuration')
        run('codesign', '--verify', '--deep', '--strict', '--verbose=2', str(app))
        if mode == 'release':
            details = subprocess.check_output(['codesign', '-dvv', str(app)], stderr=subprocess.STDOUT, text=True)
            (staging / 'codesign-details.txt').write_text(details)
            if 'Authority=Developer ID Application:' not in details or not re.search(r'flags=.*\bruntime\b', details):
                raise ValueError('Developer ID signature or hardened runtime missing')
            run('codesign', '-d', '--extract-certificates', str(app), cwd=temp, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            fingerprint = subprocess.check_output(['openssl', 'x509', '-inform', 'DER', '-in', str(temp / 'codesign0'), '-noout', '-fingerprint', '-sha1'], text=True)
            if fingerprint.strip().split('=')[-1].replace(':', '').upper() != identity.upper():
                raise ValueError('app certificate does not match the requested identity')
            zip_path = temp / 'notary.zip'
            run('ditto', '-c', '-k', '--keepParent', str(app), str(zip_path))
            result = subprocess.check_output(['xcrun', 'notarytool', 'submit', str(zip_path), '--keychain-profile', profile, '--wait', '--output-format', 'json'], text=True)
            (staging / 'notarization.json').write_text(result)
            if json.loads(result).get('status') != 'Accepted':
                raise ValueError('Apple notarization did not return Accepted')
            run('xcrun', 'stapler', 'staple', str(app))
            run('xcrun', 'stapler', 'validate', str(app))
            run('codesign', '--verify', '--deep', '--strict', '--verbose=2', str(app))
            run('spctl', '--assess', '--type', 'execute', '--verbose=2', str(app))
            archive = staging / name
            archive_env = dict(env, COPYFILE_DISABLE='1')
            run('tar', '-czf', str(archive), '-C', str(staging), APP, env=archive_env)
            run(str(cli), 'signer', 'sign', '--app-version', version, str(archive), cwd=DESKTOP, env=env)
            sig_path = staging / (name + '.sig')
            signature = sig_path.read_text().strip()
            check_signature(archive, signature, public_key)
            manifest_path = staging / 'manifest.json'
            manifest_path.write_text(json.dumps({'version': version, 'tag_name': 'v' + version, 'platforms': {target: {'url': url, 'signature': signature}}}, indent=2) + '\n')
            metadata = {'version': version, 'target': target, 'archive': name, 'archive_sha256': digest(archive), 'signature_sha256': digest(sig_path), 'manifest_sha256': digest(manifest_path)}
            (staging / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
            verify(staging, public_key)
            write_checksum(archive)
        staging.rename(output)
    print(f'{mode} package written to {output}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('adhoc', 'release', 'verify', 'merge'))
    parser.add_argument('--output', required=True, type=Path, help='new package directory, or existing directory for verify')
    parser.add_argument('--target', choices=('darwin-aarch64', 'darwin-x86_64'))
    parser.add_argument('--cli', type=Path, default=DESKTOP / 'node_modules/.bin/tauri')
    parser.add_argument('--package', action='append', type=Path, default=[], help='verified per-target package directory for merge; pass twice')
    args = parser.parse_args()
    if args.mode == 'verify':
        verify(args.output, required('BAR_UPDATER_PUBLIC_KEY'))
    elif args.mode == 'merge':
        merge_packages(args.package, args.output, required('BAR_UPDATER_PUBLIC_KEY'))
    else:
        if not args.target:
            parser.error('--target is required for packaging')
        package(args.mode, args.target, args.output, args.cli.resolve(strict=True))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, KeyError, struct.error, tarfile.TarError, plistlib.InvalidFileException, subprocess.CalledProcessError) as exc:
        print(f'package-macos: {exc}', file=sys.stderr)
        sys.exit(1)
