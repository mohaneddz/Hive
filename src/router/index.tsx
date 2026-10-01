import { lazy, Suspense, type ReactNode } from "react";
import { createHashRouter } from "react-router-dom";

import { AppShell } from "@/components/layout/AppShell";
import { routes } from "@/config/routes";

const AlbumDetailPage = lazy(() => import("@/pages/AlbumDetailPage").then((m) => ({ default: m.AlbumDetailPage })));
const CollectionsPage = lazy(() => import("@/pages/CollectionsPage").then((m) => ({ default: m.CollectionsPage })));
const EditorPage = lazy(() => import("@/pages/EditorPage").then((m) => ({ default: m.EditorPage })));
const ExplorerPage = lazy(() => import("@/pages/ExplorerPage").then((m) => ({ default: m.ExplorerPage })));
const GalleryPage = lazy(() => import("@/pages/GalleryPage").then((m) => ({ default: m.GalleryPage })));
const GallerySettingsPage = lazy(() => import("@/pages/GallerySettingsPage").then((m) => ({ default: m.GallerySettingsPage })));
const HomePage = lazy(() => import("@/pages/HomePage").then((m) => ({ default: m.HomePage })));
const NotFoundPage = lazy(() => import("@/pages/NotFoundPage").then((m) => ({ default: m.NotFoundPage })));
const PeoplePage = lazy(() => import("@/pages/PeoplePage").then((m) => ({ default: m.PeoplePage })));
const PlacesPage = lazy(() => import("@/pages/PlacesPage").then((m) => ({ default: m.PlacesPage })));
const SearchPage = lazy(() => import("@/pages/SearchPage").then((m) => ({ default: m.SearchPage })));
const TrashPage = lazy(() => import("@/pages/TrashPage").then((m) => ({ default: m.TrashPage })));
const UtilitiesPage = lazy(() => import("@/pages/UtilitiesPage").then((m) => ({ default: m.UtilitiesPage })));
const ViewerPage = lazy(() => import("@/pages/ViewerPage").then((m) => ({ default: m.ViewerPage })));

function page(element: ReactNode) {
  return (
    <Suspense
      fallback={
        <div className="grid min-h-[45vh] place-items-center text-sm font-semibold text-ink-muted">
          Loading view…
        </div>
      }
    >
      {element}
    </Suspense>
  );
}

export const router = createHashRouter([
  {
    element: <AppShell />,
    children: [
      { index: true, element: page(<HomePage />) },
      { path: routes.gallery.path, element: page(<GalleryPage />) },
      { path: routes.collections.path, element: page(<CollectionsPage />) },
      { path: routes.people.path, element: page(<PeoplePage />) },
      { path: routes.places.path, element: page(<PlacesPage />) },
      { path: routes.explorer.path, element: page(<ExplorerPage />) },
      { path: routes.search.path, element: page(<SearchPage />) },
      { path: routes.utilities.path, element: page(<UtilitiesPage />) },
      { path: routes.trash.path, element: page(<TrashPage />) },
      { path: "/albums/:id", element: page(<AlbumDetailPage />) },
      { path: "/media/:id", element: page(<ViewerPage />) },
      { path: "/media/:id/edit", element: page(<EditorPage />) },
      { path: routes.settings.path, element: page(<GallerySettingsPage />) },
      { path: "*", element: page(<NotFoundPage />) },
    ],
  },
]);
