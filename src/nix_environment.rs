use std::process::Command;

use anyhow::Result;
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use thiserror::Error;

pub enum NixEnvironment {
    Native,
    Container {
        cache_directory_path: PathBuf,
        apptainer_args: Vec<String>,
    },
}

#[derive(Error, Debug)]
pub enum NixEnvironmentError {
    #[error("failed to check if nix is available: {0}")]
    NixAvailabilityCheckError(std::io::Error),

    #[error("nix is not available as a command")]
    NixNotAvailable,

    #[error("failed to retreive cache directory path: {0}")]
    FailedCacheDirectoryRetreival(CacheDirectoryRetreivalError),

    #[error("failed to create store image: {0:?}")]
    FailedStoreImageCreation(Option<std::io::Error>),

    #[error("failed to pull nix container: {0:?}")]
    FailedNixContainerPull(Option<std::io::Error>),

    #[error("io error: {0}")]
    IOError(std::io::Error),
}

impl NixEnvironment {
    pub fn new(
        nix_container_url: String,
        apptainer_args: Vec<String>,
    ) -> Result<Self, NixEnvironmentError> {
        match Self::new_native() {
            Ok(native_environment) => Ok(native_environment),
            Err(NixEnvironmentError::NixNotAvailable) => {
                Self::new_container(nix_container_url, apptainer_args)
            }
            Err(err) => Err(err),
        }
    }

    pub fn new_native() -> Result<Self, NixEnvironmentError> {
        if !Self::check_nix_available()? {
            return Err(NixEnvironmentError::NixNotAvailable);
        }
        Ok(Self::Native)
    }

    pub fn new_container(
        nix_container_url: String,
        apptainer_args: Vec<String>,
    ) -> Result<Self, NixEnvironmentError> {
        let cache_directory_path = get_cache_directory_path()
            .map_err(|err| NixEnvironmentError::FailedCacheDirectoryRetreival(err))?;
        let container_env = Self::Container {
            cache_directory_path,
            apptainer_args,
        };

        if !std::fs::exists(container_env.cache_directory_path())
            .map_err(|err| NixEnvironmentError::IOError(err))?
        {
            std::fs::create_dir_all(container_env.cache_directory_path())
                .map_err(|err| NixEnvironmentError::IOError(err))?;
        }

        if !std::fs::exists(container_env.nix_container_cache_path())
            .map_err(|err| NixEnvironmentError::IOError(err))?
        {
            container_env.pull_nix_container(nix_container_url)?;
        }

        if !std::fs::exists(container_env.store_image_path())
            .map_err(|err| NixEnvironmentError::IOError(err))?
        {
            container_env.create_store_image()?;
        }

        Ok(container_env)
    }

    fn check_nix_available() -> Result<bool, NixEnvironmentError> {
        Ok(Command::new("which")
            .arg("nix")
            .status()
            .map_err(|err| NixEnvironmentError::NixAvailabilityCheckError(err))?
            .success())
    }

    fn create_store_image(&self) -> Result<(), NixEnvironmentError> {
        assert!(self.is_containerized());
        (!Command::new("apptainer")
            .arg("overlay")
            .arg("create")
            .arg("--size")
            .arg("10000")
            .arg(self.store_image_path())
            .status()
            .map_err(|err| NixEnvironmentError::FailedStoreImageCreation(Some(err)))?
            .success())
        .then_some(NixEnvironmentError::FailedStoreImageCreation(None))
        .map_or(Ok(()), |err| Err(err))
    }

    fn pull_nix_container(&self, nix_container_url: String) -> Result<(), NixEnvironmentError> {
        match self {
            Self::Container { .. } => (!Command::new("apptainer")
                .arg("pull")
                .arg(self.nix_container_cache_path())
                .arg(nix_container_url)
                .status()
                .map_err(|err| NixEnvironmentError::FailedNixContainerPull(Some(err)))?
                .success())
            .then_some(NixEnvironmentError::FailedNixContainerPull(None))
            .map_or(Ok(()), |err| Err(err)),
            Self::Native => unreachable!(),
        }
    }

    pub fn is_containerized(&self) -> bool {
        match self {
            Self::Native => false,
            Self::Container { .. } => true,
        }
    }

    pub fn nix_container_cache_path(&self) -> PathBuf {
        match self {
            NixEnvironment::Native => unreachable!(),
            NixEnvironment::Container {
                cache_directory_path: cache_directory,
                ..
            } => cache_directory.join("nix.sif"),
        }
    }

    pub fn store_image_path(&self) -> PathBuf {
        match self {
            NixEnvironment::Native => unreachable!(),
            NixEnvironment::Container {
                cache_directory_path: cache_directory,
                ..
            } => cache_directory.join("store.img"),
        }
    }

    fn cache_directory_path(&self) -> &Path {
        match self {
            NixEnvironment::Native => unreachable!(),
            NixEnvironment::Container {
                cache_directory_path: cache_directory,
                ..
            } => &cache_directory,
        }
    }
}

#[derive(Error, Debug)]
pub enum CacheDirectoryRetreivalError {
    #[error("failed to read XDG_CACHE_HOME: {0}")]
    XdgCacheHomeRetreival(std::env::VarError),

    #[error("failed to read HOME: {0}")]
    HomeRetreival(std::env::VarError),
}

fn get_cache_directory_path() -> Result<PathBuf, CacheDirectoryRetreivalError> {
    Ok(std::env::var("XDG_CACHE_HOME")
        .map_err(|err| CacheDirectoryRetreivalError::XdgCacheHomeRetreival(err))
        .map(|cache_home| PathBuf::from(cache_home).join("nixflow"))
        .or(std::env::var("HOME")
            .map_err(|err| CacheDirectoryRetreivalError::HomeRetreival(err))
            .map(|home| PathBuf::from(home).join(".nixflow")))?)
}
