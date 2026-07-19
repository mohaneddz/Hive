import { useEffect, useState } from "react";

export type Theme = "light" | "dark";

const storageKey = "hive:theme";

function getInitialTheme(): Theme {
  const stored = localStorage.getItem(storageKey);
  if (stored === "light" || stored === "dark") return stored;
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export function useTheme() {
  const [theme, setTheme] = useState<Theme>(getInitialTheme);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    localStorage.setItem(storageKey, theme);
  }, [theme]);

  return { theme, setTheme, toggleTheme: () => setTheme((value) => value === "light" ? "dark" : "light") };
}
