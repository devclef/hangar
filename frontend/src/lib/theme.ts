import { writable } from 'svelte/store';
import type { ThemeMode } from './api';

/**
 * UI theme handling. The persisted choice lives in the server settings
 * document (`settings.theme`); this module only owns the live state and
 * the `dark` class on <html>. A cached copy in localStorage lets index.html
 * apply the theme before first paint, so there is no flash of the wrong
 * color scheme.
 */

const STORAGE_KEY = 'hangar-theme';

function media(): MediaQueryList {
  return window.matchMedia('(prefers-color-scheme: dark)');
}

/** Which scheme `mode` resolves to right now. */
export function effectiveMode(mode: ThemeMode): 'light' | 'dark' {
  if (mode !== 'system') return mode;
  return media().matches ? 'dark' : 'light';
}

let systemListener: (() => void) | null = null;
let lastMode: ThemeMode = 'system';

function syncClass() {
  document.documentElement.classList.toggle('dark', effectiveMode(lastMode) === 'dark');
}

/**
 * Switch the UI to `mode` immediately: updates the `dark` class, tracks OS
 * changes while in `system` mode, and mirrors the choice to localStorage.
 */
export function applyTheme(mode: ThemeMode) {
  lastMode = mode;
  if (mode === 'system') {
    if (!systemListener) {
      systemListener = () => syncClass();
      media().addEventListener('change', systemListener);
    }
  } else if (systemListener) {
    media().removeEventListener('change', systemListener);
    systemListener = null;
  }
  syncClass();
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // private mode etc.; the server setting still persists the choice
  }
}

/** Current mode, as a store (header toggle icon, settings radios). */
export const themeMode = writable<ThemeMode>('system');

/** Apply locally and update the store (the caller persists to the API). */
export function setThemeMode(mode: ThemeMode) {
  applyTheme(mode);
  themeMode.set(mode);
}
