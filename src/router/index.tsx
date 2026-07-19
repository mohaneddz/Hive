import { createHashRouter } from "react-router-dom";

import { AppShell } from "@/components/layout/AppShell";
import { ArtistsPage } from "@/pages/ArtistsPage";
import { CollectionPage } from "@/pages/CollectionPage";
import { DiscoverPage } from "@/pages/DiscoverPage";
import { GallerySettingsPage } from "@/pages/GallerySettingsPage";
import { SavedPage } from "@/pages/SavedPage";
import { NotFoundPage } from "@/pages/NotFoundPage";
import { routes } from "@/config/routes";

export const router = createHashRouter([
  {
    element: <AppShell />,
    children: [
      { index: true, element: <DiscoverPage /> },
      { path: routes.collection.path, element: <CollectionPage /> },
      { path: routes.artists.path, element: <ArtistsPage /> },
      { path: routes.saved.path, element: <SavedPage /> },
      { path: routes.settings.path, element: <GallerySettingsPage /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
