import { useState } from "react";
import { Sun, Moon } from "lucide-react";
import { getStoredTheme, toggleTheme } from "../lib/theme";

export function ThemeToggle() {
  const [theme, setTheme] = useState(getStoredTheme);

  const handleClick = () => {
    setTheme(toggleTheme());
  };

  return (
    <button
      onClick={handleClick}
      className="rounded p-1.5 text-muted hover:bg-border/40 hover:text-fg"
      title={theme === "dark" ? "Passer en mode clair" : "Passer en mode sombre"}
    >
      {theme === "dark" ? <Sun size={14} /> : <Moon size={14} />}
    </button>
  );
}
