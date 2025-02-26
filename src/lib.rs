use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;
use walkdir::WalkDir;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

// Handle user input
pub async fn input_handler(
    tx: mpsc::Sender<(KeyCode, KeyModifiers)>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        if let Ok(Event::Key(key)) = event::read() {
            // Prevent user releasing the key also trigger the event
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Esc => {
                        let _ = tx.send((key.code, key.modifiers)).await;
                        break;
                    }
                    _ => {
                        let _ = tx.send((key.code, key.modifiers)).await;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Walk through the directory and return a list of directories
///
/// # Arguments
///
/// * `path` - The path to walk through
///
/// # Returns
///
/// A list of directories
pub fn walk_dir(path: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
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

/// Fuzzy match the user input with the directory list
///
/// Return a list of matched directories with the highest score
///
/// # Arguments
///
/// * `dir_list` - A list of directories
/// * `input` - User input
///
/// # Returns
///
/// A list of matched directories (up to 10) with the highest score
pub fn fuzzy(dir_list: Vec<String>, input: String) -> Result<Vec<String>, Box<dyn std::error::Error>> {
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

    let matched_dir: Vec<String> = score
        .clone()
        .into_sorted_vec()
        .iter()
        .map(|x| x.0 .1.clone())
        .collect();

    Ok(matched_dir)
}
