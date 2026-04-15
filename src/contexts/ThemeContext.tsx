import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react';

export type ThemeMode = 'dark' | 'light' | 'system';
export type AccentColor = 'cyan' | 'magenta' | 'gold' | 'green';

interface ThemeContextType {
  theme: ThemeMode;
  accent: AccentColor;
  setTheme: (t: ThemeMode) => void;
  setAccent: (a: AccentColor) => void;
}

const ThemeContext = createContext<ThemeContextType | null>(null);

export function useTheme() {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error('useTheme must be used within ThemeProvider');
  return ctx;
}

const STORAGE_KEY = 'anturix_theme';

const ACCENT_TOKENS: Record<AccentColor, { primary: string; ring: string; glow: string }> = {
  cyan:    { primary: 'oklch(0.82 0.18 195)', ring: 'oklch(0.82 0.18 195)', glow: '0 0 20px oklch(0.82 0.18 195 / 0.4)' },
  magenta: { primary: 'oklch(0.7 0.22 340)',  ring: 'oklch(0.7 0.22 340)',  glow: '0 0 20px oklch(0.7 0.22 340 / 0.4)' },
  gold:    { primary: 'oklch(0.82 0.16 85)',   ring: 'oklch(0.82 0.16 85)',  glow: '0 0 20px oklch(0.82 0.16 85 / 0.4)' },
  green:   { primary: 'oklch(0.72 0.19 145)',  ring: 'oklch(0.72 0.19 145)', glow: '0 0 20px oklch(0.72 0.19 145 / 0.4)' },
};

const DARK_TOKENS = {
  background: 'oklch(0.13 0.025 260)',
  foreground: 'oklch(0.95 0.01 250)',
  card: 'oklch(0.16 0.02 260)',
  'card-foreground': 'oklch(0.95 0.01 250)',
  popover: 'oklch(0.14 0.025 260)',
  'popover-foreground': 'oklch(0.95 0.01 250)',
  'primary-foreground': 'oklch(0.13 0.025 260)',
  secondary: 'oklch(0.18 0.02 260)',
  'secondary-foreground': 'oklch(0.85 0.02 250)',
  muted: 'oklch(0.2 0.015 260)',
  'muted-foreground': 'oklch(0.6 0.02 250)',
  border: 'oklch(0.25 0.02 260)',
  input: 'oklch(0.2 0.02 260)',
  sidebar: 'oklch(0.11 0.025 260)',
  'sidebar-foreground': 'oklch(0.7 0.02 250)',
  'sidebar-border': 'oklch(0.2 0.02 260)',
};

const LIGHT_TOKENS = {
  background: 'oklch(0.98 0.005 260)',
  foreground: 'oklch(0.15 0.02 260)',
  card: 'oklch(1 0 0)',
  'card-foreground': 'oklch(0.15 0.02 260)',
  popover: 'oklch(0.99 0.003 260)',
  'popover-foreground': 'oklch(0.15 0.02 260)',
  'primary-foreground': 'oklch(0.98 0.005 260)',
  secondary: 'oklch(0.93 0.01 260)',
  'secondary-foreground': 'oklch(0.25 0.02 260)',
  muted: 'oklch(0.92 0.008 260)',
  'muted-foreground': 'oklch(0.45 0.02 250)',
  border: 'oklch(0.88 0.01 260)',
  input: 'oklch(0.92 0.008 260)',
  sidebar: 'oklch(0.96 0.005 260)',
  'sidebar-foreground': 'oklch(0.35 0.02 250)',
  'sidebar-border': 'oklch(0.88 0.01 260)',
};

function applyTokens(tokens: Record<string, string>) {
  const root = document.documentElement;
  for (const [key, value] of Object.entries(tokens)) {
    root.style.setProperty(`--${key}`, value);
  }
}

function getResolvedMode(mode: ThemeMode): 'dark' | 'light' {
  if (mode === 'system') {
    return typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: light)').matches
      ? 'light' : 'dark';
  }
  return mode;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeMode>('dark');
  const [accent, setAccentState] = useState<AccentColor>('cyan');

  // Load from localStorage after mount
  useEffect(() => {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw);
        if (parsed.theme) setThemeState(parsed.theme);
        if (parsed.accent) setAccentState(parsed.accent);
      }
    } catch {}
  }, []);

  // Apply theme + accent whenever they change (client only)
  useEffect(() => {
    const resolved = getResolvedMode(theme);
    applyTokens(resolved === 'light' ? LIGHT_TOKENS : DARK_TOKENS);
    const accentTokens = ACCENT_TOKENS[accent];
    applyTokens({
      primary: accentTokens.primary,
      ring: accentTokens.ring,
      'chart-1': accentTokens.primary,
    });
    document.documentElement.style.setProperty('--glow-cyan', accentTokens.glow);
  }, [theme, accent]);

  // Listen for system theme changes
  useEffect(() => {
    if (theme !== 'system') return;
    const mq = window.matchMedia('(prefers-color-scheme: light)');
    const handler = () => {
      applyTokens(mq.matches ? LIGHT_TOKENS : DARK_TOKENS);
    };
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, [theme]);

  const setTheme = useCallback((t: ThemeMode) => {
    setThemeState(t);
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      const prev = raw ? JSON.parse(raw) : {};
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...prev, theme: t }));
    } catch {}
  }, []);

  const setAccent = useCallback((a: AccentColor) => {
    setAccentState(a);
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      const prev = raw ? JSON.parse(raw) : {};
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...prev, accent: a }));
    } catch {}
  }, []);

  return (
    <ThemeContext.Provider value={{ theme, accent, setTheme, setAccent }}>
      {children}
    </ThemeContext.Provider>
  );
}
