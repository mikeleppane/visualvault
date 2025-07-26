use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph},
};
use tracing::info;
use visualvault_app::App;
use visualvault_models::AppState;
use visualvault_utils::format_bytes;

mod dashboard;
mod duplicate_detector;
mod file_details;
mod filtering;
mod progress;
mod search;
mod settings;

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(5), // Increased header height from 3 to 5
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Status bar
        ])
        .split(f.area());

    // Draw header
    draw_header(f, chunks[0], app);

    // Draw main content based on state
    match app.state {
        AppState::Dashboard => dashboard::draw(f, chunks[1], app),
        AppState::Settings => settings::draw(f, chunks[1], app),
        AppState::Search => search::draw(f, chunks[1], app),
        AppState::FileDetails(file_idx) => {
            // Draw dashboard in background
            dashboard::draw(f, chunks[1], app);
            // Draw file details modal on top
            if let Some(file) = app.cached_files.get(file_idx) {
                file_details::draw_modal(f, file);
            }
        }
        AppState::Scanning | AppState::Organizing => {
            info!("Drawing progress overlay for state: {:?}", app.statistics);
            // Draw dashboard in background
            dashboard::draw(f, chunks[1], app);
            // Draw progress overlay on top
            progress::draw_progress_overlay(f, app);
        }
        AppState::DuplicateReview => duplicate_detector::draw(f, chunks[1], app),
        AppState::Filters => filtering::draw(f, chunks[1], app),
    }

    // Draw status bar
    draw_status_bar(f, chunks[2], app);

    // Draw help overlay if needed
    if app.show_help {
        draw_help_overlay(f, app);
    }
}

#[allow(clippy::cognitive_complexity)]
fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    // Create ASCII art logo
    let logo_lines = [
        "╔═══════════════════════════════════════════════════════════════════╗",
        "║  🖼️  ╦  ╦╦╔═╗╦ ╦╔═╗╦    ╦  ╦╔═╗╦ ╦╦  ╔╦╗  🖼️                     ║",
        "║      ╚╗╔╝║╚═╗║ ║╠═╣║    ╚╗╔╝╠═╣║ ║║   ║                           ║",
        "║       ╚╝ ╩╚═╝╚═╝╩ ╩╩═╝   ╚╝ ╩ ╩╚═╝╩═╝ ╩   Media Organizer v0.6    ║",
        "╚═══════════════════════════════════════════════════════════════════╝",
    ];

    // Create the header content with multiple styled spans
    let mut header_lines = Vec::new();

    for (i, line) in logo_lines.iter().enumerate() {
        let styled_line = match i {
            0 | 4 => {
                // Border lines
                Line::from(vec![Span::styled(
                    *line,
                    Style::default().fg(Color::Rgb(100, 100, 100)),
                )])
            }
            1..=3 => {
                // Logo lines with gradient effect
                let parts: Vec<&str> = line.split("  ").collect();
                let mut spans = Vec::new();

                for (j, part) in parts.iter().enumerate() {
                    if part.contains("🖼️") {
                        spans.push(Span::raw("🖼️"));
                    } else if part.contains("╦") || part.contains("╚") || part.contains("╩") {
                        // ASCII art characters with cyan gradient
                        let color = match i {
                            2 => Color::Rgb(0, 200, 200),
                            3 => Color::Rgb(0, 150, 150),
                            _ => Color::Cyan,
                        };
                        spans.push(Span::styled(
                            *part,
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ));
                    } else if part.contains("Media Organizer") {
                        spans.push(Span::styled(
                            *part,
                            Style::default()
                                .fg(Color::Rgb(150, 150, 150))
                                .add_modifier(Modifier::ITALIC),
                        ));
                    } else {
                        spans.push(Span::raw(*part));
                    }

                    if j < parts.len() - 1 {
                        spans.push(Span::raw("  "));
                    }
                }

                if i == 1 {
                    // Add a space before the logo
                    spans.insert(spans.len() - 2, Span::raw(" "));
                }

                Line::from(spans)
            }
            _ => Line::from(*line),
        };

        header_lines.push(styled_line);
    }

    // Add state indicator with icons
    let state_text = match app.state {
        AppState::Dashboard => ("📊", "Dashboard", Color::Green),
        AppState::Settings => ("⚙️", "Settings", Color::Yellow),
        AppState::Scanning => ("🔍", "Scanning...", Color::Cyan),
        AppState::Organizing => ("📁", "Organizing...", Color::Blue),
        AppState::Search => ("🔎", "Search", Color::White),
        AppState::FileDetails(_) => ("📄", "File Details", Color::White),
        AppState::DuplicateReview => ("🔄", "Duplicate Review", Color::Magenta),
        AppState::Filters => ("🔧", "Filters", Color::Magenta),
    };

    // Create centered header block
    let header_block = Block::default().borders(Borders::NONE).padding(Padding::ZERO);

    let header_content = Paragraph::new(header_lines)
        .block(header_block)
        .alignment(Alignment::Center);

    f.render_widget(header_content, area);

    // Add current state indicator in the top right
    let state_indicator = format!("{} {}", state_text.0, state_text.1);
    #[allow(clippy::cast_possible_truncation)]
    let state_area = Rect {
        x: area.x + area.width.saturating_sub(state_indicator.len() as u16 + 4),
        y: area.y + 1,
        width: state_indicator.len() as u16 + 2,
        height: 1,
    };

    let state_widget = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        Span::styled(
            state_indicator,
            Style::default().fg(state_text.2).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]))
    .style(Style::default().bg(Color::Rgb(30, 30, 30)));

    f.render_widget(state_widget, state_area);
}

