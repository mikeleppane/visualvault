#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
use ahash::AHashMap;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span},
    widgets::{
        Bar, BarChart, BarGroup, Block, BorderType, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table, Tabs,
    },
};

use visualvault_app::App;
use visualvault_models::AppState;
use visualvault_utils::format_bytes;

// Beautiful color palette
const ACCENT_COLOR: Color = Color::Rgb(139, 233, 253); // Cyan
const SUCCESS_COLOR: Color = Color::Rgb(80, 250, 123); // Green
const WARNING_COLOR: Color = Color::Rgb(255, 184, 108); // Orange
const ERROR_COLOR: Color = Color::Rgb(255, 85, 85); // Red
const MUTED_COLOR: Color = Color::Rgb(98, 114, 164); // Gray
const BACKGROUND_ALT: Color = Color::Rgb(30, 30, 46); // Dark background

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    // Add a subtle background
    let background = Block::default().style(Style::default().bg(Color::Rgb(24, 24, 37)));
    f.render_widget(background, area);

    let tabs = vec!["📊 Overview", "📁 Files", "📈 Types", "📅 Timeline"];
    let selected_tab = app.selected_tab;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Draw enhanced tabs
    let tabs_widget = Tabs::new(tabs)
        .select(selected_tab)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED_COLOR)),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(ACCENT_COLOR)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider(symbols::DOT);

    f.render_widget(tabs_widget, chunks[0]);

    // Draw content based on selected tab with smooth transitions
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
            Constraint::Length(14), // Charts (increased height)
            Constraint::Min(0),     // Recent activity
        ])
        .split(area);

    // Statistics cards with animations
    draw_stats_cards(f, chunks[0], app);

    // Charts section
    let chart_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .margin(1)
        .split(chunks[1]);

    draw_storage_gauge(f, chart_chunks[0], app);
    draw_file_type_distribution(f, chart_chunks[1], app);

    // Recent activity with icons
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

    // Calculate actual duplicate count from duplicate_groups if available
    let duplicate_count = if let Some(ref groups) = app.duplicate_groups {
        if groups.is_empty() {
            stats.duplicate_count
        } else {
            groups.iter().map(|group| group.len().saturating_sub(1)).sum()
        }
    } else {
        stats.duplicate_count
    };

    // Calculate duplicate size if we have duplicate groups
    let duplicate_size = if let Some(ref groups) = app.duplicate_groups {
        if groups.is_empty() {
            0
        } else {
            groups
                .iter()
                .map(|group| group.iter().skip(1).map(|f| f.size).sum::<u64>())
                .sum()
        }
    } else {
        0
    };

    let cards = [
        ("📄 Total Files", stats.total_files.to_string(), ACCENT_COLOR, "files"),
        (
            "💾 Total Size",
            format_bytes(stats.total_size),
            SUCCESS_COLOR,
            "storage",
        ),
        (
            "🔄 Duplicates",
            duplicate_count.to_string(),
            WARNING_COLOR,
            "duplicates",
        ),
        ("🗑️  Wasted Space", format_bytes(duplicate_size), ERROR_COLOR, "wasted"),
    ];

    #[allow(clippy::cast_precision_loss)]
    for (i, (title, value, color, _card_type)) in cards.iter().enumerate() {
        // Create a gradient effect with borders
        let card = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(*color))
            .title(Span::styled(
                format!(" {title} "),
                Style::default().fg(*color).add_modifier(Modifier::BOLD),
            ))
            .style(Style::default().bg(BACKGROUND_ALT));

        // Add a subtle progress indicator
        let progress = match i {
            0 => (stats.total_files as f64 / 10000.0).min(1.0), // Assume 10k files is "full"
            1 => (stats.total_size as f64 / (10u64.pow(9) as f64)).min(1.0), // 1GB as reference
            2 => (duplicate_count as f64 / stats.total_files.max(1) as f64).min(1.0),
            3 => (duplicate_size as f64 / stats.total_size.max(1) as f64).min(1.0),
            _ => 0.0,
        };

        let inner_area = card.inner(chunks[i]);
        f.render_widget(card, chunks[i]);

        // Split inner area for value and mini gauge
        let inner_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(2), Constraint::Length(1)])
            .split(inner_area);

        // Render the value with animation effect
        let value_paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                value,
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
        ])
        .alignment(Alignment::Center);

        f.render_widget(value_paragraph, inner_chunks[1]);

        // Render mini progress bar
        let gauge = Gauge::default()
            .block(Block::default())
            .gauge_style(Style::default().fg(*color).bg(Color::Rgb(40, 40, 55)))
            .ratio(progress)
            .label("");

        f.render_widget(gauge, inner_chunks[2]);
    }
}

