//! External visual-styling layer for sidebar thread/terminal rows.
//!
//! Pure data + parsing crate. Sits low in the dependency graph so its
//! consumers (`ui`, `acp_thread`, `terminal`, `sidebar`) can wire it up without
//! creating cycles.

use agent_client_protocol::schema as acp;
use gpui::{Hsla, Rgba, SharedString};
use std::sync::OnceLock;

/// Namespaced ACP `_meta` key. Producers attach a JSON object under this key
/// describing how the row should look.
pub const ZED_THREAD_STYLE_META_KEY: &str = "zed.dev/thread_style";

const BADGE_MAX_LEN: usize = 16;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThreadVisualStyle {
    pub accent_color: Option<Hsla>,
    pub icon_color: Option<Hsla>,
    pub background_tint: Option<Hsla>,
    pub badge: Option<SharedString>,
    pub state: Option<SharedString>,
}

impl ThreadVisualStyle {
    /// Returns `Some(_)` only for the four canonical states. Unknown state
    /// strings render as decoration but do not override a thread's intrinsic
    /// status icon.
    pub fn known_state(&self) -> Option<KnownState> {
        match self.state.as_deref()? {
            "running" => Some(KnownState::Running),
            "waiting-for-confirmation" => Some(KnownState::WaitingForConfirmation),
            "completed" => Some(KnownState::Completed),
            "error" => Some(KnownState::Error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownState {
    Running,
    WaitingForConfirmation,
    Completed,
    Error,
}

/// Parse the `zed.dev/thread_style` entry from an ACP `_meta` map.
///
/// Returns `None` if the key is absent or its value is not a JSON object.
/// Invalid fields inside the object are silently dropped (per the
/// "ignore-and-fall-back" contract) rather than failing the whole parse.
pub fn parse_meta(meta: &acp::Meta) -> Option<ThreadVisualStyle> {
    let value = meta.get(ZED_THREAD_STYLE_META_KEY)?;
    let object = value.as_object()?;

    let mut style = ThreadVisualStyle::default();
    if let Some(s) = object.get("accent_color").and_then(|v| v.as_str()) {
        style.accent_color = parse_hex(s);
    }
    if let Some(s) = object.get("icon_color").and_then(|v| v.as_str()) {
        style.icon_color = parse_hex(s);
    }
    if let Some(s) = object.get("background_tint").and_then(|v| v.as_str()) {
        style.background_tint = parse_hex(s);
    }
    if let Some(s) = object.get("badge").and_then(|v| v.as_str()) {
        style.badge = clamp_badge(s);
    }
    if let Some(s) = object.get("state").and_then(|v| v.as_str()) {
        style.state = clean_state(s);
    }
    Some(style)
}

/// Convenience wrapper for the common `Option<acp::Meta>` shape exposed by
/// most ACP carrier types.
pub fn parse_meta_opt(meta: &Option<acp::Meta>) -> Option<ThreadVisualStyle> {
    meta.as_ref().and_then(parse_meta)
}

/// Extract a `[zed-style …]` marker from a terminal title, returning the
/// parsed style (if any) and the cleaned title with the marker removed.
///
/// The marker is matched anywhere in the string. Surrounding whitespace is
/// collapsed so the visible title never carries a leading/trailing gap.
pub fn parse_title_marker(title: &str) -> (Option<ThreadVisualStyle>, String) {
    let re = marker_regex();
    let Some(captures) = re.captures(title) else {
        return (None, title.to_owned());
    };
    let full_match = captures.get(0).expect("regex always has a 0th group");
    let body = captures.get(1).map_or("", |m| m.as_str());

    let mut style = ThreadVisualStyle::default();
    for token in body.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        match key {
            "accent" => style.accent_color = parse_hex(value),
            "icon" => style.icon_color = parse_hex(value),
            "background" => style.background_tint = parse_hex(value),
            "badge" => style.badge = clamp_badge(value),
            "state" => style.state = clean_state(value),
            _ => {}
        }
    }

    let mut cleaned = String::with_capacity(title.len().saturating_sub(full_match.len()));
    cleaned.push_str(&title[..full_match.start()]);
    cleaned.push_str(&title[full_match.end()..]);
    let cleaned = cleaned.trim().to_owned();
    (Some(style), cleaned)
}

fn marker_regex() -> &'static regex::Regex {
    static MARKER_RE: OnceLock<regex::Regex> = OnceLock::new();
    MARKER_RE.get_or_init(|| {
        // Body is optional so a bare `[zed-style]` acts as an explicit reset.
        regex::Regex::new(r"\[zed-style(?:\s+([^\]]*))?\]")
            .expect("zed-style marker regex is valid")
    })
}

fn parse_hex(input: &str) -> Option<Hsla> {
    let s = input.strip_prefix('#')?;
    let rgba = match s.len() {
        3 => {
            let bytes = s.as_bytes();
            let r = decode_nibble(bytes[0])?;
            let g = decode_nibble(bytes[1])?;
            let b = decode_nibble(bytes[2])?;
            Rgba {
                r: (r * 17) as f32 / 255.0,
                g: (g * 17) as f32 / 255.0,
                b: (b * 17) as f32 / 255.0,
                a: 1.0,
            }
        }
        6 => {
            let hex = u32::from_str_radix(s, 16).ok()?;
            gpui::rgb(hex)
        }
        8 => {
            let hex = u32::from_str_radix(s, 16).ok()?;
            gpui::rgba(hex)
        }
        _ => return None,
    };
    Some(rgba.into())
}

fn decode_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn clamp_badge(input: &str) -> Option<SharedString> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.len() <= BADGE_MAX_LEN {
        return Some(SharedString::from(trimmed.to_owned()));
    }
    let mut boundary = BADGE_MAX_LEN;
    while !trimmed.is_char_boundary(boundary) {
        boundary -= 1;
    }
    Some(SharedString::from(trimmed[..boundary].to_owned()))
}

fn clean_state(input: &str) -> Option<SharedString> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(SharedString::from(trimmed.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn hsla_approx(actual: Hsla, expected: Hsla) -> bool {
        approx_eq(actual.h, expected.h)
            && approx_eq(actual.s, expected.s)
            && approx_eq(actual.l, expected.l)
            && approx_eq(actual.a, expected.a)
    }

    #[test]
    fn parse_hex_accepts_three_six_and_eight_digit_forms() {
        let rrggbb = parse_hex("#3b82f6").expect("six-digit hex parses");
        let rgb = parse_hex("#3bf").expect("three-digit hex parses");
        let rrggbbaa = parse_hex("#3b82f680").expect("eight-digit hex parses");

        // Three-digit form expands by nibble repetition.
        let expanded: Hsla = gpui::rgb(0x33bbff).into();
        assert!(hsla_approx(rgb, expanded));

        // Eight-digit form preserves alpha.
        assert!(approx_eq(rrggbbaa.a, 0x80 as f32 / 255.0));

        // Sanity: a known purple expansion.
        assert!(rrggbb.l > 0.0);
    }

    #[test]
    fn parse_hex_rejects_invalid_input() {
        assert!(parse_hex("not-a-color").is_none());
        assert!(parse_hex("#zzz").is_none());
        assert!(parse_hex("#12345").is_none(), "5-digit form is not accepted");
        assert!(parse_hex("3b82f6").is_none(), "missing # prefix");
        assert!(parse_hex("").is_none());
    }

    #[test]
    fn clamp_badge_trims_and_truncates() {
        assert_eq!(clamp_badge("  working  ").as_deref(), Some("working"));
        assert_eq!(clamp_badge("").as_deref(), None);
        assert_eq!(clamp_badge("   ").as_deref(), None);

        let long = "continuous-integration";
        let clamped = clamp_badge(long).unwrap();
        assert!(clamped.len() <= BADGE_MAX_LEN);
        assert!(long.starts_with(clamped.as_ref()));

        // Multibyte safety: the truncation must land on a UTF-8 boundary.
        let multibyte = "🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀";
        let clamped = clamp_badge(multibyte).unwrap();
        assert!(clamped.len() <= BADGE_MAX_LEN);
        assert!(clamped.is_char_boundary(clamped.len()));
    }

    #[test]
    fn parse_title_marker_strips_marker_and_keeps_remainder() {
        let (style, cleaned) = parse_title_marker(
            "[zed-style accent=#3b82f6 badge=working state=running] my task",
        );
        let style = style.expect("marker present");
        assert_eq!(cleaned, "my task");
        assert!(style.accent_color.is_some());
        assert_eq!(style.badge.as_deref(), Some("working"));
        assert_eq!(style.state.as_deref(), Some("running"));
    }

    #[test]
    fn parse_title_marker_handles_all_supported_keys() {
        let (style, cleaned) = parse_title_marker(
            "[zed-style accent=#3b82f6 icon=#ff0000 background=#0000ff80 badge=hi state=deploying unknown=ignored] title",
        );
        let style = style.expect("marker present");
        assert_eq!(cleaned, "title");
        assert!(style.accent_color.is_some());
        assert!(style.icon_color.is_some());
        assert!(style.background_tint.is_some());
        assert_eq!(style.badge.as_deref(), Some("hi"));
        assert_eq!(style.state.as_deref(), Some("deploying"));
    }

    #[test]
    fn parse_title_marker_empty_marker_yields_default_style() {
        // `[zed-style]` is the explicit reset form.
        let (style, cleaned) = parse_title_marker("[zed-style] back to plain");
        let style = style.expect("empty marker still counts as a marker");
        assert_eq!(style, ThreadVisualStyle::default());
        assert_eq!(cleaned, "back to plain");

        // Whitespace-only body is equivalent.
        let (style, cleaned) = parse_title_marker("[zed-style ] back to plain");
        let style = style.expect("whitespace-only marker still counts as a marker");
        assert_eq!(style, ThreadVisualStyle::default());
        assert_eq!(cleaned, "back to plain");
    }

    #[test]
    fn parse_title_marker_requires_word_boundary_after_zed_style() {
        // `[zed-styleX]` must NOT be parsed as our marker.
        let (style, cleaned) = parse_title_marker("[zed-styleX] not a marker");
        assert!(style.is_none());
        assert_eq!(cleaned, "[zed-styleX] not a marker");
    }

    #[test]
    fn parse_title_marker_no_marker_returns_original_title() {
        let (style, cleaned) = parse_title_marker("nothing fancy here");
        assert!(style.is_none());
        assert_eq!(cleaned, "nothing fancy here");
    }

    #[test]
    fn parse_title_marker_collapses_surrounding_whitespace() {
        let (_, cleaned) = parse_title_marker("   [zed-style state=running]    actual title   ");
        assert_eq!(cleaned, "actual title");
    }

    #[test]
    fn parse_meta_absent_key_returns_none() {
        let meta = acp::Meta::from_iter([("something_else".into(), json!("value"))]);
        assert!(parse_meta(&meta).is_none());
    }

    #[test]
    fn parse_meta_present_but_non_object_returns_none() {
        let meta = acp::Meta::from_iter([(ZED_THREAD_STYLE_META_KEY.into(), json!("not-an-object"))]);
        assert!(parse_meta(&meta).is_none());
    }

    #[test]
    fn parse_meta_present_but_empty_returns_default() {
        let meta = acp::Meta::from_iter([(ZED_THREAD_STYLE_META_KEY.into(), json!({}))]);
        let style = parse_meta(&meta).expect("empty object parses");
        assert_eq!(style, ThreadVisualStyle::default());
    }

    #[test]
    fn parse_meta_drops_invalid_hex_silently() {
        let meta = acp::Meta::from_iter([(
            ZED_THREAD_STYLE_META_KEY.into(),
            json!({
                "accent_color": "not-a-color",
                "badge": "ok",
            }),
        )]);
        let style = parse_meta(&meta).expect("object parses even with invalid hex");
        assert!(style.accent_color.is_none(), "invalid hex dropped");
        assert_eq!(style.badge.as_deref(), Some("ok"));
    }

    #[test]
    fn parse_meta_reads_all_fields() {
        let meta = acp::Meta::from_iter([(
            ZED_THREAD_STYLE_META_KEY.into(),
            json!({
                "accent_color": "#3b82f6",
                "icon_color": "#ff0000",
                "background_tint": "#0000ff80",
                "badge": "working",
                "state": "running",
            }),
        )]);
        let style = parse_meta(&meta).expect("full object parses");
        assert!(style.accent_color.is_some());
        assert!(style.icon_color.is_some());
        assert!(style.background_tint.is_some());
        assert_eq!(style.badge.as_deref(), Some("working"));
        assert_eq!(style.state.as_deref(), Some("running"));
        assert_eq!(style.known_state(), Some(KnownState::Running));
    }

    #[test]
    fn known_state_matches_only_canonical_strings() {
        let mut style = ThreadVisualStyle::default();

        for (input, expected) in [
            ("running", Some(KnownState::Running)),
            ("waiting-for-confirmation", Some(KnownState::WaitingForConfirmation)),
            ("completed", Some(KnownState::Completed)),
            ("error", Some(KnownState::Error)),
            ("deploying", None),
            ("Running", None),
            ("", None),
        ] {
            style.state = if input.is_empty() {
                None
            } else {
                Some(SharedString::from(input.to_owned()))
            };
            assert_eq!(style.known_state(), expected, "input = {input:?}");
        }
    }

    #[test]
    fn clean_state_trims_and_lowercases() {
        assert_eq!(clean_state("  Running  ").as_deref(), Some("running"));
        assert_eq!(
            clean_state("Waiting-For-Confirmation").as_deref(),
            Some("waiting-for-confirmation")
        );
        assert_eq!(clean_state("   ").as_deref(), None);
    }
}
