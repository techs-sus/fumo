use crate::project::Secrets;
use serde::{Deserialize, Serialize};
use serde_repr::Deserialize_repr;
use std::collections::HashMap;
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

#[derive(Deserialize_repr, Debug, Clone)]
#[repr(u8)]
pub enum ScriptType {
	Regular = 0,
	Package = 1,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Script {
	pub id: String,
	pub name: String,
	pub description: String,
	#[serde(rename = "type")]
	pub script_type: ScriptType,
	pub creator: String,
	pub creator_icon: String,
	pub editable: bool,
	pub is_favorite: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EditorScriptInfo {
	pub name: String,
	#[serde(rename = "type")]
	pub script_type: ScriptType,
	pub description: String,
	pub is_public: bool,
	pub whitelist: Vec<String>,
	pub source: Source,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Editor {
	pub success: bool,
	pub script_info: EditorScriptInfo,
}

#[derive(Debug)]
pub enum EditorUpdate<'a> {
	Description(&'a str),
	Module { name: &'a str, source: &'a str },
	MainSource(&'a str),
	// Vec<Id>; directly writes to database
	Whitelist(Vec<&'a str>),
	Name(&'a str),
	Publicity(bool),
}

#[derive(Deserialize, Debug, Clone)]
pub struct Source {
	pub main: String,
	// key -> source
	pub modules: HashMap<String, String>,
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

	/// Gets the editor for an id.
	pub async fn get_editor(&self, id: &str) -> anyhow::Result<Editor> {
		Ok(serde_json::from_slice(
			&self
				.client
				.get(format!("{BASE_URL}/api/script/editor"))
				.query(&[("id", id)])
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

	pub async fn set_editor(&self, id: &str, updates: &[EditorUpdate<'_>]) -> anyhow::Result<()> {
		// We use the lifetime 'a to denote that these structs will only live throughout this function.
		#[derive(Serialize, Debug, Clone)]
		#[serde(rename_all = "camelCase")]
		struct ScriptInfo<'a> {
			pub source: Source<'a>,
			#[serde(skip_serializing_if = "Option::is_none")]
			pub description: Option<&'a str>,
			#[serde(skip_serializing_if = "Option::is_none")]
			pub whitelist: Option<Vec<&'a str>>,
			#[serde(skip_serializing_if = "Option::is_none")]
			pub name: Option<&'a str>,
			#[serde(skip_serializing_if = "Option::is_none")]
			pub is_public: Option<bool>,
		}

		#[derive(Serialize, Debug, Clone)]
		#[serde(rename_all = "camelCase")]
		struct Source<'a> {
			#[serde(skip_serializing_if = "Option::is_none")]
			pub modules: Option<HashMap<&'a str, &'a str>>,
			#[serde(skip_serializing_if = "Option::is_none")]
			pub main: Option<&'a str>,
		}

		#[derive(Serialize, Debug, Clone)]
		#[serde(rename_all = "camelCase")]
		struct SetEditor<'a> {
			pub script_id: &'a str,
			pub script_info: ScriptInfo<'a>,
		}

		let mut request_body = SetEditor {
			script_id: id,
			script_info: ScriptInfo {
				source: Source {
					modules: None,
					main: None,
				},
				whitelist: None,
				description: None,
				name: None,
				is_public: None,
			},
		};

		for update in updates {
			match update {
				EditorUpdate::Description(value) => request_body.script_info.description = Some(value),
				EditorUpdate::Module { name, source } => match request_body.script_info.source.modules {
					None => {
						request_body.script_info.source.modules = Some(HashMap::from([(*name, *source)]));
					}
					Some(ref mut modules) => {
						modules.insert(*name, *source);
					}
				},
				EditorUpdate::MainSource(source) => {
					request_body.script_info.source.main = Some(source);
				}
				EditorUpdate::Whitelist(whitelist) => {
					request_body.script_info.whitelist = Some(whitelist.clone())
				}
				EditorUpdate::Name(name) => request_body.script_info.name = Some(name),
				EditorUpdate::Publicity(public) => request_body.script_info.is_public = Some(*public),
			}
		}
		self
			.client
			.put(format!("{BASE_URL}/api/script/editor"))
			.header(
				"Cookie",
				format!("session={}", self.secrets.session.clone()),
			)
			.header("User-Agent", USER_AGENT)
			.header("Content-Type", "application/json")
			.body(serde_json::to_string(&request_body)?)
			.send()
			.await?
			.bytes()
			.await?;
		Ok(())
	}
}
