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
    pub fn new(width: u32, height: u32, show_labels: bool, background_color: impl Into<String>) -> Self {
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
}
