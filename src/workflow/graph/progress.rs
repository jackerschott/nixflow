use indicatif::ProgressStyle;

use crate::workflow::job::JobProgressStyle;

pub fn build_progress_style(job_index: u32, job_count: u32) -> JobProgressStyle {
    JobProgressStyle {
        bar_style: ProgressStyle::default_bar()
            .template(&format!(
                "[{job_index}/{job_count}] {{msg:.green}}  {{pos}}/{{len}}",
                job_index = job_index,
                job_count = job_count,
            ))
            .expect("expected template string to be correct"),
        spinner_style: ProgressStyle::default_spinner()
            .template(&format!(
                "[{job_index}/{job_count}] {{msg:.green}} {{spinner}}",
                job_index = job_index,
                job_count = job_count,
            ))
            .expect("expected template string to be correct"),
        failed_bar_style: ProgressStyle::default_bar()
            .template(&format!(
                "[{job_index}/{job_count}] {{msg:.red}}  {{pos}}/{{len}}",
                job_index = job_index,
                job_count = job_count,
            ))
            .expect("expected template string to be correct"),
        failed_spinner_style: ProgressStyle::default_spinner()
            .template(&format!(
                "[{job_index}/{job_count}] {{msg:.red}} {{spinner}}",
                job_index = job_index,
                job_count = job_count,
            ))
            .expect("expected template string to be correct"),
    }
}