fn draw_storage_gauge(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" 💿 Storage Overview ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED_COLOR))
        .style(Style::default().bg(BACKGROUND_ALT));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Split area into source and destination sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner_area);

    // Draw source folder stats with enhanced visuals
    draw_folder_stats_enhanced(f, chunks[0], app, true);

    // Draw destination folder stats with enhanced visuals
    draw_folder_stats_enhanced(f, chunks[1], app, false);
}

fn draw_folder_stats_enhanced(f: &mut Frame, area: Rect, app: &App, is_source: bool) {
    let settings = app.settings.try_read();

    if let Ok(settings) = settings {
        let (folder_path, _, color, icon) = if is_source {
            (settings.source_folder.as_ref(), "Source", ACCENT_COLOR, "📥")
        } else {
            (settings.destination_folder.as_ref(), "Destination", SUCCESS_COLOR, "📤")
        };

        let content = if let Some(path) = folder_path {
            let folder_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown");
            let stats = app.folder_stats_cache.get(path);

            let mut lines = vec![Line::from(vec![
                Span::styled(format!("{icon} "), Style::default().fg(color)),
                Span::styled(
                    folder_name,
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
            ])];

            if let Some(full_path) = path.to_str() {
                lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(
                        truncate_path(full_path, area.width as usize - 7),
                        Style::default().fg(MUTED_COLOR).add_modifier(Modifier::DIM),
                    ),
                ]));
            }

            if let Some(stats) = stats {
                lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled("📊 ", Style::default().fg(WARNING_COLOR)),
                    Span::styled(
                        format!(
                            "{} files • {} folders • {} media",
                            format_number(stats.total_files),
                            format_number(stats.total_dirs),
                            format_number(stats.media_files)
                        ),
                        Style::default().fg(Color::Yellow),
                    ),
                ]));

                lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled("💾 ", Style::default().fg(Color::Magenta)),
                    Span::styled(
                        format_bytes(stats.total_size),
                        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(
                        "⏳ Calculating...",
                        Style::default()
                            .fg(MUTED_COLOR)
                            .add_modifier(Modifier::ITALIC | Modifier::SLOW_BLINK),
                    ),
                ]));
            }

            lines
        } else {
            vec![
                Line::from(vec![Span::styled(
                    format!("{icon} Not configured"),
                    Style::default().fg(MUTED_COLOR).add_modifier(Modifier::ITALIC),
                )]),
                Line::from(vec![
                    Span::raw("   "),
                    Span::styled(
                        "Press 's' to configure",
                        Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
                    ),
                ]),
            ]
        };

        let paragraph = Paragraph::new(content).alignment(Alignment::Left);

        f.render_widget(paragraph, area);
    }
}

