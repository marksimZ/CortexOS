import {
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type Ref,
} from "react";
import { kinds } from "./knowledge-types";
import { relations, type Point } from "./graph-data";

export type LayoutMode = "mindmap" | "freeform";

export interface ViewportApi {
  capture: () => { camera: Point; target: Point };
  focus: (point: Point) => void;
  exitFocus: () => void;
  reset: () => void;
  fit: () => void;
  restore: (camera: Point, target: Point) => void;
}

export interface DisplayNode {
  id: string;
  title: string;
  kind: string;
  badge?: string;
}

export interface DisplayEdge {
  id: string;
  fromNodeId: string;
  toNodeId: string;
  relation: string;
}

interface Props {
  label?: string;
  layoutMode?: LayoutMode;
  showEdgeLabels?: boolean;
  onReady?: () => void;
  nodes: DisplayNode[];
  edges: DisplayEdge[];
  positions: Record<string, Point>;
  selected: string | null;
  multiSelected?: string[];
  camera: Point;
  target: Point;
  apiRef: Ref<ViewportApi>;
  onSelect: (id: string) => void;
  onMultiSelect?: (ids: string[]) => void;
  onNodeDrag?: (id: string, next: Point) => void;
  onChange: () => void;
  onFailure: () => void;
  onExitFocus?: () => void;
  onFocused?: () => void;
}

const CARD_W = 216;
const CARD_H = 62;
const LEVEL_GAP_X = 296;
const ROW_GAP_Y = 88;
const WORLD_SCALE_X = 165;
const WORLD_SCALE_Y = 115;

interface MindMapTreeLayout {
  coords: Record<string, { x: number; y: number }>;
  childrenOf: Record<string, string[]>;
  hiddenNodes: Set<string>;
}

