use super::JobError;

#[derive(Debug)]
pub struct ErrorCatcher {
    pub warnings: Vec<JobError>,
    pub as_dummy: bool,
}
impl ErrorCatcher {
    pub fn new(as_dummy: bool) -> Self {
        Self { warnings: Vec::new(), as_dummy }
    }
}

pub trait TryCatch<T, JobExecutionError> {
    fn try_catch(
        self,
        catcher: &mut ErrorCatcher,
    ) -> Result<Option<T>, JobExecutionError>;
}

impl<T> TryCatch<T, JobError> for Result<T, JobError> {
    fn try_catch(
        self,
        catcher: &mut ErrorCatcher,
    ) -> Result<Option<T>, JobError> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(err) if !catcher.as_dummy => {
                catcher.warnings.push(err);
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }
}
