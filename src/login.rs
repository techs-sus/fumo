use crate::{
	client::{BASE_URL, Client, DOMAIN},
	error::{Context, Error},
	project::{read_file, write_file},
};
use chrono::{DateTime, Utc};
use chrono::{Months, serde::ts_seconds};
use directories::ProjectDirs;
use headless_chrome::{
	Browser, LaunchOptionsBuilder, browser::default_executable, protocol::cdp::Target::CreateTarget,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

pub fn get_config_directory() -> Result<PathBuf, Error> {
	Ok(
		ProjectDirs::from("com", "techs-sus", "fumosclub cli")
			.context(Error::ConfigDirectoryNotFound)?
			.config_local_dir()
			.to_path_buf(),
	)
}

/// secrets.json
#[derive(Deserialize, Serialize, Debug, Clone)]
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

pub async fn use_browser_token() -> Secrets {
	let secrets = rookie::load(Some(vec![DOMAIN.to_string()]))
		.unwrap()
		.into_iter()
		.filter(|cookie| cookie.name == "session")
		.map(|cookie| Secrets {
			session: cookie.value,
			expires: DateTime::from_timestamp(
				cookie
					.expires
					.map(|expiry| expiry as i64)
					.unwrap_or_else(|| {
						Utc::now()
							.checked_add_months(Months::new(3))
							.unwrap()
							.timestamp()
					}),
				0,
			)
			.unwrap(),
		})
		.collect::<Vec<Secrets>>();

	match secrets.is_empty() {
		true => {
			panic!("no session cookies were found in any browser supported by rookie");
		}

		// show deduplicated list of cookies, and ask user which one to use
		false => {
			let option_to_session =
				futures::future::join_all(secrets.into_iter().map(|secret| async move {
					let client = Client::new(secret.clone());
					(secret, client.get_details().await)
				}))
				.await
				.into_iter()
				.filter_map(|(secret, result)| result.map(|details| (secret, details)).ok())
				.map(|(secret, details)| {
					(
						format!(
							"{} ({}, roblox user {})",
							details.name, details.id, details.roblox_user
						),
						secret,
					)
				})
				.collect::<HashMap<String, Secrets>>();

			let select =
				inquire::Select::new("Pick a session to use.", option_to_session.keys().collect());
			let selected_key = select.prompt().unwrap();

			option_to_session[selected_key].to_owned()
		}
	}
}

pub fn use_headful_chrome() -> Secrets {
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
			for_tab: None,
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

	let session = loop {
		if let Some(session) = tab
			.get_cookies()
			.expect("failed getting cookies")
			.into_iter()
			.find(|cookie| cookie.name == "session")
		{
			break session;
		}

		std::thread::yield_now();
	};

	Secrets {
		session: session.value,
		expires: DateTime::from_timestamp(session.expires as i64, 0u32)
			.expect("failed creating DateTime<Utc> for session expiry"),
	}
}
