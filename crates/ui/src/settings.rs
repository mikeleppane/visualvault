use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, List, ListItem, Paragraph, Tabs},
};
use visualvault_config::Settings;

use std::path::Path;

use visualvault_app::App;
use visualvault_models::EditingField;
use visualvault_models::InputMode;
use visualvault_utils::format_bytes;

// Beautiful color palette (matching dashboard)
const ACCENT_COLOR: Color = Color::Rgb(139, 233, 253); // Cyan
const SUCCESS_COLOR: Color = Color::Rgb(80, 250, 123); // Green
const WARNING_COLOR: Color = Color::Rgb(255, 184, 108); // Orange
const ERROR_COLOR: Color = Color::Rgb(255, 85, 85); // Red
const MUTED_COLOR: Color = Color::Rgb(98, 114, 164); // Gray
const BACKGROUND_ALT: Color = Color::Rgb(30, 30, 46); // Dark background
const HIGHLIGHT_BG: Color = Color::Rgb(69, 71, 90); // Selection background

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    // Add a subtle background
    let background = Block::default().style(Style::default().bg(Color::Rgb(24, 24, 37)));
    f.render_widget(background, area);

    let tabs = vec!["⚙️  General", "📁 Organization", "🚀 Performance"];
    let selected_tab = app.selected_tab;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Draw enhanced tabs
    let tabs_widget = Tabs::new(tabs)
        .select(selected_tab)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED_COLOR)),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(ACCENT_COLOR)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider(symbols::DOT);

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
#[allow(clippy::cognitive_complexity)]
fn draw_general_settings(f: &mut Frame, area: Rect, app: &App) {
    let settings = &app.settings_cache;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(5),  // Source folder
            Constraint::Length(5),  // Destination folder
            Constraint::Length(11), // Options
            Constraint::Min(0),     // Help text
        ])
        .split(area);

    // Source folder
    let is_editing_source =
        app.input_mode == InputMode::Insert && app.editing_field == Some(EditingField::SourceFolder);

    let source_text = if is_editing_source {
        app.input_buffer.clone()
    } else if let Some(path) = &settings.source_folder {
        truncate_path(path.display().to_string(), 60)
    } else {
        "Not configured".to_string()
    };

    let source_style = if is_editing_source {
        Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)
    } else if settings.source_folder.is_some() {
        Style::default().fg(SUCCESS_COLOR)
    } else {
        Style::default().fg(WARNING_COLOR)
    };

    let source_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(get_enhanced_border_style(app.selected_setting == 0, is_editing_source))
        .style(Style::default().bg(if app.selected_setting == 0 {
            BACKGROUND_ALT
        } else {
            Color::default()
        }));

    let source_inner = source_block.inner(chunks[0]);
    f.render_widget(source_block, chunks[0]);

    let source = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "📥 Source Folder",
                Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD),
            ),
            if is_editing_source {
                Span::styled(
                    " (editing)",
                    Style::default()
                        .fg(WARNING_COLOR)
                        .add_modifier(Modifier::ITALIC | Modifier::SLOW_BLINK),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(&source_text, source_style),
            if is_editing_source {
                Span::styled(
                    "│",
                    Style::default().fg(WARNING_COLOR).add_modifier(Modifier::SLOW_BLINK),
                )
            } else {
                Span::raw("")
            },
        ]),
    ]);
    f.render_widget(source, source_inner);

    // Destination folder
    let is_editing_dest =
        app.input_mode == InputMode::Insert && app.editing_field == Some(EditingField::DestinationFolder);

    let dest_text = if is_editing_dest {
        app.input_buffer.clone()
    } else if let Some(path) = &settings.destination_folder {
        truncate_path(path.display().to_string(), 60)
    } else {
        "Not configured".to_string()
    };

    let dest_style = if is_editing_dest {
        Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)
    } else if settings.destination_folder.is_some() {
        Style::default().fg(SUCCESS_COLOR)
    } else {
        Style::default().fg(WARNING_COLOR)
    };

    let dest_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(get_enhanced_border_style(app.selected_setting == 1, is_editing_dest))
        .style(Style::default().bg(if app.selected_setting == 1 {
            BACKGROUND_ALT
        } else {
            Color::default()
        }));

    let dest_inner = dest_block.inner(chunks[1]);
    f.render_widget(dest_block, chunks[1]);

    let destination = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "📤 Destination Folder",
                Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD),
            ),
            if is_editing_dest {
                Span::styled(
                    " (editing)",
                    Style::default()
                        .fg(WARNING_COLOR)
                        .add_modifier(Modifier::ITALIC | Modifier::SLOW_BLINK),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(&dest_text, dest_style),
            if is_editing_dest {
                Span::styled(
                    "│",
                    Style::default().fg(WARNING_COLOR).add_modifier(Modifier::SLOW_BLINK),
                )
            } else {
                Span::raw("")
            },
        ]),
    ]);
    f.render_widget(destination, dest_inner);

    // Options with enhanced styling
    let options = [
        (
            settings.recurse_subfolders,
            "🔄 Recurse into subfolders",
            "Scan all subdirectories recursively",
        ),
        (
            settings.verbose_output,
            "📝 Verbose output",
            "Show detailed processing information",
        ),
        (
            settings.undo_enabled,
            "↩️  Enable undo history",
            "Keep a history of changes for undo operations",
        ),
    ];

    let option_items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(idx, (enabled, name, desc))| {
            let is_selected = app.selected_setting == idx + 2;
            let checkbox = if *enabled {
                Span::styled("✅", Style::default().fg(SUCCESS_COLOR))
            } else {
                Span::styled("⬜", Style::default().fg(MUTED_COLOR))
            };

            let name_style = if is_selected {
                Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD)
            } else if *enabled {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Rgb(180, 180, 180))
            };

            let bg_style = if is_selected {
                Style::default().bg(HIGHLIGHT_BG)
            } else {
                Style::default()
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::raw(" "),
                    checkbox,
                    Span::raw("  "),
                    Span::styled(*name, name_style),
                ])
                .style(bg_style),
                Line::from(vec![
                    Span::raw("      "),
                    Span::styled(*desc, Style::default().fg(MUTED_COLOR).add_modifier(Modifier::ITALIC)),
                ])
                .style(bg_style),
                Line::from("").style(bg_style), // Add spacing
            ])
        })
        .collect();

    let options_list = List::new(option_items).block(
        Block::default()
            .title(" ⚙️  Options ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED_COLOR))
            .style(Style::default().bg(BACKGROUND_ALT)),
    );
    f.render_widget(options_list, chunks[2]);

    // Enhanced help text
    draw_enhanced_help_text(f, chunks[3]);
}

