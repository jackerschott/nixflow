use std::{intrinsics::unreachable, process::Command};

use anyhow::Result;
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use thiserror::Error;

pub enum NixEnvironment {
    Native,
    Portable {
        store_cache_path: PathBuf,
    },
    PortableDistributed {
        store_cache_path_local: PathBuf,
        store_cache_path_distributed: PathBuf,
    },
}

#[derive(Error, Debug)]
pub enum NixEnvironmentError {

}

impl NixEnvironment {
    pub fn new(
        nix_container_url: String,
        nix_binary_cache_path: PathBuf,
        apptainer_args: Vec<String>,
    ) -> Result<Self, NixEnvironmentError> {
        match Self::new_native() {
            Ok(native_environment) => Ok(native_environment),
            Err(NixEnvironmentError::NixNotAvailable) => {
                Self::new_container(nix_container_url, nix_binary_cache_path, apptainer_args)
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
        nix_binary_cache_path: PathBuf,
        apptainer_args: Vec<String>,
    ) -> Result<Self, NixEnvironmentError> {
        let cache_directory_path = get_nixflow_cache_directory_path()
            .map_err(|err| NixEnvironmentError::FailedCacheDirectoryRetreival(err))?;
        let container_env = Self::Container {
            container_cache_directory_path: cache_directory_path,
            nix_binary_cache_path,
            apptainer_args,
        };

        if !std::fs::exists(container_env.container_cache_directory_path())
            .map_err(|err| NixEnvironmentError::IOError(err))?
        {
            std::fs::create_dir_all(container_env.container_cache_directory_path())
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
                container_cache_directory_path: cache_directory,
                ..
            } => cache_directory.join("nix.sif"),
        }
    }

    pub fn store_image_path(&self) -> PathBuf {
        match self {
            NixEnvironment::Native => unreachable!(),
            NixEnvironment::Container {
                container_cache_directory_path: cache_directory,
                ..
            } => cache_directory.join("store.img"),
        }
    }

    fn container_cache_directory_path(&self) -> &Path {
        match self {
            NixEnvironment::Native => unreachable!(),
            NixEnvironment::Container {
                container_cache_directory_path: cache_directory,
                ..
            } => &cache_directory,
        }
    }

    pub fn nix_store_binary_execution_command(
        &self,
        binary_path: &Path,
        input_output_directory_paths: &Vec<PathBuf>,
    ) -> Command {
        match self {
            NixEnvironment::Native => Command::new(binary_path),
            NixEnvironment::Container { .. } => {
                let mut command = Command::new("apptainer");
                command
                    .arg("exec")
                    .arg("--cleanenv")
                    .arg("--contain")
                    .arg("--overlay")
                    .arg(self.store_image_path());

                for path in input_output_directory_paths {
                    command.arg("--bind");
                    command.arg(format!("{path}:{path}"));
                }

                command
                    .arg(self.nix_container_cache_path())
                    .arg(binary_path);

                return command;
            }
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

fn get_nixflow_cache_directory_path() -> Result<PathBuf, CacheDirectoryRetreivalError> {
    Ok(std::env::var("XDG_CACHE_HOME")
        .map_err(|err| CacheDirectoryRetreivalError::XdgCacheHomeRetreival(err))
        .map(|cache_home| PathBuf::from(cache_home).join("nixflow"))
        .or(std::env::var("HOME")
            .map_err(|err| CacheDirectoryRetreivalError::HomeRetreival(err))
            .map(|home| PathBuf::from(home).join(".nixflow")))?)
}
