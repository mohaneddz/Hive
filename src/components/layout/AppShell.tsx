import { Outlet } from "react-router-dom";

import { Sidebar } from "@/components/layout/Sidebar";
import { Titlebar } from "@/components/layout/Titlebar";
import { useTheme } from "@/hooks/useTheme";

export function AppShell() {
  useTheme();
  return (
    <div className="flex h-screen min-h-0 w-screen flex-col overflow-hidden bg-canvas">
      <Titlebar />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <main className="min-w-0 flex-1 overflow-y-auto">
          <div className="mx-auto min-h-full w-full max-w-[1440px] px-8 py-7">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  );
}
