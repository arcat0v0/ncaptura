#[derive(Clone, Copy)]
pub enum OverlayKind {
    Tool,
    Hud,
}

pub use implementation::configure;

#[cfg(target_os = "linux")]
mod implementation {
    use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

    use super::OverlayKind;

    pub fn configure(window: &adw::ApplicationWindow, kind: OverlayKind, namespace: &'static str) {
        if !gtk4_layer_shell::is_supported() {
            return;
        }
        window.init_layer_shell();
        match kind {
            OverlayKind::Tool => {
                window.set_layer(Layer::Top);
                window.set_keyboard_mode(KeyboardMode::Exclusive);
            }
            OverlayKind::Hud => {
                window.set_layer(Layer::Overlay);
                window.set_anchor(Edge::Top, true);
                window.set_anchor(Edge::Right, true);
                window.set_margin(Edge::Top, 12);
                window.set_margin(Edge::Right, 12);
                window.set_keyboard_mode(KeyboardMode::OnDemand);
            }
        }
        window.set_namespace(Some(namespace));
    }
}

#[cfg(not(target_os = "linux"))]
mod implementation {
    use super::OverlayKind;

    pub fn configure(_: &adw::ApplicationWindow, _: OverlayKind, _: &'static str) {}
}