#[allow(clippy::too_many_lines)]
fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(area);

    // Left section - shortcuts
    let shortcuts = match app.state {
        AppState::Dashboard => "q:Quit | ?:Help | Tab:Switch",
        AppState::Settings => "q:Back | S:Save | R:Reset",
        AppState::FileDetails(_) => "ESC/q:Close | ↑↓:Navigate",
        _ => "q:Quit | ?:Help",
    };

    let left = Paragraph::new(shortcuts)
        .style(Style::default().fg(Color::Rgb(150, 150, 150)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(60, 60, 60))),
        );

    // Center section - messages or current operation
    let center_content = if let Some(error) = &app.error_message {
        vec![Line::from(vec![
            Span::styled("❌ ", Style::default().fg(Color::Red)),
            Span::styled(error, Style::default().fg(Color::Red)),
        ])]
    } else if let Some(success) = &app.success_message {
        vec![Line::from(vec![
            Span::styled("✅ ", Style::default().fg(Color::Green)),
            Span::styled(success, Style::default().fg(Color::Green)),
        ])]
    } else {
        // Show state-specific status
        match app.state {
            AppState::FileDetails(idx) => {
                if let Some(file) = app.cached_files.get(idx) {
                    vec![Line::from(vec![
                        Span::styled("📋 ", Style::default().fg(Color::Cyan)),
                        Span::styled(format!("Viewing: {}", file.name), Style::default().fg(Color::Cyan)),
                    ])]
                } else {
                    vec![Line::from(vec![Span::styled(
                        "Ready",
                        Style::default().fg(Color::Rgb(100, 100, 100)),
                    )])]
                }
            }
            AppState::Scanning => {
                vec![Line::from(vec![
                    Span::styled(
                        "⟳ ",
                        Style::default().fg(Color::Blue).add_modifier(Modifier::SLOW_BLINK),
                    ),
                    Span::styled("Scanning files...", Style::default().fg(Color::Blue)),
                ])]
            }
            AppState::Organizing => {
                vec![Line::from(vec![
                    Span::styled(
                        "⟳ ",
                        Style::default().fg(Color::Blue).add_modifier(Modifier::SLOW_BLINK),
                    ),
                    Span::styled("Organizing files...", Style::default().fg(Color::Blue)),
                ])]
            }
            _ => {
                vec![Line::from(vec![Span::styled(
                    "Ready",
                    Style::default().fg(Color::Rgb(100, 100, 100)),
                )])]
            }
        }
    };

    let center = Paragraph::new(center_content).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(60, 60, 60))),
    );

    // Right section - stats
    let stats = match app.state {
        AppState::FileDetails(idx) => {
            // Show file-specific stats when viewing details
            if let Some(file) = app.cached_files.get(idx) {
                format!(
                    "{} | {} | File {}/{}",
                    file.file_type,
                    format_bytes(file.size),
                    idx + 1,
                    app.cached_files.len()
                )
            } else {
                format!(
                    "Files: {} | Tab: {}/{}",
                    app.statistics.total_files,
                    app.selected_tab + 1,
                    app.get_tab_count()
                )
            }
        }
        _ => {
            format!(
                "Files: {} | Tab: {}/{}",
                app.statistics.total_files,
                app.selected_tab + 1,
                app.get_tab_count()
            )
        }
    };

    let right = Paragraph::new(stats)
        .alignment(Alignment::Right)
        .style(Style::default().fg(Color::Rgb(150, 150, 150)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(60, 60, 60))),
        );

    f.render_widget(left, chunks[0]);
    f.render_widget(center, chunks[1]);
    f.render_widget(right, chunks[2]);
}

