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
    // Split area into source and destination sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Draw source folder stats
    draw_folder_stats(f, chunks[0], app, true);

    // Draw destination folder stats
    draw_folder_stats(f, chunks[1], app, false);
}

fn draw_folder_stats(f: &mut Frame, area: Rect, app: &App, is_source: bool) {
    let settings = app.settings.try_read();

    if let Ok(settings) = settings {
        let (folder_path, title, color) = if is_source {
            (
                settings.source_folder.as_ref(),
                " 📁 Source Folder ",
                Color::Cyan,
            )
        } else {
            (
                settings.destination_folder.as_ref(),
                " 📂 Destination Folder ",
                Color::Green,
            )
        };

        let content = if let Some(path) = folder_path {
            // Get folder name
            let folder_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown");

            // Get cached stats if available
            let stats = app.folder_stats_cache.get(path);

            let mut lines = vec![Line::from(vec![Span::styled(
                folder_name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )])];

            // Show the full path instead of just parent
            if let Some(full_path) = path.to_str() {
                lines.push(Line::from(vec![Span::styled(
                    truncate_path(full_path, area.width as usize - 4),
                    Style::default().fg(Color::Gray),
                )]));
            }

            // Add stats if available
            if let Some(stats) = stats {
                lines.push(Line::from(vec![Span::styled(
                    format!(
                        "{} files, {} folders",
                        format_number(stats.total_files),
                        format_number(stats.total_dirs)
                    ),
                    Style::default().fg(Color::Yellow),
                )]));

                lines.push(Line::from(vec![Span::styled(
                    format!(
                        "{} media files ({})",
                        format_number(stats.media_files),
                        format_bytes(stats.total_size)
                    ),
                    Style::default().fg(Color::Magenta),
                )]));
            } else {
                // Stats are being calculated
                lines.push(Line::from(vec![Span::styled(
                    "Calculating...",
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::ITALIC),
                )]));
            }

            lines
        } else {
            vec![
                Line::from(vec![Span::styled(
                    "Not configured",
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::ITALIC),
                )]),
                Line::from(vec![Span::styled(
                    "Press 's' to configure",
                    Style::default().fg(Color::DarkGray),
                )]),
            ]
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color));

        let paragraph = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Left);

        f.render_widget(paragraph, area);
    }
}

// Helper function to truncate long paths
fn truncate_path(path: &str, max_width: usize) -> String {
    if path.len() <= max_width {
        path.to_string()
    } else if max_width > 3 {
        format!("...{}", &path[path.len() - (max_width - 3)..])
    } else {
        "...".to_string()
    }
}

// Helper function to format numbers with commas
fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let mut count = 0;

    for ch in s.chars().rev() {
        if count == 3 {
            result.push(',');
            count = 0;
        }
        result.push(ch);
        count += 1;
    }

    result.chars().rev().collect()
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

