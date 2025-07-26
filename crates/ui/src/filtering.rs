use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
};

use visualvault_app::App;
use visualvault_models::FilterFocus;
use visualvault_models::InputMode;
use visualvault_utils::format_bytes;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Tabs
            Constraint::Min(10),   // Content
            Constraint::Length(4), // Help
        ])
        .split(area);

    // Header
    draw_header(f, chunks[0], app);

    // Tabs
    draw_tabs(f, chunks[1], app);

    // Content based on selected tab
    match app.filter_tab {
        0 => draw_date_filters(f, chunks[2], app),
        1 => draw_size_filters(f, chunks[2], app),
        2 => draw_type_filters(f, chunks[2], app),
        3 => draw_regex_filters(f, chunks[2], app),
        _ => {}
    }

    // Help
    draw_help(f, chunks[3], app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let active_count = app.filter_set.active_filter_count();
    let status = if app.filter_set.is_active {
        format!(" ({active_count} active filters)")
    } else {
        " (inactive)".to_string()
    };

    let header = Paragraph::new(format!("🔍 Advanced Filters{status}"))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
    f.render_widget(header, area);
}

fn draw_tabs(f: &mut Frame, area: Rect, app: &App) {
    let titles = vec!["Date Range", "Size", "Media Type", "Regex"];
    let tabs = Tabs::new(titles)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .select(app.filter_tab)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(tabs, area);
}

fn draw_date_filters(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(area);

    // List of date ranges
    let items: Vec<ListItem> = if app.filter_set.date_ranges.is_empty() {
        vec![ListItem::new(" No date filters set. Press 'a' to add one.").style(Style::default().fg(Color::DarkGray))]
    } else {
        app.filter_set
            .date_ranges
            .iter()
            .enumerate()
            .map(|(idx, range)| {
                let from_str = range
                    .from
                    .map_or("Any".to_string(), |d| d.format("%Y-%m-%d").to_string());
                let to_str = range.to.map_or("Any".to_string(), |d| d.format("%Y-%m-%d").to_string());
                let selected = app.filter_focus == FilterFocus::DateRange && app.selected_filter_index == idx;

                ListItem::new(vec![Line::from(vec![
                    Span::raw(&range.name),
                    Span::styled(" | ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("{from_str} → {to_str}")),
                ])])
                .style(if selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                })
            })
            .collect()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Date Ranges (a: add, d: delete, e: edit) ")
                .borders(Borders::ALL)
                .border_style(if app.filter_focus == FilterFocus::DateRange {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Gray)
                }),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(list, chunks[0]);

    // Input area for new date range
    if app.input_mode == InputMode::Editing && app.filter_focus == FilterFocus::DateRange {
        draw_date_input(f, chunks[1], app);
    }
}

fn draw_date_input(f: &mut Frame, area: Rect, app: &App) {
    let input = Paragraph::new(app.filter_input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .title(" Enter date range (format: YYYY-MM-DD to YYYY-MM-DD or 'last 7 days') ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );
    f.render_widget(input, area);
}

fn draw_size_filters(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(area);

    // List of size ranges
    let items: Vec<ListItem> = app
        .filter_set
        .size_ranges
        .iter()
        .enumerate()
        .map(|(idx, range)| {
            let min_str = range.min_bytes.map_or("0".to_string(), format_bytes);
            let max_str = range.max_bytes.map_or("∞".to_string(), format_bytes);
            let selected = app.filter_focus == FilterFocus::SizeRange && app.selected_filter_index == idx;

            ListItem::new(vec![Line::from(vec![
                Span::raw(&range.name),
                Span::styled(" | ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{min_str} → {max_str}")),
            ])])
            .style(if selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Size Ranges (a: add, d: delete) ")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(list, chunks[0]);

    // Input area for new size range
    if app.input_mode == InputMode::Editing && app.filter_focus == FilterFocus::SizeRange {
        draw_size_input(f, chunks[1], app);
    }
}

fn draw_size_input(f: &mut Frame, area: Rect, app: &App) {
    let input = Paragraph::new(app.filter_input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .title(" Enter size range (e.g., '10MB-100MB', '>50MB', '<1GB') ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );
    f.render_widget(input, area);
}

fn draw_type_filters(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .filter_set
        .media_types
        .iter()
        .enumerate()
        .map(|(idx, mt)| {
            let checkbox = if mt.enabled { "☑" } else { "☐" };
            let selected = app.filter_focus == FilterFocus::MediaType && app.selected_filter_index == idx;
            let extensions = mt.extensions.join(", ");

            ListItem::new(vec![
                Line::from(vec![
                    Span::raw(format!("{checkbox} ")),
                    Span::styled(
                        mt.media_type.to_string(),
                        if mt.enabled {
                            Style::default().fg(Color::Green)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                ]),
                Line::from(vec![
                    Span::raw("   "),
                    Span::styled(extensions, Style::default().fg(Color::DarkGray)),
                ]),
            ])
            .style(if selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Media Types (space: toggle, e: edit extensions) ")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(list, area);
}

fn draw_regex_filters(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(area);

    // List of regex patterns
    let items: Vec<ListItem> = app
        .filter_set
        .regex_patterns
        .iter()
        .enumerate()
        .map(|(idx, pattern)| {
            let checkbox = if pattern.enabled { "☑" } else { "☐" };
            let selected = app.filter_focus == FilterFocus::RegexPattern && app.selected_filter_index == idx;
            let case = if pattern.case_sensitive { "CS" } else { "CI" };

            ListItem::new(vec![Line::from(vec![
                Span::raw(format!("{checkbox} ")),
                Span::styled(&pattern.pattern, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!(" [{}] [{}]", pattern.target, case),
                    Style::default().fg(Color::DarkGray),
                ),
            ])])
            .style(if selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Regex Patterns (a: add, d: delete, space: toggle) ")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(list, chunks[0]);

    // Input area for new regex
    if app.input_mode == InputMode::Editing && app.filter_focus == FilterFocus::RegexPattern {
        draw_regex_input(f, chunks[1], app);
    }
}

fn draw_regex_input(f: &mut Frame, area: Rect, app: &App) {
    let input = Paragraph::new(app.filter_input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .title(" Enter regex pattern (e.g., '.*\\.tmp$' for temp files) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );
    f.render_widget(input, area);
}

fn draw_help(f: &mut Frame, area: Rect, app: &App) {
    let help_text = if app.input_mode == InputMode::Editing {
        vec![Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Green)),
            Span::raw(" - Save | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" - Cancel"),
        ])]
    } else {
        vec![Line::from(vec![
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" - Next tab | "),
            Span::styled("Shift+Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" - Previous tab | "),
            Span::styled("↑↓", Style::default().fg(Color::Yellow)),
            Span::raw(" - Navigate | "),
            Span::styled("a", Style::default().fg(Color::Green)),
            Span::raw(" - Add | "),
            Span::styled("d", Style::default().fg(Color::Red)),
            Span::raw(" - Delete | "),
            Span::styled("c", Style::default().fg(Color::Yellow)),
            Span::raw(" - Clear all | "),
            Span::styled("t", Style::default().fg(Color::Yellow)),
            Span::raw(" - Toggle filters | "),
            Span::styled("Enter", Style::default().fg(Color::Green)),
            Span::raw(" - Apply | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" - Back"),
        ])]
    };

    let help = Paragraph::new(help_text).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(help, area);
}