#[allow(clippy::too_many_lines)]
fn draw_organization_settings(f: &mut Frame, area: Rect, app: &App) {
    let settings = &app.settings_cache;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(11), // Organization mode
            Constraint::Length(13), // File type options
            Constraint::Min(0),     // Preview
        ])
        .split(area);

    // Organization mode with enhanced styling
    let org_modes = [
        ("yearly", "📅 Yearly", "Organize by year (2024/filename.jpg)"),
        (
            "monthly",
            "📆 Monthly",
            "Organize by month (2024/03-March/filename.jpg)",
        ),
        ("type", "🗂️  By Type", "Organize by file type (Images/filename.jpg)"),
    ];

    let mode_items: Vec<ListItem> = org_modes
        .iter()
        .enumerate()
        .map(|(idx, (mode, name, desc))| {
            let is_selected = settings.organize_by == *mode;
            let is_focused = app.selected_setting == idx;

            let radio = if is_selected {
                Span::styled("🔘", Style::default().fg(SUCCESS_COLOR))
            } else {
                Span::styled("⚪", Style::default().fg(MUTED_COLOR))
            };

            let name_style = if is_focused {
                Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let bg_style = if is_focused {
                Style::default().bg(HIGHLIGHT_BG)
            } else {
                Style::default()
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::raw(" "),
                    radio,
                    Span::raw("  "),
                    Span::styled(*name, name_style),
                ])
                .style(bg_style),
                Line::from(vec![
                    Span::raw("      "),
                    Span::styled(*desc, Style::default().fg(MUTED_COLOR).add_modifier(Modifier::ITALIC)),
                ])
                .style(bg_style),
                Line::from("").style(bg_style),
            ])
        })
        .collect();

    let org_list = List::new(mode_items).block(
        Block::default()
            .title(" 🗂️  Organization Mode ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED_COLOR))
            .style(Style::default().bg(BACKGROUND_ALT)),
    );
    f.render_widget(org_list, chunks[0]);

    // File type options with icons
    let type_options = [
        (
            settings.separate_videos,
            "🎬 Separate video folders",
            "Put videos in dedicated Video folders",
        ),
        (
            settings.keep_original_structure,
            "🏗️  Keep folder structure",
            "Preserve relative folder paths",
        ),
        (
            settings.rename_duplicates,
            "🔀 Rename duplicates",
            "Add suffix to duplicate filenames",
        ),
        (
            settings.lowercase_extensions,
            "🔡 Lowercase extensions",
            "Convert file extensions to lowercase",
        ),
    ];

    let type_items: Vec<ListItem> = type_options
        .iter()
        .enumerate()
        .map(|(idx, (enabled, name, desc))| {
            let is_selected = app.selected_setting == idx + 3;
            let checkbox = if *enabled {
                Span::styled("✅", Style::default().fg(SUCCESS_COLOR))
            } else {
                Span::styled("⬜", Style::default().fg(MUTED_COLOR))
            };

            let name_style = if is_selected {
                Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD)
            } else if *enabled {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Rgb(180, 180, 180))
            };

            let bg_style = if is_selected {
                Style::default().bg(HIGHLIGHT_BG)
            } else {
                Style::default()
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::raw(" "),
                    checkbox,
                    Span::raw("  "),
                    Span::styled(*name, name_style),
                ])
                .style(bg_style),
                Line::from(vec![
                    Span::raw("      "),
                    Span::styled(*desc, Style::default().fg(MUTED_COLOR).add_modifier(Modifier::ITALIC)),
                ])
                .style(bg_style),
                Line::from("").style(bg_style),
            ])
        })
        .collect();

    let type_list = List::new(type_items).block(
        Block::default()
            .title(" 📁 File Type Options ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED_COLOR))
            .style(Style::default().bg(BACKGROUND_ALT)),
    );
    f.render_widget(type_list, chunks[1]);

    // Enhanced preview
    draw_enhanced_organization_preview(f, chunks[2], app);
}

