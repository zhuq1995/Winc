//! 最左侧的项目列表：一行一个项目，当前项目高亮；鼠标停在条目上时右端出现关闭按钮。
//!
//! 列表只在开着多个项目时显示（见 `App::projects_width`），每个项目的编辑器、终端等状态互相
//! 独立，点击条目切换当前项目。

use super::*;
use crate::layout::{PROJECT_GAP, PROJECT_PAD, PROJECT_ROW, projects_max_scroll, scroll_to_show};

/// 条目内容与卡片边缘的间距（逻辑像素）。
const PAD: f32 = 8.0;

/// 绘制项目列表，并记录每个条目的可点击区域（点击、悬停在 `app::input` 里按它判断）。
pub(in crate::view) fn draw_projects(app: &mut App, frame: &mut Frame<'_>) {
    let area = app.layout.projects;
    app.projects_hits.clear();
    app.projects_close.clear();
    if area.w <= 0.0 || area.h <= 0.0 {
        return;
    }

    let p = app.palette;
    frame.rect_clipped(area, p.sidebar_bg, RADIUS_CARD, area);
    let clip = area;

    let projects = app.shared.projects();
    app.projects_keys = projects.iter().map(|project| project.key).collect();
    let max = projects_max_scroll(projects.len(), area.h);
    app.projects_scroll = app.projects_scroll.clamp(0.0, max);
    if app.projects_reveal {
        if let Some(index) = projects
            .iter()
            .position(|project| Some(project.key) == app.key)
        {
            let top = PROJECT_PAD + index as f32 * (PROJECT_ROW + PROJECT_GAP);
            app.projects_scroll =
                scroll_to_show(app.projects_scroll, top, top + PROJECT_ROW, area.h).clamp(0.0, max);
        }
        app.projects_reveal = false;
    }
    let active = app.key;
    let mut y = area.y + PROJECT_PAD - app.projects_scroll;
    for project in projects.iter() {
        let row = Rect::new(area.x + PAD / 2.0, y, (area.w - PAD).max(0.0), PROJECT_ROW);
        y += PROJECT_ROW + PROJECT_GAP;
        if row.bottom() <= area.y {
            continue;
        }
        if row.y >= area.bottom() {
            break;
        }

        let current = Some(project.key) == active;
        let hovered = matches!(
            app.hover,
            Hover::Project(key) | Hover::ProjectClose(key) if key == project.key
        );
        if current || hovered {
            let fill = if current { p.selected } else { p.hover };
            frame.rect_clipped(row, fill, RADIUS_ROW, clip);
        }

        // 名称前的方块：项目名的前两个字母（没有名字时是「W」），颜色按项目名分配。
        let name = project
            .name
            .clone()
            .unwrap_or_else(|| t!("workspace-projects-start-page").to_owned());
        let badge = p.project_badge(&name, 0);
        let badge_rect = Rect::new(row.x + PAD, row.y + (PROJECT_ROW - 18.0) / 2.0, 18.0, 18.0);
        frame.rect_clipped(badge_rect, badge.background, 4.0, clip);
        let w = frame.measure(FontKind::Ui, app.fonts.ui_small, &badge.initials);
        label(
            frame,
            app,
            app.fonts.ui_small,
            (badge_rect.x + (badge_rect.w - w) / 2.0, badge_rect),
            &badge.initials,
            badge.text,
            clip,
        );

        // 右端留着关闭按钮的位置；终端里有程序在运行时，关闭按钮以左显示一个圆点。
        let room = if hovered || project.busy { 22.0 } else { 0.0 };
        let text_x = badge_rect.right() + PAD;
        let text_max = (row.right() - PAD - room - text_x).max(0.0);
        let text = truncate_end(frame, &name, app.fonts.ui, text_max);
        let color = if current { p.text } else { p.text_muted };
        ui_text(frame, app, text_x, row, &text, color, clip);

        if project.busy && !hovered {
            let dot = Rect::new(
                row.right() - 12.0,
                row.y + (PROJECT_ROW - 6.0) / 2.0,
                6.0,
                6.0,
            );
            frame.rect_clipped(dot, p.modified_dot, 3.0, clip);
        }
        if hovered {
            let close = Rect::new(
                row.right() - 24.0,
                row.y + (PROJECT_ROW - 18.0) / 2.0,
                18.0,
                18.0,
            );
            let color = if app.hover == Hover::ProjectClose(project.key) {
                p.text
            } else {
                p.text_muted
            };
            icons::close(
                frame,
                (close.x + close.w / 2.0, close.y + close.h / 2.0),
                7.0,
                color,
                clip,
            );
            let close = close.intersect(area);
            if close.w > 0.0 && close.h > 0.0 {
                app.projects_close.push((close, project.key));
            }
        }

        let row = row.intersect(area);
        if row.w > 0.0 && row.h > 0.0 {
            app.projects_hits.push((row, project.key));
        }
    }
}