fn draw_file_type_distribution(f: &mut Frame, area: Rect, app: &App) {
    let stats = &app.statistics;
    let mut data: Vec<(&str, u64)> = stats.media_types.iter().map(|(k, v)| (k.as_str(), *v as u64)).collect();
    data.sort_by(|a, b| b.1.cmp(&a.1));
    data.truncate(5);

    // Use emoji icons for file types
    let get_type_icon = |file_type: &str| -> &str {
        match file_type.to_lowercase().as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "image" => "🖼️",
            "mp4" | "avi" | "mkv" | "mov" | "video" => "🎬",
            "mp3" | "wav" | "flac" | "audio" => "🎵",
            "pdf" | "doc" | "txt" | "document" => "📄",
            "raw" | "dng" | "cr2" => "📸",
            _ => "📎",
        }
    };

    let bars: Vec<Bar> = data
        .iter()
        .map(|(label, value)| {
            let icon = get_type_icon(label);
            Bar::default()
                .value(*value)
                .label(Line::from(format!("{icon} {label}")))
                .style(Style::default().fg(get_enhanced_type_color(label)))
                .value_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        })
        .collect();

    let bar_group = BarGroup::default().bars(&bars);
    let bar_chart = BarChart::default()
        .block(
            Block::default()
                .title(" 📊 File Types ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED_COLOR))
                .style(Style::default().bg(BACKGROUND_ALT)),
        )
        .data(bar_group)
        .bar_width(5)
        .bar_gap(2)
        .value_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    f.render_widget(bar_chart, area);
}

#[allow(clippy::too_many_lines)]
fn draw_recent_activity(f: &mut Frame, area: Rect, app: &App) {
    let mut activities = Vec::new();

    // Add scan activity if available
    if let Some(scan_result) = &app.last_scan_result {
        activities.push(ListItem::new(Line::from(vec![
            Span::styled("🔍 ", Style::default().fg(ACCENT_COLOR)),
            Span::raw("Scanned "),
            Span::styled(
                format!("{}", scan_result.files_found),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" files in {}", format_duration(scan_result.duration))),
            Span::styled(
                format!(" • {}", format_relative_time(scan_result.timestamp)),
                Style::default().fg(MUTED_COLOR).add_modifier(Modifier::DIM),
            ),
        ])));
    }

    // Add organization activity if available
    if let Some(org_result) = &app.last_organize_result {
        let (icon, color) = if org_result.success {
            ("✅", SUCCESS_COLOR)
        } else {
            ("❌", ERROR_COLOR)
        };

        let dest_name = org_result
            .destination
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("destination");

        activities.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{icon} "), Style::default().fg(color)),
            Span::raw("Organized "),
            Span::styled(
                format!("{}/{}", org_result.files_organized, org_result.files_total),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" files to "),
            Span::styled(dest_name, Style::default().fg(Color::White).underlined()),
            Span::styled(
                format!(" • {}", format_relative_time(org_result.timestamp)),
                Style::default().fg(MUTED_COLOR).add_modifier(Modifier::DIM),
            ),
        ])));
    }

    // Add duplicate detection activity
    if let Some(ref duplicate_groups) = app.duplicate_groups {
        if !duplicate_groups.is_empty() {
            let total_duplicates: usize = duplicate_groups.iter().map(|group| group.len().saturating_sub(1)).sum();

            let duplicate_size: u64 = duplicate_groups
                .iter()
                .map(|group| group.iter().skip(1).map(|f| f.size).sum::<u64>())
                .sum();

            activities.push(ListItem::new(Line::from(vec![
                Span::styled("⚠️  ", Style::default().fg(WARNING_COLOR)),
                Span::raw("Found "),
                Span::styled(
                    format!("{total_duplicates}"),
                    Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" duplicate files in "),
                Span::styled(
                    format!("{}", duplicate_groups.len()),
                    Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" groups • "),
                Span::styled(
                    format_bytes(duplicate_size),
                    Style::default().fg(ERROR_COLOR).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" wasted"),
            ])));
        }
    }

    // Add current operation status with animated spinner
    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner_idx = (chrono::Local::now().timestamp_millis() / 100) as usize % spinner_frames.len();

    if app.state == AppState::Scanning {
        let progress = app.progress.try_read();
        if let Ok(progress) = progress {
            activities.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", spinner_frames[spinner_idx]),
                    Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD),
                ),
                Span::styled("Scanning in progress... ", Style::default().fg(ACCENT_COLOR)),
                Span::styled(
                    format!("{}", progress.current),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" files found"),
            ])));
        }
    } else if app.state == AppState::Organizing {
        let progress = app.progress.try_read();
        if let Ok(progress) = progress {
            let percentage = if progress.total > 0 {
                (progress.current as f32 / progress.total as f32 * 100.0) as u8
            } else {
                0
            };

            activities.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", spinner_frames[spinner_idx]),
                    Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD),
                ),
                Span::styled("Organizing files... ", Style::default().fg(SUCCESS_COLOR)),
                Span::styled(
                    format!("{}/{}", progress.current, progress.total),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" • "),
                Span::styled(
                    format!("{percentage}%"),
                    Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD),
                ),
            ])));
        }
    }

    // Add error messages if any
    if let Some(error) = &app.error_message {
        activities.insert(
            0,
            ListItem::new(Line::from(vec![
                Span::styled("🚨 ", Style::default().fg(ERROR_COLOR)),
                Span::styled(error, Style::default().fg(ERROR_COLOR).add_modifier(Modifier::BOLD)),
            ])),
        );
    }

    // Add success messages if any
    if let Some(success) = &app.success_message {
        activities.insert(
            0,
            ListItem::new(Line::from(vec![
                Span::styled("🎉 ", Style::default().fg(SUCCESS_COLOR)),
                Span::styled(success, Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD)),
            ])),
        );
    }

    // If no activities, show placeholder with helpful hint
    if activities.is_empty() {
        activities.push(ListItem::new(Line::from(vec![
            Span::styled("💡 ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "No recent activity. Press ",
                Style::default().fg(MUTED_COLOR).add_modifier(Modifier::ITALIC),
            ),
            Span::styled(
                "'r'",
                Style::default()
                    .fg(ACCENT_COLOR)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
            Span::styled(
                " to scan for files.",
                Style::default().fg(MUTED_COLOR).add_modifier(Modifier::ITALIC),
            ),
        ])));
    }

    let list = List::new(activities).block(
        Block::default()
            .title(" 📋 Recent Activity ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED_COLOR))
            .style(Style::default().bg(BACKGROUND_ALT)),
    );

    f.render_widget(list, area);
}

// Enhanced helper functions with better formatting
fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{secs}s")
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
        format!("{}m ago", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}h ago", duration.num_hours())
    } else if duration.num_days() < 7 {
        format!("{}d ago", duration.num_days())
    } else {
        timestamp.format("%b %d").to_string()
    }
}

