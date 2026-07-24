import { createHashRouter } from "react-router-dom";

import { AppShell } from "@/components/layout/AppShell";
import { GalleryPage } from "@/pages/GalleryPage";
import { GallerySettingsPage } from "@/pages/GallerySettingsPage";
import { NotFoundPage } from "@/pages/NotFoundPage";
import { SearchPage } from "@/pages/SearchPage";
import { ViewerPage } from "@/pages/ViewerPage";
import { routes } from "@/config/routes";

export const router = createHashRouter([
  {
    element: <AppShell />,
    children: [
      { index: true, element: <GalleryPage /> },
      { path: routes.search.path, element: <SearchPage /> },
      { path: "/media/:id", element: <ViewerPage /> },
      { path: routes.settings.path, element: <GallerySettingsPage /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
