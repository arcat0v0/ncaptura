use super::compositor::Compositor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    RecordRegion,
    RecordFullscreen,
    RecordWindow,
    ListWindows,
}

impl Capability {
    pub fn label(self) -> &'static str {
        match self {
            Capability::RecordRegion => "区域录屏",
            Capability::RecordFullscreen => "全屏录屏",
            Capability::RecordWindow => "窗口录屏",
            Capability::ListWindows => "窗口列表",
        }
    }
}

pub fn supported(kind: Compositor, capability: Capability) -> bool {
    use Capability as C;
    use Compositor as S;

    match capability {
        C::RecordFullscreen | C::RecordWindow => true,
        C::RecordRegion | C::ListWindows => matches!(kind, S::Wlroots),
    }
}

#[cfg(test)]
mod tests {
    use super::{Capability, supported};
    use crate::platform::compositor::Compositor;

    const ALL: [Capability; 4] = [
        Capability::RecordRegion,
        Capability::RecordFullscreen,
        Capability::RecordWindow,
        Capability::ListWindows,
    ];

    #[test]
    fn wlroots_supports_everything() {
        for capability in ALL {
            assert!(supported(Compositor::Wlroots, capability));
        }
    }

    #[test]
    fn kde_and_gnome_support_fullscreen_and_window_recording() {
        for kind in [Compositor::Kde, Compositor::Gnome] {
            for capability in ALL {
                let expected = matches!(
                    capability,
                    Capability::RecordFullscreen | Capability::RecordWindow
                );
                assert_eq!(
                    supported(kind, capability),
                    expected,
                    "{kind:?} {capability:?}"
                );
            }
        }
    }
}
