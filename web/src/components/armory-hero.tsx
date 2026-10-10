"use client";

import { useEffect, useRef, useState } from "react";

// The hero in the middle of the Armory, turning: one sheet of frames side by
// side (lib/armory's SHEET), dragged round with the mouse or a finger, the
// wheel bringing them closer. Until the sheet comes, or if the server does
// not draw heroes, the still art stays.

/** Pixels of drag that turn the hero one frame. */
const DRAG_PER_FRAME = 10;
const ZOOM = { min: 1, max: 1.6, step: 0.0015 };

export function ArmoryHero({
  src,
  frames,
  size,
  fallback,
  hint,
  label,
}: {
  src: string;
  frames: number;
  size: [number, number];
  fallback: React.ReactNode;
  hint: string;
  label: string;
}) {
  const [ready, setReady] = useState(false);
  const [frame, setFrame] = useState(0);
  const [zoom, setZoom] = useState(1);
  const [touched, setTouched] = useState(false);
  const stage = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; frame: number } | null>(null);

  useEffect(() => {
    const picture = new Image();
    picture.onload = () => setReady(true);
    picture.src = src;
    return () => {
      picture.onload = null;
    };
  }, [src]);

  // A slow turn until the visitor takes the hero in hand.
  useEffect(() => {
    if (!ready || touched || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const timer = window.setInterval(() => setFrame((current) => (current + 1) % frames), 160);
    return () => window.clearInterval(timer);
  }, [ready, touched, frames]);

  // The wheel zooms while over the hero, instead of scrolling the page.
  useEffect(() => {
    const element = stage.current;
    if (!element || !ready) return;
    const wheel = (event: WheelEvent) => {
      event.preventDefault();
      setTouched(true);
      setZoom((current) => Math.min(ZOOM.max, Math.max(ZOOM.min, current - event.deltaY * ZOOM.step)));
    };
    element.addEventListener("wheel", wheel, { passive: false });
    return () => element.removeEventListener("wheel", wheel);
  }, [ready]);

  if (!ready) return <>{fallback}</>;

  const turn = (by: number) => setFrame((current) => (((current + by) % frames) + frames) % frames);
  return (
    <div className="absolute inset-0 flex flex-col items-center justify-end">
      <div
        ref={stage}
        role="slider"
        tabIndex={0}
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={frames - 1}
        aria-valuenow={frame}
        className="relative w-full max-w-[400px] cursor-grab touch-none overflow-hidden rounded-lg outline-none select-none focus-visible:ring-2 focus-visible:ring-gold active:cursor-grabbing"
        style={{ aspectRatio: `${size[0]} / ${size[1]}` }}
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId);
          drag.current = { x: event.clientX, frame };
          setTouched(true);
        }}
        onPointerMove={(event) => {
          if (!drag.current) return;
          const steps = Math.round((event.clientX - drag.current.x) / DRAG_PER_FRAME);
          setFrame((((drag.current.frame - steps) % frames) + frames) % frames);
        }}
        onPointerUp={() => {
          drag.current = null;
        }}
        onPointerCancel={() => {
          drag.current = null;
        }}
        onKeyDown={(event) => {
          const keys: Record<string, () => void> = {
            ArrowLeft: () => turn(1),
            ArrowRight: () => turn(-1),
            "+": () => setZoom((current) => Math.min(ZOOM.max, current + 0.15)),
            "-": () => setZoom((current) => Math.max(ZOOM.min, current - 0.15)),
          };
          const action = keys[event.key];
          if (!action) return;
          event.preventDefault();
          setTouched(true);
          action();
        }}
      >
        <div
          aria-hidden="true"
          className="absolute inset-0 origin-[50%_70%] bg-no-repeat transition-transform duration-150 drop-shadow-[0_18px_30px_rgb(0_0_0/0.6)]"
          style={{
            backgroundImage: `url(${src})`,
            backgroundSize: `${frames * 100}% 100%`,
            backgroundPosition: `${(frame / (frames - 1)) * 100}% 0`,
            transform: `scale(${zoom})`,
          }}
        />
      </div>
      <p className="pointer-events-none mt-1 text-center text-xs text-slate-blue">{hint}</p>
    </div>
  );
}
