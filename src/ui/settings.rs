use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
};

use crate::{app::App, utils::format_bytes};
use std::path::Path;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let tabs = vec!["General", "Organization", "Performance"];
    let selected_tab = app.selected_tab;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Draw tabs
    let tabs_widget = Tabs::new(tabs)
        .select(selected_tab)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(100, 100, 100))),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    f.render_widget(tabs_widget, chunks[0]);

    // Draw content based on selected tab
    match selected_tab {
        0 => draw_general_settings(f, chunks[1], app),
        1 => draw_organization_settings(f, chunks[1], app),
        2 => draw_performance_settings(f, chunks[1], app),
        _ => {}
    }
}

#[allow(clippy::too_many_lines)]
fn draw_general_settings(f: &mut Frame, area: Rect, app: &App) {
    let settings = &app.settings_cache;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(4), // Source folder
            Constraint::Length(4), // Destination folder
            Constraint::Length(8), // Options
            Constraint::Min(0),    // Help text
        ])
        .split(area);

    // Source folder
    let is_editing_source = app.input_mode == crate::app::InputMode::Insert
        && app.editing_field == Some(crate::app::EditingField::SourceFolder);

    let source_text = if is_editing_source {
        app.input_buffer.clone()
    } else if let Some(path) = &settings.source_folder {
        truncate_path(path.display().to_string(), 60)
    } else {
        "Not configured".to_string()
    };

    let source_style = if is_editing_source {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if settings.source_folder.is_some() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };

    let source = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "📁 Source Folder",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            if is_editing_source {
                Span::styled(
                    " (editing)",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::raw("   "),
            Span::styled(&source_text, source_style),
            if is_editing_source {
                Span::styled(
                    "_",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK),
                )
            } else {
                Span::raw("")
            },
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(get_border_style(app.selected_setting == 0)),
    );
    f.render_widget(source, chunks[0]);

    // Destination folder
    let is_editing_dest = app.input_mode == crate::app::InputMode::Insert
        && app.editing_field == Some(crate::app::EditingField::DestinationFolder);

    let dest_text = if is_editing_dest {
        app.input_buffer.clone()
    } else if let Some(path) = &settings.destination_folder {
        truncate_path(path.display().to_string(), 60)
    } else {
        "Not configured".to_string()
    };

    let dest_style = if is_editing_dest {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if settings.destination_folder.is_some() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };

    let destination = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "📁 Destination Folder",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            if is_editing_dest {
                Span::styled(
                    " (editing)",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::raw("   "),
            Span::styled(&dest_text, dest_style),
            if is_editing_dest {
                Span::styled(
                    "_",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK),
                )
            } else {
                Span::raw("")
            },
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(get_border_style(app.selected_setting == 1)),
    );
    f.render_widget(destination, chunks[1]);

    // Options
    let options = [
        (
            settings.recurse_subfolders,
            "Recurse into subfolders",
            "Scan all subdirectories recursively",
        ),
        (
            settings.verbose_output,
            "Verbose output",
            "Show detailed processing information",
        ),
    ];

    let option_items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(idx, (enabled, name, desc))| {
            let is_selected = app.selected_setting == idx + 2;
            let checkbox = if *enabled { "☑" } else { "☐" };
            let style = if is_selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(format!(" {checkbox} "), Style::default().fg(Color::Green)),
                    Span::styled(*name, style),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled(*desc, Style::default().fg(Color::Rgb(150, 150, 150))),
                ]),
            ])
        })
        .collect();

    let options_list = List::new(option_items).block(
        Block::default()
            .title(" Options ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 100, 100))),
    );
    f.render_widget(options_list, chunks[2]);

    // Help text
    draw_help_text(f, chunks[3]);
}

