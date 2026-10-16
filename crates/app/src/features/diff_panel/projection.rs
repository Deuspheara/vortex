//! Pure projection for stacked and side-by-side review. Source line identities survive pairing.
use super::state::DiffPresentation;
use crate::features::shell::state::{DiffFile, DiffRow, DiffRowKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewCell {
    pub source_row: usize,
    pub old: Option<usize>,
    pub new: Option<usize>,
    pub text: String,
    pub kind: DiffRowKind,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewRow {
    File {
        file: usize,
    },
    Section {
        file: usize,
        label: String,
    },
    Code {
        file: usize,
        left: Option<ReviewCell>,
        right: Option<ReviewCell>,
    },
}
fn cell(row: &DiffRow, source_row: usize) -> Option<ReviewCell> {
    let (old, new, text) = match row {
        DiffRow::Context {
            old_line,
            new_line,
            text,
        } => (Some(*old_line), Some(*new_line), text),
        DiffRow::Added { new_line, text } => (None, Some(*new_line), text),
        DiffRow::Removed { old_line, text } => (Some(*old_line), None, text),
        _ => return None,
    };
    Some(ReviewCell {
        source_row,
        old,
        new,
        text: text.clone(),
        kind: row.kind(),
    })
}
pub fn review_rows(files: &[DiffFile], presentation: &DiffPresentation) -> Vec<ReviewRow> {
    let mut result = Vec::new();
    for (file_ix, file) in files.iter().enumerate() {
        result.push(ReviewRow::File { file: file_ix });
        if presentation.collapsed.contains(&file.path) {
            continue;
        }
        let mut index = 0;
        while index < file.flat_rows.len() {
            let row = &file.flat_rows[index];
            match row {
                DiffRow::HunkHeader { label } => result.push(ReviewRow::Section {
                    file: file_ix,
                    label: label.clone(),
                }),
                DiffRow::Collapsed { count } => result.push(ReviewRow::Section {
                    file: file_ix,
                    label: format!("{count} unchanged lines"),
                }),
                DiffRow::Added { .. } | DiffRow::Removed { .. } if presentation.split => {
                    let mut old = Vec::new();
                    let mut new = Vec::new();
                    while let Some(row @ (DiffRow::Added { .. } | DiffRow::Removed { .. })) =
                        file.flat_rows.get(index)
                    {
                        let entry = cell(row, index).expect("code line");
                        if entry.old.is_some() {
                            old.push(entry);
                        } else {
                            new.push(entry);
                        }
                        index += 1;
                    }
                    let mut old = old.into_iter();
                    let mut new = new.into_iter();
                    loop {
                        let left = old.next();
                        let right = new.next();
                        if left.is_none() && right.is_none() {
                            break;
                        }
                        result.push(ReviewRow::Code {
                            file: file_ix,
                            left,
                            right,
                        });
                    }
                    continue;
                }
                _ => {
                    if let Some(entry) = cell(row, index) {
                        result.push(if presentation.split {
                            ReviewRow::Code {
                                file: file_ix,
                                left: Some(entry.clone()),
                                right: Some(entry),
                            }
                        } else {
                            ReviewRow::Code {
                                file: file_ix,
                                left: None,
                                right: Some(entry),
                            }
                        });
                    }
                }
            }
            index += 1;
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn file() -> DiffFile {
        DiffFile {
            path: "src/main.rs".into(),
            added: 2,
            removed: 1,
            hunks: vec![],
            flat_rows: vec![
                DiffRow::Context {
                    old_line: 1,
                    new_line: 1,
                    text: "fn main() {".into(),
                },
                DiffRow::Removed {
                    old_line: 2,
                    text: "old();".into(),
                },
                DiffRow::Added {
                    new_line: 2,
                    text: "new();".into(),
                },
                DiffRow::Added {
                    new_line: 3,
                    text: "extra();".into(),
                },
            ],
        }
    }
    #[test]
    fn split_pairs_replacements_without_losing_extra_lines_or_source_numbers() {
        let rows = review_rows(
            &[file()],
            &DiffPresentation {
                split: true,
                ..Default::default()
            },
        );
        assert_eq!(rows.len(), 4);
        let ReviewRow::Code {
            left: Some(left),
            right: Some(right),
            ..
        } = &rows[2]
        else {
            panic!("replacement pair")
        };
        assert_eq!((left.old, right.new), (Some(2), Some(2)));
        assert_eq!((&*left.text, &*right.text), ("old();", "new();"));
        assert!(
            matches!(&rows[3],ReviewRow::Code {left:None,right:Some(line),..} if line.new==Some(3))
        );
    }
    #[test]
    fn collapse_keeps_headers_for_every_file() {
        let mut presentation = DiffPresentation::default();
        presentation.collapsed.insert("src/main.rs".into());
        assert_eq!(
            review_rows(&[file(), file()], &presentation),
            vec![ReviewRow::File { file: 0 }, ReviewRow::File { file: 1 }]
        );
    }
}
