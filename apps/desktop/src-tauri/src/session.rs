//! Holds the transcript of a live listening session and writes it out.

use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use crate::stream::{LiveSegment, Track};

/// A transcribed segment as the UI sees it: the stream's own segment plus a
/// stable id, so the list can be keyed without relying on array position.
#[derive(Serialize, Clone, Debug)]
pub struct Entry {
    pub id: u64,
    pub track: Track,
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Default)]
struct State {
    entries: Vec<Entry>,
    next_id: u64,
    started_at: Option<SystemTime>,
}

#[derive(Default)]
pub struct Session {
    state: Mutex<State>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the start of a session, clearing anything already held.
    pub fn begin(&self) {
        let mut s = self.state.lock().unwrap();
        s.entries.clear();
        s.next_id = 0;
        s.started_at = Some(SystemTime::now());
    }

    pub fn push(&self, segment: LiveSegment) -> Entry {
        let mut s = self.state.lock().unwrap();
        let entry = Entry {
            id: s.next_id,
            track: segment.track,
            text: segment.text,
            start_ms: segment.start_ms,
            end_ms: segment.end_ms,
        };
        s.next_id += 1;
        // Insert by start time, not arrival time. The two tracks are
        // transcribed through one queue, so a short utterance from one can
        // finish after a longer, earlier one from the other.
        let at = s
            .entries
            .partition_point(|e| e.start_ms <= entry.start_ms);
        s.entries.insert(at, entry.clone());
        entry
    }

    pub fn entries(&self) -> Vec<Entry> {
        self.state.lock().unwrap().entries.clone()
    }

    pub fn clear(&self) {
        let mut s = self.state.lock().unwrap();
        s.entries.clear();
        s.started_at = None;
    }

    pub fn is_empty(&self) -> bool {
        self.state.lock().unwrap().entries.is_empty()
    }

    /// Renders the transcript as plain text, ready for the clipboard.
    pub fn to_text(&self) -> String {
        render(&self.entries(), Format::Text)
    }

    /// Writes the transcript into `dir/sessions` and returns the file path.
    pub fn write(&self, dir: &Path, format: Format) -> Result<PathBuf> {
        let entries = self.entries();
        let started = self.state.lock().unwrap().started_at;
        let stamp = timestamp(started.unwrap_or_else(SystemTime::now));

        let out_dir = dir.join("sessions");
        fs::create_dir_all(&out_dir)?;
        let path = out_dir.join(format!("transcript-{stamp}.{}", format.extension()));
        fs::write(&path, render(&entries, format))?;
        Ok(path)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Text,
    Markdown,
    Srt,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "txt" | "text" => Some(Self::Text),
            "md" | "markdown" => Some(Self::Markdown),
            "srt" => Some(Self::Srt),
            _ => None,
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Text => "txt",
            Self::Markdown => "md",
            Self::Srt => "srt",
        }
    }
}

fn speaker(track: Track) -> &'static str {
    match track {
        Track::Mic => "Me",
        Track::System => "Them",
    }
}

fn render(entries: &[Entry], format: Format) -> String {
    match format {
        Format::Text => entries
            .iter()
            .map(|e| format!("[{}] {}: {}", clock(e.start_ms), speaker(e.track), e.text))
            .collect::<Vec<_>>()
            .join("\n"),
        Format::Markdown => {
            let mut out = String::from("# Transcript\n\n");
            for e in entries {
                out.push_str(&format!(
                    "**{}** · `{}`\n\n{}\n\n",
                    speaker(e.track),
                    clock(e.start_ms),
                    e.text
                ));
            }
            out
        }
        Format::Srt => {
            let mut out = String::new();
            for (i, e) in entries.iter().enumerate() {
                out.push_str(&format!(
                    "{}\n{} --> {}\n{}: {}\n\n",
                    i + 1,
                    srt_time(e.start_ms),
                    srt_time(e.end_ms),
                    speaker(e.track),
                    e.text
                ));
            }
            out
        }
    }
}

/// `mm:ss`, or `h:mm:ss` once a session runs past an hour.
fn clock(ms: u64) -> String {
    let total = ms / 1000;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

fn srt_time(ms: u64) -> String {
    let total = ms / 1000;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        total / 3600,
        (total % 3600) / 60,
        total % 60,
        ms % 1000
    )
}

fn timestamp(t: SystemTime) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Calendar maths without pulling in a date library: days since the epoch
    // converted with the civil-from-days algorithm.
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(track: Track, text: &str, start_ms: u64, end_ms: u64) -> LiveSegment {
        LiveSegment {
            track,
            text: text.into(),
            start_ms,
            end_ms,
        }
    }

    #[test]
    fn entries_get_stable_increasing_ids() {
        let s = Session::new();
        s.begin();
        assert_eq!(s.push(seg(Track::System, "one", 0, 1_000)).id, 0);
        assert_eq!(s.push(seg(Track::Mic, "two", 1_000, 2_000)).id, 1);
        assert_eq!(s.entries().len(), 2);
    }

    #[test]
    fn begin_resets_a_previous_session() {
        let s = Session::new();
        s.begin();
        s.push(seg(Track::System, "old", 0, 1));
        s.begin();
        assert!(s.is_empty());
        assert_eq!(s.push(seg(Track::System, "new", 0, 1)).id, 0);
    }

    #[test]
    fn entries_are_ordered_by_time_not_arrival() {
        let s = Session::new();
        s.begin();
        // A late-arriving segment that started earlier must sort ahead.
        s.push(seg(Track::System, "second", 10_000, 11_000));
        s.push(seg(Track::Mic, "first", 1_000, 2_000));
        let texts: Vec<String> = s.entries().into_iter().map(|e| e.text).collect();
        assert_eq!(texts, vec!["first", "second"]);
    }

    #[test]
    fn text_export_labels_both_speakers() {
        let s = Session::new();
        s.begin();
        s.push(seg(Track::System, "hello", 0, 1_000));
        s.push(seg(Track::Mic, "hi", 61_000, 62_000));
        let out = s.to_text();
        assert!(out.contains("[00:00] Them: hello"), "got: {out}");
        assert!(out.contains("[01:01] Me: hi"), "got: {out}");
    }

    #[test]
    fn srt_export_is_well_formed() {
        let s = Session::new();
        s.begin();
        s.push(seg(Track::System, "hello", 1_500, 3_250));
        let out = render(&s.entries(), Format::Srt);
        assert!(out.starts_with("1\n00:00:01,500 --> 00:00:03,250\n"), "got: {out}");
    }

    #[test]
    fn clock_grows_to_hours() {
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(65_000), "01:05");
        assert_eq!(clock(3_725_000), "1:02:05");
    }

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }

    #[test]
    fn format_parses_aliases() {
        assert_eq!(Format::parse("md"), Some(Format::Markdown));
        assert_eq!(Format::parse("srt"), Some(Format::Srt));
        assert_eq!(Format::parse("docx"), None);
    }
}