#[allow(clippy::too_many_lines)]
fn draw_organization_settings(f: &mut Frame, area: Rect, app: &App) {
    let settings = &app.settings_cache;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(8),  // Organization mode
            Constraint::Length(10), // File type options
            Constraint::Min(0),     // Preview
        ])
        .split(area);

    // Organization mode
    let org_modes = [
        ("yearly", "Yearly", "Organize by year (2024/filename.jpg)"),
        ("monthly", "Monthly", "Organize by month (2024/03-March/filename.jpg)"),
        (
            "type",
            "By Type",
            "Organize by file type (Images/filename.jpg, Videos/filename.mp4, Documents/filename.pdf, Others/filename.ext)",
        ),
    ];

    let mode_items: Vec<ListItem> = org_modes
        .iter()
        .enumerate()
        .map(|(idx, (mode, name, desc))| {
            let is_selected = settings.organize_by == *mode;
            let is_focused = app.selected_setting == idx;
            let radio = if is_selected { "◉" } else { "○" };

            let style = if is_focused {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(format!(" {radio} "), Style::default().fg(Color::Green)),
                    Span::styled(*name, style),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled(*desc, Style::default().fg(Color::Rgb(150, 150, 150))),
                ]),
            ])
        })
        .collect();

    let org_list = List::new(mode_items).block(
        Block::default()
            .title(" Organization Mode ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 100, 100))),
    );
    f.render_widget(org_list, chunks[0]);

    // File type options
    let type_options = [
        (
            settings.separate_videos,
            "Separate video folders",
            "Put videos in dedicated Video folders",
        ),
        (
            settings.keep_original_structure,
            "Keep folder structure",
            "Preserve relative folder paths",
        ),
        (
            settings.rename_duplicates,
            "Rename duplicates",
            "Add suffix to duplicate filenames",
        ),
        (
            settings.lowercase_extensions,
            "Lowercase extensions",
            "Convert file extensions to lowercase",
        ),
    ];

    let type_items: Vec<ListItem> = type_options
        .iter()
        .enumerate()
        .map(|(idx, (enabled, name, desc))| {
            let is_selected = app.selected_setting == idx + 3;
            let checkbox = if *enabled { "☑" } else { "☐" };
            let style = if is_selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(format!(" {checkbox} "), Style::default().fg(Color::Green)),
                    Span::styled(*name, style),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled(*desc, Style::default().fg(Color::Rgb(150, 150, 150))),
                ]),
            ])
        })
        .collect();

    let type_list = List::new(type_items).block(
        Block::default()
            .title(" File Type Options ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 100, 100))),
    );
    f.render_widget(type_list, chunks[1]);

    // Preview
    draw_organization_preview(f, chunks[2], app);
}

#[allow(clippy::too_many_lines)]
fn draw_performance_settings(f: &mut Frame, area: Rect, app: &App) {
    let settings = &app.settings_cache;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(4), // Thread count
            Constraint::Length(4), // Buffer size
            Constraint::Length(8), // Performance options
            Constraint::Min(0),    // Info
        ])
        .split(area);

    // Thread count
    let is_editing_threads = app.input_mode == crate::app::InputMode::Insert
        && app.editing_field == Some(crate::app::EditingField::WorkerThreads);

    let thread_text = if is_editing_threads {
        format!("{} threads", app.input_buffer)
    } else {
        format!("{} threads", settings.worker_threads)
    };

    let thread_count = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "🔧 Worker Threads",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            if is_editing_threads {
                Span::styled(
                    " (editing)",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::raw("   "),
            Span::styled(
                thread_text,
                if is_editing_threads {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
            if is_editing_threads {
                Span::styled(
                    "_",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK),
                )
            } else {
                Span::raw("")
            },
            Span::styled(
                format!(" (Available: {})", num_cpus::get()),
                Style::default().fg(Color::Rgb(150, 150, 150)),
            ),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(get_border_style(app.selected_setting == 0)),
    );
    f.render_widget(thread_count, chunks[0]);

    // Buffer size
    let is_editing_buffer = app.input_mode == crate::app::InputMode::Insert
        && app.editing_field == Some(crate::app::EditingField::BufferSize);

    let buffer_text = if is_editing_buffer {
        format!("{} MB", app.input_buffer)
    } else {
        format_bytes(settings.buffer_size as u64)
    };

    let buffer_size = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "💾 Buffer Size",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            if is_editing_buffer {
                Span::styled(
                    " (editing)",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::raw("   "),
            Span::styled(
                buffer_text,
                if is_editing_buffer {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
            if is_editing_buffer {
                Span::styled(
                    "_",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK),
                )
            } else {
                Span::raw("")
            },
            Span::styled(" per operation", Style::default().fg(Color::Rgb(150, 150, 150))),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(get_border_style(app.selected_setting == 1)),
    );
    f.render_widget(buffer_size, chunks[1]);

    // Performance options
    let perf_options = [
        (
            settings.enable_cache,
            "Enable file cache",
            "Cache file metadata for faster operations",
        ),
        (
            settings.parallel_processing,
            "Parallel processing",
            "Process multiple files simultaneously",
        ),
        (
            settings.skip_hidden_files,
            "Skip hidden files",
            "Ignore hidden files and directories",
        ),
        (
            settings.optimize_for_ssd,
            "Optimize for SSD",
            "Use settings optimized for solid-state drives",
        ),
    ];

    let perf_items: Vec<ListItem> = perf_options
        .iter()
        .enumerate()
        .map(|(idx, (enabled, name, desc))| {
            let is_selected = app.selected_setting == idx + 2;
            let checkbox = if *enabled { "☑" } else { "☐" };
            let style = if is_selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(format!(" {checkbox} "), Style::default().fg(Color::Green)),
                    Span::styled(*name, style),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled(*desc, Style::default().fg(Color::Rgb(150, 150, 150))),
                ]),
            ])
        })
        .collect();

    let perf_list = List::new(perf_items).block(
        Block::default()
            .title(" Performance Options ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 100, 100))),
    );
    f.render_widget(perf_list, chunks[2]);

    // Info
    draw_performance_info(f, chunks[3]);
}

