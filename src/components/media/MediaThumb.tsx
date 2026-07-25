import { useEffect, useState } from "react";
import { ImageOff } from "lucide-react";

import { readMediaUrl } from "@/lib/tauri";
import { cn } from "@/utils/cn";

type Stage = "loading" | "ready" | "fallback" | "failed";

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
  const [stage, setStage] = useState<Stage>("loading");

  useEffect(() => {
    let cancelled = false;
    let objectUrl: string | null = null;
    setUrl(null);
    setStage("loading");

    readMediaUrl(mediaId, variant)
      .then((created) => {
        if (cancelled) {
          URL.revokeObjectURL(created);
          return;
        }
        objectUrl = created;
        setUrl(created);
        setStage("ready");
      })
      .catch(() => {
        // No thumbnail row for this variant (e.g. generation failed or hasn't run yet) — fall
        // back to the full-resolution original rather than showing a permanently broken tile.
        if (cancelled || variant === "original") {
          if (!cancelled) setStage("failed");
          return;
        }
        readMediaUrl(mediaId, "original")
          .then((created) => {
            if (cancelled) {
              URL.revokeObjectURL(created);
              return;
            }
            objectUrl = created;
            setUrl(created);
            setStage("fallback");
          })
          .catch(() => {
            if (!cancelled) setStage("failed");
          });
      });

    return () => {
      cancelled = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [mediaId, variant]);

  if (stage === "failed") {
    return (
      <div className={cn("grid place-items-center bg-shell text-ink-muted", className)}>
        <ImageOff size={20} />
      </div>
    );
  }

  if (!url) {
    return <div className={cn("animate-pulse bg-shell", className)} />;
  }

  return <img src={url} alt={alt} className={className} onError={() => setStage("failed")} />;
}
