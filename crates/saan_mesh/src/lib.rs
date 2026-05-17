use serde::Deserialize;
use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, TAU};
use wasm_bindgen::prelude::*;

const NODE_RADIUS: f64 = 24.0;

/// Configuration controlling how the lineage mesh is rendered in the browser.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderConfig {
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Whether to display node labels.
    pub show_labels: bool,
    /// Background colour as a CSS hex string (e.g. "#1a1a2e").
    pub background_color: String,
}

impl RenderConfig {
    pub fn new(
        width: u32,
        height: u32,
        show_labels: bool,
        background_color: impl Into<String>,
    ) -> Self {
        Self {
            width,
            height,
            show_labels,
            background_color: background_color.into(),
        }
    }
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            show_labels: true,
            background_color: String::from("#1a1a2e"),
        }
    }
}

#[derive(Deserialize)]
struct NodeData {
    id: String,
    label: String,
}

#[derive(Deserialize)]
struct EdgeData {
    from_id: String,
    to_id: String,
}

/// Render a lineage graph to an SVG string.
///
/// `nodes_json`: JSON array of `{id, label}` objects.
/// `edges_json`: JSON array of `{from_id, to_id}` objects.
/// `config_json`: optional JSON `{width, height, background_color}` (empty string uses defaults).
#[wasm_bindgen]
pub fn render_graph(nodes_json: &str, edges_json: &str, config_json: &str) -> String {
    let nodes: Vec<NodeData> = serde_json::from_str(nodes_json).unwrap_or_default();
    let edges: Vec<EdgeData> = serde_json::from_str(edges_json).unwrap_or_default();
    let config = parse_config(config_json);
    svg_from_data(&nodes, &edges, &config)
}

#[derive(Deserialize, Default)]
struct ConfigData {
    width: Option<u32>,
    height: Option<u32>,
    show_labels: Option<bool>,
    background_color: Option<String>,
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

fn parse_config(json: &str) -> RenderConfig {
    let data: ConfigData = serde_json::from_str(json).unwrap_or_default();
    let defaults = RenderConfig::default();
    RenderConfig {
        width: data.width.unwrap_or(defaults.width),
        height: data.height.unwrap_or(defaults.height),
        show_labels: data.show_labels.unwrap_or(defaults.show_labels),
        background_color: data
            .background_color
            .as_deref()
            .and_then(sanitize_css_color)
            .map(str::to_owned)
            .unwrap_or(defaults.background_color),
    }
}

fn svg_from_data(nodes: &[NodeData], edges: &[EdgeData], config: &RenderConfig) -> String {
    let w = config.width as f64;
    let h = config.height as f64;
    let n = nodes.len();

    let positions: Vec<(f64, f64)> = match n {
        0 => vec![],
        1 => vec![(w / 2.0, h / 2.0)],
        _ => {
            let cx = w / 2.0;
            let cy = h / 2.0;
            let r = (w.min(h) * 0.35).max(100.0);
            (0..n)
                .map(|i| {
                    let angle = TAU * i as f64 / n as f64 - FRAC_PI_2;
                    (cx + r * angle.cos(), cy + r * angle.sin())
                })
                .collect()
        }
    };

    let id_to_pos: HashMap<&str, (f64, f64)> = nodes
        .iter()
        .zip(positions.iter())
        .map(|(node, &pos)| (node.id.as_str(), pos))
        .collect();

    let mut svg = String::new();

    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" style=\"background:{};\">",
        config.width, config.height, config.background_color
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
            id_to_pos.get(edge.from_id.as_str()),
            id_to_pos.get(edge.to_id.as_str()),
        ) {
            let dx = x2 - x1;
            let dy = y2 - y1;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1.0 {
                continue;
            }
            let ux = dx / len;
            let uy = dy / len;
            svg.push_str(&format!(
                "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
                 stroke=\"#888\" stroke-width=\"1.5\" marker-end=\"url(#arrow)\"/>",
                x1 + NODE_RADIUS * ux,
                y1 + NODE_RADIUS * uy,
                x2 - NODE_RADIUS * ux,
                y2 - NODE_RADIUS * uy,
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
        if config.show_labels {
            svg.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#fff\" font-size=\"11\" \
                 font-family=\"monospace\" text-anchor=\"middle\" dy=\".35em\">{}</text>",
                cx, cy, label
            ));
        }
    }
    svg.push_str("</g>");

    svg.push_str("</g>");

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

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_dimensions() {
        let cfg = RenderConfig::default();
        assert_eq!(cfg.width, 1280);
        assert_eq!(cfg.height, 720);
    }

    #[test]
    fn default_config_shows_labels() {
        let cfg = RenderConfig::default();
        assert!(cfg.show_labels);
    }

    #[test]
    fn default_config_has_dark_background() {
        let cfg = RenderConfig::default();
        assert_eq!(cfg.background_color, "#1a1a2e");
    }

    #[test]
    fn custom_config_stores_values() {
        let cfg = RenderConfig::new(1920, 1080, false, "#ffffff");
        assert_eq!(cfg.width, 1920);
        assert_eq!(cfg.height, 1080);
        assert!(!cfg.show_labels);
        assert_eq!(cfg.background_color, "#ffffff");
    }

