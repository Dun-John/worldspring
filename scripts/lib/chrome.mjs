// Shared Chrome launcher for the screenshot scripts. Under WSL it starts Windows Chrome (profile on the
// Windows disk, given as a Windows path) and never falls back to Linux Chrome, whose crash dumps filled the disk.
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const WSL_CHROME = '/mnt/c/Program Files/Google/Chrome/Application/chrome.exe';
const isWsl = () => {
  if (process.env.WSL_DISTRO_NAME) return true;
  try {
    return /microsoft/i.test(readFileSync('/proc/version', 'utf8'));
  } catch {
    return false;
  }
};

// Starts Chrome with the given extra args (the url goes last); returns { chrome, profile }.
export function launchChrome(profilePrefix, args, port = 0) {
  const wsl = isWsl();
  const candidates = wsl
    ? [process.env.CHROME_PATH, WSL_CHROME]
    : [process.env.CHROME_PATH, 'C:/Program Files/Google/Chrome/Application/chrome.exe', '/mnt/c/Program Files/Google/Chrome/Application/chrome.exe', '/usr/bin/google-chrome'];
  const chromePath = candidates.filter(Boolean).find((p) => existsSync(p));
  if (!chromePath) throw new Error(wsl ? `Windows Chrome not found at ${WSL_CHROME} (set CHROME_PATH to a chrome.exe); Linux Chrome is never used under WSL` : 'no Chrome found (set CHROME_PATH)');
  if (wsl && !chromePath.toLowerCase().endsWith('.exe')) throw new Error(`under WSL only Windows Chrome may be used, not ${chromePath}`);
  const win = chromePath.startsWith('/mnt/');
  const profile = mkdtempSync(join(win ? '/mnt/c/Users/Public' : tmpdir(), profilePrefix));
  const profileArg = win ? execFileSync('wslpath', ['-w', profile]).toString().trim() : profile;
  const chrome = spawn(chromePath, [`--user-data-dir=${profileArg}`, `--remote-debugging-port=${port}`, ...args]);
  return { chrome, profile };
}
