//! Our icons recoloured in the SVG itself (Qt 6.4 fallback, Ubuntu 24.04):
//! Qt 6.5+ tints them on the GPU with QtQuick.Effects instead.

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

/// "#rrggbb" or Qt's "#aarrggbb" → ("#rrggbb", opacity).
fn split_color(color: &str) -> (String, f64) {
    let hex = color.trim_start_matches('#');
    match hex.len() {
        8 => {
            let alpha = u8::from_str_radix(&hex[..2], 16).unwrap_or(255);
            (format!("#{}", &hex[2..]), f64::from(alpha) / 255.0)
        }
        6 => (format!("#{hex}"), 1.0),
        _ => ("#000000".into(), 1.0),
    }
}

/// A data URL of the icon with its black strokes and fills in `color`.
pub fn recoloured(name: &str, color: &str) -> String {
    let Some((_, svg)) = ICONS.iter().find(|(n, _)| *n == name) else {
        return String::new();
    };
    let (rgb, opacity) = split_color(color);
    let mut svg = svg.replace("#000000", &rgb);
    if opacity < 1.0 {
        svg = svg.replacen("<svg ", &format!("<svg opacity=\"{opacity:.3}\" "), 1);
    }
    format!(
        "data:image/svg+xml;base64,{}",
        glib::base64_encode(svg.as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_are_recoloured() {
        let url = recoloured("folder", "#806a55db");
        let svg = glib::base64_decode(url.trim_start_matches("data:image/svg+xml;base64,"));
        let svg = String::from_utf8(svg).unwrap();
        assert!(svg.contains("#6a55db") && !svg.contains("#000000"));
        assert!(svg.contains("opacity=\"0.502\""));
        assert_eq!(recoloured("no-such-icon", "#000000"), "");
    }
}
