use std::env;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compositor {
    Wlroots,
    Kde,
    Gnome,
}

impl Compositor {
    pub fn label(self) -> &'static str {
        match self {
            Compositor::Wlroots => "wlr 协议合成器",
            Compositor::Kde => "KDE",
            Compositor::Gnome => "GNOME",
        }
    }
}

pub fn detect() -> Compositor {
    from_session_desktop(env::var("XDG_CURRENT_DESKTOP").ok().as_deref())
}

fn from_session_desktop(desktop: Option<&str>) -> Compositor {
    match desktop {
        Some(value) => {
            if value
                .split(':')
                .any(|name| name.eq_ignore_ascii_case("kde"))
            {
                Compositor::Kde
            } else if value
                .split(':')
                .any(|name| name.eq_ignore_ascii_case("gnome"))
            {
                Compositor::Gnome
            } else {
                Compositor::Wlroots
            }
        }
        None => Compositor::Wlroots,
    }
}

#[cfg(test)]
mod tests {
    use super::{Compositor, from_session_desktop};

    #[test]
    fn detects_kde_session() {
        assert_eq!(from_session_desktop(Some("KDE")), Compositor::Kde);
        assert_eq!(from_session_desktop(Some("kde")), Compositor::Kde);
        assert_eq!(from_session_desktop(Some("KDE:pidgin")), Compositor::Kde);
    }

    #[test]
    fn detects_gnome_session() {
        assert_eq!(from_session_desktop(Some("GNOME")), Compositor::Gnome);
        assert_eq!(from_session_desktop(Some("gnome")), Compositor::Gnome);
        assert_eq!(
            from_session_desktop(Some("ubuntu:GNOME")),
            Compositor::Gnome
        );
    }

    #[test]
    fn defaults_to_wlr_protocols() {
        assert_eq!(from_session_desktop(Some("niri")), Compositor::Wlroots);
        assert_eq!(from_session_desktop(Some("sway")), Compositor::Wlroots);
        assert_eq!(from_session_desktop(Some("Hyprland")), Compositor::Wlroots);
        assert_eq!(from_session_desktop(Some("")), Compositor::Wlroots);
        assert_eq!(from_session_desktop(None), Compositor::Wlroots);
    }

    #[test]
    fn kde_takes_precedence_over_gnome() {
        assert_eq!(from_session_desktop(Some("KDE:GNOME")), Compositor::Kde);
    }
}
