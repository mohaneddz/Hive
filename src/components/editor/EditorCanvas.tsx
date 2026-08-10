import { useEffect, useRef } from "react";

import type { EditOps } from "@/types/media";

/**
 * The colour pipeline, applied to one pixel.
 *
 * This is a line-for-line port of `adjust_pixel` in `commands/editor.rs`. The
 * order is fixed — brightness → contrast → saturation → grayscale → sepia →
 * temperature — and both sides must change together, or the preview stops
 * matching the file that gets written.
 */
function adjustPixel(rgb: [number, number, number], ops: EditOps): [number, number, number] {
  let [r, g, b] = rgb;

  const applyLevels = (value: number) => (value * ops.brightness - 0.5) * ops.contrast + 0.5;
  r = applyLevels(r);
  g = applyLevels(g);
  b = applyLevels(b);

  // Rec. 709 luma, the same coefficients CSS saturate() uses.
  const luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  r = luma + (r - luma) * ops.saturation;
  g = luma + (g - luma) * ops.saturation;
  b = luma + (b - luma) * ops.saturation;

  if (ops.grayscale > 0) {
    const grey = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    r += (grey - r) * ops.grayscale;
    g += (grey - g) * ops.grayscale;
    b += (grey - b) * ops.grayscale;
  }

  if (ops.sepia > 0) {
    // The matrix from the CSS Filter Effects specification.
    const tonedR = 0.393 * r + 0.769 * g + 0.189 * b;
    const tonedG = 0.349 * r + 0.686 * g + 0.168 * b;
    const tonedB = 0.272 * r + 0.534 * g + 0.131 * b;
    r += (tonedR - r) * ops.sepia;
    g += (tonedG - g) * ops.sepia;
    b += (tonedB - b) * ops.sepia;
  }

  if (ops.temperature !== 0) {
    const shift = ops.temperature * 0.3;
    r *= 1 + shift;
    b *= 1 - shift;
  }

  return [r, g, b];
}

const clamp255 = (value: number) => Math.max(0, Math.min(255, Math.round(value * 255)));

/**
 * Draws the source image with the adjustments baked in.
 *
 * Rotation, flipping and cropping happen through the canvas transform — the same
 * order Rust applies them — and the colour pass runs over the pixels afterwards,
 * so what is on screen is what `apply_edits` will write.
 */
export function EditorCanvas({
  image,
  ops,
  className,
  elementRef,
}: {
  image: HTMLImageElement | null;
  ops: EditOps;
  className?: string;
  /**
   * Handed out so callers can measure where the picture actually landed.
   *
   * `object-contain` centres the bitmap inside whatever box CSS gives the
   * element, and the two rarely have the same shape. A caller that measures the
   * element, or worse its wrapper, maps clicks against a rectangle wider than
   * the picture — which is how a selection ends up somewhere else entirely.
   */
  elementRef?: React.RefObject<HTMLCanvasElement | null>;
}) {
  const ownRef = useRef<HTMLCanvasElement>(null);
  const canvasRef = elementRef ?? ownRef;
  const frameRef = useRef<number | null>(null);

  useEffect(() => {
    if (!image) return;
    const canvas = canvasRef.current;
    if (!canvas) return;

    // Coalesce bursts of slider movement into one repaint per frame.
    if (frameRef.current !== null) cancelAnimationFrame(frameRef.current);

    frameRef.current = requestAnimationFrame(() => {
      const context = canvas.getContext("2d", { willReadFrequently: true });
      if (!context) return;

      const quarterTurned = ops.rotation === 90 || ops.rotation === 270;
      const rotatedWidth = quarterTurned ? image.naturalHeight : image.naturalWidth;
      const rotatedHeight = quarterTurned ? image.naturalWidth : image.naturalHeight;

      const crop = ops.crop;
      const outputWidth = Math.max(1, Math.round(rotatedWidth * (crop?.width ?? 1)));
      const outputHeight = Math.max(1, Math.round(rotatedHeight * (crop?.height ?? 1)));
      canvas.width = outputWidth;
      canvas.height = outputHeight;

      context.save();
      // Shift so the crop's top-left lands at the canvas origin.
      context.translate(
        -Math.round(rotatedWidth * (crop?.x ?? 0)),
        -Math.round(rotatedHeight * (crop?.y ?? 0)),
      );
      context.translate(rotatedWidth / 2, rotatedHeight / 2);
      context.rotate((ops.rotation * Math.PI) / 180);
      context.scale(ops.flipHorizontal ? -1 : 1, ops.flipVertical ? -1 : 1);
      context.drawImage(image, -image.naturalWidth / 2, -image.naturalHeight / 2);
      context.restore();

      const frame = context.getImageData(0, 0, outputWidth, outputHeight);
      const pixels = frame.data;
      for (let index = 0; index < pixels.length; index += 4) {
        const [r, g, b] = adjustPixel(
          [pixels[index] / 255, pixels[index + 1] / 255, pixels[index + 2] / 255],
          ops,
        );
        pixels[index] = clamp255(r);
        pixels[index + 1] = clamp255(g);
        pixels[index + 2] = clamp255(b);
      }
      context.putImageData(frame, 0, 0);
    });

    return () => {
      if (frameRef.current !== null) cancelAnimationFrame(frameRef.current);
    };
  }, [image, ops]);

  return <canvas ref={canvasRef} className={className} />;
}