#[allow(clippy::too_many_lines)]
fn draw_performance_settings(f: &mut Frame, area: Rect, app: &App) {
    let settings = &app.settings_cache;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(5),  // Thread count
            Constraint::Length(5),  // Buffer size
            Constraint::Length(13), // Performance options
            Constraint::Min(0),     // Info
        ])
        .split(area);

    // Thread count with visual gauge
    let is_editing_threads =
        app.input_mode == InputMode::Insert && app.editing_field == Some(EditingField::WorkerThreads);

    let thread_text = if is_editing_threads {
        app.input_buffer.clone()
    } else {
        settings.worker_threads.to_string()
    };

    let max_threads = num_cpus::get();
    let thread_ratio = settings.worker_threads as f64 / max_threads as f64;

    let thread_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(get_enhanced_border_style(app.selected_setting == 0, is_editing_threads))
        .style(Style::default().bg(if app.selected_setting == 0 {
            BACKGROUND_ALT
        } else {
            Color::default()
        }));

    let thread_inner = thread_block.inner(chunks[0]);
    f.render_widget(thread_block, chunks[0]);

    // Split inner area for label and gauge
    let thread_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1)])
        .split(thread_inner);

    let thread_count = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            "⚙️  Worker Threads",
            Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            thread_text,
            if is_editing_threads {
                Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD)
            },
        ),
        if is_editing_threads {
            Span::styled(
                "│",
                Style::default().fg(WARNING_COLOR).add_modifier(Modifier::SLOW_BLINK),
            )
        } else {
            Span::raw("")
        },
        Span::styled(format!(" / {max_threads} available"), Style::default().fg(MUTED_COLOR)),
    ])]);
    f.render_widget(thread_count, thread_chunks[0]);

    // Thread usage gauge
    let gauge_color = if thread_ratio > 0.8 {
        WARNING_COLOR
    } else {
        SUCCESS_COLOR
    };

    let thread_gauge = Gauge::default()
        .gauge_style(Style::default().fg(gauge_color).bg(Color::Rgb(40, 40, 55)))
        .ratio(thread_ratio.min(1.0))
        .label("");
    f.render_widget(thread_gauge, thread_chunks[1]);

    // Buffer size with visual representation
    let is_editing_buffer = app.input_mode == InputMode::Insert && app.editing_field == Some(EditingField::BufferSize);

    let buffer_text = if is_editing_buffer {
        app.input_buffer.clone()
    } else {
        format_bytes(settings.buffer_size as u64)
    };

    let buffer_ratio = (settings.buffer_size as f64 / (512f64 * 1024f64 * 1024f64)).min(1.0); // Max 512MB

    let buffer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(get_enhanced_border_style(app.selected_setting == 1, is_editing_buffer))
        .style(Style::default().bg(if app.selected_setting == 1 {
            BACKGROUND_ALT
        } else {
            Color::default()
        }));

    let buffer_inner = buffer_block.inner(chunks[1]);
    f.render_widget(buffer_block, chunks[1]);

    // Split inner area for label and gauge
    let buffer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1)])
        .split(buffer_inner);

    let buffer_size = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            "💾 Buffer Size",
            Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            buffer_text,
            if is_editing_buffer {
                Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD)
            },
        ),
        if is_editing_buffer {
            Span::styled(
                "│",
                Style::default().fg(WARNING_COLOR).add_modifier(Modifier::SLOW_BLINK),
            )
        } else {
            Span::raw("")
        },
        Span::styled(" per operation", Style::default().fg(MUTED_COLOR)),
    ])]);
    f.render_widget(buffer_size, buffer_chunks[0]);

    // Buffer usage gauge
    let buffer_gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Magenta).bg(Color::Rgb(40, 40, 55)))
        .ratio(buffer_ratio)
        .label("");
    f.render_widget(buffer_gauge, buffer_chunks[1]);

    // Performance options with enhanced icons
    let perf_options = [
        (
            settings.enable_cache,
            "🗄️  Enable file cache",
            "Cache file metadata for faster operations",
        ),
        (
            settings.parallel_processing,
            "⚡ Parallel processing",
            "Process multiple files simultaneously",
        ),
        (
            settings.skip_hidden_files,
            "👻 Skip hidden files",
            "Ignore hidden files and directories",
        ),
        (
            settings.optimize_for_ssd,
            "💿 Optimize for SSD",
            "Use settings optimized for solid-state drives",
        ),
    ];

    let perf_items: Vec<ListItem> = perf_options
        .iter()
        .enumerate()
        .map(|(idx, (enabled, name, desc))| {
            let is_selected = app.selected_setting == idx + 2;
            let checkbox = if *enabled {
                Span::styled("✅", Style::default().fg(SUCCESS_COLOR))
            } else {
                Span::styled("⬜", Style::default().fg(MUTED_COLOR))
            };

            let name_style = if is_selected {
                Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD)
            } else if *enabled {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Rgb(180, 180, 180))
            };

            let bg_style = if is_selected {
                Style::default().bg(HIGHLIGHT_BG)
            } else {
                Style::default()
            };

            ListItem::new(vec![
                Line::from(vec![
                    Span::raw(" "),
                    checkbox,
                    Span::raw("  "),
                    Span::styled(*name, name_style),
                ])
                .style(bg_style),
                Line::from(vec![
                    Span::raw("      "),
                    Span::styled(*desc, Style::default().fg(MUTED_COLOR).add_modifier(Modifier::ITALIC)),
                ])
                .style(bg_style),
                Line::from("").style(bg_style),
            ])
        })
        .collect();

    let perf_list = List::new(perf_items).block(
        Block::default()
            .title(" 🚀 Performance Options ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED_COLOR))
            .style(Style::default().bg(BACKGROUND_ALT)),
    );
    f.render_widget(perf_list, chunks[2]);

    // Enhanced performance info
    draw_enhanced_performance_info(f, chunks[3]);
}

