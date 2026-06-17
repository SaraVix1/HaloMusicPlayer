import { useEffect, useRef, useState } from "react";
import {
  useLocation,
  useNavigate,
  useNavigationType,
} from "react-router-dom";
import { ArrowLeft, ArrowRight, X, Minus, Square, PictureInPicture2 } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openMiniPlayer } from "@/lib/ipc";
import { cn } from "@/lib/utils";

function useHistoryNav() {
  const location = useLocation();
  const navType = useNavigationType();
  const stackRef = useRef<string[]>([location.key]);
  const [index, setIndex] = useState(0);

  useEffect(() => {
    const stack = stackRef.current;
    if (navType === "PUSH") {
      stack.length = index + 1;
      stack.push(location.key);
      setIndex(index + 1);
    } else if (navType === "POP") {
      const found = stack.indexOf(location.key);
      if (found >= 0) setIndex(found);
    } else if (navType === "REPLACE") {
      stack[index] = location.key;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [location.key]);

  return {
    canBack: index > 0,
    canForward: index < stackRef.current.length - 1,
  };
}

export default function TitleBar() {
  const win = getCurrentWindow();
  const navigate = useNavigate();
  const { canBack, canForward } = useHistoryNav();

  return (
    <div className="h-8 flex items-center justify-between bg-background border-b border-border shrink-0 select-none">
      <div className="flex items-center pl-2 gap-0.5 shrink-0">
        <button
          onClick={() => canBack && navigate(-1)}
          disabled={!canBack}
          className={cn(
            "w-7 h-6 flex items-center justify-center rounded transition-colors",
            canBack
              ? "text-muted-foreground hover:bg-muted hover:text-foreground"
              : "text-muted-foreground/30 cursor-default",
          )}
          aria-label="Back"
          title="Back"
        >
          <ArrowLeft size={14} />
        </button>
        <button
          onClick={() => canForward && navigate(1)}
          disabled={!canForward}
          className={cn(
            "w-7 h-6 flex items-center justify-center rounded transition-colors",
            canForward
              ? "text-muted-foreground hover:bg-muted hover:text-foreground"
              : "text-muted-foreground/30 cursor-default",
          )}
          aria-label="Forward"
          title="Forward"
        >
          <ArrowRight size={14} />
        </button>
      </div>
      <div data-tauri-drag-region className="flex-1 h-full flex items-center px-3">
        <span className="text-sm font-semibold tracking-wide pointer-events-none">Halo</span>
      </div>
      <div className="flex">
        <button
          onClick={() => openMiniPlayer().catch(console.error)}
          className="w-11 h-8 flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
          aria-label="Mini player"
          title="Mini player (Ctrl+M)"
        >
          <PictureInPicture2 size={13} />
        </button>
        <button
          onClick={() => win.minimize()}
          className="w-11 h-8 flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
        >
          <Minus size={14} />
        </button>
        <button
          onClick={() => win.toggleMaximize()}
          className="w-11 h-8 flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
        >
          <Square size={11} />
        </button>
        <button
          onClick={() => win.close()}
          className="w-11 h-8 flex items-center justify-center text-muted-foreground hover:bg-destructive hover:text-destructive-foreground transition-colors"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
