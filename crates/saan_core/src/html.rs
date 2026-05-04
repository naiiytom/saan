fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn wrap_svg_in_html(svg: &str, title: &str) -> String {
    let title = escape_html(title);
    format!(
        "<!DOCTYPE html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"UTF-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n\
         <title>{title}</title>\n\
         <style>\n\
           body {{ margin: 0; overflow: hidden; background: #1a1a2e; }}\n\
           svg {{ display: block; width: 100vw; height: 100vh; }}\n\
         </style>\n\
         </head>\n\
         <body>\n\
         {svg}\n\
         <script>\n\
         (function() {{\n\
           var svg = document.querySelector('svg');\n\
           var vp = document.getElementById('viewport');\n\
           if (!svg || !vp) return;\n\
           var tx = 0, ty = 0, scale = 1;\n\
           var dragging = false, startX, startY;\n\
           function update() {{\n\
             vp.setAttribute('transform',\n\
               'translate(' + tx + ',' + ty + ') scale(' + scale + ')');\n\
           }}\n\
           svg.addEventListener('mousedown', function(e) {{\n\
             dragging = true;\n\
             startX = e.clientX - tx;\n\
             startY = e.clientY - ty;\n\
             e.preventDefault();\n\
           }});\n\
           window.addEventListener('mousemove', function(e) {{\n\
             if (!dragging) return;\n\
             tx = e.clientX - startX;\n\
             ty = e.clientY - startY;\n\
             update();\n\
           }});\n\
           window.addEventListener('mouseup', function() {{ dragging = false; }});\n\
           svg.addEventListener('wheel', function(e) {{\n\
             e.preventDefault();\n\
             var factor = e.deltaY < 0 ? 1.1 : 0.9;\n\
             var rect = svg.getBoundingClientRect();\n\
             var mx = e.clientX - rect.left;\n\
             var my = e.clientY - rect.top;\n\
             tx = mx - factor * (mx - tx);\n\
             ty = my - factor * (my - ty);\n\
             scale *= factor;\n\
             update();\n\
           }}, {{ passive: false }});\n\
         }})();\n\
         </script>\n\
         </body>\n\
         </html>",
        title = title,
        svg = svg,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_contains_doctype() {
        let html = wrap_svg_in_html("<svg/>", "Test");
        assert!(html.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn wrap_contains_svg_and_title() {
        let html = wrap_svg_in_html("<svg id=\"test\"/>", "My Graph");
        assert!(html.contains("<svg id=\"test\"/>"));
        assert!(html.contains("My Graph"));
    }

    #[test]
    fn wrap_contains_pan_zoom_script() {
        let html = wrap_svg_in_html("<svg/>", "T");
        assert!(html.contains("<script>"));
        assert!(html.contains("viewport"));
    }
}
