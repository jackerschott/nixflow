use std::num::ParseIntError;

use regex::Regex;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct ProgressScanningInfo {
    #[serde(rename = "indicatorMax")]
    pub indicator_max: u32,

    #[serde(rename = "indicatorRegex")]
    pub indicator_regex_pattern: String,
}

#[derive(Debug)]
pub struct ProgressScanner {
    info: ProgressScanningInfo,
    indicator_regex: Regex,
}

impl ProgressScanner {
    pub fn new(info: &ProgressScanningInfo) -> Result<Self, ProgressScanError> {
        Ok(Self {
            info: info.clone(),
            indicator_regex: Self::indicator_regex(&info.indicator_regex_pattern)?,
        })
    }

    fn indicator_regex(pattern: &str) -> Result<Regex, ProgressScanError> {
        let regex = Regex::new(pattern)?;
        if regex.captures_len() != 2 {
            return Err(ProgressScanError::InvalidCaptureGroupCount {
                pattern: pattern.to_owned(),
                count: regex.captures_len(),
            });
        }

        return Ok(regex);
    }

    pub fn read_progress(&mut self, log_contents: String) -> Result<u32, ProgressScanError> {
        Ok(log_contents
            .lines()
            .filter_map(|line| self.indicator_regex.captures(line).map(|capture| (line, capture)))
            .map(|(line, capture)| {
                let capture_match = capture.get(1).expect(
                    "expected there to be no regex match where there is a \
                    non-participating capture group among two capture groups",
                );

                capture_match.as_str().parse().map_err(|parsing_error| {
                    ProgressScanError::NonIntegerMatch {
                        pattern: self.info.indicator_regex_pattern.clone(),
                        line: line.to_owned(),
                        capture_match: capture_match.as_str().to_owned(),
                        parsing_error,
                    }
                })
            })
            .collect::<Result<Vec<u32>, _>>()?
            .into_iter()
            .max()
            .unwrap_or(0))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProgressScanError {
    #[error("{0}")]
    CompilationFailure(#[from] regex::Error),

    #[error("invalid number of capture groups in `{pattern}`, expected 2, got {count}")]
    InvalidCaptureGroupCount { pattern: String, count: usize },

    #[error("expected an integer, got `{capture_match}` by applying `{pattern}` to `{line}`\n{parsing_error}")]
    NonIntegerMatch {
        pattern: String,
        line: String,
        capture_match: String,
        parsing_error: ParseIntError,
    },
}
