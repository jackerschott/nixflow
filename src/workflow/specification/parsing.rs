use camino::Utf8PathBuf as PathBuf;
use miette::{Context, IntoDiagnostic, SourceOffset};
use serde::{Deserialize, de::value::MapAccessDeserializer};
use serde_with::DeserializeAs;

use super::TargetItem;

pub struct TargetList;
impl<'de> DeserializeAs<'de, Vec<TargetItem>> for TargetList {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<TargetItem>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Vec<TargetItem>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("target or list of targets")
            }
            fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
            where
                S: serde::de::SeqAccess<'de>,
            {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(items)
            }
            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                Ok(vec![TargetItem::deserialize(MapAccessDeserializer::new(
                    map,
                ))?])
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("failed to parse workflow specification")]
#[diagnostic(
    help = "the full faulty workflow specification was written to\n{input_inspection_path}"
)]
pub struct ParsingError {
    cause: serde_json::Error,
    #[source_code]
    input: String,
    #[label("{cause}")]
    location: SourceOffset,

    input_inspection_path: PathBuf,
}

pub trait WithSourceIndication<T> {
    fn with_source_indication<S: Into<String>>(self, input: S)
    -> miette::Result<miette::Result<T>>;
}

impl<T> WithSourceIndication<T> for serde_json::Result<T> {
    fn with_source_indication<S: Into<String>>(
        self,
        input: S,
    ) -> miette::Result<miette::Result<T>> {
        match self {
            Ok(value) => Ok(Ok(value)),
            Err(cause) => {
                let input_inspection_file = tempfile::NamedTempFile::new()
                    .into_diagnostic()
                    .context("failed to create a temporary input inspection file")?;
                let input_inspection_path = input_inspection_file
                    .into_temp_path()
                    .keep()
                    .into_diagnostic()
                    .context("failed to keep temporary input inspection file")?;
                let input_inspection_path = PathBuf::from_path_buf(input_inspection_path).expect(
                    &format!("expected the input inspection path to be valid utf8"),
                );

                let input = input.into();
                Ok(Err((ParsingError {
                    location: SourceOffset::from_location(&input, cause.line(), cause.column()),
                    cause,
                    input,
                    input_inspection_path,
                })
                .into()))
            }
        }
    }
}
