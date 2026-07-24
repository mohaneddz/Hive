import { LayoutGrid, Search, Settings, type LucideIcon } from "lucide-react";

export interface AppRoute {
  path: string;
  label: string;
  description: string;
  icon: LucideIcon;
}

export const routes = {
  gallery: {
    path: "/",
    label: "Gallery",
    description: "Every photo and video",
    icon: LayoutGrid,
  },
  search: {
    path: "/search",
    label: "Search",
    description: "Find anything, fast",
    icon: Search,
  },
  settings: {
    path: "/settings",
    label: "Settings",
    description: "Make Hive yours",
    icon: Settings,
  },
} satisfies Record<string, AppRoute>;

export const navigationRoutes = [routes.gallery, routes.search];