fn draw_enhanced_organization_preview(f: &mut Frame, area: Rect, app: &App) {
    let settings = &app.settings_cache;
    let preview_examples = vec![
        ("🖼️  Image", "IMG_20240315_143022.jpg", "image"),
        ("🎬 Video", "vacation_video.mp4", "video"),
        ("📄 Document", "report_2024.pdf", "document"),
    ];

    let mut preview_lines = vec![];

    for (desc, filename, file_type) in preview_examples {
        preview_lines.push(Line::from(vec![
            Span::styled(desc, Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD)),
            Span::raw(": "),
            Span::styled(filename, Style::default().fg(Color::White)),
        ]));
        preview_lines.push(Line::from(vec![
            Span::raw("  ➜ "),
            Span::styled(
                get_preview_path(settings, filename, file_type),
                Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::ITALIC),
            ),
        ]));
        preview_lines.push(Line::from(""));
    }

    let preview = Paragraph::new(preview_lines)
        .block(
            Block::default()
                .title(" 👁️  Organization Preview ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED_COLOR))
                .style(Style::default().bg(BACKGROUND_ALT)),
        )
        .alignment(Alignment::Left);
    f.render_widget(preview, area);
}

fn draw_enhanced_help_text(f: &mut Frame, area: Rect) {
    let help_lines = vec![
        Line::from(vec![Span::styled(
            "⌨️  Keyboard Shortcuts",
            Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Enter", Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled("Edit selected setting", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Space", Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled("Toggle checkbox/radio", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("↑/↓", Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)),
            Span::raw("   "),
            Span::styled("Navigate settings", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("S", Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD)),
            Span::raw("     "),
            Span::styled("Save settings", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("R", Style::default().fg(ERROR_COLOR).add_modifier(Modifier::BOLD)),
            Span::raw("     "),
            Span::styled("Reset to defaults", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Esc", Style::default().fg(MUTED_COLOR).add_modifier(Modifier::BOLD)),
            Span::raw("   "),
            Span::styled("Cancel editing", Style::default().fg(Color::White)),
        ]),
    ];

    let help = Paragraph::new(help_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED_COLOR))
            .style(Style::default().bg(BACKGROUND_ALT)),
    );
    f.render_widget(help, area);
}

fn draw_enhanced_performance_info(f: &mut Frame, area: Rect) {
    let info_lines = vec![
        Line::from(vec![Span::styled(
            "💡 Performance Tips",
            Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("• Use "),
            Span::styled("worker threads = CPU cores", Style::default().fg(WARNING_COLOR)),
            Span::raw(" for balanced performance"),
        ]),
        Line::from(vec![
            Span::raw("• Larger "),
            Span::styled("buffer sizes", Style::default().fg(WARNING_COLOR)),
            Span::raw(" improve throughput but use more RAM"),
        ]),
        Line::from(vec![
            Span::raw("• "),
            Span::styled("SSD optimization", Style::default().fg(WARNING_COLOR)),
            Span::raw(" reduces write amplification"),
        ]),
        Line::from(vec![
            Span::raw("• "),
            Span::styled("File cache", Style::default().fg(WARNING_COLOR)),
            Span::raw(" speeds up repeated scans significantly"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "⚠️  Note: ",
                Style::default().fg(ERROR_COLOR).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "High thread counts may increase CPU usage",
                Style::default().fg(MUTED_COLOR).add_modifier(Modifier::ITALIC),
            ),
        ]),
    ];

    let info = Paragraph::new(info_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED_COLOR))
            .style(Style::default().bg(BACKGROUND_ALT)),
    );
    f.render_widget(info, area);
}

fn get_preview_path(settings: &Settings, filename: &str, file_type: &str) -> String {
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

fn get_enhanced_border_style(is_selected: bool, is_editing: bool) -> Style {
    if is_editing {
        Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)
    } else if is_selected {
        Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(MUTED_COLOR)
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
            "{}…{}", // Using proper ellipsis character
            &truncated[..start_len],
            &truncated[truncated.len() - end_len..]
        )
    } else {
        truncated
    }
}
