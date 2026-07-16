//! Inline suppression: `// pinoc:allow(CODE)` or `// pinoc:allow(CODE) — reason`.
//! An allow covers a finding on its own line (trailing comment) or the line below.

const MARKER: &str = "pinoc:allow(";

pub struct Allow {
    pub file: String,
    pub line: usize,
    pub code: String,
    /// Parsed but surfaced later (e.g. `--explain`); kept so the contract is stable.
    #[allow(dead_code)]
    pub reason: Option<String>,
    pub matched: bool,
}

#[derive(Default)]
pub struct Suppressions {
    pub allows: Vec<Allow>,
}

impl Suppressions {
    pub fn scan(&mut self, file: &str, src: &str) {
        for (i, line) in src.lines().enumerate() {
            // Only honor the marker inside a `//` line comment, not in string
            // literals or other code that happens to contain the text.
            let Some(comment_at) = line.find("//") else {
                continue;
            };
            let comment = &line[comment_at..];
            let Some(pos) = comment.find(MARKER) else {
                continue;
            };
            let after = &comment[pos + MARKER.len()..];
            let Some(end) = after.find(')') else {
                continue;
            };
            let code = after[..end].trim().to_string();
            if code.is_empty() {
                continue;
            }
            let rest = after[end + 1..].trim_start_matches(['-', '—', ' ']).trim();
            self.allows.push(Allow {
                file: file.to_string(),
                line: i + 1,
                code,
                reason: (!rest.is_empty()).then(|| rest.to_string()),
                matched: false,
            });
        }
    }

    /// True if an allow on the finding's line or the line above matches its code.
    pub fn is_suppressed(&mut self, file: &str, line: usize, code: &str) -> bool {
        let mut hit = false;
        for a in &mut self.allows {
            let code_hit = a.code == code || a.code == "*";
            if a.file == file && code_hit && (a.line == line || a.line + 1 == line) {
                a.matched = true;
                hit = true;
            }
        }
        hit
    }

    pub fn unused(&self) -> impl Iterator<Item = &Allow> {
        self.allows.iter().filter(|a| !a.matched)
    }
}
