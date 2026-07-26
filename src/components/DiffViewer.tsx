import { useState } from "react";
import { ChevronDown, ChevronRight, Undo2 } from "lucide-react";
import { ipc } from "../lib/ipc";

export interface DiffInfo {
  path: string;
  before: string;
  after: string;
  created: boolean;
}

interface DiffLine {
  type: "add" | "remove" | "context";
  text: string;
}

function computeDiffLines(before: string, after: string): DiffLine[] {
  const oldLines = before.split("\n");
  const newLines = after.split("\n");
  const result: DiffLine[] = [];

  // Simple LCS-based diff
  const lcs = lcsMatrix(oldLines, newLines);
  let i = oldLines.length;
  let j = newLines.length;

  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
      result.unshift({ type: "context", text: oldLines[i - 1] });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || lcs[i][j - 1] >= lcs[i - 1][j])) {
      result.unshift({ type: "add", text: newLines[j - 1] });
      j--;
    } else {
      result.unshift({ type: "remove", text: oldLines[i - 1] });
      i--;
    }
  }

  return result;
}

function lcsMatrix(a: string[], b: string[]): number[][] {
  const m = a.length;
  const n = b.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () => Array(n + 1).fill(0));
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      if (a[i - 1] === b[j - 1]) {
        dp[i][j] = dp[i - 1][j - 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
      }
    }
  }
  return dp;
}

export function DiffViewer({
  diffInfo,
  workspace,
}: {
  diffInfo: DiffInfo;
  workspace: string;
}) {
  const [open, setOpen] = useState(true);
  const [reverted, setReverted] = useState(false);
  const [reverting, setReverting] = useState(false);

  const lines = computeDiffLines(diffInfo.before, diffInfo.after);
  const adds = lines.filter((l) => l.type === "add").length;
  const removes = lines.filter((l) => l.type === "remove").length;

  const handleRevert = async () => {
    setReverting(true);
    try {
      await ipc.fileRevert({
        workspace,
        path: diffInfo.path,
        content: diffInfo.before,
      });
      setReverted(true);
    } catch (e) {
      console.error("Revert failed:", e);
    } finally {
      setReverting(false);
    }
  };

  return (
    <div className="mt-1 rounded border border-border overflow-hidden">
      <div className="flex items-center bg-surface px-2 py-1 text-xs">
        <button
          onClick={() => setOpen(!open)}
          className="flex items-center gap-2 font-semibold text-muted hover:text-fg"
        >
          {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
          {diffInfo.created ? "Nouveau fichier" : "Diff"}
          <span className="ml-2 flex gap-2">
            <span className="text-green-400">+{adds}</span>
            <span className="text-red-400">-{removes}</span>
          </span>
        </button>
        <div className="ml-auto">
          {reverted ? (
            <span className="text-green-400 text-[10px]">Annulé</span>
          ) : (
            <button
              onClick={handleRevert}
              disabled={reverting}
              className="flex items-center gap-1 rounded bg-red-500/10 px-2 py-0.5 text-red-400 hover:bg-red-500/20 disabled:opacity-50"
              title="Annuler cette modification"
            >
              <Undo2 size={10} />
              {reverting ? "..." : "Annuler"}
            </button>
          )}
        </div>
      </div>
      {open && (
        <pre className="max-h-60 overflow-y-auto bg-surface text-xs font-mono leading-relaxed">
          {lines.map((l, i) => (
            <div key={i} className="flex">
              <span
                className={`w-5 shrink-0 select-none text-right pr-1 ${
                  l.type === "add"
                    ? "text-green-500/60"
                    : l.type === "remove"
                      ? "text-red-500/60"
                      : "text-muted/30"
                }`}
              >
                {l.type === "add" ? "+" : l.type === "remove" ? "-" : " "}
              </span>
              <span
                className={`flex-1 px-2 ${
                  l.type === "add"
                    ? "bg-green-500/10 text-green-300"
                    : l.type === "remove"
                      ? "bg-red-500/10 text-red-300"
                      : "text-fg"
                }`}
              >
                {l.text}
              </span>
            </div>
          ))}
        </pre>
      )}
    </div>
  );
}