#[allow(clippy::too_many_lines)]
fn draw_help_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(90, 85, f.area());
    f.render_widget(Clear, area);

    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "🖼️  VisualVault Help - Media Organizer v0.6",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "📊 Dashboard Navigation",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Tab/Shift+Tab - Switch between tabs (Files, Images, Videos, Metadata)"),
        Line::from("  ↑/↓           - Navigate items in current tab"),
        Line::from("  PgUp/PgDn     - Navigate pages quickly"),
        Line::from("  Enter         - View file details"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "🔍 Core Operations",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  r             - Scan source folder for media files"),
        Line::from("  o             - Organize files to destination"),
        Line::from("  f             - Search files by name/type"),
        Line::from("  F             - Advanced filters (date, size, type, regex)"),
        Line::from("  u             - Update folder statistics"),
        Line::from("  D             - Duplicate detector and cleanup"),
        Line::from("  Ctrl+Z        - Undo last operation (if enabled, see settings)"),
        Line::from("  Ctrl+R        - Redo last undone operation (if enabled, see settings)"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "🔄 Duplicate Management",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  s             - Scan for duplicates (in duplicate view)"),
        Line::from("  ←/→           - Switch between group list and file list"),
        Line::from("  Space         - Select/deselect individual files"),
        Line::from("  a             - Select all but first file in group"),
        Line::from("  d             - Delete selected duplicate files"),
        Line::from("  D             - Delete ALL duplicates from ALL groups"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "🔧 Advanced Filters (Press F)",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Tab/Shift+Tab - Switch filter categories"),
        Line::from("  a             - Add new filter to current category"),
        Line::from("  d             - Delete selected filter"),
        Line::from("  Space         - Toggle filter on/off"),
        Line::from("  t             - Toggle all filters active/inactive"),
        Line::from("  c             - Clear all filters"),
        Line::from("  Enter         - Apply filters and return to dashboard"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "📝 Filter Examples",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC),
        )]),
        Line::from("  Dates: 'today', 'last 7 days', '2024-01-01 to 2024-12-31'"),
        Line::from("  Sizes: '>10MB', '<1GB', '10MB-100MB'"),
        Line::from("  Regex: '.*\\.tmp$' (temp files), 'IMG_.*' (camera files)"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "🔍 Search & File Details",
            Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  / or Enter    - Start typing to search (in search view)"),
        Line::from("  Esc           - Clear search and return to dashboard"),
        Line::from("  ↑/↓           - Navigate search results"),
        Line::from("  Enter         - View file details from search"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "⚙️  Settings & Configuration",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  s             - Open settings"),
        Line::from("  S             - Save settings (in settings view)"),
        Line::from("  R             - Reset to defaults (in settings view)"),
        Line::from("  Tab           - Switch settings tabs"),
        Line::from("  Space         - Toggle checkboxes"),
        Line::from("  Enter         - Edit text fields"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "🎯 Quick Actions",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  d             - Return to dashboard from anywhere"),
        Line::from("  ?/F1          - Toggle this help"),
        Line::from("  q             - Quit application"),
        Line::from("  Esc           - Cancel current action/go back"),
        Line::from("  Ctrl+C        - Force quit"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "📊 Status Indicators",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC),
        )]),
        Line::from("  Green messages  - Success/completed operations"),
        Line::from("  Red messages    - Errors or warnings"),
        Line::from("  Blue spinner    - Operations in progress"),
        Line::from("  Yellow borders  - Currently active/focused elements"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "💡 Tips",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
        )]),
        Line::from("  • Use filters to work with specific file types or date ranges"),
        Line::from("  • Duplicate detector shows potential space savings"),
        Line::from("  • Settings are automatically saved when changed"),
        Line::from("  • Search supports partial matches and file extensions"),
        Line::from("  • Use ↑/↓ or PgUp/PgDn to scroll this help"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press any key (except arrows) to close this help",
            Style::default()
                .fg(Color::Rgb(150, 150, 150))
                .add_modifier(Modifier::ITALIC),
        )]),
    ];

    // Calculate scroll position - use help_scroll from App
    let content_height = help_text.len();
    let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
    let max_scroll = content_height.saturating_sub(visible_height);
    let scroll_offset = app.help_scroll.min(max_scroll);

    #[allow(clippy::cast_possible_truncation)]
    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(format!(
                    " VisualVault Help & Keyboard Shortcuts [{}%] ",
                    if max_scroll == 0 {
                        100
                    } else {
                        (scroll_offset * 100) / max_scroll
                    }
                ))
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().bg(Color::Rgb(15, 15, 25)))
        .scroll((scroll_offset as u16, 0));

    f.render_widget(help, area);

    // Add scroll indicators
    if max_scroll > 0 {
        // Top scroll indicator
        if scroll_offset > 0 {
            let up_arrow = Paragraph::new("▲")
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Center);
            let up_area = Rect {
                x: area.x + area.width - 2,
                y: area.y + 1,
                width: 1,
                height: 1,
            };
            f.render_widget(up_arrow, up_area);
        }

        // Bottom scroll indicator
        if scroll_offset < max_scroll {
            let down_arrow = Paragraph::new("▼")
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Center);
            let down_area = Rect {
                x: area.x + area.width - 2,
                y: area.y + area.height - 2,
                width: 1,
                height: 1,
            };
            f.render_widget(down_arrow, down_area);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
