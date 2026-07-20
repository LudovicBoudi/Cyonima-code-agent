import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";

interface DiffLine {
  type: "add" | "remove" | "context";
  lineNum: number;
  text: string;
}

function parseDiff(text: string): DiffLine[] {
  const lines = text.split("\n");
  const result: DiffLine[] = [];
  let lineNum = 0;

  for (const raw of lines) {
    if (raw.startsWith("+")) {
      lineNum++;
      result.push({ type: "add", lineNum, text: raw.slice(1) });
    } else if (raw.startsWith("-")) {
      lineNum++;
      result.push({ type: "remove", lineNum, text: raw.slice(1) });
    } else if (raw.startsWith("@@")) {
      // hunk header — skip
      continue;
    } else {
      lineNum++;
      result.push({ type: "context", lineNum, text: raw });
    }
  }
  return result;
}

export function DiffViewer({ content }: { content: string }) {
  const [open, setOpen] = useState(true);
  const lines = parseDiff(content);
  const adds = lines.filter((l) => l.type === "add").length;
  const removes = lines.filter((l) => l.type === "remove").length;

  return (
    <div className="mt-1 rounded border border-border overflow-hidden">
      <button
        onClick={() => setOpen(!open)}
        className="flex w-full items-center gap-2 bg-surface px-2 py-1 text-xs font-semibold text-muted hover:text-fg"
      >
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        Diff
        <span className="ml-auto flex gap-2">
          <span className="text-green-400">+{adds}</span>
          <span className="text-red-400">-{removes}</span>
        </span>
      </button>
      {open && (
        <pre className="max-h-60 overflow-y-auto bg-surface text-xs font-mono leading-relaxed">
          {lines.map((l, i) => (
            <div key={i} className="flex">
              <span className="w-8 shrink-0 select-none text-right pr-2 text-muted/50">
                {l.lineNum}
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
                {l.type === "add" ? "+" : l.type === "remove" ? "-" : " "}
                {l.text}
              </span>
            </div>
          ))}
        </pre>
      )}
    </div>
  );
}
