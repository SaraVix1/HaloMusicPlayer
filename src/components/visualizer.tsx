import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

const NUM_BANDS = 24;

interface VisualizerProps {
  isPlaying: boolean;
  className?: string;
}

export function Visualizer({ isPlaying, className }: VisualizerProps) {
  const barRefs = useRef<(HTMLDivElement | null)[]>([]);
  const targetRef = useRef<number[]>(Array(NUM_BANDS).fill(0));
  const currentRef = useRef<number[]>(Array(NUM_BANDS).fill(0));
  const isPlayingRef = useRef(isPlaying);
  const rafRef = useRef(0);

  // Keep the ref in sync without restarting the animation loop.
  useEffect(() => { isPlayingRef.current = isPlaying; }, [isPlaying]);

  useEffect(() => {
    // Receive spectrum data from Rust (33 ms cadence).
    const unlistenPromise = listen<number[]>("spectrum", (event) => {
      targetRef.current = event.payload;
    });

    // Animation loop: lerp current toward target, write heights directly to DOM.
    const animate = () => {
      const cur = currentRef.current;
      const tgt = targetRef.current;
      const bars = barRefs.current;
      // Rise quickly, decay slowly when paused/stopped.
      const speed = isPlayingRef.current ? 0.22 : 0.06;
      for (let i = 0; i < NUM_BANDS; i++) {
        cur[i] += (tgt[i] - cur[i]) * speed;
        const bar = bars[i];
        if (bar) {
          bar.style.height = `${Math.max(1.5, cur[i] * 100)}%`;
        }
      }
      rafRef.current = requestAnimationFrame(animate);
    };

    rafRef.current = requestAnimationFrame(animate);

    return () => {
      cancelAnimationFrame(rafRef.current);
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  return (
    <div
      className={`flex items-end w-full h-full gap-px ${className ?? ""}`}
      aria-hidden="true"
    >
      {Array.from({ length: NUM_BANDS }, (_, i) => (
        <div
          key={i}
          ref={(el) => { barRefs.current[i] = el; }}
          className="flex-1 rounded-t-[1px] bg-foreground/20"
          style={{ height: "1.5%" }}
        />
      ))}
    </div>
  );
}
