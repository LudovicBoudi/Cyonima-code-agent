export function StatusBar() {
  return (
    <footer className="flex h-6 items-center gap-3 border-t border-border px-3 text-xs text-muted">
      <span>Cyonima v0.1.0</span>
      <span>•</span>
      <span>Aucune session active</span>
      <span className="ml-auto">MIT • 100% local</span>
    </footer>
  );
}