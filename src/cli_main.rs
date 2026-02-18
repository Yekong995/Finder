use std::env;
use std::io::stdout;
use std::rc::Rc;

use clap::Parser;
use clipboard_rs::{Clipboard, ClipboardContext};
use finder::*;
use rusqlite::{Connection, params};
use tokio::sync::mpsc;

use crossterm::{
    event::{KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect, Spacing},
    style::{Color, Modifier, Style},
    symbols::merge::MergeStrategy,
    text::{Span, Text},
    widgets::{Block, BorderType, Borders, List, ListDirection, ListItem, ListState, Paragraph},
};

// Beginning of args parser

#[derive(Parser)]
#[command(name = "Finder", version = "0.3.0", about = "A simple file finder")]
struct Args {
    /// Mode of operation (search or add)
    /// Add for adding directory to database
    #[arg(default_value = "search", required = true)]
    mode: String,
    // Update database
    // #[arg(short, long)]
    // update: bool,
}

// End of args parser

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let db_path = env::var("USERPROFILE")? + "\\AppData\\finder.db";
    let mut conn = Connection::open(db_path)?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS files (
            file_name TEXT    NOT NULL,
            full_path TEXT    NOT NULL UNIQUE,
            mod_time  INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_mod_time  ON files(mod_time DESC);
        CREATE INDEX IF NOT EXISTS idx_file_name ON files(file_name);

        CREATE TABLE IF NOT EXISTS walk_history (
            path      TEXT    NOT NULL
        );
        ",
    )?;

    match args.mode.as_str() {
        "search" => search_box(&mut conn).await?,
        "add" => add_dir(&mut conn).await?,
        _ => println!("Please select a valid mode"),
    }

    Ok(())
}

async fn search_box(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    let mut select_mode: bool = false;
    let mut pos: u64 = 0;

    // Enable raw mode
    enable_raw_mode()?;
    let mut buffer = stdout();
    execute!(buffer, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(buffer);
    let mut terminal = Terminal::new(backend)?;

    let (tx, mut rx) = mpsc::channel(32);
    let input_task = tokio::spawn(input_handler(tx));

    // Let the user search
    let mut input: String = String::new();
    let mut results: Vec<File> = vec![
        File {
            name: "Not Found".into(),
            path: "Not Found".into()
        };
        20
    ]; // Initialize results with default values

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Length(1),
                        Constraint::Length(3),
                        Constraint::Min(1),
                    ]
                    .as_ref(),
                )
                .split(f.area());

            let result_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [Constraint::Fill(1), Constraint::Fill(3)]
                    .as_ref(),
                )
                .spacing(Spacing::Overlap(1))
                .split(chunks[2]);

            let message: String = "Press 'Esc' to exit | 'Ctrl + r' to switch tab | 'Enter' to search / copy selected path".to_string();
            let _ = msg(f, chunks[0], message);
            let _ = input_box(input.clone(), f, chunks[1], select_mode, "Search".to_string());
            let _ = search_result(&results, f, result_chunks, select_mode, pos);
        })?;

        if let Some((key, modifiers)) = rx.recv().await {
            match key {
                KeyCode::Char('r') => {
                    if modifiers.contains(KeyModifiers::CONTROL) {
                        select_mode = !select_mode;
                    } else if select_mode == false {
                        input.push('r');
                    }
                }

                KeyCode::Char(c) => {
                    if select_mode == false {
                        input.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if select_mode == false {
                        input.pop();
                    }
                }

                KeyCode::Down => {
                    if select_mode == true {
                        pos = (pos + 1).min(results.len() as u64 - 1);
                    }
                }
                KeyCode::Up => {
                    if select_mode == true {
                        pos = pos.saturating_sub(1);
                    }
                }
                KeyCode::Enter => {
                    if select_mode == true {
                        let selected = results.get(pos as usize).unwrap().path.clone();
                        let ctx = ClipboardContext::new().unwrap();
                        ctx.set_text(selected.clone()).unwrap();
                    } else {
                        results = query_n_fuzzy(conn, &input)?;
                    }
                }

                KeyCode::Esc => break,
                _ => (),
            }
        };

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    input_task.abort();

    Ok(())
}

