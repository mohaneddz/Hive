import { useEffect, useState } from "react";
import { User } from "lucide-react";

import { readFaceCropUrl } from "@/lib/tauri";
import { cn } from "@/utils/cn";

export function FaceCrop({ faceId, alt, className }: { faceId: string; alt: string; className?: string }) {
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let objectUrl: string | null = null;
    setUrl(null);
    setFailed(false);

    readFaceCropUrl(faceId)
      .then((created) => {
        if (cancelled) {
          URL.revokeObjectURL(created);
          return;
        }
        objectUrl = created;
        setUrl(created);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });

    return () => {
      cancelled = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [faceId]);

  if (failed) {
    return (
      <div className={cn("grid place-items-center bg-shell text-ink-muted", className)}>
        <User size={18} />
      </div>
    );
  }

  if (!url) {
    return <div className={cn("animate-pulse bg-shell", className)} />;
  }

  return <img src={url} alt={alt} className={className} />;
}
