//! Bridge between the data-only `thread_visual_style` crate and the GPUI
//! render helpers used by `ThreadItem`. Lives in `ui` so the data crate
//! never has to depend on `ui` (no cycle).

use super::thread_item::AgentThreadStatus;
use crate::prelude::*;
use gpui::{Hsla, px};
use thread_visual_style::{KnownState, ThreadVisualStyle};

impl From<KnownState> for AgentThreadStatus {
    fn from(state: KnownState) -> Self {
        match state {
            KnownState::Running => AgentThreadStatus::Running,
            KnownState::WaitingForConfirmation => AgentThreadStatus::WaitingForConfirmation,
            KnownState::Completed => AgentThreadStatus::Completed,
            KnownState::Error => AgentThreadStatus::Error,
        }
    }
}

/// Pick a default accent color for the given canonical state, using
/// theme tokens that already exist.
pub fn default_accent_for(state: KnownState, cx: &App) -> Hsla {
    match state {
        KnownState::Running => cx.theme().colors().text_accent,
        KnownState::WaitingForConfirmation => cx.theme().status().warning,
        KnownState::Completed => cx.theme().status().success,
        KnownState::Error => cx.theme().status().error,
    }
}

/// Resolve the icon color for the agent icon slot. Mirrors the existing
/// `self.icon_color.unwrap_or(Color::Muted)` fallback when `style` is `None`,
/// and otherwise applies the documented preference chain.
///
/// `status` is accepted for forward compatibility — currently the icon
/// color computed here is only used for the non-status agent icon (Running /
/// Error / WaitingForConfirmation paint their own status icons with their own
/// hardcoded colors), so the result is identical to the upstream default for
/// those states.
pub fn resolve_icon_color(
    explicit: Option<Color>,
    style: Option<&ThreadVisualStyle>,
    _status: AgentThreadStatus,
    cx: &App,
) -> Color {
    if let Some(explicit) = explicit {
        return explicit;
    }
    if let Some(style) = style {
        if let Some(icon) = style.icon_color {
            return Color::Custom(icon);
        }
        if let Some(state) = style.known_state() {
            return Color::Custom(default_accent_for(state, cx));
        }
        if let Some(accent) = style.accent_color {
            return Color::Custom(accent);
        }
    }
    Color::Muted
}

/// Wrap a rendered row with an optional 2px left accent rail.
///
/// When `style` is `None` (or carries no accent and no canonical state) the
/// row is returned unchanged. The rail sits as an absolute-positioned overlay
/// on the left edge of the row, so the row's own selection / focus / hover
/// indicators stay visible above the rail. Pointer events fall through to the
/// row because the rail does not call `occlude()`.
///
/// Note: `background_tint` is parsed and stored, but not yet rendered. A
/// background tint applied from outside the row would either be hidden by the
/// row's own opaque backgrounds (placed behind) or block interaction (placed
/// in front). The cleanest fix requires `ThreadItem` to blend the tint into
/// its internal `base_bg`, deferred to a follow-up.
pub fn wrap_with_visual_style<E: IntoElement>(
    row: E,
    style: Option<&ThreadVisualStyle>,
    _base_bg: Hsla,
    cx: &App,
) -> AnyElement {
    let Some(style) = style else {
        return row.into_any_element();
    };

    let accent = style
        .accent_color
        .or_else(|| style.known_state().map(|state| default_accent_for(state, cx)));

    let Some(accent) = accent else {
        return row.into_any_element();
    };

    div()
        .relative()
        .w_full()
        .child(row)
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(2.0))
                .bg(accent),
        )
        .into_any_element()
}