    #[test]
    fn config_equality() {
        let a = RenderConfig::default();
        let b = RenderConfig::default();
        assert_eq!(a, b);
    }

    #[test]
    fn configs_with_different_dimensions_are_not_equal() {
        let a = RenderConfig::default();
        let b = RenderConfig::new(800, 600, true, "#1a1a2e");
        assert_ne!(a, b);
    }

    #[test]
    fn render_graph_empty_input_returns_svg() {
        let svg = render_graph("[]", "[]", "");
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("Empty graph"));
    }

    #[test]
    fn render_graph_single_node_returns_circle() {
        let svg = render_graph(r#"[{"id":"a","label":"A"}]"#, "[]", "");
        assert!(svg.contains("<circle"));
        assert!(svg.contains(">A<"));
    }

    #[test]
    fn render_graph_edge_returns_line() {
        let svg = render_graph(
            r#"[{"id":"a","label":"A"},{"id":"b","label":"B"}]"#,
            r#"[{"from_id":"a","to_id":"b"}]"#,
            "",
        );
        assert!(svg.contains("<line"));
    }

    #[test]
    fn render_graph_invalid_json_returns_empty_svg() {
        let svg = render_graph("not json", "[]", "");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("Empty graph"));
    }

    #[test]
    fn render_graph_show_labels_false_omits_text_elements() {
        let config = "{\"show_labels\":false}";
        let svg = render_graph(
            r#"[{"id":"a","label":"Alpha"}]"#,
            "[]",
            config,
        );
        assert!(svg.contains("<circle"), "circle must still be present");
        assert!(
            !svg.contains(">Alpha</text>"),
            "label text must be absent when show_labels is false"
        );
    }

    #[test]
    fn render_graph_show_labels_true_includes_text_elements() {
        let config = "{\"show_labels\":true}";
        let svg = render_graph(
            r#"[{"id":"a","label":"Alpha"}]"#,
            "[]",
            config,
        );
        assert!(svg.contains(">Alpha</text>"), "label text must appear");
    }

    #[test]
    fn parse_config_partial_json_uses_defaults_for_missing_fields() {
        // Only override width; everything else should fall back to defaults.
        let cfg = parse_config("{\"width\":800}");
        assert_eq!(cfg.width, 800, "overridden field must be applied");
        assert_eq!(
            cfg.height,
            RenderConfig::default().height,
            "missing height must use default"
        );
        assert_eq!(
            cfg.show_labels,
            RenderConfig::default().show_labels,
            "missing show_labels must use default"
        );
    }

    #[test]
    fn parse_config_empty_string_returns_defaults() {
        let cfg = parse_config("");
        assert_eq!(cfg, RenderConfig::default());
    }

    #[test]
    fn parse_config_invalid_json_returns_defaults() {
        let cfg = parse_config("not json at all");
        assert_eq!(cfg, RenderConfig::default());
    }

    #[test]
    fn render_graph_edge_with_unknown_node_id_is_skipped() {
        // Only node "a" exists; edge references missing node "z".
        let svg = render_graph(
            r#"[{"id":"a","label":"A"}]"#,
            r#"[{"from_id":"a","to_id":"z"}]"#,
            "",
        );
        assert!(!svg.contains("<line"), "line for missing endpoint must not appear");
    }

    #[test]
    fn render_graph_custom_background_color_appears_in_svg() {
        let config = "{\"background_color\":\"#ff0000\"}";
        let svg = render_graph("[]", "[]", config);
        assert!(svg.contains("#ff0000"), "custom background must appear in SVG");
    }

    #[test]
    fn render_graph_custom_dimensions_appear_in_svg() {
        let config = "{\"width\":400,\"height\":300}";
        let svg = render_graph("[]", "[]", config);
        assert!(svg.contains("width=\"400\""), "custom width must appear");
        assert!(svg.contains("height=\"300\""), "custom height must appear");
    }

    #[test]
    fn sanitize_css_color_accepts_three_digit_hex() {
        assert_eq!(sanitize_css_color("#abc"), Some("#abc"));
    }

    #[test]
    fn sanitize_css_color_accepts_six_digit_hex() {
        assert_eq!(sanitize_css_color("#1a2b3c"), Some("#1a2b3c"));
    }

    #[test]
    fn sanitize_css_color_rejects_injection_attempt() {
        let malicious = "red\"></svg><script>alert(1)</script>";
        assert!(sanitize_css_color(malicious).is_none());
    }

    #[test]
    fn sanitize_css_color_rejects_css_without_hash() {
        assert!(sanitize_css_color("red").is_none());
        assert!(sanitize_css_color("rgb(255,0,0)").is_none());
    }

    #[test]
    fn parse_config_malicious_background_color_falls_back_to_default() {
        let cfg = parse_config(r#"{"background_color":"red\"><script>alert(1)</script>"}"#);
        assert_eq!(cfg.background_color, RenderConfig::default().background_color);
        assert!(!cfg.background_color.contains('<'));
    }

    #[test]
    fn render_graph_malicious_background_color_not_in_output() {
        let config = r#"{"background_color":"red\"></svg><script>alert(1)</script>"}"#;
        let svg = render_graph("[]", "[]", config);
        assert!(!svg.contains("<script>"), "injected script must not appear");
        assert!(svg.contains("background:#1a1a2e"), "must fall back to default color");
    }
}
