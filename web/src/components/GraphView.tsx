import { useEffect, useRef } from "react";

interface Props {
  svgString: string;
}

export function GraphView({ svgString }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    el.innerHTML = svgString;
    const svg = el.querySelector("svg");
    if (!svg) return;
    svg.style.width = "100%";
    svg.style.height = "100%";

    // Pan/zoom
    const vp = svg.querySelector<SVGGElement>("#viewport");
    if (!vp) return;
    let tx = 0, ty = 0, scale = 1;
    let dragging = false, startX = 0, startY = 0;

    const update = () => {
      vp.setAttribute("transform", `translate(${tx},${ty}) scale(${scale})`);
    };

    const onMouseDown = (e: MouseEvent) => {
      dragging = true;
      startX = e.clientX - tx;
      startY = e.clientY - ty;
      e.preventDefault();
    };
    const onMouseMove = (e: MouseEvent) => {
      if (!dragging) return;
      tx = e.clientX - startX;
      ty = e.clientY - startY;
      update();
    };
    const onMouseUp = () => { dragging = false; };
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const factor = e.deltaY < 0 ? 1.1 : 0.9;
      const rect = svg.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      tx = mx - factor * (mx - tx);
      ty = my - factor * (my - ty);
      scale *= factor;
      update();
    };

    svg.addEventListener("mousedown", onMouseDown);
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    svg.addEventListener("wheel", onWheel, { passive: false });

    return () => {
      svg.removeEventListener("mousedown", onMouseDown);
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      svg.removeEventListener("wheel", onWheel);
    };
  }, [svgString]);

  return <div ref={containerRef} style={{ width: "100%", height: "100%" }} />;
}
