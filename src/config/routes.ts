import {
  Compass,
  Frame,
  Heart,
  Palette,
  Settings,
  type LucideIcon,
} from "lucide-react";

export interface AppRoute {
  path: string;
  label: string;
  description: string;
  icon: LucideIcon;
}

export const routes = {
  discover: {
    path: "/",
    label: "Discover",
    description: "Find work that moves you",
    icon: Compass,
  },
  collection: {
    path: "/collection",
    label: "Collection",
    description: "Browse every work",
    icon: Frame,
  },
  artists: {
    path: "/artists",
    label: "Artists",
    description: "Meet the makers",
    icon: Palette,
  },
  saved: {
    path: "/saved",
    label: "Saved",
    description: "Your personal selection",
    icon: Heart,
  },
  settings: {
    path: "/settings",
    label: "Settings",
    description: "Make Hive yours",
    icon: Settings,
  },
} satisfies Record<string, AppRoute>;

export const navigationRoutes = [
  routes.discover,
  routes.collection,
  routes.artists,
  routes.saved,
];
