use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinTheme {
    pub name: String,
    pub primary_color: String,
    pub secondary_color: String,
    pub spinner_faces: Vec<String>,
    pub banner_title: String,
}

impl Default for SkinTheme {
    fn default() -> Self {
        Self {
            name: "Zymatica Gold".to_string(),
            primary_color: "\x1b[38;2;255;215;0m".to_string(), // Gold ANSI
            secondary_color: "\x1b[38;2;138;43;226m".to_string(), // BlueViolet ANSI
            spinner_faces: vec![
                " ( ˶ˆ ᗜ ˆ˵ ) ".to_string(),
                " ( ≡^∇^≡ ) ".to_string(),
                " ( づ ◕‿◕ )づ ".to_string(),
                " ( •̀ ω •́ )✧ ".to_string(),
            ],
            banner_title: "Zymatica Agent ☤ (Native Rust & C++ Engine)".to_string(),
        }
    }
}

pub struct SkinEngine {
    theme: SkinTheme,
    spinner_idx: usize,
}

impl SkinEngine {
    pub fn new(theme: SkinTheme) -> Self {
        Self {
            theme,
            spinner_idx: 0,
        }
    }

    pub fn render_banner(&self) -> String {
        let reset = "\x1b[0m";
        format!(
            "{}{}\n============================================================\n          {}\n============================================================{}",
            self.theme.primary_color, self.theme.secondary_color, self.theme.banner_title, reset
        )
    }

    pub fn tick_spinner(&mut self) -> String {
        let face = &self.theme.spinner_faces[self.spinner_idx % self.theme.spinner_faces.len()];
        self.spinner_idx += 1;
        format!(
            "{}{}{}\x1b[0m",
            self.theme.primary_color,
            face,
            reset_ansi()
        )
    }

    pub fn render_response_box(&self, title: &str, content: &str) -> String {
        format!(
            "{}[{}]\x1b[0m\n{}\n",
            self.theme.primary_color, title, content
        )
    }
}

fn reset_ansi() -> &'static str {
    "\x1b[0m"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skin_engine_rendering() {
        let mut skin = SkinEngine::new(SkinTheme::default());
        let banner = skin.render_banner();
        assert!(banner.contains("Zymatica Agent"));

        let spin1 = skin.tick_spinner();
        let spin2 = skin.tick_spinner();
        assert_ne!(spin1, spin2);
    }
}
