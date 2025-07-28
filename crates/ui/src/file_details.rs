use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table},
};
use tracing::info;
use visualvault_models::{FileType, MediaFile, MediaMetadata};
use visualvault_utils::format_bytes;

#[allow(clippy::too_many_lines)]
pub fn draw_modal(f: &mut Frame, file: &MediaFile) {
    let area = centered_rect(70, 80, f.area());

    // Clear the area first
    f.render_widget(Clear, area);

    // Create the main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(10), // Basic info
            Constraint::Length(8),  // File system info
            Constraint::Min(5),     // Metadata (if available)
            Constraint::Length(3),  // Help text
        ])
        .split(area);

    // Main block
    let block = Block::default()
        .title(" File Details ")
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Rgb(20, 20, 30)));

    f.render_widget(block, area);

    // Title with file icon
    let icon = match file.file_type {
        FileType::Image => "🖼️",
        FileType::Video => "🎬",
        FileType::Document => "📄",
        FileType::Other => "📎",
    };

    let title = Paragraph::new(vec![Line::from(vec![
        Span::raw(format!("{icon} ")),
        Span::styled(
            &*file.name,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
    ])])
    .alignment(Alignment::Center);

    f.render_widget(title, chunks[0]);

    // Basic information table
    let file_type = file.file_type.to_string();
    let size = format_bytes(file.size);
    let created = file.created.format("%Y-%m-%d %H:%M:%S").to_string();
    let modified = file.modified.format("%Y-%m-%d %H:%M:%S").to_string();
    let basic_info = vec![
        Row::new(vec!["Type", &file_type]),
        Row::new(vec!["Size", &size]),
        Row::new(vec!["Extension", &file.extension]),
        Row::new(vec!["Created", &created]),
        Row::new(vec!["Modified", &modified]),
    ];

    let basic_table = Table::new(basic_info, [Constraint::Percentage(30), Constraint::Percentage(70)])
        .block(
            Block::default()
                .title(" Basic Information ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .row_highlight_style(Style::default().fg(Color::Yellow))
        .column_spacing(2);

    f.render_widget(basic_table, chunks[1]);

    // File system information
    let full_path = file.path.display().to_string();
    let parent = file
        .path
        .parent()
        .map_or_else(|| "N/A".to_string(), |p| p.display().to_string());
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(&file.path).ok().map_or_else(
            || "Unknown".to_string(),
            |m| format!("{:o}", m.permissions().mode() & 0o777),
        )
    };

    #[cfg(not(unix))]
    let permissions = {
        // On Windows, check if file is read-only
        std::fs::metadata(&file.path)
            .ok()
            .map(|m| {
                if m.permissions().readonly() {
                    "Read-only".to_string()
                } else {
                    "Read/Write".to_string()
                }
            })
            .unwrap_or_else(|| "Unknown".to_string())
    };

    let fs_info = vec![
        Row::new(vec!["Full Path", &full_path]),
        Row::new(vec!["Directory", &parent]),
        Row::new(vec!["Permissions", &permissions]),
    ];

    let fs_table = Table::new(fs_info, [Constraint::Percentage(30), Constraint::Percentage(70)])
        .block(
            Block::default()
                .title(" File System ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .column_spacing(2);

    f.render_widget(fs_table, chunks[2]);

    info!("Metadata section (for images): {}", &file.metadata.is_some());

    // Metadata section (for images)
    if file.file_type == FileType::Image {
        if let Some(MediaMetadata::Image(metadata)) = &file.metadata {
            let metadata_text = vec![
                Line::from(format!("Width: {} px", metadata.width)),
                Line::from(format!("Height: {} px", metadata.height)),
                Line::from(format!("Format: {}", metadata.format)),
                Line::from(format!("Color Type: {}", metadata.color_type)),
            ];

            let metadata_paragraph = Paragraph::new(metadata_text)
                .block(
                    Block::default()
                        .title(" Image Metadata ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Gray)),
                )
                .alignment(Alignment::Left);

            f.render_widget(metadata_paragraph, chunks[3]);
        } else {
            let no_metadata = Paragraph::new("No image metadata available")
                .block(
                    Block::default()
                        .title(" Image Metadata ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Gray)),
                )
                .alignment(Alignment::Center);

            f.render_widget(no_metadata, chunks[3]);
        }
    } else {
        // For non-images, show file content preview or other relevant info
        let preview = Paragraph::new("No additional metadata available for this file type")
            .block(
                Block::default()
                    .title(" Additional Information ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .alignment(Alignment::Center);

        f.render_widget(preview, chunks[3]);
    }

    // Help text
    let help = Paragraph::new(vec![Line::from(vec![
        Span::styled("ESC", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" or "),
        Span::styled("q", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" to close"),
    ])])
    .alignment(Alignment::Center)
    .style(Style::default().fg(Color::Rgb(150, 150, 150)));

    f.render_widget(help, chunks[4]);
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
