use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::app::{App, InputMode};
use crate::models::FileType;
use crate::utils::format_bytes;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    // Create main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search bar
            Constraint::Min(10),   // Results
            Constraint::Length(3), // Status bar
        ])
        .split(area);

    // Draw search input
    draw_search_bar(f, chunks[0], app);

    // Draw search results
    draw_search_results(f, chunks[1], app);

    // Draw search status
    draw_search_status(f, chunks[2], app);
}

fn draw_search_bar(f: &mut Frame, area: Rect, app: &App) {
    let input_style = if app.input_mode == InputMode::Insert {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let input = Paragraph::new(app.search_input.as_str()).style(input_style).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(if app.input_mode == InputMode::Insert {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Gray)
            })
            .title(" Search Files ")
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(input, area);

    // Show cursor when in insert mode
    if app.input_mode == InputMode::Insert {
        f.set_cursor_position((area.x + app.search_input.len() as u16 + 1, area.y + 1));
    }
}

fn draw_search_results(f: &mut Frame, area: Rect, app: &App) {
    if app.search_results.is_empty() && !app.search_input.is_empty() {
        // No results found
        let no_results = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "No files found matching your search criteria",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
            )]),
            Line::from(""),
            Line::from(vec![Span::raw("Try adjusting your search terms")]),
        ])
        .block(
            Block::default()
                .title(" Search Results ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .alignment(Alignment::Center);

        f.render_widget(no_results, area);
        return;
    }

    if app.search_results.is_empty() {
        // Initial state - no search performed
        let help_text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "🔍 Search for files",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from("• Press Enter to start typing"),
            Line::from("• Type to search file names"),
            Line::from("• Search is case-insensitive"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Tips:",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )]),
            Line::from("  - Search updates as you type"),
            Line::from("  - Use partial names to find files"),
            Line::from("  - Press ESC to clear search"),
        ];

        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title(" Search Results ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .alignment(Alignment::Center);

        f.render_widget(help, area);
        return;
    }

    // Display search results as a table
    let header = Row::new(vec!["Name", "Type", "Size", "Modified", "Path"])
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1)
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .search_results
        .iter()
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(4) as usize)
        .map(|file| {
            Row::new(vec![
                Cell::from(file.name.clone()),
                Cell::from(file.file_type.to_string()).style(Style::default().fg(get_type_color(&file.file_type))),
                Cell::from(format_bytes(file.size)),
                Cell::from(file.modified.format("%Y-%m-%d").to_string()),
                Cell::from(file.path.parent().map(|p| p.display().to_string()).unwrap_or_default()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(10),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(35),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(format!(" Search Results ({}) ", app.search_results.len()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    f.render_widget(table, area);
}

fn draw_search_status(f: &mut Frame, area: Rect, app: &App) {
    let status_text = if app.input_mode == InputMode::Insert {
        "Press ESC to stop editing | Enter to search"
    } else if !app.search_results.is_empty() {
        "Enter: View details | ↑↓: Navigate | /: New search | ESC: Back"
    } else {
        "Press Enter to start searching | ESC to go back"
    };

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Rgb(150, 150, 150)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(60, 60, 60))),
        )
        .alignment(Alignment::Center);

    f.render_widget(status, area);
}

fn get_type_color(file_type: &FileType) -> Color {
    match file_type {
        FileType::Image => Color::Green,
        FileType::Video => Color::Blue,
        FileType::Document => Color::Yellow,
        FileType::Other => Color::Gray,
    }
}
