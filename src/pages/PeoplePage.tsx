import { Users } from "lucide-react";

import { Card } from "@/components/ui/Card";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";

export function PeoplePage() {
  return (
    <div>
      <GalleryPageHeader
        eyebrow="People"
        title="Faces Hive has recognized."
        description="Hive will group photos by the people in them, entirely on-device."
      />

      <Card className="mt-8 flex flex-col items-center gap-3 p-16 text-center">
        <div className="grid size-14 place-items-center rounded-2xl bg-cream text-honey-deep">
          <Users size={22} />
        </div>
        <p className="text-sm font-extrabold text-ink">Face recognition isn't running yet</p>
        <p className="max-w-sm text-xs text-ink-muted">
          This is part of the local AI pipeline — face detection and clustering all happen on your
          device, nothing is uploaded. It hasn't been built yet, so there's nothing to show here.
        </p>
      </Card>
    </div>
  );
}
