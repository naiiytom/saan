import { useState, useEffect } from "react";
import { GraphView } from "./components/GraphView";
import init, { render_graph } from "saan_mesh";

const EXAMPLE_NODES = JSON.stringify([
  { id: "raw.orders", label: "raw.orders" },
  { id: "stg.orders", label: "stg.orders" },
  { id: "marts.summary", label: "marts.summary" },
]);

const EXAMPLE_EDGES = JSON.stringify([
  { from_id: "raw.orders", to_id: "stg.orders" },
  { from_id: "stg.orders", to_id: "marts.summary" },
]);

export default function App() {
  const [svgOutput, setSvgOutput] = useState<string>("");
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    init().then(() => setReady(true)).catch((e: unknown) => {
      setError(e instanceof Error ? e.message : "Failed to load WASM module");
    });
  }, []);

  useEffect(() => {
    if (!ready) return;
    setSvgOutput(render_graph(EXAMPLE_NODES, EXAMPLE_EDGES, ""));
  }, [ready]);

  return (
    <div style={{ width: "100vw", height: "100vh", background: "#1a1a2e" }}>
      {error ? (
        <p style={{ color: "#f66", padding: "2rem" }}>Error: {error}</p>
      ) : svgOutput ? (
        <GraphView svgString={svgOutput} />
      ) : (
        <p style={{ color: "#888", padding: "2rem" }}>Loading WASM module…</p>
      )}
    </div>
  );
}
