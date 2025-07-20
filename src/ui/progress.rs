use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph},
};

use crate::app::App;

#[allow(clippy::significant_drop_tightening)]
pub fn draw_progress_overlay(f: &mut Frame, app: &App) {
    // Get progress data
    let Ok(progress) = app.progress.try_read() else { return };

    // Create centered overlay area
    let area = centered_rect(60, 30, f.area());

    // Clear the area for the overlay
    f.render_widget(Clear, area);

    // Create layout for progress components
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Progress bar
            Constraint::Length(2), // Stats
            Constraint::Length(2), // Message
            Constraint::Length(2), // Time info
        ])
        .split(area);

    // Main block with border
    let block = Block::default()
        .title(" Operation Progress ")
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Rgb(20, 20, 30)));

    f.render_widget(block, area);

    // Title with operation icon
    let (icon, operation) = match app.state {
        crate::app::AppState::Scanning => ("🔍", "Scanning Files"),
        crate::app::AppState::Organizing => ("📁", "Organizing Files"),
        _ => ("⏳", "Processing"),
    };

    let title = Paragraph::new(vec![Line::from(vec![Span::styled(
        format!("{icon} {operation}"),
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )])])
    .alignment(Alignment::Center);

    f.render_widget(title, chunks[0]);

    // Progress bar
    let percentage = progress.percentage();
    let label = if progress.total > 0 {
        format!("{percentage:.0}%")
    } else {
        "Calculating...".to_string()
    };

    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Rgb(40, 40, 40)))
        .percent(percentage as u16)
        .label(label)
        .use_unicode(true);

    f.render_widget(gauge, chunks[1]);

    // Statistics
    let stats_text = if progress.total > 0 {
        format!("{} / {} items", progress.current, progress.total)
    } else {
        format!("{} items processed", progress.current)
    };

    let stats = Paragraph::new(vec![Line::from(vec![Span::styled(
        stats_text,
        Style::default().fg(Color::Yellow),
    )])])
    .alignment(Alignment::Center);

    f.render_widget(stats, chunks[2]);

    // Current message
    if !progress.message.is_empty() {
        let message = Paragraph::new(vec![Line::from(vec![Span::styled(
            &progress.message,
            Style::default()
                .fg(Color::Rgb(150, 150, 150))
                .add_modifier(Modifier::ITALIC),
        )])])
        .alignment(Alignment::Center);

        f.render_widget(message, chunks[3]);
    }

    // Time information
    let elapsed = progress.elapsed();
    let time_info = if let Some(eta) = progress.eta() {
        format!("Elapsed: {} | ETA: {}", format_duration(elapsed), format_duration(eta))
    } else {
        format!("Elapsed: {}", format_duration(elapsed))
    };

    let time_paragraph = Paragraph::new(vec![Line::from(vec![Span::styled(
        time_info,
        Style::default().fg(Color::Green),
    )])])
    .alignment(Alignment::Center);

    f.render_widget(time_paragraph, chunks[4]);
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
