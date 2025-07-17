use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
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
            Constraint::Length(3), // Header
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
        } /* AppState::Settings => settings::draw(f, chunks[1], app),
          AppState::Scanning | AppState::Organizing => progress::draw(f, chunks[1], app),
          AppState::Search => search::draw(f, chunks[1], app),
          AppState::DuplicateReview => draw_duplicate_review(f, chunks[1], app), */
    }

    // Draw status bar
    draw_status_bar(f, chunks[2], app);

    // Draw help overlay if needed
    if app.show_help {
        draw_help_overlay(f);
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let header_text = format!(
        " 🖼️  VisualVault - {} ",
        match app.state {
            AppState::Dashboard => "Dashboard",
            AppState::Settings => "Settings",
            AppState::Scanning => "Scanning Files",
            AppState::Organizing => "Organizing Files",
            AppState::Search => "Search",
            AppState::DuplicateReview => "Duplicate Review",
        }
    );

    let header = Paragraph::new(header_text)
        .style(
            Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .bg(Color::Rgb(0, 122, 204))
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(header, area);
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let (status_text, status_color) = if let Some(error) = &app.error_message {
        (format!(" ❌ {error} "), Color::Red)
    } else if let Some(success) = &app.success_message {
        (format!(" ✅ {success} "), Color::Green)
    } else {
        let mode_indicator = match app.input_mode {
            crate::app::InputMode::Normal => "NORMAL",
            crate::app::InputMode::Insert => "INSERT",
        };

        (
            format!(" {mode_indicator} | Press '?' for help | Tab: Navigate | Q: Quit "),
            Color::Rgb(100, 100, 100),
        )
    };

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(status_color).bg(Color::Rgb(40, 40, 40)))
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(status, area);
}

fn draw_duplicate_review(f: &mut Frame, area: Rect, _app: &App) {
    let content = Paragraph::new("Duplicate file review - Coming soon").block(
        Block::default()
            .title(" 🔍 Duplicates ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(content, area);
}

fn draw_help_overlay(f: &mut Frame) {
    let area = centered_rect(70, 80, f.area());

    let help_text = vec![
        "",
        " Navigation:",
        "   Tab/Shift+Tab  Navigate between tabs",
        "   ↑/↓ or j/k     Move selection up/down",
        "   PgUp/PgDn      Page up/down",
        "   Enter          Select/Edit",
        "",
        " Global Shortcuts:",
        "   ?/F1           Toggle this help",
        "   q/Esc          Quit (or exit mode)",
        "   d              Go to Dashboard",
        "   s              Go to Settings",
        "   f              Search/Filter files",
        "   u              Review duplicates",
        "",
        " Actions:",
        "   r              Scan/Rescan files",
        "   o              Organize files",
        "   Delete         Delete selected file",
        "",
        " Search Mode:",
        "   Type to filter files",
        "   Enter to apply, Esc to cancel",
        "",
    ];

    let help = Paragraph::new(help_text.join("\n"))
        .block(
            Block::default()
                .title(" ❓ Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Rgb(20, 20, 20))),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(Clear, area);
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
