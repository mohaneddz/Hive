import { createHashRouter } from "react-router-dom";

import { AppShell } from "@/components/layout/AppShell";
import { AlbumDetailPage } from "@/pages/AlbumDetailPage";
import { CollectionsPage } from "@/pages/CollectionsPage";
import { EditorPage } from "@/pages/EditorPage";
import { ExplorerPage } from "@/pages/ExplorerPage";
import { GalleryPage } from "@/pages/GalleryPage";
import { GallerySettingsPage } from "@/pages/GallerySettingsPage";
import { HomePage } from "@/pages/HomePage";
import { NotFoundPage } from "@/pages/NotFoundPage";
import { PeoplePage } from "@/pages/PeoplePage";
import { PlacesPage } from "@/pages/PlacesPage";
import { SearchPage } from "@/pages/SearchPage";
import { TrashPage } from "@/pages/TrashPage";
import { UtilitiesPage } from "@/pages/UtilitiesPage";
import { ViewerPage } from "@/pages/ViewerPage";
import { routes } from "@/config/routes";

export const router = createHashRouter([
  {
    element: <AppShell />,
    children: [
      { index: true, element: <HomePage /> },
      { path: routes.gallery.path, element: <GalleryPage /> },
      { path: routes.collections.path, element: <CollectionsPage /> },
      { path: routes.people.path, element: <PeoplePage /> },
      { path: routes.places.path, element: <PlacesPage /> },
      { path: routes.explorer.path, element: <ExplorerPage /> },
      { path: routes.search.path, element: <SearchPage /> },
      { path: routes.utilities.path, element: <UtilitiesPage /> },
      { path: routes.trash.path, element: <TrashPage /> },
      { path: "/albums/:id", element: <AlbumDetailPage /> },
      { path: "/media/:id", element: <ViewerPage /> },
      { path: "/media/:id/edit", element: <EditorPage /> },
      { path: routes.settings.path, element: <GallerySettingsPage /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
