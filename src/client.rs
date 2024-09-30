use crate::project::Secrets;
use serde::Deserialize;
const USER_AGENT: &str = "fumosync-rs (github.com/techs-sus/fumosync)";
const BASE_URL: &str = "https://fumosclubv1.vercel.app";

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountDetails {
	pub success: bool,
	pub id: String,
	pub name: String,
	pub icon: String,
	pub roblox_user: String,
	pub discord_user_id: String,
	pub num_sessions: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ScriptList {
	pub success: bool,
	pub scripts: Vec<Script>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Script {
	pub id: String,
	pub name: String,
	pub description: String,
	#[serde(rename = "type")]
	pub script_type: i64,
	pub creator: String,
	pub creator_icon: String,
	pub editable: bool,
	pub is_favorite: bool,
}

pub struct Client {
	pub secrets: Secrets,
	client: reqwest::Client,
}

impl Client {
	pub fn new(secrets: Secrets) -> Self {
		Self {
			secrets,
			client: reqwest::Client::new(),
		}
	}

	pub async fn get_details(&self) -> anyhow::Result<AccountDetails> {
		Ok(serde_json::from_slice(
			&self
				.client
				.get(format!("{BASE_URL}/api/account/getdetails"))
				.header(
					"Cookie",
					format!("session={}", self.secrets.session.clone()),
				)
				.header("User-Agent", USER_AGENT)
				.send()
				.await?
				.bytes()
				.await?,
		)?)
	}

	pub async fn list_scripts(&self) -> anyhow::Result<ScriptList> {
		Ok(serde_json::from_slice(
			&self
				.client
				.get(format!("{BASE_URL}/api/script/home/getscripts"))
				.header(
					"Cookie",
					format!("session={}", self.secrets.session.clone()),
				)
				.header("User-Agent", USER_AGENT)
				.send()
				.await?
				.bytes()
				.await?,
		)?)
	}
}
