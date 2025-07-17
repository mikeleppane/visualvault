use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Bar, BarChart, BarGroup, Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row,
        Table, Tabs,
    },
};
use std::collections::HashMap;

use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let tabs = vec!["Overview", "Files", "Types", "Timeline"];
    let selected_tab = app.selected_tab;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Draw tabs
    let tabs_widget = Tabs::new(tabs)
        .select(selected_tab)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs_widget, chunks[0]);

    // Draw content based on selected tab
    match selected_tab {
        0 => draw_overview(f, chunks[1], app),
        1 => draw_files_list(f, chunks[1], app),
        2 => draw_types_chart(f, chunks[1], app),
        3 => draw_timeline(f, chunks[1], app),
        _ => {}
    }
}

fn draw_overview(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),  // Stats cards
            Constraint::Length(12), // Charts
            Constraint::Min(0),     // Recent activity
        ])
        .split(area);

    // Statistics cards
    draw_stats_cards(f, chunks[0], app);

    // Charts section
    let chart_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    draw_storage_gauge(f, chart_chunks[0], app);
    draw_file_type_distribution(f, chart_chunks[1], app);

    // Recent activity
    draw_recent_activity(f, chunks[2], app);
}

fn draw_stats_cards(f: &mut Frame, area: Rect, app: &App) {
    let stats = &app.statistics;
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    // Use the cached statistics from the app
    let cards = vec![
        ("Total Files", stats.total_files.to_string(), Color::Cyan),
        ("Total Size", format_bytes(stats.total_size), Color::Green),
        (
            "Duplicates",
            stats.duplicate_count.to_string(),
            Color::Yellow,
        ),
        (
            "Media Types",
            stats.media_types.len().to_string(),
            Color::Magenta,
        ),
    ];

    for (i, (title, value, color)) in cards.iter().enumerate() {
        let card = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(*color))
            .title(format!(" {} ", title))
            .title_style(Style::default().fg(*color).add_modifier(Modifier::BOLD));

        let content = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                value,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
        ])
        .block(card)
        .alignment(ratatui::layout::Alignment::Center);

        f.render_widget(content, chunks[i]);
    }
}

fn draw_storage_gauge(f: &mut Frame, area: Rect, app: &App) {
    let stats = &app.statistics;
    let used_percent = if stats.total_size > 0 {
        ((stats.organized_size as f64 / stats.total_size as f64) * 100.0) as u16
    } else {
        0
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(" Storage Overview ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .gauge_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::ITALIC),
        )
        .percent(used_percent)
        .label(format!(
            "{} / {} ({}%)",
            format_bytes(stats.organized_size),
            format_bytes(stats.total_size),
            used_percent
        ));

    f.render_widget(gauge, area);
}

fn draw_file_type_distribution(f: &mut Frame, area: Rect, app: &App) {
    let stats = &app.statistics;
    let mut data: Vec<(&str, u64)> = stats
        .media_types
        .iter()
        .map(|(k, v)| (k.as_str(), *v as u64))
        .collect();
    data.sort_by(|a, b| b.1.cmp(&a.1));
    data.truncate(5);

    let bars: Vec<Bar> = data
        .iter()
        .map(|(label, value)| {
            Bar::default()
                .value(*value)
                .label(Line::from(label.to_string()))
                .style(Style::default().fg(get_type_color(label)))
        })
        .collect();

    let bar_group = BarGroup::default().bars(&bars);
    let bar_chart = BarChart::default()
        .block(
            Block::default()
                .title(" File Types ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .data(bar_group)
        .bar_width(3)
        .bar_gap(1)
        .value_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(bar_chart, area);
}

fn draw_recent_activity(f: &mut Frame, area: Rect, _app: &App) {
    let activities = vec![
        ListItem::new(Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::raw("Scanned 1,234 files"),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("↗ ", Style::default().fg(Color::Blue)),
            Span::raw("Organized 456 images to /Pictures/2024"),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("⚠ ", Style::default().fg(Color::Yellow)),
            Span::raw("Found 23 duplicate files"),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::raw("Moved 78 videos to /Videos/2024"),
        ])),
    ];

    let list = List::new(activities).block(
        Block::default()
            .title(" Recent Activity ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    f.render_widget(list, area);
}

fn draw_files_list(f: &mut Frame, area: Rect, app: &App) {
    let files = &app.cached_files;
    let rows: Vec<Row> = files
        .iter()
        .skip(app.scroll_offset)
        .take(20)
        .enumerate()
        .map(|(idx, file)| {
            let is_selected = app.selected_file_index == app.scroll_offset + idx;
            let style = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(file.name.clone()),
                Cell::from(file.file_type.to_string())
                    .style(Style::default().fg(get_type_color(&file.file_type.to_string()))),
                Cell::from(format_bytes(file.size)),
                Cell::from(file.modified.format("%Y-%m-%d %H:%M").to_string()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(30),
        ],
    )
    .header(
        Row::new(vec!["Name", "Type", "Size", "Modified"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .title(" Files ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    f.render_widget(table, area);
}

fn draw_types_chart(f: &mut Frame, area: Rect, app: &App) {
    let stats = &app.statistics;
    let mut type_data: Vec<(String, usize, u64)> = stats
        .media_types
        .iter()
        .map(|(k, v)| (k.clone(), *v, stats.type_sizes.get(k).copied().unwrap_or(0)))
        .collect();
    type_data.sort_by(|a, b| b.1.cmp(&a.1));

    let rows: Vec<Row> = type_data
        .iter()
        .map(|(file_type, count, size)| {
            Row::new(vec![
                Cell::from(file_type.clone()).style(Style::default().fg(get_type_color(file_type))),
                Cell::from(format!("{}", count)),
                Cell::from(format_bytes(*size)),
                Cell::from(format!(
                    "{:.1}%",
                    (*size as f64 / stats.total_size as f64) * 100.0
                )),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ],
    )
    .header(
        Row::new(vec!["Type", "Count", "Size", "Percentage"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .title(" File Type Statistics ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    f.render_widget(table, area);
}

fn draw_timeline(f: &mut Frame, area: Rect, app: &App) {
    let stats = &app.statistics;
    let mut timeline_data: Vec<(String, usize)> = stats
        .files_by_date
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    timeline_data.sort_by(|a, b| b.0.cmp(&a.0));

    let data: Vec<(&str, u64)> = timeline_data
        .iter()
        .take(12)
        .map(|(date, count)| (date.as_str(), *count as u64))
        .collect();

    let bars: Vec<Bar> = data
        .iter()
        .map(|(label, value)| {
            Bar::default()
                .value(*value)
                .label(Line::from(label.to_string()))
                .style(Style::default().fg(Color::Cyan))
        })
        .collect();

    let bar_group = BarGroup::default().bars(&bars);
    let bar_chart = BarChart::default()
        .block(
            Block::default()
                .title(" Files by Month ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .data(bar_group)
        .bar_width(5)
        .bar_gap(1)
        .value_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(bar_chart, area);
}

fn get_type_color(file_type: &str) -> Color {
    match file_type.to_lowercase().as_str() {
        "image" | "jpg" | "jpeg" | "png" | "gif" => Color::Green,
        "video" | "mp4" | "avi" | "mkv" | "mov" => Color::Blue,
        "audio" | "mp3" | "wav" | "flac" => Color::Magenta,
        "document" | "pdf" | "doc" | "txt" => Color::Yellow,
        _ => Color::Gray,
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}
