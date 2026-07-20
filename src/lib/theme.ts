export type Theme = "dark" | "light";

const STORAGE_KEY = "cyonima-theme";

export function getStoredTheme(): Theme {
  const v = localStorage.getItem(STORAGE_KEY);
  if (v === "light" || v === "dark") return v;
  return "dark";
}

export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  root.classList.remove("dark", "light");
  root.classList.add(theme);
  localStorage.setItem(STORAGE_KEY, theme);
}

export function toggleTheme(): Theme {
  const next = getStoredTheme() === "dark" ? "light" : "dark";
  applyTheme(next);
  return next;
}
