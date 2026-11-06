//! Renders memo content: tags, memo references and links are clickable; inline
//! markup becomes bold, highlight, underline and strikethrough runs.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, FontWeight, HighlightStyle, InteractiveText, IntoElement, ParentElement as _,
    SharedString, StrikethroughStyle, Styled as _, StyledText, UnderlineStyle, div, px,
};

use crate::core::parse::{self, InlineStyle, LineKind, RichLine, Target};

use super::theme::Typography;
use super::{Command, dispatch};

fn highlight_for(style: &InlineStyle, cx: &App) -> HighlightStyle {
    let theme = cx.theme();
    let mut h = HighlightStyle::default();
    if style.bold {
        h.font_weight = Some(FontWeight::BOLD);
    }
    if style.highlight {
        h.background_color = Some(theme.yellow.opacity(0.35));
    }
    if style.underline {
        h.underline = Some(UnderlineStyle {
            thickness: px(1.),
            color: None,
            wavy: false,
        });
    }
    if style.strike {
        h.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.),
            color: None,
        });
    }
    if style.tag {
        h.color = Some(theme.blue);
        h.background_color = Some(theme.blue.opacity(0.1));
    }
    if style.link {
        h.color = Some(theme.blue);
        h.underline = Some(UnderlineStyle {
            thickness: px(1.),
            color: Some(theme.blue.opacity(0.4)),
            wavy: false,
        });
    }
    if style.memo_ref {
        h.color = Some(theme.blue);
        h.background_color = Some(theme.secondary);
    }
    if style.search_hit {
        h.background_color = Some(theme.yellow.opacity(0.6));
    }
    h
}

fn line_text(scope: &SharedString, index: usize, line: &RichLine, cx: &App) -> AnyElement {
    let highlights: Vec<_> = line
        .styles
        .iter()
        .map(|(range, style)| (range.clone(), highlight_for(style, cx)))
        .collect();
    let styled = StyledText::new(line.text.clone()).with_highlights(highlights);
    if line.targets.is_empty() {
        return styled.into_any_element();
    }
    let ranges: Vec<_> = line.targets.iter().map(|(r, _)| r.clone()).collect();
    let targets: Vec<Target> = line.targets.iter().map(|(_, t)| t.clone()).collect();
    InteractiveText::new((scope.clone(), index), styled)
        .on_click(ranges, move |ix, window, cx| {
            let command = match &targets[ix] {
                Target::Tag(tag) => Command::OpenTag(tag.clone()),
                Target::Memo(id) => Command::OpenMemo(id.clone()),
                Target::Url(url) => Command::OpenUrl(url.clone()),
            };
            dispatch(command, window, cx);
        })
        .into_any_element()
}

/// Renders memo content. With `max_lines`, returns whether content was cut.
pub fn render_content(
    scope: impl Into<SharedString>,
    content: &str,
    keywords: &[String],
    max_lines: Option<usize>,
    ref_label: &dyn Fn(&str) -> String,
    typography: Typography,
    cx: &App,
) -> (AnyElement, bool) {
    let scope: SharedString = scope.into();
    let lines = parse::parse_rich(content, keywords, ref_label);
    let total_chars: usize = lines.iter().map(|l| l.text.chars().count()).sum();
    let (limit, truncated) = match max_lines {
        Some(max) if lines.len() > max || total_chars > max * 60 => {
            let mut count = 0;
            let mut chars = 0;
            for line in &lines {
                chars += line.text.chars().count().max(20);
                count += 1;
                if count >= max || chars > max * 45 {
                    break;
                }
            }
            (count.max(1), true)
        }
        _ => (lines.len(), false),
    };
    let muted = cx.theme().muted_foreground;
    let border = cx.theme().border;
    let element = typography
        .apply(v_flex().gap_1())
        .children(lines.iter().take(limit).enumerate().map(|(i, line)| {
            match &line.kind {
                LineKind::Blank => div().h_2().into_any_element(),
                LineKind::Paragraph => div().child(line_text(&scope, i, line, cx)).into_any_element(),
                LineKind::Bullet => h_flex()
                    .items_start()
                    .gap_2()
                    .child(div().text_color(muted).child("•"))
                    .child(div().flex_1().min_w_0().child(line_text(&scope, i, line, cx)))
                    .into_any_element(),
                LineKind::Ordered(n) => h_flex()
                    .items_start()
                    .gap_1p5()
                    .child(div().text_color(muted).child(format!("{n}.")))
                    .child(div().flex_1().min_w_0().child(line_text(&scope, i, line, cx)))
                    .into_any_element(),
                LineKind::Quote => div()
                    .border_l_2()
                    .border_color(border)
                    .pl_3()
                    .text_color(muted)
                    .child(line_text(&scope, i, line, cx))
                    .into_any_element(),
            }
        }))
        .into_any_element();
    (element, truncated)
}
