use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
};
use visualvault_app::App;
use visualvault_models::{DuplicateFocus, DuplicateGroup, DuplicateStats};
use visualvault_utils::format_bytes;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    // Remove the header since it's now handled by the main UI
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Stats
            Constraint::Min(10),   // Duplicate groups
            Constraint::Length(3), // Help
        ])
        .split(area);

    // Stats section
    if let Some(stats) = &app.duplicate_stats {
        draw_stats(f, chunks[0], stats);
        draw_duplicate_groups(f, chunks[1], stats, app);
    } else {
        draw_no_scan(f, chunks[0]);
    }

    // Help section
    draw_help(f, chunks[2]);
}

fn draw_stats(f: &mut Frame, area: Rect, stats: &DuplicateStats) {
    let stats_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    // Total groups
    let groups = Paragraph::new(vec![
        Line::from("Duplicate Groups"),
        Line::from(vec![Span::styled(
            stats.total_groups.to_string(),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )]),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );
    f.render_widget(groups, stats_chunks[0]);

    // Total duplicates
    let duplicates = Paragraph::new(vec![
        Line::from("Total Duplicates"),
        Line::from(vec![Span::styled(
            stats.total_duplicates.to_string(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );
    f.render_widget(duplicates, stats_chunks[1]);

    // Wasted space
    let wasted = Paragraph::new(vec![
        Line::from("Wasted Space"),
        Line::from(vec![Span::styled(
            format_bytes(stats.total_wasted_space),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )]),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );
    f.render_widget(wasted, stats_chunks[2]);
}

fn draw_duplicate_groups(f: &mut Frame, area: Rect, stats: &DuplicateStats, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Left: Group list
    let items: Vec<ListItem> = stats
        .groups
        .iter()
        .enumerate()
        .map(|(idx, group)| {
            let selected = app.selected_duplicate_group == idx;
            let style = if selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::raw(format!("{} files, ", group.files.len())),
                    Span::styled(format_bytes(group.wasted_space), Style::default().fg(Color::Red)),
                    Span::raw(" wasted"),
                ]),
                Line::from(vec![Span::styled(
                    &group.files[0].name,
                    Style::default().fg(Color::Gray),
                )]),
            ])
            .style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(if app.duplicate_focus == DuplicateFocus::GroupList {
                    " Duplicate Groups [ACTIVE] "
                } else {
                    " Duplicate Groups "
                })
                .borders(Borders::ALL)
                .border_style(if app.duplicate_focus == DuplicateFocus::GroupList {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Gray)
                }),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(list, chunks[0], &mut app.duplicate_list_state.clone());

    // Right: Selected group details
    if let Some(group) = stats.groups.get(app.selected_duplicate_group) {
        draw_group_details(f, chunks[1], group, app);
    }
}

fn draw_group_details(f: &mut Frame, area: Rect, group: &DuplicateGroup, app: &App) {
    let rows: Vec<Row> = group
        .files
        .iter()
        .enumerate()
        .map(|(idx, file)| {
            let selected = app.selected_duplicate_items.contains(&idx);
            let checkbox = if selected { "☑" } else { "☐" };
            let path = truncate_path(&file.path.display().to_string(), 40);

            // Highlight the currently focused file when in FileList focus
            let is_focused = app.duplicate_focus == DuplicateFocus::FileList && idx == app.selected_file_in_group;

            Row::new(vec![
                checkbox.to_string(),
                file.name.clone(),
                format_bytes(file.size),
                path,
            ])
            .style(if selected {
                Style::default().fg(Color::Red)
            } else if is_focused {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Percentage(30),
            Constraint::Length(10),
            Constraint::Percentage(50),
        ],
    )
    .header(Row::new(vec!["", "Name", "Size", "Path"]).style(Style::default().add_modifier(Modifier::BOLD)))
    .block(
        Block::default()
            .title(if app.duplicate_focus == DuplicateFocus::FileList {
                " Files in Group (Space to select) [ACTIVE] "
            } else {
                " Files in Group (Space to select) "
            })
            .borders(Borders::ALL)
            .border_style(if app.duplicate_focus == DuplicateFocus::FileList {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Gray)
            }),
    )
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(table, area);
}

fn truncate_path(path: &str, max_width: usize) -> String {
    if path.len() <= max_width {
        path.to_string()
    } else if max_width > 3 {
        format!("...{}", &path[path.len() - (max_width - 3)..])
    } else {
        "...".to_string()
    }
}

fn draw_no_scan(f: &mut Frame, area: Rect) {
    let message = Paragraph::new(vec![
        Line::from(""),
        Line::from("No duplicate scan performed yet."),
        Line::from(""),
        Line::from("Press 's' to start scanning for duplicates."),
    ])
    .style(Style::default().fg(Color::Gray))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    f.render_widget(message, area);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let help_text = vec![Line::from(vec![
        Span::styled("s", Style::default().fg(Color::Yellow)),
        Span::raw(" - Scan | "),
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        Span::raw(" - Navigate | "),
        Span::styled("←→", Style::default().fg(Color::Yellow)),
        Span::raw(" - Switch panes | "),
        Span::styled("Space", Style::default().fg(Color::Yellow)),
        Span::raw(" - Select | "),
        Span::styled("a", Style::default().fg(Color::Yellow)),
        Span::raw(" - Select all but first | "),
        Span::styled("d", Style::default().fg(Color::Red)),
        Span::raw(" - Delete selected | "),
        Span::styled("D", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" - DELETE ALL DUPLICATES | "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" - Back"),
    ])];

    let help = Paragraph::new(help_text).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(help, area);
}
