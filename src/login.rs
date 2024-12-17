use crate::{
	client::BASE_URL,
	error::{Context, Error},
	project::{read_file, write_file},
};
use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use headless_chrome::protocol::cdp::Network::Cookie;
use headless_chrome::{
	browser::default_executable, protocol::cdp::Target::CreateTarget, Browser, LaunchOptionsBuilder,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn get_config_directory() -> Result<PathBuf, Error> {
	Ok(
		ProjectDirs::from("com", "techs-sus", "fumosclub cli")
			.context(Error::ConfigDirectoryNotFound)?
			.config_local_dir()
			.to_path_buf(),
	)
}

/// secrets.json
#[derive(Deserialize, Serialize, Debug)]
pub struct Secrets {
	pub session: String,
	#[serde(with = "ts_seconds")]
	pub expires: DateTime<Utc>,
}

/// Forcefully saves session secrets.
pub async fn save_session_secrets(secrets: Secrets) -> Result<(), Error> {
	write_file(
		get_config_directory()?.join("secrets.json"),
		&serde_json::to_string_pretty(&secrets)?,
	)
	.await
}

/// Gets session secrets, errors if secrets are expired.
pub async fn get_session_secrets() -> Result<Secrets, Error> {
	let secrets_string = read_file(get_config_directory()?.join("secrets.json")).await?;
	let secrets: Secrets = serde_json::from_str(&secrets_string)?;
	if secrets.expires <= Utc::now() {
		return Err(Error::SecretsExpired(secrets.expires));
	}

	let client = crate::client::Client::new(secrets);
	client.ensure_user_authenticated().await?;

	Ok(client.secrets)
}

/// Returns a session cookie.
pub fn use_browser_token() -> Secrets {
	let browser = Browser::new(
		LaunchOptionsBuilder::default()
			.headless(false)
			.path(Some(
				default_executable().expect("failed finding chrome/chromium executable"),
			))
			.build()
			.expect("failed building launch options"),
	)
	.expect("failed creating browser");

	let tab = browser
		.new_tab_with_options(CreateTarget {
			url: BASE_URL.to_string(),
			width: None,
			height: None,
			browser_context_id: None,
			enable_begin_frame_control: None,
			new_window: None,
			background: None,
		})
		.expect("failed creating new tab");

	tab
		.wait_until_navigated()
		.expect("failed waiting for new tab to navigate");

	// cleans up tabs which Magically existed
	let id = tab.get_target_id();
	browser
		.get_tabs()
		.lock()
		.expect("to lock browser")
		.iter()
		.for_each(|tab| {
			if tab.get_target_id() != id {
				tab.close(false).ok();
			}
		});

	while tab
		.get_cookies()
		.expect("failed getting cookies")
		.is_empty()
	{
		std::thread::yield_now();
	}

	let session: Cookie = tab
		.get_cookies()
		.expect("failed getting cookies")
		.into_iter()
		.find(|cookie| cookie.name == "session")
		.expect("failed finding session cookie");

	Secrets {
		session: session.value,
		expires: DateTime::from_timestamp(session.expires as i64, 0_u32)
			.expect("failed creating DateTime<Utc> for session expiry"),
	}
}
