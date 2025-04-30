use std::collections::HashMap;
use camino::Utf8PathBuf;

enum OutputPaths {
    Single(),
    Multiple(),
}
struct Input<'s> {
    path: Utf8PathBuf,
    parent_step: &'s Step<'s>,
}
struct Output {
    path: Utf8PathBuf,
}

struct Step<'i> {
    name: String,
    inputs: HashMap<String, Vec<Input<'i>>>,
    outputs: HashMap<String, Vec<Output>>,
}

struct Target<'i>(Input<'i>);
