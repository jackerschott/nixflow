use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use std::{
    str::FromStr,
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
pub enum ByteCountUnit {
    KiloBytes,
    MegaBytes,
    GigaBytes,
    TerraBytes,
}
impl ByteCountUnit {
    pub fn as_slurm_suffix(self) -> &'static str {
        match self {
            Self::KiloBytes => "K",
            Self::MegaBytes => "M",
            Self::GigaBytes => "G",
            Self::TerraBytes => "T",
        }
    }
}
impl FromStr for ByteCountUnit {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "kB" => Ok(Self::KiloBytes),
            "MB" => Ok(Self::MegaBytes),
            "GB" => Ok(Self::GigaBytes),
            "TB" => Ok(Self::TerraBytes),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MemorySize {
    AllAvailable,
    Fixed((u16, ByteCountUnit)),
}
impl FromStr for MemorySize {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "all_available" {
            return Ok(Self::AllAvailable)
        }

        for unit in ["kB", "MB", "GB", "TB"] {
            if let Some(size) = s.strip_suffix(unit) {
                let size = u16::from_str(size).map_err(|err| {
                    format!(
                        "found a valid unit suffix (`{unit}`), but failed \
                        to parse `{size}` as an unsigned 16-bit integer \
                        (no whitespace allowed)\n{err}"
                    )
                })?;
                if size == 0 {
                    return Err(format!(
                        "found zero `{unit}`, but memory sizes should always be non-zero"
                    ));
                }

                return Ok(Self::Fixed((
                    size,
                    ByteCountUnit::from_str(unit).expect("we only loop over valid units"),
                )));
            }
        }

        Err(format!(
            "expected an unsigned 16-bit integer with kB, MB, GB or TB \
            as a suffix (no whitespace), got `{s}`"
        ))
    }
}

pub trait FormatSlurmTime {
    fn format_slurm_time(&self) -> String;
}
impl FormatSlurmTime for Duration {
    fn format_slurm_time(&self) -> String {
        let total_seconds = self.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct SlurmExecutionOptions {
    pub(super) account: String,

    #[serde(default)]
    pub(super) quality_of_service: Option<String>,

    #[serde(default)]
    pub(super) constraint: Option<String>,

    #[serde(default)]
    pub(super) partitions: Option<Vec<String>>,

    pub(super) runtime: Duration,

    #[serde_as(as = "DisplayFromStr")]
    pub(super) memory_size: MemorySize,

    pub(super) cpu_count: u16,

    #[serde(default)]
    pub(super) gpu_count: u16,
}
