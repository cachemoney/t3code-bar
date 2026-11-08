#!/usr/bin/env python3
"""Generate a Homebrew cask from verified macOS release packages; never publish."""
import argparse
import importlib.util
import json
import os
from pathlib import Path

SPEC = importlib.util.spec_from_file_location('macos_package', Path(__file__).with_name('package-macos.py'))
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


def generate(directories, public_key):
    entries = {}
    version = None
    for directory in directories:
        package.verify(directory, public_key)
        metadata = json.loads((directory / 'metadata.json').read_text())
        target = metadata['target']
        if target in entries:
            raise ValueError('duplicate package architecture')
        if version is not None and metadata['version'] != version:
            raise ValueError('all packages must have the same version')
        version = metadata['version']
        expected = f'https://github.com/eliophan/t3code-bar/releases/download/v{version}/{metadata["archive"]}'
        manifest = json.loads((directory / 'manifest.json').read_text())
        if manifest['platforms'][target]['url'] != expected:
            raise ValueError('package URL must match the versioned GitHub Release asset')
        entries[target] = metadata['archive_sha256']
    if not entries:
        raise ValueError('at least one verified package is required')
    lines = ['# frozen_string_literal: true', '', 'cask "t3-code-bar" do']
    if len(entries) == 2:
        lines += ['  arch arm: "darwin-aarch64", intel: "darwin-x86_64"', '', f'  version "{version}"',
                  f'  sha256 arm: "{entries["darwin-aarch64"]}",', f'         intel: "{entries["darwin-x86_64"]}"']
        target = '#{arch}'
    else:
        target = next(iter(entries))
        lines += [f'  version "{version}"', f'  sha256 "{entries[target]}"']
    lines += ['', f'  url "https://github.com/eliophan/t3code-bar/releases/download/v#{{version}}/T3-Code-Bar-#{{version}}-{target}.app.tar.gz"',
              '  name "T3 Code Bar"', '  desc "T3 Code inbox and usage in the menu bar"',
              '  homepage "https://github.com/eliophan/t3code-bar"', '']
    if len(entries) == 1:
        lines += ['  depends_on arch: :' + ('arm64' if target == 'darwin-aarch64' else 'x86_64')]
    lines += ['  depends_on macos: ">= :sonoma"', '', '  app "T3 Code Bar Tauri.app"', 'end', '']
    return '\n'.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', required=True, action='append', type=Path, help='verified release directory; repeat for both architectures')
    parser.add_argument('--output', required=True, type=Path, help='new t3-code-bar.rb file')
    args = parser.parse_args()
    if args.output.exists():
        parser.error('output already exists; refusing to overwrite')
    key = os.environ.get('BAR_UPDATER_PUBLIC_KEY', '').strip()
    if not key:
        parser.error('BAR_UPDATER_PUBLIC_KEY is required to verify the release packages')
    content = generate(args.package, key)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x') as output:
        output.write(content)
    print(f'Wrote {args.output}; nothing installed or published.')


if __name__ == '__main__':
    main()
