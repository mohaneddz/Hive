import { NavLink } from "react-router-dom";
import { HardDrive } from "lucide-react";

import { appConfig } from "@/config/app";
import { navigationRoutes, routes } from "@/config/routes";
import { cn } from "@/utils/cn";
import { HiveMark } from "@/components/brand/HiveMark";
import { useLibraryStats } from "@/hooks/useLibraryStats";
import { formatBytes } from "@/utils/format";

export function Sidebar() {
  const { stats } = useLibraryStats();

  return (
    <aside className="flex w-[236px] shrink-0 flex-col border-r border-ink/[.07] bg-shell px-3 pb-4 pt-6">
      <div className="mb-6 flex flex-col items-center gap-2 px-2 text-center">
        <HiveMark className="size-9" />
        <p className="text-sm font-extrabold uppercase tracking-[.28em] text-ink">
          {appConfig.name}
        </p>
      </div>

      <nav className="space-y-0.5" aria-label="Main navigation">
        {navigationRoutes.map((route) => (
          <NavLink
            key={route.path}
            to={route.path}
            end={route.path === "/"}
            className={({ isActive }) =>
              cn("sidebar-link", isActive && "sidebar-link-active")
            }
          >
            <route.icon size={17} strokeWidth={1.8} />
            <span>{route.label}</span>
          </NavLink>
        ))}
      </nav>

      <div className="mt-auto space-y-1">
        <NavLink
          to={routes.settings.path}
          className={({ isActive }) => cn("sidebar-link", isActive && "sidebar-link-active")}
        >
          <routes.settings.icon size={17} strokeWidth={1.8} />
          <span>{routes.settings.label}</span>
        </NavLink>

        <div className="mt-3 rounded-2xl border border-ink/[.07] bg-panel/70 p-3">
          <div className="flex items-center gap-2 text-[11px] font-bold text-ink-muted">
            <HardDrive size={14} />
            <span>Local storage</span>
          </div>
          <p className="mt-1.5 text-xs font-extrabold text-ink">
            {stats ? `${formatBytes(stats.totalBytes)} indexed` : "—"}
          </p>
          <p className="mt-0.5 text-[10px] text-ink-muted">
            {stats ? `${stats.totalItems.toLocaleString()} items on this device` : "Add a folder to begin"}
          </p>
        </div>
      </div>
    </aside>
  );
}
