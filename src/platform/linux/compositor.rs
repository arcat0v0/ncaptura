use std::env;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compositor {
    Wlr,
    Kde,
}

pub fn detect() -> Compositor {
    from_session_desktop(env::var("XDG_CURRENT_DESKTOP").ok().as_deref())
}

fn from_session_desktop(desktop: Option<&str>) -> Compositor {
    match desktop {
        Some(value)
            if value
                .split(':')
                .any(|name| name.eq_ignore_ascii_case("kde")) =>
        {
            Compositor::Kde
        }
        _ => Compositor::Wlr,
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
    fn defaults_to_wlr_protocols() {
        assert_eq!(from_session_desktop(Some("niri")), Compositor::Wlr);
        assert_eq!(from_session_desktop(Some("GNOME")), Compositor::Wlr);
        assert_eq!(from_session_desktop(Some("wlroots")), Compositor::Wlr);
        assert_eq!(from_session_desktop(Some("")), Compositor::Wlr);
        assert_eq!(from_session_desktop(None), Compositor::Wlr);
    }
}