function computeMindMapLayout(
  nodes: DisplayNode[],
  edges: DisplayEdge[],
  collapsed: Set<string>,
  preferredRoot: string | null,
): MindMapTreeLayout {
  const nodeMap = new Map(nodes.map((n) => [n.id, n]));
  const outgoing = new Map<string, string[]>();
  const inDegree = new Map<string, number>();
  for (const n of nodes) {
    outgoing.set(n.id, []);
    inDegree.set(n.id, 0);
  }
  for (const edge of edges) {
    if (!nodeMap.has(edge.fromNodeId) || !nodeMap.has(edge.toNodeId)) continue;
    if (edge.fromNodeId === edge.toNodeId) continue;
    outgoing.get(edge.fromNodeId)!.push(edge.toNodeId);
    inDegree.set(edge.toNodeId, (inDegree.get(edge.toNodeId) ?? 0) + 1);
  }

  // Sort candidate roots: preferredRoot first, then context/decision nodes with in-degree 0, then highest out-degree.
  const kindPriority: Record<string, number> = {
    context: 0,
    decision: 1,
    knowledge: 2,
    hypothesis: 3,
    evidence: 4,
    conflict: 5,
  };
  const sortedCandidates = [...nodes].sort((a, b) => {
    if (a.id === preferredRoot) return -1;
    if (b.id === preferredRoot) return 1;
    const inA = inDegree.get(a.id) ?? 0;
    const inB = inDegree.get(b.id) ?? 0;
    if ((inA === 0) !== (inB === 0)) return inA === 0 ? -1 : 1;
    const outDiff = (outgoing.get(b.id)?.length ?? 0) - (outgoing.get(a.id)?.length ?? 0);
    if (outDiff !== 0) return outDiff;
    return (kindPriority[a.kind] ?? 9) - (kindPriority[b.kind] ?? 9);
  });

  // Build spanning forest so cycles never hang and every node gets a deterministic tree parent.
  const visited = new Set<string>();
  const treeChildren: Record<string, string[]> = {};
  const roots: string[] = [];
  const isolated: DisplayNode[] = [];

  for (const n of nodes) treeChildren[n.id] = [];

  for (const candidate of sortedCandidates) {
    if (visited.has(candidate.id)) continue;
    const hasAnyEdge =
      (outgoing.get(candidate.id)?.length ?? 0) > 0 || (inDegree.get(candidate.id) ?? 0) > 0;
    if (!hasAnyEdge) {
      visited.add(candidate.id);
      isolated.push(candidate);
      continue;
    }
    roots.push(candidate.id);
    visited.add(candidate.id);
    const queue = [candidate.id];
    while (queue.length > 0) {
      const current = queue.shift()!;
      const neighbors = outgoing.get(current) ?? [];
      for (const next of neighbors) {
        if (!visited.has(next)) {
          visited.add(next);
          treeChildren[current].push(next);
          queue.push(next);
        }
      }
      // Also pull in reverse neighbors if they had no unvisited outgoing parent so connected components stay together.
      for (const edge of edges) {
        if (edge.toNodeId === current && !visited.has(edge.fromNodeId) && nodeMap.has(edge.fromNodeId)) {
          visited.add(edge.fromNodeId);
          treeChildren[current].push(edge.fromNodeId);
          queue.push(edge.fromNodeId);
        }
      }
    }
  }

  // Mark descendants of collapsed nodes as hidden.
  const hiddenNodes = new Set<string>();
  const markHidden = (id: string) => {
    for (const child of treeChildren[id] ?? []) {
      hiddenNodes.add(child);
      markHidden(child);
    }
  };
  for (const id of collapsed) {
    if (treeChildren[id]?.length) markHidden(id);
  }

  const coords: Record<string, { x: number; y: number }> = {};
  let cursorY = 0;

  const layoutSubtree = (id: string, depth: number): number => {
    const visibleKids = collapsed.has(id)
      ? []
      : (treeChildren[id] ?? []).filter((cid) => !hiddenNodes.has(cid));
    const x = depth * LEVEL_GAP_X;
    if (visibleKids.length === 0) {
      const y = cursorY;
      cursorY += ROW_GAP_Y;
      coords[id] = { x, y };
      return y;
    }
    const childYs = visibleKids.map((cid) => layoutSubtree(cid, depth + 1));
    const midY = (childYs[0] + childYs[childYs.length - 1]) / 2;
    coords[id] = { x, y: midY };
    return midY;
  };

  for (const rootId of roots) {
    if (hiddenNodes.has(rootId)) continue;
    layoutSubtree(rootId, 0);
    cursorY += 28; // Subtle breathing gap between independent topic trees
  }

  // Place isolated unlinked cards in a balanced grid that fills the default viewport width first.
  if (isolated.length > 0) {
    const baseY = roots.length > 0 ? cursorY + 24 : 0;
    const cols = Math.max(1, Math.min(4, Math.ceil(Math.sqrt(isolated.length * 1.35))));
    const compactGapX = 244;
    isolated.forEach((node, idx) => {
      const col = idx % cols;
      const row = Math.floor(idx / cols);
      coords[node.id] = {
        x: col * compactGapX,
        y: baseY + row * ROW_GAP_Y,
      };
    });
  }

  // Center coordinates around (0, 0) for natural initial viewport framing.
  const values = Object.values(coords);
  if (values.length > 0) {
    const minX = Math.min(...values.map((v) => v.x));
    const maxX = Math.max(...values.map((v) => v.x));
    const minY = Math.min(...values.map((v) => v.y));
    const maxY = Math.max(...values.map((v) => v.y));
    const cx = (minX + maxX) / 2;
    const cy = (minY + maxY) / 2;
    for (const key of Object.keys(coords)) {
      coords[key] = { x: coords[key].x - cx, y: coords[key].y - cy };
    }
  }

  return { coords, childrenOf: treeChildren, hiddenNodes };
}