async fn add_dir(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut buffer = stdout();
    execute!(buffer, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(buffer);
    let mut terminal = Terminal::new(backend)?;

    let (tx, mut rx) = mpsc::channel(32);
    let input_task = tokio::spawn(input_handler(tx));

    let mut items: Vec<String> = Vec::new();
    let mut pos: u64 = 0;

    let mut stmt = conn.prepare("SELECT * FROM walk_history")?;
    let rows = stmt.query_map([], |row| Ok(row.get::<_, String>(0)?))?;
    items = rows.collect::<Result<_, _>>()?;
    stmt.finalize()?;
    if items.is_empty() {
        items.push("No History Found".to_string());
    }

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)].as_ref())
                .split(f.area());

            let message: String =
                "Press 'Esc' to exit | Press 'Del' / 'Ctrl + d' to delete | 'P' to paste"
                    .to_string();
            msg(f, chunks[0], message);
            add_box(&items, f, chunks[1], pos);
        })?;

        if let Some((key, modifiers)) = rx.recv().await {
            match key {
                KeyCode::Delete => {
                    if items.len() == 1 {
                        items.push("No History Found".to_string());
                    }
                    items.remove(pos as usize);
                    pos = min(pos, items.len() as u64 - 1);
                }

                KeyCode::Char('d') => {
                    if modifiers.contains(KeyModifiers::CONTROL) {
                        if items.len() == 1 {
                            items.push("No History Found".to_string());
                        }
                        items.remove(pos as usize);
                        pos = min(pos, items.len() as u64 - 1);
                    }
                }

                KeyCode::Up => {
                    if pos > 0 {
                        pos -= 1;
                    }
                }

                KeyCode::Down => {
                    if pos < (items.len() - 1) as u64 {
                        pos += 1;
                    }
                }

                KeyCode::Char('p') => {
                    let ctx = ClipboardContext::new().unwrap();
                    if ctx.has(clipboard_rs::ContentFormat::Text) {
                        let txt = ctx.get_text().unwrap();
                        items.push(txt);
                    }
                }

                KeyCode::Esc => break,
                _ => (),
            }
        };

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    input_task.abort();

    conn.execute("DELETE FROM walk_history", params![])?;
    let tx = conn.transaction()?;
    let mut stmt = tx.prepare("INSERT INTO walk_history (path) VALUES (?)")?;
    for item in items.clone() {
        stmt.execute(params![item.as_str()])?;
    }
    stmt.finalize()?;
    tx.commit()?;

    println!("Walking Directory... Time depends on yours file size");
    for item in items {
        let _ = walk_dir(item.as_str(), conn);
    }

    Ok(())
}

// Beginning of render function

fn msg(f: &mut Frame, chunks: Rect, msg: String) {
    let message = Paragraph::new(Text::from(Span::styled(
        msg,
        Style::default().fg(Color::LightYellow),
    )));
    f.render_widget(message, chunks);
}

fn input_box(
    input: String,
    f: &mut Frame,
    chunks: Rect,
    focus: bool,
    msg: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let border_style = if focus == false {
        Style::reset()
    } else {
        Style::new().dim()
    };

    let input_box = Paragraph::new(Text::from(Span::styled(
        input,
        Style::default().fg(Color::LightYellow),
    )))
    .block(
        Block::default()
            .title(format!(" {} ", msg))
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .borders(Borders::ALL),
    );
    f.render_widget(input_box, chunks);

    Ok(())
}

fn search_result(items: &Vec<File>, f: &mut Frame, chunks: Rc<[Rect]>, focus: bool, pos: u64) {
    let border_style = if focus == true {
        Style::reset()
    } else {
        Style::new().dim()
    };

    let name: Vec<_> = items
        .iter()
        .map(|x| ListItem::new(x.name.as_str()))
        .collect();

    let path: Vec<_> = items
        .iter()
        .map(|x| ListItem::new(x.path.as_str()))
        .collect();

    let list1 = List::new(name)
        .block(
            Block::default()
                .title(" FileName ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style)
                .merge_borders(MergeStrategy::Fuzzy),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("➤ ")
        .direction(ListDirection::TopToBottom);

    let list2 = List::new(path)
        .block(
            Block::default()
                .title(" FilePath ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style)
                .merge_borders(MergeStrategy::Fuzzy),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .direction(ListDirection::TopToBottom);

    let mut state = ListState::default();
    state.select(Some(pos as usize));
    f.render_stateful_widget(list1, chunks[0], &mut state);
    f.render_stateful_widget(list2, chunks[1], &mut state);
}

fn add_box(items: &[String], f: &mut Frame, chunks: Rect, pos: u64) {
    let items: Vec<_> = items.iter().map(|x| ListItem::new(x.as_str())).collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" File History ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default()),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("➤ ")
        .direction(ListDirection::TopToBottom);

    let mut state = ListState::default();
    state.select(Some(pos as usize));

    f.render_stateful_widget(list, chunks, &mut state);
}