fn draw_files_list(f: &mut Frame, area: Rect, app: &App) {
    let files = &app.cached_files;

    // Create a beautiful file list with icons
    let rows: Vec<Row> = files
        .iter()
        .skip(app.scroll_offset)
        .take((area.height as usize).saturating_sub(4))
        .enumerate()
        .map(|(idx, file)| {
            let is_selected = app.selected_file_index == app.scroll_offset + idx;

            let style = if is_selected {
                Style::default().bg(Color::Rgb(69, 71, 90)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let type_icon = match file.file_type.to_string().to_lowercase().as_str() {
                "image" => "🖼️",
                "video" => "🎬",
                "audio" => "🎵",
                "raw" => "📸",
                _ => "📄",
            };

            Row::new(vec![
                Cell::from(format!("{} {}", type_icon, file.name)),
                Cell::from(file.file_type.to_string())
                    .style(Style::default().fg(get_enhanced_type_color(&file.file_type.to_string()))),
                Cell::from(format_bytes(file.size)).style(Style::default().fg(Color::Cyan)),
                Cell::from(file.modified.format("%Y-%m-%d %H:%M").to_string()).style(Style::default().fg(MUTED_COLOR)),
            ])
            .style(style)
        })
        .collect();

    let header_style = Style::default()
        .fg(ACCENT_COLOR)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);

    let table = Table::new(
        rows.clone(),
        [
            Constraint::Percentage(40),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(30),
        ],
    )
    .header(
        Row::new(vec!["Name", "Type", "Size", "Modified"])
            .style(header_style)
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .title(format!(
                " 📁 Files ({}/{}) ",
                app.scroll_offset + rows.len().min(1),
                files.len()
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED_COLOR))
            .style(Style::default().bg(BACKGROUND_ALT)),
    )
    .row_highlight_style(Style::default().bg(Color::Rgb(69, 71, 90)).add_modifier(Modifier::BOLD))
    .highlight_symbol("▶ ");

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

    #[allow(clippy::cast_precision_loss)]
    let rows: Vec<Row> = type_data
        .iter()
        .enumerate()
        .map(|(idx, (file_type, count, size))| {
            let percentage = if stats.total_size == 0 {
                0.0
            } else {
                (*size as f64 / stats.total_size as f64) * 100.0
            };

            let type_icon = match file_type.to_lowercase().as_str() {
                "jpg" | "jpeg" | "png" | "gif" | "image" => "🖼️",
                "mp4" | "avi" | "mkv" | "mov" | "video" => "🎬",
                "mp3" | "wav" | "flac" | "audio" => "🎵",
                "pdf" | "doc" | "txt" | "document" => "📄",
                "raw" | "dng" | "cr2" => "📸",
                _ => "📎",
            };

            let style = if idx % 2 == 0 {
                Style::default().bg(Color::Rgb(40, 42, 54))
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(format!("{type_icon} {file_type}"))
                    .style(Style::default().fg(get_enhanced_type_color(file_type))),
                Cell::from(format!("{count}")).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Cell::from(format_bytes(*size)).style(Style::default().fg(Color::Cyan)),
                Cell::from(format!("{percentage:.1}%")).style(Style::default().fg(Color::Yellow)),
                Cell::from(create_mini_bar(percentage)).style(Style::default().fg(get_enhanced_type_color(file_type))),
            ])
            .style(style)
        })
        .collect();

    let header_style = Style::default()
        .fg(ACCENT_COLOR)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(25),
        ],
    )
    .header(
        Row::new(vec!["Type", "Count", "Size", "%", "Distribution"])
            .style(header_style)
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .title(" 📊 File Type Statistics ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED_COLOR))
            .style(Style::default().bg(BACKGROUND_ALT)),
    );

    f.render_widget(table, area);
}

