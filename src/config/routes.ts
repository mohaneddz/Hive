import {
  Bookmark,
  Compass,
  Home,
  LayoutGrid,
  MapPin,
  Search,
  Settings,
  Trash2,
  Users,
  Wrench,
  type LucideIcon,
} from "lucide-react";

export interface AppRoute {
  path: string;
  label: string;
  description: string;
  icon: LucideIcon;
}

export const routes = {
  home: {
    path: "/",
    label: "Home",
    description: "Pick up where you left off",
    icon: Home,
  },
  gallery: {
    path: "/gallery",
    label: "Gallery",
    description: "Every photo and video",
    icon: LayoutGrid,
  },
  collections: {
    path: "/collections",
    label: "Collections",
    description: "Albums and smart groupings",
    icon: Bookmark,
  },
  people: {
    path: "/people",
    label: "People",
    description: "Faces Hive has recognized",
    icon: Users,
  },
  places: {
    path: "/places",
    label: "Places",
    description: "Where your photos were taken",
    icon: MapPin,
  },
  explorer: {
    path: "/explorer",
    label: "Explorer",
    description: "Browse the raw file tree",
    icon: Compass,
  },
  search: {
    path: "/search",
    label: "Search",
    description: "Find anything, fast",
    icon: Search,
  },
  utilities: {
    path: "/utilities",
    label: "Utilities",
    description: "Tools for your library",
    icon: Wrench,
  },
  trash: {
    path: "/trash",
    label: "Trash",
    description: "Recently deleted items",
    icon: Trash2,
  },
  settings: {
    path: "/settings",
    label: "Settings",
    description: "Make Hive yours",
    icon: Settings,
  },
} satisfies Record<string, AppRoute>;

export const navigationRoutes = [
  routes.home,
  routes.gallery,
  routes.collections,
  routes.people,
  routes.places,
  routes.explorer,
  routes.search,
  routes.utilities,
  routes.trash,
];
