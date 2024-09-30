use crate::project::Secrets;
use directories::ProjectDirs;
use headless_chrome::{
	browser::default_executable, protocol::cdp::Target::CreateTarget, Browser, LaunchOptionsBuilder,
};
use std::path::PathBuf;

pub fn get_config_directory() -> PathBuf {
	ProjectDirs::from("com", "techs-sus", "fumosync")
		.expect("to get directories")
		.config_local_dir()
		.to_path_buf()
}

pub async fn save_session_secrets(session: String) -> anyhow::Result<()> {
	let bytes = tokio::fs::read(get_config_directory().join("secrets.json"))
		.await
		.expect("failed reading secrets");

	let mut secrets = serde_json::from_slice::<Secrets>(&bytes)?;
	secrets.session = session;

	tokio::fs::write(
		get_config_directory().join("secrets.json"),
		serde_json::to_string_pretty(&secrets)?,
	)
	.await?;
	Ok(())
}

pub async fn get_session_secrets() -> Secrets {
	let bytes = tokio::fs::read(get_config_directory().join("secrets.json"))
		.await
		.expect("failed reading secrets");

	serde_json::from_slice::<Secrets>(&bytes).expect("failed deserializing secrets")
}

/// Returns a session cookie.
pub fn use_browser_token() -> String {
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
			url: "https://fumosclubv1.vercel.app/".to_owned(),
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
	{}

	let session = tab
		.get_cookies()
		.expect("failed getting cookies")
		.into_iter()
		.find(|cookie| cookie.name == "session")
		.expect("failed finding session cookie");

	session.value
}
