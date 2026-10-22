import { useEffect, useRef, useState } from "react";
import { ImageOff } from "lucide-react";

import { getThumbnailUrl, getViewableMediaUrl } from "@/lib/tauri";
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
  const host = useRef<HTMLDivElement>(null);
  const [nearby, setNearby] = useState(variant === "original");
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (variant === "original") return;
    const element = host.current;
    if (!element || typeof IntersectionObserver === "undefined") {
      setNearby(true);
      return;
    }
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        setNearby(true);
        observer.disconnect();
      }
    }, { rootMargin: "400px" });
    observer.observe(element);
    return () => observer.disconnect();
  }, [mediaId, variant]);

  useEffect(() => {
    setUrl(null);
    setFailed(false);
    if (!nearby) return;
    let cancelled = false;
    const request = variant === "original"
      ? getViewableMediaUrl(mediaId)
      : getThumbnailUrl(mediaId, variant);
    request.then((result) => {
      if (!cancelled) setUrl(result);
    }).catch(() => {
      if (!cancelled) setFailed(true);
    });
    return () => { cancelled = true; };
  }, [mediaId, variant, nearby]);

  return (
    <div ref={host} className={cn("overflow-hidden", className)}>
      {failed ? (
        <div className="grid size-full place-items-center bg-shell text-ink-muted"><ImageOff size={20} /></div>
      ) : url ? (
        <img src={url} alt={alt} loading="lazy" decoding="async" className="size-full object-cover" onError={() => setFailed(true)} />
      ) : (
        <div className="size-full animate-pulse bg-shell" />
      )}
    </div>
  );
}