fn draw_timeline(f: &mut Frame, area: Rect, app: &App) {
    let stats = &app.statistics;

    // Group files by year
    let mut files_by_year: AHashMap<String, (usize, u64)> = AHashMap::new();

    for file in &app.cached_files {
        let year = file.created.format("%Y").to_string();
        let entry = files_by_year.entry(year).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += file.size;
    }

    let mut timeline_data: Vec<(String, usize, u64)> = files_by_year
        .into_iter()
        .map(|(year, (count, size))| (year, count, size))
        .collect();

    timeline_data.sort_by(|a, b| b.0.cmp(&a.0));

    // Create layout with two sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Draw enhanced bar chart
    draw_timeline_chart_enhanced(f, chunks[0], &timeline_data);

    // Draw detailed statistics table
    draw_timeline_table_enhanced(f, chunks[1], &timeline_data, stats.total_size);
}

fn draw_timeline_chart_enhanced(f: &mut Frame, area: Rect, timeline_data: &[(String, usize, u64)]) {
    let chart_data: Vec<(&str, u64)> = timeline_data
        .iter()
        .take(12)
        .map(|(year, count, _)| (year.as_str(), *count as u64))
        .collect();

    let bars: Vec<Bar> = chart_data
        .iter()
        .enumerate()
        .map(|(idx, (label, value))| {
            // Create a gradient effect
            let color = match idx % 6 {
                0 => Color::Rgb(139, 233, 253), // Cyan
                1 => Color::Rgb(80, 250, 123),  // Green
                2 => Color::Rgb(255, 184, 108), // Orange
                3 => Color::Rgb(255, 121, 198), // Pink
                4 => Color::Rgb(189, 147, 249), // Purple
                _ => Color::Rgb(241, 250, 140), // Yellow
            };

            Bar::default()
                .value(*value)
                .label(Line::from(format!("📅 {}", *label)))
                .style(Style::default().fg(color))
                .value_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        })
        .collect();

    let bar_group = BarGroup::default().bars(&bars);

    let bar_chart = BarChart::default()
        .block(
            Block::default()
                .title(" 📅 Files by Year ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED_COLOR))
                .style(Style::default().bg(BACKGROUND_ALT)),
        )
        .data(bar_group)
        .bar_width(if timeline_data.len() > 10 { 3 } else { 5 })
        .bar_gap(1);

    f.render_widget(bar_chart, area);
}

fn draw_timeline_table_enhanced(f: &mut Frame, area: Rect, timeline_data: &[(String, usize, u64)], total_size: u64) {
    let total_files: usize = timeline_data.iter().map(|(_, count, _)| count).sum();

    let rows: Vec<Row> = timeline_data
        .iter()
        .enumerate()
        .map(|(idx, (year, count, size))| {
            #[allow(clippy::cast_precision_loss)]
            let percentage = if total_files > 0 {
                (*count as f64 / total_files as f64) * 100.0
            } else {
                0.0
            };
            #[allow(clippy::cast_precision_loss)]
            let size_percentage = if total_size > 0 {
                (*size as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };

            let style = if idx % 2 == 0 {
                Style::default().bg(Color::Rgb(40, 42, 54))
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(format!("📅 {year}")).style(Style::default().fg(ACCENT_COLOR)),
                Cell::from(count.to_string()).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Cell::from(format!("{percentage:.1}%")).style(Style::default().fg(Color::Yellow)),
                Cell::from(format_bytes(*size)).style(Style::default().fg(SUCCESS_COLOR)),
                Cell::from(format!("{size_percentage:.1}%")).style(Style::default().fg(Color::Magenta)),
                Cell::from(if *count > 0 {
                    format_bytes(*size / *count as u64)
                } else {
                    "0 B".to_string()
                })
                .style(Style::default().fg(Color::Blue)),
            ])
            .style(style)
        })
        .collect();

    let footer = Row::new(vec![
        Cell::from("📊 TOTAL").style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Cell::from(total_files.to_string()).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Cell::from("100.0%").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from(format_bytes(total_size)).style(Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD)),
        Cell::from("100.0%").style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Cell::from(if total_files > 0 {
            format_bytes(total_size / total_files as u64)
        } else {
            "0 B".to_string()
        })
        .style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().bg(Color::Rgb(69, 71, 90)));

    let mut all_rows = rows;
    if !timeline_data.is_empty() {
        all_rows.push(Row::new(vec![Cell::from(""); 6]).height(1));
        all_rows.push(footer);
    }

    let header_style = Style::default()
        .fg(ACCENT_COLOR)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);

    let table = Table::new(
        all_rows,
        [
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
        ],
    )
    .header(
        Row::new(vec!["Year", "Files", "Files %", "Total Size", "Size %", "Avg Size"])
            .style(header_style)
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .title(" 📊 Detailed Statistics by Year ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED_COLOR))
            .style(Style::default().bg(BACKGROUND_ALT)),
    );

    f.render_widget(table, area);
}

// Enhanced color palette for file types
fn get_enhanced_type_color(file_type: &str) -> Color {
    match file_type.to_lowercase().as_str() {
        "image" | "jpg" | "jpeg" | "png" | "gif" => Color::Rgb(80, 250, 123), // Green
        "video" | "mp4" | "avi" | "mkv" | "mov" => Color::Rgb(139, 233, 253), // Cyan
        "audio" | "mp3" | "wav" | "flac" => Color::Rgb(255, 121, 198),        // Pink
        "document" | "pdf" | "doc" | "txt" => Color::Rgb(241, 250, 140),      // Yellow
        "raw" | "dng" | "cr2" | "nef" => Color::Rgb(189, 147, 249),           // Purple
        _ => Color::Rgb(148, 163, 184),                                       // Gray
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

// Helper to create mini progress bars
fn create_mini_bar(percentage: f64) -> String {
    let width = 10;
    let filled = (percentage / 10.0).round() as usize;
    let empty = width - filled;

    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}
