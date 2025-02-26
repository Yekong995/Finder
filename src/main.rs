use std::env;
use std::io::stdout;

use finder::*;
use tokio::sync::mpsc;
use clipboard_rs::{ClipboardContext, Clipboard};

use crossterm::{
    event::{KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the user's profile path
    let env_path = env::var("USERPROFILE")? + "\\AppData\\";
    let dir_list: Vec<String> = walk_dir(&env_path)?;
    let mut status_code: bool = false;
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

    loop {
        let matched_dir = fuzzy(dir_list.clone(), input.clone())?;

        let items: Vec<ListItem> = matched_dir
            .iter()
            .map(|x| ListItem::new(Span::raw(x.clone())))
            .collect();

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

            let _ = render_msg(f, chunks[0]);
            let _ = render_input(input.clone(), f, chunks[1], status_code);
            let _ = render_result(items, f, chunks[2], status_code, pos);
        })?;

        if let Some((key, modifiers)) = rx.recv().await {

            match key {
                KeyCode::Char('r') => {
                    if modifiers.contains(KeyModifiers::CONTROL)
                    {
                        status_code = !status_code;
                    } else if status_code == false {
                        input.push('r');
                    }
                }

                KeyCode::Char(c) => {
                    if status_code == false {
                        input.push(c);
                    }
                },
                KeyCode::Backspace => {
                    if status_code == false {
                        input.pop();
                    }
                }

                KeyCode::Down => {
                    if status_code == true {
                        pos = (pos + 1).min(matched_dir.len() as u64 - 1);
                    }
                }
                KeyCode::Up => {
                    if status_code == true {
                        pos = pos.saturating_sub(1);
                    }
                }
                KeyCode::Enter => {
                    if status_code == true {
                        let selected = matched_dir.get(pos as usize).unwrap();
                        let ctx = ClipboardContext::new().unwrap();
                        ctx.set_text(selected.clone()).unwrap();
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

fn render_msg(f: &mut Frame, chunks: Rect) -> Result<(), Box<dyn std::error::Error>> {
    let message = Paragraph::new(Text::from(Span::styled(
        "Press 'Esc' to exit | 'Ctrl + r' to switch tab | 'Enter' to copy selected path",
        Style::default().fg(Color::LightYellow),
    )));
    f.render_widget(message, chunks);

    Ok(())
}

fn render_input(
    input: String,
    f: &mut Frame,
    chunks: Rect,
    focus: bool,
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
        Block::bordered()
            .title(" Search ")
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .borders(Borders::ALL),
    );
    f.render_widget(input_box, chunks);

    Ok(())
}

fn render_result(
    items: Vec<ListItem>,
    f: &mut Frame,
    chunks: Rect,
    focus: bool,
    pos: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let border_style = if focus == true {
        Style::reset()
    } else {
        Style::new().dim()
    };

    let mut items = items;
    let selected = items.get_mut(pos as usize).unwrap();
    if focus == true {
        *selected = selected.clone().style(Style::default().bg(Color::DarkGray));
    }

    let list = List::new(items).block(
        Block::bordered()
            .title(" Results ")
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .borders(Borders::ALL),
    );
    f.render_widget(list, chunks);

    Ok(())
}
