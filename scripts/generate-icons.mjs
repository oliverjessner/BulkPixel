#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { copyFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const artwork = path.join(root, 'src/assets/icons');
const logo = path.join(artwork, 'logo.png');
const appIcon = path.join(artwork, 'icon_app.png');

// The tightly cropped logo is the master. macOS uses an 856px surface on a
// transparent 1024px canvas, matching the icon artboard and the previous icon.
execFileSync('magick', [
    logo, '-resize', '856x856', '-gravity', 'center', '-background', 'none',
    '-extent', '1024x1024', appIcon,
], { stdio: 'inherit' });
copyFileSync(logo, path.join(artwork, 'icon_logo.png'));
copyFileSync(logo, path.join(root, 'src/logo.png'));
execFileSync('magick', [
    logo, '-background', '#29ec8f', '-alpha', 'remove', '-alpha', 'off',
    path.join(artwork, 'icon_logo_filled.png'),
], { stdio: 'inherit' });

execFileSync(process.execPath, [
    path.join(root, 'node_modules/@tauri-apps/cli/tauri.js'),
    'icon', appIcon, '--output', path.join(root, 'src-tauri/icons'),
], { cwd: root, stdio: 'inherit' });
