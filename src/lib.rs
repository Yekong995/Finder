use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::UNIX_EPOCH;

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use rusqlite::{Connection, params};

#[cfg(feature = "cli")]
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
#[cfg(feature = "cli")]
use tokio::sync::mpsc;
#[cfg(feature = "cli")]
use walkdir::WalkDir;

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct File {
    pub name: String,
    pub path: String,
}

pub fn min(a: u64, b: u64) -> u64 {
    if a < b { a } else { b }
}

// Handle user input
#[cfg(feature = "cli")]
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

#[cfg(feature = "cli")]
fn query(conn: &mut Connection) -> Result<Vec<File>, Box<dyn std::error::Error>> {
    let mut files: Vec<File> = conn
        .prepare(
            "SELECT file_name, full_path FROM files
             ORDER BY mod_time DESC",
        )?
        .query_map([], |row| {
            Ok(File {
                name: row.get(0)?,
                path: row.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    if files.is_empty() {
        files.push(File {
            name: "Not Found".into(),
            path: "Not Found".into(),
        });
    }

    Ok(files)
}

#[cfg(feature = "cli")]
pub fn query_n_fuzzy(
    conn: &mut Connection,
    keyword: &str,
) -> Result<Vec<File>, Box<dyn std::error::Error>> {
    let tmp_results = query(conn)?;
    let mut files: Vec<File> = Vec::new();

    let matcher = SkimMatcherV2::default();
    let mut scores: BinaryHeap<Reverse<(i64, File)>> = BinaryHeap::new();

    for file in tmp_results {
        let score = matcher.fuzzy_match(&file.name, &keyword).unwrap_or(0);

        if scores.len() < 20 {
            scores.push(Reverse((score, file)));
        } else if let Some(Reverse((top_score, _))) = scores.peek() {
            if score > *top_score {
                scores.pop();
                scores.push(Reverse((score, file)));
            }
        }
    }

    files = scores
        .into_sorted_vec()
        .into_iter()
        .map(|Reverse((_, file))| file)
        .collect();

    if files.is_empty() {
        files.push(File {
            name: "No files found".to_string(),
            path: "".to_string(),
        });
    }

    Ok(files)
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
#[cfg(feature = "cli")]
pub fn walk_dir(path: &str, conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    let walker = WalkDir::new(path).into_iter();

    conn.execute("DELETE FROM files", params![])?;
    let tx = conn.transaction()?;
    let mut stmt =
        tx.prepare("INSERT INTO files (file_name, full_path, mod_time) VALUES (?, ?, ?)")?;

    for entry in walker {
        if entry.is_err() {
            continue;
        }
        let entry = entry?;
        let file_path = entry.path().to_str().unwrap();
        let file_name = entry.file_name().to_str().unwrap();
        let mod_time = entry.metadata()?.modified()?;
        let mod_time = mod_time
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        stmt.execute(params![file_name, file_path, mod_time])?;
    }
    stmt.finalize()?;
    tx.commit()?;

    Ok(())
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
pub fn fuzzy(
    dir_list: Vec<String>,
    input: String,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
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
        .map(|x| x.0.1.clone())
        .collect();

    Ok(matched_dir)
}