fn draw_organization_preview(f: &mut Frame, area: Rect, app: &App) {
    let settings = &app.settings_cache;
    let preview_examples = vec![
        ("Image file", "IMG_20240315_143022.jpg", "image"),
        ("Video file", "vacation_video.mp4", "video"),
        ("Document", "report_2024.pdf", "document"),
    ];

    let mut preview_lines = vec![
        Line::from(vec![Span::styled(
            "📋 Preview",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
    ];

    for (desc, filename, file_type) in preview_examples {
        preview_lines.push(Line::from(vec![
            Span::styled(format!("{desc}: "), Style::default().fg(Color::Rgb(150, 150, 150))),
            Span::styled(filename, Style::default().fg(Color::Yellow)),
        ]));
        preview_lines.push(Line::from(vec![
            Span::raw("  → "),
            Span::styled(
                get_preview_path(settings, filename, file_type),
                Style::default().fg(Color::Green),
            ),
        ]));
        preview_lines.push(Line::from(""));
    }

    let preview = Paragraph::new(preview_lines).block(
        Block::default()
            .title(" Organization Preview ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 100, 100))),
    );
    f.render_widget(preview, area);
}

fn draw_help_text(f: &mut Frame, area: Rect) {
    let help_lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Keyboard shortcuts:",
            Style::default().fg(Color::Cyan),
        )]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" - Edit selected setting"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Space", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" - Toggle checkbox/radio button"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("↑/↓", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" - Navigate between settings"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("S", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" - Save settings"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("R", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" - Reset to defaults"),
        ]),
    ];

    let help = Paragraph::new(help_lines).style(Style::default().fg(Color::Rgb(200, 200, 200)));
    f.render_widget(help, area);
}

fn draw_performance_info(f: &mut Frame, area: Rect) {
    let info_lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "ℹ️  Performance Tips:",
            Style::default().fg(Color::Cyan),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("• Use "),
            Span::styled("worker threads = CPU cores", Style::default().fg(Color::Yellow)),
            Span::raw(" for balanced performance"),
        ]),
        Line::from(vec![
            Span::raw("• Larger "),
            Span::styled("buffer sizes", Style::default().fg(Color::Yellow)),
            Span::raw(" improve throughput but use more memory"),
        ]),
        Line::from(vec![
            Span::raw("• "),
            Span::styled("SSD optimization", Style::default().fg(Color::Yellow)),
            Span::raw(" reduces write amplification"),
        ]),
        Line::from(vec![
            Span::raw("• "),
            Span::styled("File cache", Style::default().fg(Color::Yellow)),
            Span::raw(" speeds up repeated scans"),
        ]),
    ];

    let info = Paragraph::new(info_lines).style(Style::default().fg(Color::Rgb(200, 200, 200)));
    f.render_widget(info, area);
}

fn get_preview_path(settings: &crate::config::Settings, filename: &str, file_type: &str) -> String {
    let base = if let Some(dest) = &settings.destination_folder {
        dest.display().to_string()
    } else {
        "/destination".to_string()
    };

    let path = match settings.organize_by.as_str() {
        "yearly" => format!("{base}/2024/{filename}"),
        "monthly" => format!("{base}/2024/03-March/{filename}"),
        "daily" => format!("{base}/2024/03/15/{filename}"),
        "type" => format!("{}/{}/{}", base, capitalize_type(file_type), filename),
        "type-date" => format!("{}/{}/2024/{}", base, capitalize_type(file_type), filename),
        _ => format!("{base}/{filename}"),
    };

    if settings.separate_videos && file_type == "video" {
        path.replace(&format!("/{}/", capitalize_type(file_type)), "/Videos/")
    } else {
        path
    }
}

fn capitalize_type(file_type: &str) -> String {
    match file_type {
        "image" => "Images",
        "video" => "Videos",
        "audio" => "Audio",
        "document" => "Documents",
        _ => "Other",
    }
    .to_string()
}

fn get_border_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Rgb(100, 100, 100))
    }
}

fn truncate_path(path: String, max_len: usize) -> String {
    if path.len() <= max_len {
        return path;
    }

    let home = dirs::home_dir();
    let mut truncated = path.clone();

    // Replace home directory with ~
    if let Some(home_path) = home {
        if let Ok(relative) = Path::new(&path).strip_prefix(&home_path) {
            truncated = format!("~/{}", relative.display());
        }
    }

    // If still too long, truncate from the middle
    if truncated.len() > max_len {
        let start_len = max_len / 2 - 2;
        let end_len = max_len - start_len - 3;
        format!(
            "{}...{}",
            &truncated[..start_len],
            &truncated[truncated.len() - end_len..]
        )
    } else {
        truncated
    }
}
