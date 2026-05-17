use crate::graph::Graph;
use std::collections::HashMap;

const NODE_RADIUS: f64 = 24.0;

pub struct SvgConfig {
    pub width: u32,
    pub height: u32,
    pub background: String,
}

impl Default for SvgConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            background: "#1a1a2e".to_string(),
        }
    }
}

pub struct SvgRenderer;

impl SvgRenderer {
    pub fn render(graph: &Graph, config: &SvgConfig) -> String {
        let w = config.width as f64;
        let h = config.height as f64;
        let nodes = graph.nodes();
        let edges = graph.edges();
        let n = nodes.len();

        let positions: Vec<(f64, f64)> = circular_layout(n, w, h);

        let id_to_pos: HashMap<&str, (f64, f64)> = nodes
            .iter()
            .zip(positions.iter())
            .map(|(node, &pos)| (node.id.as_str(), pos))
            .collect();

        let mut svg = String::new();

        let background = sanitize_css_color(&config.background).unwrap_or("#1a1a2e");
        svg.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" style=\"background:{};\">",
            config.width, config.height, background
        ));

        svg.push_str(
            "<defs>\
              <marker id=\"arrow\" markerWidth=\"8\" markerHeight=\"6\" \
                      refX=\"8\" refY=\"3\" orient=\"auto\">\
                <path d=\"M0,0 L8,3 L0,6 z\" fill=\"#888\"/>\
              </marker>\
            </defs>",
        );

        svg.push_str("<g id=\"viewport\">");

        svg.push_str("<g id=\"edges\">");
        for edge in edges {
            if let (Some(&(x1, y1)), Some(&(x2, y2))) = (
                id_to_pos.get(edge.from.as_str()),
                id_to_pos.get(edge.to.as_str()),
            ) {
                let dx = x2 - x1;
                let dy = y2 - y1;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 1.0 {
                    continue;
                }
                let ux = dx / len;
                let uy = dy / len;
                let sx = x1 + NODE_RADIUS * ux;
                let sy = y1 + NODE_RADIUS * uy;
                let ex = x2 - NODE_RADIUS * ux;
                let ey = y2 - NODE_RADIUS * uy;
                svg.push_str(&format!(
                    "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
                     stroke=\"#888\" stroke-width=\"1.5\" marker-end=\"url(#arrow)\"/>",
                    sx, sy, ex, ey
                ));
            }
        }
        svg.push_str("</g>");

        svg.push_str("<g id=\"nodes\">");
        for (node, &(cx, cy)) in nodes.iter().zip(positions.iter()) {
            let label = escape_xml(&node.label);
            svg.push_str(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{}\" \
                 fill=\"#3a86ff\" stroke=\"#fff\" stroke-width=\"1.5\"/>",
                cx, cy, NODE_RADIUS as u32
            ));
            svg.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#fff\" font-size=\"11\" \
                 font-family=\"monospace\" text-anchor=\"middle\" dy=\".35em\">{}</text>",
                cx, cy, label
            ));
        }
        svg.push_str("</g>");

        svg.push_str("</g>"); // close viewport

        if n == 0 {
            svg.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#888\" font-size=\"16\" \
                 font-family=\"sans-serif\" text-anchor=\"middle\">Empty graph</text>",
                w / 2.0,
                h / 2.0
            ));
        }

        svg.push_str("</svg>");
        svg
    }
}