fn draw_recent_activity(f: &mut Frame, area: Rect, app: &App) {
    let mut activities = Vec::new();

    // Add scan activity if available
    if let Some(scan_result) = &app.last_scan_result {
        activities.push(ListItem::new(Line::from(vec![
            Span::styled("🔍 ", Style::default().fg(Color::Cyan)),
            Span::raw(format!(
                "Scanned {} files in {}",
                scan_result.files_found,
                format_duration(scan_result.duration)
            )),
            Span::styled(
                format!(" ({})", format_relative_time(scan_result.timestamp)),
                Style::default().fg(Color::Rgb(150, 150, 150)),
            ),
        ])));
    }

    // Add organization activity if available
    if let Some(org_result) = &app.last_organize_result {
        let icon = if org_result.success { "✓" } else { "✗" };
        let color = if org_result.success {
            Color::Green
        } else {
            Color::Red
        };

        activities.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{} ", icon), Style::default().fg(color)),
            Span::raw(format!(
                "Organized {} of {} files to {}",
                org_result.files_organized,
                org_result.files_total,
                org_result.destination.display()
            )),
            Span::styled(
                format!(" ({})", format_relative_time(org_result.timestamp)),
                Style::default().fg(Color::Rgb(150, 150, 150)),
            ),
        ])));
    }

    // Add duplicate detection activity
    if app.statistics.duplicate_count > 0 {
        activities.push(ListItem::new(Line::from(vec![
            Span::styled("⚠ ", Style::default().fg(Color::Yellow)),
            Span::raw(format!(
                "Found {} duplicate files ({})",
                app.statistics.duplicate_count,
                format_bytes(app.statistics.duplicate_count as u64)
            )),
        ])));
    }

    // Add current operation status
    if app.state == crate::app::AppState::Scanning {
        let progress = app.progress.try_read();
        if let Ok(progress) = progress {
            activities.push(ListItem::new(Line::from(vec![
                Span::styled(
                    "⟳ ",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::SLOW_BLINK),
                ),
                Span::raw(format!(
                    "Scanning in progress... {} files found",
                    progress.current
                )),
            ])));
        }
    } else if app.state == crate::app::AppState::Organizing {
        let progress = app.progress.try_read();
        if let Ok(progress) = progress {
            activities.push(ListItem::new(Line::from(vec![
                Span::styled(
                    "⟳ ",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::SLOW_BLINK),
                ),
                Span::raw(format!(
                    "Organizing files... {}/{}",
                    progress.current, progress.total
                )),
            ])));
        }
    }

    // Add error messages if any
    if let Some(error) = &app.error_message {
        activities.insert(
            0,
            ListItem::new(Line::from(vec![
                Span::styled("✗ ", Style::default().fg(Color::Red)),
                Span::styled(error, Style::default().fg(Color::Red)),
                Span::styled(" (recent)", Style::default().fg(Color::Rgb(150, 150, 150))),
            ])),
        );
    }

    // Add success messages if any
    if let Some(success) = &app.success_message {
        activities.insert(
            0,
            ListItem::new(Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::styled(success, Style::default().fg(Color::Green)),
            ])),
        );
    }

    // If no activities, show placeholder
    if activities.is_empty() {
        activities.push(ListItem::new(Line::from(vec![Span::styled(
            "No recent activity. Press 'r' to scan for files.",
            Style::default()
                .fg(Color::Rgb(150, 150, 150))
                .add_modifier(Modifier::ITALIC),
        )])));
    }

    let list = List::new(activities).block(
        Block::default()
            .title(" Recent Activity ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    f.render_widget(list, area);
}

// Helper functions
fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn format_relative_time(timestamp: chrono::DateTime<chrono::Local>) -> String {
    let now = chrono::Local::now();
    let duration = now.signed_duration_since(timestamp);

    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{} min ago", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{} hours ago", duration.num_hours())
    } else if duration.num_days() < 7 {
        format!("{} days ago", duration.num_days())
    } else {
        timestamp.format("%Y-%m-%d").to_string()
    }
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

    // Group files by year instead of just using files_by_date
    let mut files_by_year: HashMap<String, (usize, u64)> = HashMap::new();

    // Process all files to group by year
    for file in &app.cached_files {
        let year = file.created.format("%Y").to_string();
        let entry = files_by_year.entry(year).or_insert((0, 0));
        entry.0 += 1; // Increment file count
        entry.1 += file.size; // Add to total size
    }

    // Convert to sorted vector
    let mut timeline_data: Vec<(String, usize, u64)> = files_by_year
        .into_iter()
        .map(|(year, (count, size))| (year, count, size))
        .collect();

    // Sort by year (newest first)
    timeline_data.sort_by(|a, b| b.0.cmp(&a.0));

    // Create layout with two sections: chart and table
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Bar chart
            Constraint::Percentage(40), // Statistics table
        ])
        .split(area);

    // Draw bar chart for file counts
    draw_timeline_chart(f, chunks[0], &timeline_data);

    // Draw detailed statistics table
    draw_timeline_table(f, chunks[1], &timeline_data, stats.total_size);
}