export default function GraphViewport({
  label = "二维知识思维导图",
  layoutMode = "mindmap",
  showEdgeLabels = false,
  onReady,
  nodes,
  edges,
  positions,
  selected,
  multiSelected = [],
  camera: _camera,
  target: _target,
  apiRef,
  onSelect,
  onMultiSelect,
  onNodeDrag,
  onChange,
  onExitFocus,
  onFocused,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [pan, setPan] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [zoom, setZoom] = useState<number>(1);
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const [marquee, setMarquee] = useState<{
    x1: number;
    y1: number;
    x2: number;
    y2: number;
  } | null>(null);
  const animRef = useRef<number | null>(null);
  const fittedOnceRef = useRef(false);

  const cancelAnimation = useCallback(() => {
    if (animRef.current !== null) {
      cancelAnimationFrame(animRef.current);
      animRef.current = null;
    }
  }, []);

  const panRef = useRef(pan);
  const zoomRef = useRef(zoom);
  panRef.current = pan;
  zoomRef.current = zoom;

  const mindmap = useMemo(
    () => computeMindMapLayout(nodes, edges, collapsed, null),
    [nodes, edges, collapsed],
  );

  const nodeCoords = useMemo(() => {
    if (layoutMode === "mindmap") return mindmap.coords;
    const out: Record<string, { x: number; y: number }> = {};
    const cols = Math.max(1, Math.min(4, Math.ceil(Math.sqrt(nodes.length * 1.35))));
    nodes.forEach((node, idx) => {
      const p = positions[node.id];
      // Use saved 2D position if reasonable; compactly pack huge legacy 3D spiral positions into default area first.
      if (p && Math.abs(p[0]) <= 12 && Math.abs(p[1]) <= 12 && p[2] === 0) {
        out[node.id] = { x: p[0] * WORLD_SCALE_X, y: -p[1] * WORLD_SCALE_Y };
      } else {
        const col = idx % cols;
        const row = Math.floor(idx / cols);
        const totalRows = Math.ceil(nodes.length / cols);
        out[node.id] = {
          x: (col - (cols - 1) / 2) * 244,
          y: (row - (totalRows - 1) / 2) * ROW_GAP_Y,
        };
      }
    });
    return out;
  }, [layoutMode, mindmap.coords, nodes, positions]);

  const visibleNodes = useMemo(
    () =>
      layoutMode === "mindmap"
        ? nodes.filter((n) => !mindmap.hiddenNodes.has(n.id))
        : nodes,
    [layoutMode, nodes, mindmap.hiddenNodes],
  );
  const visibleSet = useMemo(() => new Set(visibleNodes.map((n) => n.id)), [visibleNodes]);

  const contentBounds = useMemo(() => {
    let minX = Infinity;
    let maxX = -Infinity;
    let minY = Infinity;
    let maxY = -Infinity;
    for (const node of visibleNodes) {
      const c = nodeCoords[node.id];
      if (!c) continue;
      minX = Math.min(minX, c.x - CARD_W / 2);
      maxX = Math.max(maxX, c.x + CARD_W / 2);
      minY = Math.min(minY, c.y - CARD_H / 2);
      maxY = Math.max(maxY, c.y + CARD_H / 2);
    }
    if (!Number.isFinite(minX)) {
      return { minX: -CARD_W / 2, maxX: CARD_W / 2, minY: -CARD_H / 2, maxY: CARD_H / 2 };
    }
    return { minX, maxX, minY, maxY };
  }, [nodeCoords, visibleNodes]);

  // Bounded canvas rule: keep pan locked to the default viewport until nodes fill it,
  // and once expanded, strictly clamp pan to the expanded node bounding box + padding so nodes can NEVER get lost.
  const clampCamera = useCallback(
    (rawPan: { x: number; y: number }, rawZoom: number) => {
      const el = containerRef.current;
      const viewW = el ? el.clientWidth : 960;
      const viewH = el ? el.clientHeight : 600;
      const pad = 56;
      const contentW = contentBounds.maxX - contentBounds.minX + pad * 2;
      const contentH = contentBounds.maxY - contentBounds.minY + pad * 2;
      const fitZoom = Math.min(viewW / Math.max(1, contentW), viewH / Math.max(1, contentH));
      const minAllowedZoom = Math.min(0.85, Math.max(0.3, fitZoom * 0.92));
      const z = Math.min(2.2, Math.max(minAllowedZoom, rawZoom));

      const cx = (contentBounds.minX + contentBounds.maxX) / 2;
      const cy = (contentBounds.minY + contentBounds.maxY) / 2;
      const centerPanX = -cx * z;
      const centerPanY = -cy * z;

      const scaledW = contentW * z;
      const scaledH = contentH * z;

      let x: number;
      if (scaledW <= viewW) {
        // Fits inside default viewport width: allow only tiny elastic nudge (±32px) around center
        const slack = 32;
        x = Math.min(centerPanX + slack, Math.max(centerPanX - slack, rawPan.x));
      } else {
        // Expanded beyond default width: clamp strictly to expanded bounds
        const minPanX = viewW / 2 - (contentBounds.maxX + pad) * z;
        const maxPanX = -viewW / 2 - (contentBounds.minX - pad) * z;
        x = Math.min(maxPanX, Math.max(minPanX, rawPan.x));
      }

      let y: number;
      if (scaledH <= viewH) {
        const slack = 32;
        y = Math.min(centerPanY + slack, Math.max(centerPanY - slack, rawPan.y));
      } else {
        const minPanY = viewH / 2 - (contentBounds.maxY + pad) * z;
        const maxPanY = -viewH / 2 - (contentBounds.minY - pad) * z;
        y = Math.min(maxPanY, Math.max(minPanY, rawPan.y));
      }

      return { pan: { x, y }, zoom: z, expanded: scaledW > viewW || scaledH > viewH };
    },
    [contentBounds],
  );

  const animateCamera = useCallback(
    (rawNextPan: { x: number; y: number }, rawNextZoom: number, done?: () => void) => {
      cancelAnimation();
      const clamped = clampCamera(rawNextPan, rawNextZoom);
      const nextPan = clamped.pan;
      const nextZoom = clamped.zoom;
      const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      if (reduced) {
        setPan(nextPan);
        setZoom(nextZoom);
        done?.();
        return;
      }
      const startPan = { ...panRef.current };
      const startZoom = zoomRef.current;
      const started = performance.now();
      const duration = 200;
      const step = (now: number) => {
        const t = Math.min(1, (now - started) / duration);
        const eased = 1 - Math.pow(1 - t, 3);
        setPan({
          x: startPan.x + (nextPan.x - startPan.x) * eased,
          y: startPan.y + (nextPan.y - startPan.y) * eased,
        });
        setZoom(startZoom + (nextZoom - startZoom) * eased);
        if (t < 1) {
          animRef.current = requestAnimationFrame(step);
        } else {
          animRef.current = null;
          done?.();
        }
      };
      animRef.current = requestAnimationFrame(step);
    },
    [cancelAnimation, clampCamera],
  );

  const fitView = useCallback(() => {
    const el = containerRef.current;
    if (!el || visibleNodes.length === 0) return;
    const rect = el.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    const pad = 64;
    const spanX = Math.max(CARD_W + pad, contentBounds.maxX - contentBounds.minX + pad * 2);
    const spanY = Math.max(CARD_H + pad, contentBounds.maxY - contentBounds.minY + pad * 2);
    // Keep 100% readable scale while everything fits in default area; shrink gracefully only once filled.
    const nextZoom = Math.min(1.05, Math.max(0.45, Math.min(rect.width / spanX, rect.height / spanY)));
    const cx = (contentBounds.minX + contentBounds.maxX) / 2;
    const cy = (contentBounds.minY + contentBounds.maxY) / 2;
    animateCamera({ x: -cx * nextZoom, y: -cy * nextZoom }, nextZoom);
  }, [animateCamera, contentBounds, visibleNodes.length]);

  useEffect(() => {
    onReady?.();
    fittedOnceRef.current = false;
  }, [layoutMode]);

  useEffect(() => {
    if (visibleNodes.length > 0 && !fittedOnceRef.current) {
      fittedOnceRef.current = true;
      const id = requestAnimationFrame(() => fitView());
      return () => cancelAnimationFrame(id);
    }
    // Re-clamp camera whenever bounds change (e.g. collapsing a branch) so view never gets stranded
    if (visibleNodes.length > 0) {
      const clamped = clampCamera(panRef.current, zoomRef.current);
      setPan(clamped.pan);
      setZoom(clamped.zoom);
    }
  }, [visibleNodes.length, contentBounds, clampCamera, fitView]);

  useEffect(() => () => cancelAnimation(), [cancelAnimation]);

  useImperativeHandle(
    apiRef,
    () => ({
      capture: () => ({
        camera: [0, 9, Number((22 / zoomRef.current).toFixed(2))],
        target: [
          Number((-panRef.current.x / 40).toFixed(2)),
          Number((-panRef.current.y / 40).toFixed(2)),
          0,
        ],
      }),
      focus: (point: Point) => {
        // Find matching node coordinate or fallback to point
        let targetCoord: { x: number; y: number } | undefined;
        if (selected && nodeCoords[selected]) {
          targetCoord = nodeCoords[selected];
        } else if (point) {
          targetCoord = { x: point[0] * WORLD_SCALE_X, y: -point[1] * WORLD_SCALE_Y };
        }
        if (!targetCoord) {
          onFocused?.();
          return;
        }
        const nextZoom = Math.max(zoomRef.current, 1.0);
        animateCamera(
          { x: -targetCoord.x * nextZoom, y: -targetCoord.y * nextZoom },
          nextZoom,
          onFocused,
        );
      },
      exitFocus: () => {
        cancelAnimation();
      },
      reset: () => {
        setCollapsed(new Set());
        requestAnimationFrame(() => fitView());
      },
      fit: () => {
        fitView();
      },
      restore: (savedCam: Point, savedTarget: Point) => {
        const z = Math.min(2.2, Math.max(0.35, 22 / Math.max(6, savedCam?.[2] ?? 22)));
        animateCamera(
          { x: -(savedTarget?.[0] ?? 0) * 40, y: -(savedTarget?.[1] ?? 0) * 40 },
          z,
        );
      },
    }),
    [animateCamera, cancelAnimation, fitView, nodeCoords, onFocused, selected],
  );

  // Wheel zoom centered around pointer
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      cancelAnimation();
      const rect = el.getBoundingClientRect();
      const pointerX = event.clientX - rect.left - rect.width / 2;
      const pointerY = event.clientY - rect.top - rect.height / 2;
      const factor = event.deltaY < 0 ? 1.1 : 0.91;
      const prevZoom = zoomRef.current;
      const worldX = (pointerX - panRef.current.x) / prevZoom;
      const worldY = (pointerY - panRef.current.y) / prevZoom;
      const rawNextZoom = prevZoom * factor;
      const rawNextPan = {
        x: pointerX - worldX * rawNextZoom,
        y: pointerY - worldY * rawNextZoom,
      };
      const clamped = clampCamera(rawNextPan, rawNextZoom);
      if (clamped.zoom === prevZoom && clamped.pan.x === panRef.current.x && clamped.pan.y === panRef.current.y) return;
      setZoom(clamped.zoom);
      setPan(clamped.pan);
      onChange();
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [cancelAnimation, clampCamera, onChange]);

  const multiSet = useMemo(() => new Set(multiSelected), [multiSelected]);

  const handleCanvasPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 && event.button !== 1) return;
    if ((event.target as HTMLElement).closest(".mindmap-node-card, .mindmap-collapse-btn, .mindmap-hud")) {
      return;
    }
    cancelAnimation();
    const el = containerRef.current;
    if (!el) return;
    el.setPointerCapture(event.pointerId);
    const rect = el.getBoundingClientRect();
    const startX = event.clientX;
    const startY = event.clientY;

    // Shift + Drag on empty canvas = Marquee Box Selection!
    if (event.shiftKey && event.button === 0 && onMultiSelect) {
      const originX = startX - rect.left;
      const originY = startY - rect.top;
      setMarquee({ x1: originX, y1: originY, x2: originX, y2: originY });
      const onMove = (moveEvent: PointerEvent) => {
        const curX = moveEvent.clientX - rect.left;
        const curY = moveEvent.clientY - rect.top;
        setMarquee({ x1: originX, y1: originY, x2: curX, y2: curY });
      };
      const onUp = (upEvent: PointerEvent) => {
        el.releasePointerCapture(event.pointerId);
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
        const endX = upEvent.clientX - rect.left;
        const endY = upEvent.clientY - rect.top;
        setMarquee(null);
        const left = Math.min(originX, endX);
        const right = Math.max(originX, endX);
        const top = Math.min(originY, endY);
        const bottom = Math.max(originY, endY);
        if (right - left < 6 && bottom - top < 6) return;
        const centerW = rect.width / 2;
        const centerH = rect.height / 2;
        const hits: string[] = [];
        for (const node of visibleNodes) {
          const c = nodeCoords[node.id];
          if (!c) continue;
          const screenX = centerW + panRef.current.x + c.x * zoomRef.current;
          const screenY = centerH + panRef.current.y + c.y * zoomRef.current;
          const halfW = (CARD_W * zoomRef.current) / 2;
          const halfH = (CARD_H * zoomRef.current) / 2;
          if (
            screenX + halfW >= left &&
            screenX - halfW <= right &&
            screenY + halfH >= top &&
            screenY - halfH <= bottom
          ) {
            hits.push(node.id);
          }
        }
        onMultiSelect(hits);
      };
      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
      return;
    }

    // Normal Canvas Pan (strictly bounded to default viewport unless nodes exceed it)
    const initialPan = { ...panRef.current };
    let moved = false;
    const onMove = (moveEvent: PointerEvent) => {
      const dx = moveEvent.clientX - startX;
      const dy = moveEvent.clientY - startY;
      if (Math.abs(dx) > 3 || Math.abs(dy) > 3) moved = true;
      const clamped = clampCamera({ x: initialPan.x + dx, y: initialPan.y + dy }, zoomRef.current);
      setPan(clamped.pan);
    };
    const onUp = () => {
      el.releasePointerCapture(event.pointerId);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      if (moved) {
        onChange();
      } else {
        if (multiSelected.length > 0) onMultiSelect?.([]);
        onExitFocus?.();
      }
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  const handleNodePointerDown = (event: React.PointerEvent, node: DisplayNode) => {
    if (event.button !== 0) return;
    event.stopPropagation();
    cancelAnimation();

    if (event.shiftKey || event.ctrlKey || event.metaKey) {
      if (onMultiSelect) {
        const next = multiSet.has(node.id)
          ? multiSelected.filter((id) => id !== node.id)
          : [...multiSelected, node.id];
        onMultiSelect(next);
      }
      return;
    }

    if (layoutMode === "freeform" && onNodeDrag) {
      const startX = event.clientX;
      const startY = event.clientY;
      const startCoord = nodeCoords[node.id] ?? { x: 0, y: 0 };
      let dragged = false;
      const onMove = (moveEvent: PointerEvent) => {
        const dx = (moveEvent.clientX - startX) / zoomRef.current;
        const dy = (moveEvent.clientY - startY) / zoomRef.current;
        if (Math.abs(dx) > 3 || Math.abs(dy) > 3) dragged = true;
        if (dragged) {
          const nextWorldX = (startCoord.x + dx) / WORLD_SCALE_X;
          const nextWorldY = -(startCoord.y + dy) / WORLD_SCALE_Y;
          onNodeDrag(node.id, [
            Number(nextWorldX.toFixed(3)),
            Number(nextWorldY.toFixed(3)),
            0,
          ]);
        }
      };
      const onUp = () => {
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
        if (!dragged) {
          onSelect(node.id);
        } else {
          onChange();
        }
      };
      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
      return;
    }

    onSelect(node.id);
  };

  const toggleCollapse = (event: React.MouseEvent, id: string) => {
    event.stopPropagation();
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <div
      ref={containerRef}
      className={`mindmap-viewport mode-${layoutMode}`}
      role="application"
      aria-label={label}
      tabIndex={0}
      onPointerDown={handleCanvasPointerDown}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          if (multiSelected.length > 0) onMultiSelect?.([]);
          else onExitFocus?.();
        }
      }}
    >
      <div
        className="mindmap-world"
        style={{
          transform: `translate(calc(50% + ${pan.x}px), calc(50% + ${pan.y}px)) scale(${zoom})`,
        }}
      >
        <svg className="mindmap-edges" overflow="visible" aria-hidden="true">
          <defs>
            <marker
              id="mindmap-arrow"
              viewBox="0 0 10 10"
              refX="8"
              refY="5"
              markerWidth="6"
              markerHeight="6"
              orient="auto-start-reverse"
            >
              <path d="M 0 1.5 L 8.5 5 L 0 8.5 z" fill="var(--graph-edge)" />
            </marker>
            <marker
              id="mindmap-arrow-active"
              viewBox="0 0 10 10"
              refX="8"
              refY="5"
              markerWidth="6"
              markerHeight="6"
              orient="auto-start-reverse"
            >
              <path d="M 0 1.5 L 8.5 5 L 0 8.5 z" fill="var(--accent)" />
            </marker>
          </defs>
          {edges.map((edge) => {
            if (!visibleSet.has(edge.fromNodeId) || !visibleSet.has(edge.toNodeId)) return null;
            const from = nodeCoords[edge.fromNodeId];
            const to = nodeCoords[edge.toNodeId];
            if (!from || !to) return null;
            const rightwards = to.x >= from.x;
            const x1 = rightwards ? from.x + CARD_W / 2 : from.x - CARD_W / 2;
            const y1 = from.y;
            const x2 = rightwards ? to.x - CARD_W / 2 : to.x + CARD_W / 2;
            const y2 = to.y;
            const span = Math.max(48, Math.abs(x2 - x1) * 0.5);
            const cx1 = rightwards ? x1 + span : x1 - span;
            const cx2 = rightwards ? x2 - span : x2 + span;
            const highlighted =
              selected === edge.fromNodeId ||
              selected === edge.toNodeId ||
              (multiSet.has(edge.fromNodeId) && multiSet.has(edge.toNodeId));
            const midX = (x1 + x2) / 2;
            const midY = (y1 + y2) / 2;
            const relLabel = relations[edge.relation] ?? edge.relation;
            return (
              <g key={edge.id} className={`mindmap-edge relation-${edge.relation} ${highlighted ? "is-highlighted" : ""}`}>
                <path
                  d={`M ${x1} ${y1} C ${cx1} ${y1}, ${cx2} ${y2}, ${x2} ${y2}`}
                  markerEnd={highlighted ? "url(#mindmap-arrow-active)" : "url(#mindmap-arrow)"}
                />
                {(showEdgeLabels || highlighted) && (
                  <g transform={`translate(${midX}, ${midY})`}>
                    <rect
                      x={-20}
                      y={-10}
                      width={40}
                      height={20}
                      rx={6}
                      className="mindmap-edge-pill"
                    />
                    <text textAnchor="middle" dominantBaseline="central" className="mindmap-edge-text">
                      {relLabel}
                    </text>
                  </g>
                )}
              </g>
            );
          })}
        </svg>

        {visibleNodes.map((node) => {
          const coord = nodeCoords[node.id];
          if (!coord) return null;
          const isSelected = selected === node.id;
          const isMulti = multiSet.has(node.id);
          const kids = mindmap.childrenOf[node.id] ?? [];
          const isCollapsed = collapsed.has(node.id);
          return (
            <div
              key={node.id}
              style={{
                transform: `translate(${coord.x - CARD_W / 2}px, ${coord.y - CARD_H / 2}px)`,
                width: CARD_W,
                minHeight: CARD_H,
              }}
              className={`mindmap-node-card kind-${node.kind} ${isSelected ? "is-selected" : ""} ${isMulti ? "is-multi-selected" : ""}`}
              role="button"
              tabIndex={0}
              aria-pressed={isSelected || isMulti}
              onPointerDown={(event) => handleNodePointerDown(event, node)}
              onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  onSelect(node.id);
                }
              }}
            >
              <span className={`mindmap-node-accent node-${node.kind}`} aria-hidden="true" />
              <div className="mindmap-node-body">
                <div className="mindmap-node-meta">
                  <span className="mindmap-kind-tag">{kinds[node.kind] ?? node.kind}</span>
                  {node.badge && <span className="mindmap-draft-tag">{node.badge}</span>}
                  {isMulti && <span className="mindmap-multi-tag">已选</span>}
                </div>
                <div className="mindmap-node-title" title={node.title}>
                  {node.title}
                </div>
              </div>
              {layoutMode === "mindmap" && kids.length > 0 && (
                <button
                  type="button"
                  className="mindmap-collapse-btn"
                  title={isCollapsed ? `展开 ${kids.length} 个下级分支` : "折叠下级分支"}
                  aria-label={isCollapsed ? `展开 ${kids.length} 个下级分支` : "折叠下级分支"}
                  onClick={(event) => toggleCollapse(event, node.id)}
                >
                  {isCollapsed ? `+${kids.length}` : "−"}
                </button>
              )}
            </div>
          );
        })}
      </div>

      {marquee && (
        <div
          className="mindmap-marquee"
          style={{
            left: Math.min(marquee.x1, marquee.x2),
            top: Math.min(marquee.y1, marquee.y2),
            width: Math.abs(marquee.x2 - marquee.x1),
            height: Math.abs(marquee.y2 - marquee.y1),
          }}
        />
      )}

      <div className="mindmap-hud" aria-label="缩放比例与提示">
        <span>{Math.round(zoom * 100)}%</span>
        <span className="mindmap-hud-hint">滚轮缩放 · 拖拽平移 · Shift+拖拽框选节点</span>
      </div>
    </div>
  );
}
