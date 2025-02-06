use std::env;
use std::io::stdout;
use std::collections::BinaryHeap;
use std::cmp::Reverse;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use walkdir::WalkDir;

use crossterm::{
    event::{self, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the user's profile path
    let env_path = env::var("USERPROFILE")? + "\\AppData\\";
    let dir_list: Vec<String> = walk_dir(&env_path)?;

    // Enable raw mode
    enable_raw_mode()?;
    let mut buffer = stdout();
    execute!(buffer, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(buffer);
    let mut terminal = Terminal::new(backend)?;

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
                .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
                .split(f.area());

            let input_box = Paragraph::new(Text::from(Span::styled(
                input.clone(),
                Style::default().fg(Color::LightYellow),
            )))
            .block(Block::default().title(" Input ").borders(Borders::ALL));
            f.render_widget(input_box, chunks[0]);

            let list =
                List::new(items).block(Block::default().title(" Results ").borders(Borders::ALL));
            f.render_widget(list, chunks[1]);
        })?;

        // Check for user input
        if let event::Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char(c) => input.push(c),
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    KeyCode::Esc | KeyCode::Enter => break,
                    _ => (),
                }
            }
        }

    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

// Walk the directory and return a list of paths
fn walk_dir(path: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut list: Vec<String> = Vec::new();

    let walker = WalkDir::new(path).into_iter();
    for entry in walker {
        if entry.is_err() {
            continue;
        }
        let entry = entry?;
        let path = entry.path();
        let path = path.to_str().unwrap();
        list.push(path.to_string());
    }

    Ok(list)
}

// Fuzzy search the directory list
fn fuzzy(dir_list: Vec<String>, input: String) -> Result<Vec<String>, Box<dyn std::error::Error>> {

    // Initialize the fuzzy matcher
    let matcher = SkimMatcherV2::default();
    let mut score: BinaryHeap<Reverse<(i64, &String)>> = BinaryHeap::new();

    for dir in &dir_list {
        let match_score = matcher.fuzzy_match(&dir, &input).unwrap_or(0);

        if score.len() < 10 {
            score.push(Reverse((match_score, dir)));
        } else if let Some(Reverse((min_score, _))) = score.peek() {
            if match_score > *min_score {
                score.pop();
                score.push(Reverse((match_score, dir)));
            }
        }
    }

    let matched_dir: Vec<String> = score.clone()
        .into_sorted_vec()
        .iter()
        .map(|x| x.0 .1.clone())
        .collect();

    Ok(matched_dir)
}
