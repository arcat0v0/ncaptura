use super::compositor::Compositor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    ScreenshotRegion,
    ScreenshotFullscreen,
    ScreenshotWindow,
    RecordRegion,
    RecordFullscreen,
    RecordWindow,
    ListWindows,
}

impl Capability {
    pub fn label(self) -> &'static str {
        match self {
            Capability::ScreenshotRegion => "区域截图",
            Capability::ScreenshotFullscreen => "全屏截图",
            Capability::ScreenshotWindow => "窗口截图",
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
        C::ScreenshotRegion | C::ScreenshotFullscreen | C::ScreenshotWindow | C::RecordWindow => {
            true
        }
        C::RecordRegion | C::RecordFullscreen | C::ListWindows => matches!(kind, S::Wlroots),
    }
}

#[cfg(test)]
mod tests {
    use super::{Capability, supported};
    use crate::platform::compositor::Compositor;

    const ALL: [Capability; 7] = [
        Capability::ScreenshotRegion,
        Capability::ScreenshotFullscreen,
        Capability::ScreenshotWindow,
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
    fn kde_and_gnome_share_the_same_matrix() {
        for kind in [Compositor::Kde, Compositor::Gnome] {
            for capability in ALL {
                let expected = !matches!(
                    capability,
                    Capability::RecordRegion
                        | Capability::RecordFullscreen
                        | Capability::ListWindows
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
