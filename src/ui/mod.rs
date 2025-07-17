use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph},
};

use crate::app::{App, AppState};

mod dashboard;
mod settings;
/* mod progress;
mod search;
mod settings; */

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
        _ => {
            // Placeholder for other states
            let content = Paragraph::new("Content goes here")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Main Content "),
                )
                .style(Style::default().fg(Color::White));

            f.render_widget(Clear, chunks[1]);
            f.render_widget(content, chunks[1]);
        }
    }

    // Draw status bar
    draw_status_bar(f, chunks[2], app);

    // Draw help overlay if needed
    if app.show_help {
        draw_help_overlay(f);
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    // Create ASCII art logo
    let logo_lines = [
        "╔═══════════════════════════════════════════════════════════════════╗",
        "║  🖼️  ╦  ╦╦╔═╗╦ ╦╔═╗╦    ╦  ╦╔═╗╦ ╦╦  ╔╦╗  🖼️                     ║",
        "║      ╚╗╔╝║╚═╗║ ║╠═╣║    ╚╗╔╝╠═╣║ ║║   ║                           ║",
        "║       ╚╝ ╩╚═╝╚═╝╩ ╩╩═╝   ╚╝ ╩ ╩╚═╝╩═╝ ╩   Media Organizer v0.1    ║",
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
            1 | 2 | 3 => {
                // Logo lines with gradient effect
                let parts: Vec<&str> = line.split("  ").collect();
                let mut spans = Vec::new();

                for (j, part) in parts.iter().enumerate() {
                    if part.contains("🖼️") {
                        spans.push(Span::raw("🖼️"));
                    } else if part.contains("╦") || part.contains("╚") || part.contains("╩") {
                        // ASCII art characters with cyan gradient
                        let color = match i {
                            1 => Color::Cyan,
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
        AppState::DuplicateReview => ("🔄", "Duplicate Review", Color::Magenta),
        AppState::Search => ("🔎", "Search", Color::White),
    };

    // Create centered header block
    let header_block = Block::default()
        .borders(Borders::NONE)
        .padding(Padding::zero());

    let header_content = Paragraph::new(header_lines)
        .block(header_block)
        .alignment(Alignment::Center);

    f.render_widget(header_content, area);

    // Add current state indicator in the top right
    let state_indicator = format!("{} {}", state_text.0, state_text.1);
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
            Style::default()
                .fg(state_text.2)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]))
    .style(Style::default().bg(Color::Rgb(30, 30, 30)));

    f.render_widget(state_widget, state_area);
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    // Left section - shortcuts
    let shortcuts = match app.state {
        AppState::Dashboard => "q:Quit | ?:Help | Tab:Switch",
        AppState::Settings => "q:Back | S:Save | R:Reset",
        _ => "q:Quit | ?:Help",
    };

    let left = Paragraph::new(shortcuts)
        .style(Style::default().fg(Color::Rgb(150, 150, 150)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(60, 60, 60))),
        );

    // Center section - messages
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
        vec![Line::from(vec![Span::styled(
            "Ready",
            Style::default().fg(Color::Rgb(100, 100, 100)),
        )])]
    };

    let center = Paragraph::new(center_content)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(60, 60, 60))),
        );

    // Right section - stats
    let stats = format!(
        "Files: {} | Tab: {}/{}",
        app.statistics.total_files,
        app.selected_tab + 1,
        app.get_tab_count()
    );

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

fn draw_help_overlay(f: &mut Frame) {
    let area = centered_rect(80, 80, f.area());
    f.render_widget(Clear, area);

    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "🖼️  VisualVault Help",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Tab        - Next tab"),
        Line::from("  Shift+Tab  - Previous tab"),
        Line::from("  ↑/↓        - Navigate items"),
        Line::from("  PgUp/PgDn  - Navigate pages"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Actions",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  r          - Scan for files"),
        Line::from("  o          - Organize files"),
        Line::from("  f          - Search files"),
        Line::from("  u          - Review duplicates"),
        Line::from("  s          - Open settings"),
        Line::from("  d          - Go to dashboard"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "General",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ?/F1       - Toggle this help"),
        Line::from("  q          - Quit application"),
        Line::from("  Esc        - Cancel/Go back"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press any key to close",
            Style::default()
                .fg(Color::Rgb(150, 150, 150))
                .add_modifier(Modifier::ITALIC),
        )]),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" Help ")
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().bg(Color::Rgb(20, 20, 30)));

    f.render_widget(help, area);
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
