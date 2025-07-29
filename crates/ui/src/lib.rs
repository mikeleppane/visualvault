#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, Padding, Paragraph},
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

// Beautiful color palette (matching dashboard)
const ACCENT_COLOR: Color = Color::Rgb(139, 233, 253); // Cyan
const SUCCESS_COLOR: Color = Color::Rgb(80, 250, 123); // Green
const WARNING_COLOR: Color = Color::Rgb(255, 184, 108); // Orange
const ERROR_COLOR: Color = Color::Rgb(255, 85, 85); // Red
const MUTED_COLOR: Color = Color::Rgb(98, 114, 164); // Gray
const BACKGROUND_ALT: Color = Color::Rgb(30, 30, 46); // Dark background
const BACKGROUND_MAIN: Color = Color::Rgb(24, 24, 37); // Main background
const VERSION: &str = "0.7.1"; // Updated version

pub fn draw(f: &mut Frame, app: &mut App) {
    // Draw main background
    let background = Block::default().style(Style::default().bg(BACKGROUND_MAIN));
    f.render_widget(background, f.area());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(6), // Enhanced header height
            Constraint::Min(0),    // Main content
            Constraint::Length(4), // Enhanced status bar
        ])
        .split(f.area());

    // Draw header
    draw_enhanced_header(f, chunks[0], app);

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

    // Draw enhanced status bar
    draw_enhanced_status_bar(f, chunks[2], app);

    // Draw help overlay if needed
    if app.show_help {
        draw_help_overlay(f, app);
    }
}

#[allow(clippy::too_many_lines)]
fn draw_enhanced_header(f: &mut Frame, area: Rect, app: &App) {
    // Create gradient background for header
    let header_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(MUTED_COLOR))
        .style(Style::default().bg(BACKGROUND_ALT));

    f.render_widget(header_block, area);

    // Split header into sections
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20), // Left: Logo
            Constraint::Min(0),     // Center: Title
            Constraint::Length(25), // Right: State
        ])
        .margin(1)
        .split(area);

    // Left section - Enhanced logo
    let logo_lines = vec![
        Line::from(vec![
            Span::styled("🖼️", Style::default().fg(ACCENT_COLOR)),
            Span::raw(" "),
            Span::styled("Visual", Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("Vault", Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![Span::styled(
            "   Media Organizer",
            Style::default().fg(MUTED_COLOR).add_modifier(Modifier::ITALIC),
        )]),
    ];

    let logo = Paragraph::new(logo_lines).alignment(Alignment::Left);
    f.render_widget(logo, header_chunks[0]);

    // Center section - Animated title based on state
    #[allow(clippy::cast_possible_truncation)]
    let get_spinner = || {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let idx = (chrono::Local::now().timestamp_millis() / 100) as usize % frames.len();
        frames[idx]
    };
    let center_content = match app.state {
        AppState::Scanning => {
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        get_spinner(),
                        Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled("Scanning for media files...", Style::default().fg(ACCENT_COLOR)),
                ]),
            ]
        }
        AppState::Organizing => {
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        get_spinner(),
                        Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled("Organizing your collection...", Style::default().fg(SUCCESS_COLOR)),
                ]),
            ]
        }
        _ => {
            vec![
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Organize • Manage • Discover",
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                )]),
            ]
        }
    };

    let center = Paragraph::new(center_content).alignment(Alignment::Center);
    f.render_widget(center, header_chunks[1]);

    // Right section - Enhanced state indicator
    let state_info = match app.state {
        AppState::Dashboard => ("📊", "Dashboard", ACCENT_COLOR, "Browse your media"),
        AppState::Settings => ("⚙️", "Settings", WARNING_COLOR, "Configure options"),
        AppState::Scanning => ("🔍", "Scanning", ACCENT_COLOR, "Finding files..."),
        AppState::Organizing => ("📁", "Organizing", SUCCESS_COLOR, "Moving files..."),
        AppState::Search => ("🔎", "Search", Color::White, "Find files"),
        AppState::FileDetails(_) => ("📄", "Details", Color::White, "File information"),
        AppState::DuplicateReview => ("🔄", "Duplicates", Color::Magenta, "Review duplicates"),
        AppState::Filters => ("🔧", "Filters", Color::Magenta, "Advanced filtering"),
    };

    let state_lines = vec![
        Line::from(vec![
            Span::styled(state_info.0, Style::default().fg(state_info.2)),
            Span::raw(" "),
            Span::styled(
                state_info.1,
                Style::default().fg(state_info.2).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![Span::styled(
            state_info.3,
            Style::default().fg(MUTED_COLOR).add_modifier(Modifier::ITALIC),
        )]),
    ];

    let state_widget = Paragraph::new(state_lines).alignment(Alignment::Right).block(
        Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(MUTED_COLOR))
            .padding(Padding::horizontal(1)),
    );
    f.render_widget(state_widget, header_chunks[2]);

    // Add version number in top-right corner
    let version_area = Rect {
        x: area.x + area.width - VERSION.len() as u16 - 2,
        y: area.y,
        width: VERSION.len() as u16 + 1,
        height: 1,
    };
    let version_widget = Paragraph::new(Span::styled(
        VERSION,
        Style::default().fg(MUTED_COLOR).add_modifier(Modifier::DIM),
    ));
    f.render_widget(version_widget, version_area);
}

