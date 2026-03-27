//! The line-processing pipeline that turns a noisy stream into a compact one.

use regex::RegexSet;

use crate::rules::Rules;

/// A compiled, ready-to-run version of [`Rules`].
pub struct CompiledProfile {
    strip_ansi: bool,
    collapse_carriage_returns: bool,
    collapse_blank_lines: bool,
    dedupe_consecutive: bool,
    max_lines: usize,
    head_lines: usize,
    tail_lines: usize,
    max_rescued_lines: usize,
    drop: RegexSet,
    keep: RegexSet,
}

/// What a single stream's compression produced.
pub struct StreamResult {
    pub text: String,
    pub original_lines: usize,
    pub kept_lines: usize,
}

impl CompiledProfile {
    /// Compile rules. Invalid regexes are skipped (with the offending pattern
    /// reported in `errors`) rather than failing the whole compressor.
    pub fn compile(rules: &Rules) -> (CompiledProfile, Vec<String>) {
        let (drop, mut errors) = compile_set(&rules.drop);
        let (keep, more) = compile_set(&rules.keep);
        errors.extend(more);
        (
            CompiledProfile {
                strip_ansi: rules.strip_ansi,
                collapse_carriage_returns: rules.collapse_carriage_returns,
                collapse_blank_lines: rules.collapse_blank_lines,
                dedupe_consecutive: rules.dedupe_consecutive,
                max_lines: rules.max_lines,
                head_lines: rules.head_lines,
                tail_lines: rules.tail_lines,
                max_rescued_lines: rules.max_rescued_lines,
                drop,
                keep,
            },
            errors,
        )
    }

    /// Run the pipeline over one stream (stdout or stderr).
    pub fn run(&self, raw: &[u8]) -> StreamResult {
        // 1. Bytes -> text. We do NOT strip ANSI yet: the ANSI stripper also
        //    consumes carriage returns, which would defeat the progress-bar
        //    collapse below. CR handling first, ANSI strip per line after.
        let text = String::from_utf8_lossy(raw).into_owned();
        let original_lines = text.lines().count();

        // 2. Per-line normalization + drop/keep filtering.
        let mut lines: Vec<String> = Vec::with_capacity(original_lines);
        for raw_line in text.split('\n') {
            // A progress bar redraws with \r; keep only the final frame.
            let line = if self.collapse_carriage_returns {
                raw_line.rsplit('\r').next().unwrap_or(raw_line)
            } else {
                raw_line.trim_end_matches('\r')
            };
            let line = if self.strip_ansi {
                let stripped = strip_ansi_escapes::strip(line.as_bytes());
                String::from_utf8_lossy(&stripped).into_owned()
            } else {
                line.to_string()
            };
            if self.should_keep(&line) {
                lines.push(line);
            }
        }
        // Drop a trailing empty line introduced by a final newline.
        if lines.last().map(|l| l.is_empty()).unwrap_or(false) {
            lines.pop();
        }

        // 3. Collapse blank runs.
        if self.collapse_blank_lines {
            lines = collapse_blanks(lines);
        }

        // 4. Dedupe adjacent identical lines.
        if self.dedupe_consecutive {
            lines = dedupe(lines);
        }

        // 5. Truncate, rescuing important (keep-matching) middle lines.
        let kept_lines = lines.len();
        let lines = self.truncate(lines);

        StreamResult {
            text: lines.join("\n"),
            original_lines,
            kept_lines,
        }
    }

    fn should_keep(&self, line: &str) -> bool {
        if self.keep.is_match(line) {
            return true;
        }
        !self.drop.is_match(line)
    }

    fn is_important(&self, line: &str) -> bool {
        self.keep.is_match(line)
    }

    fn truncate(&self, lines: Vec<String>) -> Vec<String> {
        if lines.len() <= self.max_lines {
            return lines;
        }
        let head = self.head_lines.min(lines.len());
        let tail = self.tail_lines.min(lines.len().saturating_sub(head));
        let mid_start = head;
        let mid_end = lines.len() - tail;

        let mut out = Vec::with_capacity(head + tail + self.max_rescued_lines + 2);
        out.extend(lines[..head].iter().cloned());

        // Rescue important lines from the elided middle.
        let mut rescued: Vec<String> = lines[mid_start..mid_end]
            .iter()
            .filter(|l| self.is_important(l))
            .take(self.max_rescued_lines)
            .cloned()
            .collect();

        let elided = (mid_end - mid_start) - rescued.len();
        if elided > 0 {
            out.push(format!("… {elided} lines hidden by skarn-compress …"));
        }
        if !rescued.is_empty() {
            out.push("… kept important lines from the hidden region:".to_string());
            out.append(&mut rescued);
        }
        out.extend(lines[mid_end..].iter().cloned());
        out
    }
}

fn compile_set(patterns: &[String]) -> (RegexSet, Vec<String>) {
    // RegexSet::new fails if any pattern is invalid; filter invalid ones out and
    // report them so a single typo in a user rule does not break everything.
    let mut valid = Vec::with_capacity(patterns.len());
    let mut errors = Vec::new();
    for p in patterns {
        match regex::Regex::new(p) {
            Ok(_) => valid.push(p.clone()),
            Err(e) => errors.push(format!("invalid regex {p:?}: {e}")),
        }
    }
    let set = RegexSet::new(&valid).unwrap_or_else(|_| RegexSet::empty());
    (set, errors)
}

fn collapse_blanks(lines: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut prev_blank = false;
    for l in lines {
        let blank = l.trim().is_empty();
        if blank && prev_blank {
            continue;
        }
        prev_blank = blank;
        out.push(l);
    }
    out
}

fn dedupe(lines: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let mut count = 1;
        while i + count < lines.len() && lines[i + count] == lines[i] {
            count += 1;
        }
        if count > 1 && !lines[i].trim().is_empty() {
            out.push(format!("{}  (×{count})", lines[i]));
        } else {
            for _ in 0..count {
                out.push(lines[i].clone());
            }
        }
        i += count;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Rules;

    fn profile(rules: Rules) -> CompiledProfile {
        CompiledProfile::compile(&rules).0
    }

    #[test]
    fn strips_ansi() {
        let p = profile(Rules::default());
        let out = p.run(b"\x1b[31mred\x1b[0m\nplain\n");
        assert_eq!(out.text, "red\nplain");
    }

    #[test]
    fn collapses_carriage_returns() {
        let p = profile(Rules::default());
        let out = p.run(b"10%\r50%\r100% done\n");
        assert_eq!(out.text, "100% done");
    }

    #[test]
    fn dedupes_adjacent_lines() {
        let p = profile(Rules::default());
        let out = p.run(b"same\nsame\nsame\nother\n");
        assert_eq!(out.text, "same  (×3)\nother");
