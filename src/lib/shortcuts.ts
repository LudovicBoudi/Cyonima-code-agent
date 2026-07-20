import { useEffect } from "react";

type ShortcutHandler = (e: KeyboardEvent) => void;

const shortcuts: Array<{
  key: string;
  ctrl?: boolean;
  shift?: boolean;
  handler: ShortcutHandler;
}> = [];

export function registerShortcut(
  key: string,
  handler: ShortcutHandler,
  opts?: { ctrl?: boolean; shift?: boolean },
) {
  shortcuts.push({ key, ctrl: opts?.ctrl, shift: opts?.shift, handler });
}

export function useKeyboardShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      for (const s of shortcuts) {
        const ctrlMatch = s.ctrl ? e.ctrlKey || e.metaKey : !(e.ctrlKey || e.metaKey);
        const shiftMatch = s.shift ? e.shiftKey : !e.shiftKey;
        if (e.key.toLowerCase() === s.key.toLowerCase() && ctrlMatch && shiftMatch) {
          e.preventDefault();
          s.handler(e);
          return;
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
