import { useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Disc } from "lucide-react";
import { cn } from "@/lib/utils";

export default function AlbumArt({
  path,
  size = 40,
  className,
  rounded = "md",
}: {
  path: string | null | undefined;
  size?: number;
  className?: string;
  rounded?: "sm" | "md" | "lg" | "full";
}) {
  const [errored, setErrored] = useState(false);
  const radius = {
    sm: "rounded-sm",
    md: "rounded-md",
    lg: "rounded-lg",
    full: "rounded-full",
  }[rounded];

  if (!path || errored) {
    return (
      <div
        className={cn(
          "flex items-center justify-center bg-muted text-muted-foreground/40 shrink-0",
          radius,
          className,
        )}
        style={{ width: size, height: size }}
      >
        <Disc size={Math.max(12, Math.round(size * 0.5))} />
      </div>
    );
  }

  return (
    <img
      src={convertFileSrc(path)}
      width={size}
      height={size}
      loading="lazy"
      onError={() => setErrored(true)}
      className={cn("object-cover shrink-0", radius, className)}
      alt=""
      draggable={false}
    />
  );
}
