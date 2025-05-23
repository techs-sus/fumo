use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use std::path::PathBuf;
use thiserror::Error;
use tokio::io;

#[derive(Error, Debug)]
pub enum Error {
	#[error("the directory at {0} already exists")]
	DirectoryAlreadyExists(PathBuf),
	#[error("failed creating file: {0}; {1}")]
	CreateFile(PathBuf, io::Error),
	#[error("failed creating directory: {0}; {1}")]
	CreateDirectory(PathBuf, io::Error),
	#[error("failed reading file: {0}; {1}")]
	ReadFile(PathBuf, io::Error),
	#[error("failed reading directory: {0}; {1}")]
	ReadDirectory(PathBuf, io::Error),
	#[error("failed initializing project: {0}")]
	ProjectDidntInitialize(Box<Error>),
	#[error("reqwest error: {0}")]
	Reqwest(#[from] reqwest::Error),
	#[error("serde error: {0}")]
	Serde(#[from] serde_json::Error),
	#[error("generic io error: {0}")]
	Io(#[from] io::Error),
	#[error("got error response status: {0}")]
	ResponseStatus(StatusCode),
	#[error("secrets expired at {0}")]
	SecretsExpired(DateTime<Utc>),
	#[error("failed finding config directory")]
	ConfigDirectoryNotFound,
	#[error("failed diffing paths")]
	PathDiffFailed,
	#[error("secrets do not have a high enough role")]
	InsufficentAuthorization,
	#[error("invalid secrets; authentication required")]
	InvalidSecrets,
	#[error("the id specified for key generation is either invalid or designated for a package")]
	InvalidKeyGenerationTarget,
	#[error("invalid secrets; API returned a \"not logged in\" response")]
	NotLoggedIn,
	#[error("the user is banned for {:?}", .reason.as_ref().map_or_else(|| "(no reason provided)", String::as_str))]
	UserIsBanned { reason: Option<String> },
	#[error("fumosclub api error: {0}")]
	FumosclubAPI(String),
}

/// Custom context trait to convert a Option to a Result.
pub trait Context<T, E>
where
	Self: Sized,
{
	fn context(self, error: E) -> Result<T, E>;
	fn with_context<F: FnOnce() -> E>(self, f: F) -> Result<T, E>;
}

impl<T, E> Context<T, E> for Option<T> {
	fn context(self, error: E) -> Result<T, E> {
		self.map_or_else(|| Err(error), |value| Ok(value))
	}

	fn with_context<F: FnOnce() -> E>(self, f: F) -> Result<T, E> {
		self.map_or_else(|| Err(f()), |value| Ok(value))
	}
}