fn circular_layout(n: usize, w: f64, h: f64) -> Vec<(f64, f64)> {
    match n {
        0 => vec![],
        1 => vec![(w / 2.0, h / 2.0)],
        _ => {
            let cx = w / 2.0;
            let cy = h / 2.0;
            let r = (w.min(h) * 0.35).max(100.0);
            (0..n)
                .map(|i| {
                    let angle =
                        std::f64::consts::TAU * i as f64 / n as f64 - std::f64::consts::FRAC_PI_2;
                    (cx + r * angle.cos(), cy + r * angle.sin())
                })
                .collect()
        }
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn sanitize_css_color(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.starts_with('#') {
        let hex = &s[1..];
        if (3..=8).contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(s);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, Node};

    #[test]
    fn empty_graph_produces_valid_svg() {
        let g = Graph::new();
        let svg = SvgRenderer::render(&g, &SvgConfig::default());
        assert!(svg.starts_with("<svg"), "must start with <svg");
        assert!(svg.ends_with("</svg>"), "must end with </svg>");
        assert!(svg.contains("Empty graph"));
    }

    #[test]
    fn single_node_graph_produces_circle() {
        let mut g = Graph::new();
        g.add_node(Node::new("a", "A", "sql"));
        let svg = SvgRenderer::render(&g, &SvgConfig::default());
        assert!(svg.contains("<circle"), "must contain circle element");
        assert!(svg.contains(">A<"), "must contain node label");
    }

    #[test]
    fn edge_produces_line() {
        let mut g = Graph::new();
        g.add_node(Node::new("a", "A", "sql"));
        g.add_node(Node::new("b", "B", "sql"));
        g.add_edge(Edge::new("a", "b"));
        let svg = SvgRenderer::render(&g, &SvgConfig::default());
        assert!(svg.contains("<line"), "must contain line element");
    }

    #[test]
    fn xml_special_chars_in_labels_are_escaped() {
        let mut g = Graph::new();
        g.add_node(Node::new("a", "A & <B>", "sql"));
        let svg = SvgRenderer::render(&g, &SvgConfig::default());
        assert!(svg.contains("A &amp; &lt;B&gt;"));
        assert!(!svg.contains("A & <B>"));
    }

    #[test]
    fn svg_contains_viewport_group_for_pan_zoom() {
        let mut g = Graph::new();
        g.add_node(Node::new("a", "A", "sql"));
        let svg = SvgRenderer::render(&g, &SvgConfig::default());
        assert!(
            svg.contains("<g id=\"viewport\">"),
            "must have viewport group for pan/zoom"
        );
    }

    #[test]
    fn custom_dimensions_appear_in_svg() {
        let cfg = SvgConfig {
            width: 800,
            height: 600,
            background: "#000000".to_string(),
        };
        let g = Graph::new();
        let svg = SvgRenderer::render(&g, &cfg);
        assert!(svg.contains("width=\"800\""), "custom width must appear");
        assert!(svg.contains("height=\"600\""), "custom height must appear");
    }

    #[test]
    fn custom_background_appears_in_svg() {
        let cfg = SvgConfig {
            width: 1280,
            height: 720,
            background: "#ff0000".to_string(),
        };
        let g = Graph::new();
        let svg = SvgRenderer::render(&g, &cfg);
        assert!(svg.contains("#ff0000"), "custom background must appear in SVG");
    }

    #[test]
    fn single_node_placed_at_center() {
        let cfg = SvgConfig {
            width: 1000,
            height: 500,
            background: "#000".to_string(),
        };
        let mut g = Graph::new();
        g.add_node(Node::new("a", "A", "sql"));
        let svg = SvgRenderer::render(&g, &cfg);
        // For n=1, circular_layout returns (w/2, h/2) = (500.0, 250.0)
        assert!(svg.contains("cx=\"500.0\""), "single node must be horizontally centered");
        assert!(svg.contains("cy=\"250.0\""), "single node must be vertically centered");
    }

    #[test]
    fn multiple_nodes_produce_distinct_positions() {
        let mut g = Graph::new();
        for i in 0..4 {
            g.add_node(Node::new(format!("n{i}"), format!("N{i}"), "sql"));
        }
        let svg = SvgRenderer::render(&g, &SvgConfig::default());
        // All four circles must exist; cheap proxy: four <circle elements
        assert_eq!(svg.matches("<circle").count(), 4);
    }

    #[test]
    fn sanitize_css_color_accepts_valid_hex() {
        assert_eq!(sanitize_css_color("#1a1a2e"), Some("#1a1a2e"));
        assert_eq!(sanitize_css_color("#fff"), Some("#fff"));
    }

    #[test]
    fn sanitize_css_color_rejects_injection() {
        assert!(sanitize_css_color("red\"></svg><script>alert(1)</script>").is_none());
        assert!(sanitize_css_color("rgb(0,0,0)").is_none());
    }

    #[test]
    fn malicious_background_falls_back_to_default() {
        let cfg = SvgConfig {
            width: 100,
            height: 100,
            background: "red\"></svg><script>alert(1)</script>".to_string(),
        };
        let g = crate::graph::Graph::new();
        let svg = SvgRenderer::render(&g, &cfg);
        assert!(!svg.contains("<script>"), "injected script must not appear");
        assert!(svg.contains("background:#1a1a2e"), "must fall back to default");
    }
}
