#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, realpathSync, copyFileSync, chmodSync } from 'node:fs';
import { basename, join } from 'node:path';

const appPath = process.argv[2];

if (!appPath) {
  console.error('Usage: node scripts/bundle-macos-dylibs.mjs <App.app>');
  process.exit(1);
}

const executableName = readPlistValue(join(appPath, 'Contents/Info.plist'), 'CFBundleExecutable');
const executablePath = join(appPath, 'Contents/MacOS', executableName);
const frameworksDir = join(appPath, 'Contents/Frameworks');

if (!existsSync(executablePath)) {
  console.error(`Executable not found: ${executablePath}`);
  process.exit(1);
}

mkdirSync(frameworksDir, { recursive: true });

const copied = new Map();
const processed = new Set();
const targets = [executablePath];

for (let index = 0; index < targets.length; index += 1) {
  const target = targets[index];

  if (processed.has(target)) {
    continue;
  }

  processed.add(target);

  for (const dependency of homebrewDependencies(target)) {
    const bundledPath = copyDependency(dependency);
    rewriteDependency(target, dependency, bundledPath);

    if (!processed.has(bundledPath)) {
      targets.push(bundledPath);
    }
  }
}

for (const bundledPath of copied.values()) {
  const installName = bundledInstallName(bundledPath);
  run('install_name_tool', ['-id', installName, bundledPath]);

  for (const dependency of homebrewDependencies(bundledPath)) {
    const copiedDependency = copied.get(realpathSync(dependency)) ?? copyDependency(dependency);
    rewriteDependency(bundledPath, dependency, copiedDependency);
  }
}

function readPlistValue(plistPath, key) {
  return run('/usr/libexec/PlistBuddy', ['-c', `Print :${key}`, plistPath]).trim();
}

function homebrewDependencies(target) {
  return otoolDependencies(target).filter((dependency) => {
    return dependency.startsWith('/opt/homebrew/') || dependency.startsWith('/usr/local/');
  });
}

function otoolDependencies(target) {
  const output = run('otool', ['-L', target]);

  return output
    .split('\n')
    .slice(1)
    .map((line) => line.trim().split(' ')[0])
    .filter((dependency) => dependency.endsWith('.dylib'));
}

function copyDependency(dependency) {
  const realDependency = realpathSync(dependency);
  const existing = copied.get(realDependency);

  if (existing) {
    return existing;
  }

  const destination = join(frameworksDir, basename(dependency));
  copyFileSync(realDependency, destination);
  chmodSync(destination, 0o755);
  copied.set(realDependency, destination);

  console.log(`Bundled ${dependency}`);

  return destination;
}

function rewriteDependency(target, originalDependency, bundledPath) {
  run('install_name_tool', [
    '-change',
    originalDependency,
    bundledInstallName(bundledPath),
    target,
  ]);
}

function bundledInstallName(bundledPath) {
  return `@executable_path/../Frameworks/${basename(bundledPath)}`;
}

function run(command, args) {
  return execFileSync(command, args, { encoding: 'utf8' });
}
