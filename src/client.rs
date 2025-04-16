use crate::{error::Error, login::Secrets};
use git_version::git_version;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_repr::Deserialize_repr;
use std::collections::HashMap;

pub const PROGRAM_VERSION: &str = git_version!(
	prefix = "git-",
	cargo_prefix = "cargo-",
	fallback = "unknown"
);
pub const BASE_URL: &str = "https://fumosclubv1.vercel.app";
pub const DOMAIN: &str = "fumosclubv1.vercel.app";

pub fn get_user_agent() -> String {
	format!("fumo/{PROGRAM_VERSION}; (https://github.com/techs-sus/fumosync)")
}

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
	pub r#type: ScriptType,
	pub creator: String,
	pub creator_icon: String,
	pub editable: bool,
	pub is_favorite: bool,
}

#[derive(Deserialize, Clone)]
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

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Editor {
	pub success: bool,
	pub script_info: EditorScriptInfo,
}

#[derive(Debug, Clone)]
pub enum EditorUpdate<'a> {
	Description(&'a str),
	Module { name: &'a str, source: &'a str },
	MainSource(&'a str),
	// Vec<Id>; directly writes to database
	Whitelist(Vec<&'a str>),
	Name(&'a str),
	Publicity(bool),
}

#[derive(Deserialize, Clone)]
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
			client: reqwest::Client::builder()
				.user_agent(get_user_agent())
				.https_only(true)
				.build()
				.expect("failed building inner reqwest client"),
		}
	}

	/// Returns `Ok(())` if the user is authenticated.
	///
	/// # Errors
	/// - [`Error::NotLoggedIn`]
	/// - [`Error::UserIsBanned`]
	/// - [`Error::InsufficentAuthorization`]
	/// - [`Error::FumosclubAPI`]
	/// - [`Error::Reqwest`]
	/// - [`Error::Serde`]
	pub async fn ensure_user_authenticated(&self) -> Result<(), Error> {
		#[derive(Deserialize)]
		struct InitialResponse {
			success: bool,
			role: Option<i32>,
			error: Option<String>,
		}

		let value: InitialResponse = serde_json::from_slice(
			&self
				.client
				.get(format!("{BASE_URL}/api/auth/auth"))
				.header(
					"Cookie",
					format!("session={}", self.secrets.session.clone()),
				)
				.send()
				.await?
				.bytes()
				.await?,
		)?;

		if value.success {
			let role = value.role.unwrap();

			if role == -1 {
				// not logged in
				return Err(Error::NotLoggedIn);
			} else if role == -2 {
				// TODO: Get ban expiry; there is no clear way?
				#[derive(Deserialize)]
				struct Ban {
					reason: Option<String>,
				}
				#[derive(Deserialize)]
				struct BanData {
					ban: Option<Ban>,
				}

				let ban_data: BanData = serde_json::from_slice(
					&self
						.client
						.get(format!("{BASE_URL}/api/auth/getbandata"))
						.header(
							"Cookie",
							format!("session={}", self.secrets.session.clone()),
						)
						.send()
						.await?
						.bytes()
						.await?,
				)?;

				if let Some(ban) = ban_data.ban {
					return Err(Error::UserIsBanned { reason: ban.reason });
				}
			} else if role < 1 {
				// unauthorized
				return Err(Error::InsufficentAuthorization);
			};

			Ok(())
		} else {
			// ??? internal fumosclub
			Err(Error::FumosclubAPI(
				value
					.error
					.unwrap_or_else(|| String::from("(no error provided)")),
			))
		}
	}

	/// Gets the current logged in account's details.
	///
	/// # Errors
	/// - [`Error::Reqwest`]
	/// - [`Error::Serde`]
	pub async fn get_details(&self) -> Result<AccountDetails, Error> {
		Ok(serde_json::from_slice(
			&self
				.client
				.get(format!("{BASE_URL}/api/account/getdetails"))
				.header(
					"Cookie",
					format!("session={}", self.secrets.session.clone()),
				)
				.send()
				.await?
				.bytes()
				.await?,
		)?)
	}

	/// Generates a key for a fumosclub script.
	///
	/// # Errors
	/// - [`Error::InvalidKeyGenerationTarget`]
	/// - [`Error::Reqwest`]
	/// - [`Error::ResponseStatus`]
	/// - [`Error::Serde`]
	pub async fn generate_key(&self, id: &str) -> Result<String, Error> {
		#[derive(Deserialize)]
		struct Key {
			success: bool,
			require: String,
		}

		match self
			.client
			.post(format!("{BASE_URL}/api/script/generatekey"))
			.header(
				"Cookie",
				format!("session={}", self.secrets.session.clone()),
			)
			.header("Content-Type", "application/json")
			.body(serde_json::to_string(&json!({
				"scriptId": id
			}))?)
			.send()
			.await?
			.error_for_status()
		{
			Ok(response) => {
				let value: Key = serde_json::from_slice(&response.bytes().await?)?;

				Ok(value.require)
			}

			Err(error) => {
				let status = error.status().unwrap_or_else(|| unreachable!());

				Err(if status == 400 {
					Error::InvalidKeyGenerationTarget
				} else {
					Error::ResponseStatus(status)
				})
			}
		}
	}

	/// Lists all scripts this account can access.
	///
	/// # Errors
	/// - [`Error::Reqwest`]
	/// - [`Error::Serde`]
	pub async fn list_scripts(&self) -> Result<ScriptList, Error> {
		Ok(serde_json::from_slice(
			&self
				.client
				.get(format!("{BASE_URL}/api/script/home/getscripts"))
				.header(
					"Cookie",
					format!("session={}", self.secrets.session.clone()),
				)
				.send()
				.await?
				.bytes()
				.await?,
		)?)
	}

	/// Gets the editor (source data) for a script or package id.
	///
	/// # Errors
	/// - [`Error::Reqwest`]
	/// - [`Error::Serde`]
	pub async fn get_editor(&self, id: &str) -> Result<Editor, Error> {
		Ok(serde_json::from_slice(
			&self
				.client
				.get(format!("{BASE_URL}/api/script/editor"))
				.query(&[("id", id)])
				.header(
					"Cookie",
					format!("session={}", self.secrets.session.clone()),
				)
				.send()
				.await?
				.bytes()
				.await?,
		)?)
	}

	/// Updates a script or package, via the editor API.
	///
	/// # Errors
	/// - [`Error::Reqwest`]
	/// - [`Error::Serde`]
	pub async fn set_editor(&self, id: &str, updates: &[EditorUpdate<'_>]) -> Result<(), Error> {
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
					request_body.script_info.whitelist = Some(whitelist.clone());
				}
				EditorUpdate::Name(name) => request_body.script_info.name = Some(name),
				EditorUpdate::Publicity(public) => request_body.script_info.is_public = Some(*public),
			}
		}

		let response = self
			.client
			.patch(format!("{BASE_URL}/api/script/editor"))
			.header(
				"Cookie",
				format!("session={}", self.secrets.session.clone()),
			)
			.header("Content-Type", "application/json")
			.body(serde_json::to_string(&request_body)?)
			.send()
			.await?;

		match response.error_for_status() {
			Ok(..) => Ok(()),
			Err(error) => Err(Error::ResponseStatus(error.status().expect("must exist"))),
		}
	}
}