#[allow(clippy::too_many_lines)]
fn draw_enhanced_status_bar(f: &mut Frame, area: Rect, app: &App) {
    // Enhanced status bar with gradient background
    let status_block = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(MUTED_COLOR))
        .style(Style::default().bg(BACKGROUND_ALT));

    f.render_widget(status_block.clone(), area);

    let inner_area = status_block.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(37), // Shortcuts
            Constraint::Min(0),     // Messages/Status
            Constraint::Length(30), // Stats
        ])
        .margin(1)
        .split(inner_area);

    // Left section - Context-aware shortcuts with icons
    let shortcuts = match app.state {
        AppState::Dashboard => vec![
            ("⌨", "q", "Quit", MUTED_COLOR),
            ("❓", "?", "Help", ACCENT_COLOR),
            ("🔄", "Tab", "Switch", WARNING_COLOR),
        ],
        AppState::Settings => vec![
            ("◀", "q", "Back", MUTED_COLOR),
            ("💾", "S", "Save", SUCCESS_COLOR),
            ("↺", "R", "Reset", ERROR_COLOR),
        ],
        AppState::FileDetails(_) => vec![
            ("⎋", "ESC", "Close", MUTED_COLOR),
            ("↕", "↑↓", "Navigate", ACCENT_COLOR),
            ("", "", "", Color::default()),
        ],
        AppState::DuplicateReview => vec![
            ("◀", "q", "Back", MUTED_COLOR),
            ("🗑", "d", "Delete", ERROR_COLOR),
            ("☑", "a", "Select", WARNING_COLOR),
        ],
        _ => vec![
            ("◀", "q", "Quit", MUTED_COLOR),
            ("❓", "?", "Help", ACCENT_COLOR),
            ("", "", "", Color::default()),
        ],
    };

    let mut shortcut_spans = vec![];
    for (i, (icon, key, desc, color)) in shortcuts.iter().enumerate() {
        if !key.is_empty() {
            if i > 0 {
                shortcut_spans.push(Span::raw(" │ "));
            }
            shortcut_spans.push(Span::styled(*icon, Style::default().fg(*color)));
            shortcut_spans.push(Span::raw(" "));
            shortcut_spans.push(Span::styled(
                *key,
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ));
            shortcut_spans.push(Span::raw(":"));
            shortcut_spans.push(Span::styled(*desc, Style::default().fg(MUTED_COLOR)));
        }
    }

    let left = Paragraph::new(Line::from(shortcut_spans)).alignment(Alignment::Left);
    f.render_widget(left, chunks[0]);

    // Center section - Enhanced messages with animations
    let center_content = if let Some(error) = &app.error_message {
        vec![Line::from(vec![
            Span::styled("🚨 ", Style::default().fg(ERROR_COLOR)),
            Span::styled(error, Style::default().fg(ERROR_COLOR).add_modifier(Modifier::BOLD)),
        ])]
    } else if let Some(success) = &app.success_message {
        vec![Line::from(vec![
            Span::styled("✨ ", Style::default().fg(SUCCESS_COLOR)),
            Span::styled(success, Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD)),
        ])]
    } else {
        match app.state {
            AppState::FileDetails(idx) => {
                if let Some(file) = app.cached_files.get(idx) {
                    vec![Line::from(vec![
                        Span::styled("📋 ", Style::default().fg(ACCENT_COLOR)),
                        Span::raw("Viewing: "),
                        Span::styled(
                            file.name.to_string(),
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                        ),
                    ])]
                } else {
                    vec![Line::from(vec![Span::styled(
                        "✓ Ready",
                        Style::default().fg(SUCCESS_COLOR),
                    )])]
                }
            }
            AppState::Scanning => {
                let progress = app.progress.try_read();
                if let Ok(progress) = progress {
                    vec![Line::from(vec![
                        Span::styled(
                            "🔍 ",
                            Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::SLOW_BLINK),
                        ),
                        Span::raw("Found "),
                        Span::styled(
                            format!("{}", progress.current),
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" files..."),
                    ])]
                } else {
                    vec![Line::from(vec![Span::styled(
                        "🔍 Scanning...",
                        Style::default().fg(ACCENT_COLOR),
                    )])]
                }
            }
            AppState::Organizing => {
                let progress = app.progress.try_read();
                if let Ok(progress) = progress {
                    let percentage = if progress.total > 0 {
                        (progress.current as f32 / progress.total as f32 * 100.0) as u8
                    } else {
                        0
                    };
                    vec![Line::from(vec![
                        Span::styled(
                            "📦 ",
                            Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::SLOW_BLINK),
                        ),
                        Span::raw("Organizing: "),
                        Span::styled(
                            format!("{}/{}", progress.current, progress.total),
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" ("),
                        Span::styled(
                            format!("{percentage}%"),
                            Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(")"),
                    ])]
                } else {
                    vec![Line::from(vec![Span::styled(
                        "📦 Organizing...",
                        Style::default().fg(SUCCESS_COLOR),
                    )])]
                }
            }
            _ => {
                vec![Line::from(vec![
                    Span::styled("✓ ", Style::default().fg(SUCCESS_COLOR)),
                    Span::styled("Ready", Style::default().fg(Color::White)),
                ])]
            }
        }
    };

    let center = Paragraph::new(center_content).alignment(Alignment::Center);
    f.render_widget(center, chunks[1]);

    // Right section - Enhanced stats with mini gauges
    let stats_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(chunks[2]);

    let stats_text = match app.state {
        AppState::FileDetails(idx) => {
            if let Some(file) = app.cached_files.get(idx) {
                format!(
                    "📄 {} │ {} │ {}/{}",
                    file.file_type,
                    format_bytes(file.size),
                    idx + 1,
                    app.cached_files.len()
                )
            } else {
                format!(
                    "📊 {} files │ Tab {}/{}",
                    format_number(app.statistics.total_files),
                    app.selected_tab + 1,
                    app.get_tab_count()
                )
            }
        }
        _ => {
            format!(
                "📊 {} files │ {} │ Tab {}/{}",
                format_number(app.statistics.total_files),
                format_bytes(app.statistics.total_size),
                app.selected_tab + 1,
                app.get_tab_count()
            )
        }
    };

    let right = Paragraph::new(stats_text)
        .alignment(Alignment::Right)
        .style(Style::default().fg(Color::White));
    f.render_widget(right, stats_chunks[0]);

    // Add a subtle progress indicator for operations
    if matches!(app.state, AppState::Scanning | AppState::Organizing) {
        let progress = app.progress.try_read();
        if let Ok(progress) = progress {
            let ratio = if progress.total > 0 {
                progress.current as f64 / progress.total as f64
            } else {
                0.0
            };

            let color = match app.state {
                AppState::Scanning => ACCENT_COLOR,
                AppState::Organizing => SUCCESS_COLOR,
                _ => MUTED_COLOR,
            };

            let mini_gauge = Gauge::default()
                .gauge_style(Style::default().fg(color).bg(Color::Rgb(40, 40, 55)))
                .ratio(ratio)
                .label("");
            f.render_widget(mini_gauge, stats_chunks[1]);
        }
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
