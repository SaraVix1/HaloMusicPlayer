import { useLayoutEffect, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "@/lib/utils";
import { useScrollMemory } from "@/lib/scroll-memory";

interface VirtualGridProps<T> {
  items: T[];
  minItemWidth: number;
  rowHeight: number;
  gap?: number;
  px?: number;
  paddingTop?: number;
  paddingBottom?: number;
  className?: string;
  renderItem: (item: T, index: number) => React.ReactNode;
  loading?: boolean;
  empty?: React.ReactNode;
  scrollKey?: string;
}

export function VirtualGrid<T>({
  items,
  minItemWidth,
  rowHeight,
  gap = 12,
  px = 0,
  paddingTop = 0,
  paddingBottom = 24,
  className,
  renderItem,
  loading,
  empty,
  scrollKey,
}: VirtualGridProps<T>) {
  const parentRef = useRef<HTMLDivElement>(null);
  const [cols, setCols] = useState(0);

  // parentRef is always the same DOM node — useLayoutEffect can always measure it.
  useLayoutEffect(() => {
    const el = parentRef.current;
    if (!el) return;

    const measure = () => {
      const w = el.clientWidth - px * 2;
      if (w <= 0) return; // hidden (display:none) — wait for next ResizeObserver fire
      setCols(Math.max(1, Math.floor((w + gap) / (minItemWidth + gap))));
    };

    measure();
    const obs = new ResizeObserver(measure);
    obs.observe(el);
    return () => obs.disconnect();
  }, [minItemWidth, gap, px]);

  useScrollMemory(parentRef, !loading && cols > 0 && items.length > 0, scrollKey);

  const rowCount = cols > 0 ? Math.ceil(items.length / cols) : 0;

  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => rowHeight + gap,
    paddingStart: paddingTop,
    paddingEnd: paddingBottom,
    overscan: 3,
  });

  // Always render the same ref'd div so useLayoutEffect's measurement is stable.
  return (
    <div ref={parentRef} className={cn("overflow-auto", className)}>
      {loading ? (
        <p
          className="text-sm text-muted-foreground pt-4"
          style={{ paddingLeft: px, paddingRight: px }}
        >
          Loading…
        </p>
      ) : !items.length ? (
        empty ? (
          <div style={{ paddingLeft: px, paddingRight: px }}>{empty}</div>
        ) : null
      ) : cols > 0 ? (
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((vRow) => {
            const start = vRow.index * cols;
            const row = items.slice(start, start + cols);
            return (
              <div
                key={vRow.index}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  height: vRow.size - gap,
                  transform: `translateY(${vRow.start}px)`,
                  display: "grid",
                  gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`,
                  gap: `${gap}px`,
                  paddingLeft: px,
                  paddingRight: px,
                }}
              >
                {row.map((item, i) => renderItem(item, start + i))}
              </div>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}