fn draw_timeline_chart(f: &mut Frame, area: Rect, timeline_data: &[(String, usize, u64)]) {
    // Take up to 12 most recent years for the chart
    let chart_data: Vec<(&str, u64)> = timeline_data
        .iter()
        .take(12)
        .map(|(year, count, _)| (year.as_str(), *count as u64))
        .collect();

    let bars: Vec<Bar> = chart_data
        .iter()
        .enumerate()
        .map(|(idx, (label, value))| {
            // Use gradient colors for visual appeal
            let color = match idx % 4 {
                0 => Color::Cyan,
                1 => Color::Blue,
                2 => Color::Green,
                _ => Color::Magenta,
            };

            Bar::default()
                .value(*value)
                .label(Line::from(label.to_string()))
                .style(Style::default().fg(color))
                .value_style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
        })
        .collect();

    let bar_group = BarGroup::default().bars(&bars);

    let bar_chart = BarChart::default()
        .block(
            Block::default()
                .title(" Files by Year ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .data(bar_group)
        .bar_width(if timeline_data.len() > 10 { 3 } else { 5 })
        .bar_gap(1);

    f.render_widget(bar_chart, area);
}

fn draw_timeline_table(
    f: &mut Frame,
    area: Rect,
    timeline_data: &[(String, usize, u64)],
    total_size: u64,
) {
    // Calculate totals
    let total_files: usize = timeline_data.iter().map(|(_, count, _)| count).sum();

    // Create table rows with all statistics
    let rows: Vec<Row> = timeline_data
        .iter()
        .map(|(year, count, size)| {
            let percentage = if total_files > 0 {
                (*count as f64 / total_files as f64) * 100.0
            } else {
                0.0
            };

            let size_percentage = if total_size > 0 {
                (*size as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };

            Row::new(vec![
                Cell::from(year.clone()).style(Style::default().fg(Color::Cyan)),
                Cell::from(count.to_string()).style(Style::default().fg(Color::White)),
                Cell::from(format!("{:.1}%", percentage)).style(Style::default().fg(Color::Yellow)),
                Cell::from(format_bytes(*size)).style(Style::default().fg(Color::Green)),
                Cell::from(format!("{:.1}%", size_percentage))
                    .style(Style::default().fg(Color::Magenta)),
                Cell::from(if *count > 0 {
                    format_bytes(*size / *count as u64)
                } else {
                    "0 B".to_string()
                })
                .style(Style::default().fg(Color::Blue)),
            ])
        })
        .collect();

    // Add footer row with totals
    let footer = Row::new(vec![
        Cell::from("TOTAL").style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(total_files.to_string()).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("100.0%").style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(format_bytes(total_size)).style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("100.0%").style(
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(if total_files > 0 {
            format_bytes(total_size / total_files as u64)
        } else {
            "0 B".to_string()
        })
        .style(
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let mut all_rows = rows;
    if !timeline_data.is_empty() {
        all_rows.push(Row::new(vec![Cell::from(""); 6]).height(1)); // Separator
        all_rows.push(footer);
    }

    let table = Table::new(
        all_rows,
        [
            Constraint::Percentage(15), // Year
            Constraint::Percentage(15), // Count
            Constraint::Percentage(15), // Count %
            Constraint::Percentage(20), // Size
            Constraint::Percentage(15), // Size %
            Constraint::Percentage(20), // Avg Size
        ],
    )
    .header(
        Row::new(vec![
            "Year",
            "Files",
            "Files %",
            "Total Size",
            "Size %",
            "Avg Size",
        ])
        .style(Style::default().add_modifier(Modifier::BOLD))
        .bottom_margin(1),
    )
    .block(
        Block::default()
            .title(" Detailed Statistics by Year ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    )
    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol("> ");

    f.render_widget(table, area);
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
