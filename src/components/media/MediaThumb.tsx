import { useEffect, useState } from "react";
import { ImageOff } from "lucide-react";

import { readMediaUrl } from "@/lib/tauri";
import { cn } from "@/utils/cn";

export function MediaThumb({
  mediaId,
  variant = "sm",
  alt,
  className,
}: {
  mediaId: string;
  variant?: "sm" | "md" | "original";
  alt: string;
  className?: string;
}) {
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let objectUrl: string | null = null;
    setUrl(null);
    setFailed(false);

    readMediaUrl(mediaId, variant)
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
  }, [mediaId, variant]);

  if (failed) {
    return (
      <div className={cn("grid place-items-center bg-shell text-ink-muted", className)}>
        <ImageOff size={20} />
      </div>
    );
  }

  if (!url) {
    return <div className={cn("animate-pulse bg-shell", className)} />;
  }

  return <img src={url} alt={alt} className={className} />;
}
