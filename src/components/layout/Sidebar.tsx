import { Bell, ChevronRight, Settings, Upload } from "lucide-react";
import { NavLink } from "react-router-dom";

import { appConfig } from "@/config/app";
import { navigationRoutes, routes } from "@/config/routes";
import { cn } from "@/utils/cn";
import { HiveMark } from "@/components/brand/HiveMark";
import { Button } from "@/components/ui/Button";
import { ThemeToggle } from "@/components/layout/ThemeToggle";

export function Sidebar() {
  return (
    <aside className="flex w-[236px] shrink-0 flex-col border-r border-ink/[.07] bg-shell px-3 pb-4 pt-5">
      <div className="mb-7 flex items-center gap-3 px-2">
        <HiveMark />
        <div className="min-w-0">
          <p className="text-sm font-extrabold leading-tight text-ink">
            {appConfig.name}
          </p>
          <p className="truncate text-[10px] font-semibold text-ink-muted">
            {appConfig.tagline}
          </p>
        </div>
      </div>

      <Button className="mb-6 w-full" icon={<Upload size={16} />}>
        Add artwork
      </Button>

      <nav className="space-y-1" aria-label="Main navigation">
        <p className="mb-2 px-3 text-[10px] font-extrabold uppercase tracking-[.13em] text-ink-muted">
          Explore
        </p>
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
            <ChevronRight className="ml-auto opacity-0" size={14} />
          </NavLink>
        ))}
      </nav>

      <div className="mt-auto space-y-1">
        <button className="sidebar-link w-full">
          <Bell size={17} strokeWidth={1.8} />
          <span>Updates</span>
          <span className="ml-auto size-2 rounded-full bg-honey" />
        </button>
        <div className="flex items-center gap-1">
          <NavLink to={routes.settings.path} className={({ isActive }) => cn("sidebar-link flex-1", isActive && "sidebar-link-active")}>
            <Settings size={17} strokeWidth={1.8} /><span>Settings</span>
          </NavLink>
          <ThemeToggle />
        </div>
        <div className="mt-4 flex items-center gap-3 rounded-2xl border border-ink/[.07] bg-panel/70 p-2.5">
          <div className="grid size-9 place-items-center rounded-xl bg-cream text-xs font-extrabold text-ink">
            MA
          </div>
          <div className="min-w-0">
            <p className="truncate text-xs font-bold text-ink">Mohaned</p>
            <p className="truncate text-[10px] text-ink-muted">Private collection</p>
          </div>
        </div>
      </div>
    </aside>
  );
}
