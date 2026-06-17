import { useState } from "react";
import { Star } from "lucide-react";
import { cn } from "@/lib/utils";

interface StarRatingProps {
  value: number;
  onChange?: (rating: number) => void;
  size?: number;
  className?: string;
  readonly?: boolean;
}

export function StarRating({
  value,
  onChange,
  size = 14,
  className,
  readonly = false,
}: StarRatingProps) {
  const [hovered, setHovered] = useState<number | null>(null);
  const displayed = hovered ?? value;

  return (
    <div
      className={cn("flex items-center gap-0.5", className)}
      onMouseLeave={() => setHovered(null)}
    >
      {Array.from({ length: 5 }, (_, i) => {
        const star = i + 1;
        const filled = star <= displayed;
        return (
          <button
            key={star}
            type="button"
            disabled={readonly}
            onMouseEnter={() => !readonly && setHovered(star)}
            onClick={(e) => {
              e.stopPropagation();
              if (!readonly && onChange) {
                onChange(star === value ? 0 : star);
              }
            }}
            className={cn(
              "transition-colors leading-none",
              readonly
                ? "cursor-default"
                : "cursor-pointer hover:scale-110 transition-transform",
              filled ? "text-amber-400" : "text-muted-foreground/25 hover:text-amber-300",
            )}
            aria-label={`${star} star${star !== 1 ? "s" : ""}`}
          >
            <Star
              size={size}
              fill={filled ? "currentColor" : "none"}
              strokeWidth={1.5}
            />
          </button>
        );
      })}
    </div>
  );
}
