import { useEffect, useRef, useState } from "react";
import {
  ExternalLink,
  FolderOpen,
  Copy,
  FileText,
  Settings,
  Trash2,
} from "lucide-react";
import type { SearchResult } from "../types";

interface Action {
  id: string;
  label: string;
  icon: React.ReactNode;
  shortcut: string[];
  handler: () => void;
}

interface Props {
  result: SearchResult;
  onOpen: () => void;
  onReveal: () => void;
  onCopyPath: () => void;
  onCopyFilename: () => void;
  onTrash: () => void;
  onSettings: () => void;
  onClose: () => void;
}

export function ActionsPanel({
  result,
  onOpen,
  onReveal,
  onCopyPath,
  onCopyFilename,
  onTrash,
  onSettings,
  onClose,
}: Props) {
  const [selected, setSelected] = useState(0);
  const panelRef = useRef<HTMLDivElement>(null);

  const actions: Action[] = [
    {
      id: "open",
      label: "Open",
      icon: <ExternalLink size={14} />,
      shortcut: ["↵"],
      handler: onOpen,
    },
    {
      id: "reveal",
      label: "Reveal in Finder",
      icon: <FolderOpen size={14} />,
      shortcut: ["⌘", "↵"],
      handler: onReveal,
    },
    {
      id: "copy-path",
      label: "Copy Path",
      icon: <Copy size={14} />,
      shortcut: ["⌘", "C"],
      handler: onCopyPath,
    },
    {
      id: "copy-name",
      label: "Copy Filename",
      icon: <FileText size={14} />,
      shortcut: ["⌘", "⇧", "C"],
      handler: onCopyFilename,
    },
    {
      id: "trash",
      label: "Move to Trash",
      icon: <Trash2 size={14} />,
      shortcut: ["⌘", "⌫"],
      handler: onTrash,
    },
    {
      id: "settings",
      label: "Settings",
      icon: <Settings size={14} />,
      shortcut: ["⌘", ","],
      handler: onSettings,
    },
  ];

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const meta = e.metaKey || e.ctrlKey;

      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onClose();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelected((s) => Math.min(s + 1, actions.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelected((s) => Math.max(s - 1, 0));
      } else if (meta && e.key === "Enter") {
        e.preventDefault();
        onReveal();
        onClose();
      } else if (e.key === "Enter") {
        e.preventDefault();
        actions[selected].handler();
        onClose();
      } else if (meta && e.shiftKey && e.key.toLowerCase() === "c") {
        e.preventDefault();
        onCopyFilename();
        onClose();
      } else if (meta && !e.shiftKey && e.key === "c") {
        e.preventDefault();
        onCopyPath();
        onClose();
      } else if (meta && e.key === "Backspace") {
        e.preventDefault();
        onTrash();
        onClose();
      } else if (meta && e.key === ",") {
        e.preventDefault();
        onSettings();
        onClose();
      }
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [selected, actions, onClose, onReveal, onCopyPath, onCopyFilename, onTrash, onSettings]);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    window.addEventListener("mousedown", handler, true);
    return () => window.removeEventListener("mousedown", handler, true);
  }, [onClose]);

  return (
    <div
      ref={panelRef}
      className="absolute right-3 bottom-10 w-64 rounded-lg shadow-xl z-50 overflow-hidden"
      style={{
        background: "var(--bg-secondary)",
        border: "1px solid var(--border)",
      }}
    >
      <div
        className="px-3 py-2 text-xs font-medium"
        style={{
          color: "var(--text-tertiary)",
          borderBottom: "1px solid var(--border)",
        }}
      >
        {result.filename}
      </div>
      <div className="py-1">
        {actions.map((action, i) => (
          <button
            key={action.id}
            onClick={() => {
              action.handler();
              onClose();
            }}
            onMouseEnter={() => setSelected(i)}
            className="w-full px-3 py-1.5 flex items-center gap-2.5 text-sm transition-colors"
            style={{
              color: "var(--text-primary)",
              background:
                i === selected ? "var(--bg-active)" : "transparent",
            }}
          >
            <span style={{ color: "var(--text-secondary)" }}>
              {action.icon}
            </span>
            <span className="flex-1 text-left">{action.label}</span>
            <span className="flex gap-0.5">
              {action.shortcut.map((k, j) => (
                <kbd
                  key={j}
                  className="inline-block px-1 text-[10px] rounded"
                  style={{
                    background: "var(--kbd-bg)",
                    border: "1px solid var(--kbd-border)",
                    color: "var(--text-secondary)",
                  }}
                >
                  {k}
                </kbd>
              ))}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
