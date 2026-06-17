import { useLocation, useNavigate } from "react-router-dom";
import {
  Music,
  Disc,
  Library,
  Folder,
  ListMusic,
  ListOrdered,
  Settings,
  Search,
} from "lucide-react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useCommandPalette } from "@/lib/command-palette-store";
import { cn } from "@/lib/utils";

// ── Data ──────────────────────────────────────────────────────────────────────

const NAV_ITEMS = [
  { icon: Search,      label: "Search (Ctrl+K)", path: null as string | null },
  { icon: Music,       label: "All Songs",        path: "/songs" },
  { icon: Disc,        label: "Albums",            path: "/albums" },
  { icon: Library,     label: "Library",           path: "/library" },
  { icon: Folder,      label: "Folders",           path: "/folders" },
  { icon: ListMusic,   label: "Playlists",          path: "/playlists" },
  { icon: ListOrdered, label: "Queue",             path: "/queue" },
] as const;

// ── Components ────────────────────────────────────────────────────────────────

function NavButton({
  icon: Icon,
  label,
  active,
  onClick,
}: {
  icon: React.ElementType;
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger
        onClick={onClick}
        className={cn(
          "w-10 h-10 flex items-center justify-center rounded-md transition-colors",
          active
            ? "bg-sidebar-accent text-sidebar-accent-foreground"
            : "text-sidebar-foreground/50 hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground",
        )}
        aria-label={label}
      >
        <Icon size={20} />
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

export default function Sidebar() {
  const location = useLocation();
  const navigate = useNavigate();
  const openPalette = useCommandPalette((s) => s.setOpen);

  return (
    <aside className="w-14 flex flex-col items-center py-2 gap-1 bg-sidebar border-r border-sidebar-border shrink-0">
      <div className="flex flex-col items-center gap-1 flex-1">
        {NAV_ITEMS.map(({ icon, label, path }) => (
          <NavButton
            key={label}
            icon={icon}
            label={label}
            active={path !== null && location.pathname.startsWith(path)}
            onClick={() => (path ? navigate(path) : openPalette(true))}
          />
        ))}
      </div>

      <NavButton
        icon={Settings}
        label="Settings"
        active={location.pathname === "/settings"}
        onClick={() => navigate("/settings")}
      />
    </aside>
  );
}
