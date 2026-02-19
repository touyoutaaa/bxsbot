use regex::Regex;
use tracing::{info, debug};

use super::Table;

pub struct TableParser;

impl TableParser {
    pub fn new() -> Self {
        Self
    }

    /// 从全文中检测并提取表格
    pub fn extract(&self, full_text: &str) -> Vec<Table> {
        let lines: Vec<&str> = full_text.lines().collect();
        let mut tables: Vec<Table> = Vec::new();
        let table_caption_re = Regex::new(r"(?i)^Table\s+(\d+)[.:]?\s*(.*)$").unwrap();

        let mut i = 0;
        while i < lines.len() {
            let trimmed = lines[i].trim();

            // Look for "Table N" caption lines
            if table_caption_re.is_match(trimmed) {
                let caption = Some(trimmed.to_string());
                i += 1;

                // Skip blank lines after caption
                while i < lines.len() && lines[i].trim().is_empty() {
                    i += 1;
                }

                // Collect candidate table rows
                let mut raw_rows: Vec<&str> = Vec::new();
                let mut blank_count = 0;
                while i < lines.len() {
                    let row = lines[i].trim();
                    if row.is_empty() {
                        blank_count += 1;
                        if blank_count > 1 {
                            break; // Two consecutive blanks end the table
                        }
                        i += 1;
                        continue;
                    }
                    blank_count = 0;

                    // Stop if we hit another section heading or "Table N"
                    if table_caption_re.is_match(row) {
                        break;
                    }

                    raw_rows.push(row);
                    i += 1;
                }

                if raw_rows.len() >= 2 {
                    if let Some((headers, rows)) = Self::parse_rows(&raw_rows) {
                        debug!("检测到表格: {:?}, {} 行", caption, rows.len());
                        tables.push(Table { caption, headers, rows });
                    }
                }
                continue;
            }

            // Detect column-aligned blocks without "Table N" caption:
            // require at least 2 columns separated by 2+ spaces, and 3+ consecutive such lines
            if Self::looks_like_table_row(trimmed) {
                let start = i;
                let mut raw_rows: Vec<&str> = Vec::new();
                let mut blank_count = 0;
                while i < lines.len() {
                    let row = lines[i].trim();
                    if row.is_empty() {
                        blank_count += 1;
                        if blank_count > 1 {
                            break;
                        }
                        i += 1;
                        continue;
                    }
                    blank_count = 0;
                    if !Self::looks_like_table_row(row) {
                        break;
                    }
                    raw_rows.push(row);
                    i += 1;
                }

                // Need at least 3 rows for uncaptioned tables
                if raw_rows.len() >= 3 {
                    if let Some((headers, rows)) = Self::parse_rows(&raw_rows) {
                        debug!("检测到无标题表格: {} 列, {} 行", headers.len(), rows.len());
                        tables.push(Table {
                            caption: None,
                            headers,
                            rows,
                        });
                    }
                }
                // If we didn't consume anything new, advance
                if i == start {
                    i += 1;
                }
                continue;
            }

            i += 1;
        }

        info!("表格解析完成，共 {} 个", tables.len());
        tables
    }

    /// Check if a line looks like a table row
    fn looks_like_table_row(line: &str) -> bool {
        if line.len() < 5 {
            return false;
        }
        // Must have at least 2 segments separated by 2+ spaces or tab
        let multi_space_re = Regex::new(r"[\t]|\s{2,}").unwrap();
        let parts: Vec<&str> = multi_space_re.split(line).filter(|s| !s.is_empty()).collect();
        parts.len() >= 2
    }

    /// Parse raw text rows into headers and data rows
    fn parse_rows(raw_rows: &[&str]) -> Option<(Vec<String>, Vec<Vec<String>>)> {
        if raw_rows.is_empty() {
            return None;
        }

        let multi_space_re = Regex::new(r"[\t]|\s{2,}").unwrap();

        let headers: Vec<String> = multi_space_re
            .split(raw_rows[0])
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        if headers.len() < 2 {
            return None;
        }

        let rows: Vec<Vec<String>> = raw_rows[1..]
            .iter()
            .map(|row| {
                multi_space_re
                    .split(row)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.trim().to_string())
                    .collect()
            })
            .collect();

        if rows.is_empty() {
            return None;
        }

        Some((headers, rows))
    }
}
