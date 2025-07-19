use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph},
    Frame,
};

use crate::app::App;
use crate::utils::Progress;

pub fn draw_progress_overlay(f: &mut Frame, app: &App) {
    // Get progress data
    let progress = app.progress.try_read();
    if progress.is_err() {
        return; // Skip if we can't get a lock
    }
    let progress = progress.unwrap();

    // Create centered overlay area
    let area = centered_rect(60, 30, f.area());
    
    // Clear the area for the overlay
    f.render_widget(Clear, area);

    // Create layout for progress components
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(3),  // Progress bar
            Constraint::Length(2),  // Stats
            Constraint::Length(2),  // Message
            Constraint::Length(2),  // Time info
        ])
        .split(area);

    // Main block with border
    let block = Block::default()
        .title(" Operation Progress ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Rgb(20, 20, 30)));

    f.render_widget(block, area);

    // Title with operation icon
    let (icon, operation) = match app.state {
        crate::app::AppState::Scanning => ("🔍", "Scanning Files"),
        crate::app::AppState::Organizing => ("📁", "Organizing Files"),
        crate::app::AppState::DuplicateReview => ("🔄", "Detecting Duplicates"),
        _ => ("⏳", "Processing"),
    };

    let title = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            format!("{} {}", icon, operation),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ])])
    .alignment(Alignment::Center);

    f.render_widget(title, chunks[0]);

    // Progress bar
    let percentage = progress.percentage();
    let label = if progress.total > 0 {
        format!("{:.0}%", percentage)
    } else {
        "Calculating...".to_string()
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(
            Style::default()
                .fg(Color::Cyan)
                .bg(Color::Rgb(40, 40, 40))
        )
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

    let stats = Paragraph::new(vec![Line::from(vec![
        Span::styled(stats_text, Style::default().fg(Color::Yellow)),
    ])])
    .alignment(Alignment::Center);

    f.render_widget(stats, chunks[2]);

    // Current message
    if !progress.message.is_empty() {
        let message = Paragraph::new(vec![Line::from(vec![
            Span::styled(
                &progress.message,
                Style::default()
                    .fg(Color::Rgb(150, 150, 150))
                    .add_modifier(Modifier::ITALIC),
            ),
        ])])
        .alignment(Alignment::Center);

        f.render_widget(message, chunks[3]);
    }

    // Time information
    let elapsed = progress.elapsed();
    let time_info = if let Some(eta) = progress.eta() {
        format!(
            "Elapsed: {} | ETA: {}",
            format_duration(elapsed),
            format_duration(eta)
        )
    } else {
        format!("Elapsed: {}", format_duration(elapsed))
    };

    let time_paragraph = Paragraph::new(vec![Line::from(vec![
        Span::styled(time_info, Style::default().fg(Color::Green)),
    ])])
    .alignment(Alignment::Center);

    f.render_widget(time_paragraph, chunks[4]);
}

pub fn draw_progress_widget(f: &mut Frame, area: Rect, progress: &Progress) {
    // Compact progress widget for embedding in other views
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // Label
            Constraint::Length(1),  // Progress bar
            Constraint::Length(1),  // Stats
        ])
        .split(area);

    // Label
    let label = if !progress.message.is_empty() {
        &progress.message
    } else {
        "Processing..."
    };

    let label_widget = Paragraph::new(Line::from(vec![
        Span::styled(label, Style::default().fg(Color::White)),
    ]))
    .alignment(Alignment::Left);

    f.render_widget(label_widget, chunks[0]);

    // Progress bar (simple version)
    let percentage = progress.percentage();
    let filled = ((chunks[1].width as f64 * percentage / 100.0) as u16).min(chunks[1].width);
    
    let progress_line = if filled > 0 {
        let mut spans = vec![
            Span::styled(
                "█".repeat(filled as usize),
                Style::default().fg(Color::Cyan),
            ),
        ];
        
        if filled < chunks[1].width {
            spans.push(Span::styled(
                "░".repeat((chunks[1].width - filled) as usize),
                Style::default().fg(Color::Rgb(60, 60, 60)),
            ));
        }
        
        Line::from(spans)
    } else {
        Line::from(vec![Span::styled(
            "░".repeat(chunks[1].width as usize),
            Style::default().fg(Color::Rgb(60, 60, 60)),
        )])
    };

    let progress_widget = Paragraph::new(vec![progress_line]);
    f.render_widget(progress_widget, chunks[1]);

    // Stats
    let stats = if progress.total > 0 {
        format!(
            "{}/{} ({:.0}%) - {}",
            progress.current,
            progress.total,
            percentage,
            format_duration(progress.elapsed())
        )
    } else {
        format!("{} items - {}", progress.current, format_duration(progress.elapsed()))
    };

    let stats_widget = Paragraph::new(Line::from(vec![
        Span::styled(stats, Style::default().fg(Color::Rgb(150, 150, 150))),
    ]))
    .alignment(Alignment::Right);

    f.render_widget(stats_widget, chunks[2]);
}

pub fn draw_animated_spinner(f: &mut Frame, area: Rect, tick: usize) {
    // Animated spinner for indeterminate progress
    const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    
    let frame_idx = tick % SPINNER_FRAMES.len();
    let spinner = SPINNER_FRAMES[frame_idx];
    
    let spinner_widget = Paragraph::new(Line::from(vec![
        Span::styled(
            spinner,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("Processing...", Style::default().fg(Color::White)),
    ]))
    .alignment(Alignment::Center);
    
    f.render_widget(spinner_widget, area);
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
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}