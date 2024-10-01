use crate::{
	client::{Client, EditorUpdate},
	login::get_session_secrets,
};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, path::PathBuf};

/// fumosync.json
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
	pub script_name: String,
	pub script_id: String,
	pub whitelist: Vec<String>,
	pub is_public: bool,
}

/// secrets.json
#[derive(Deserialize, Serialize)]
pub struct Secrets {
	pub session: String,
}

async fn write_file(path: PathBuf, contents: &str) -> anyhow::Result<()> {
	tokio::fs::write(path.as_path(), contents)
		.await
		.context(format!("failed creating file: {}", path.display()))
}

async fn create_directory(path: PathBuf) -> anyhow::Result<()> {
	tokio::fs::create_dir(path.as_path())
		.await
		.context(format!("failed creating directory: {}", path.display()))
}

pub async fn read_configuration() -> anyhow::Result<Configuration> {
	serde_json::from_str(
		&tokio::fs::read_to_string("fumosync.json")
			.await
			.context("failed reading fumosync.json")?,
	)
	.context("failed deserializing fumosync.json")
}

/// Initializes a project for syncing within fumosclub.
pub async fn init(directory: PathBuf) -> anyhow::Result<()> {
	if directory.exists() {
		return Err(anyhow::anyhow!("the directory already exists"));
	}

	create_directory(directory.clone()).await?;
	create_directory(directory.join("pkg")).await?;
	create_directory(directory.join(".vscode")).await?;

	write_file(
		directory.join(".vscode").join("settings.json"),
		r#"{
	"luau-lsp.types.robloxSecurityLevel": "None",
	"luau-lsp.types.definitionFiles": ["types.d.luau"]
}"#,
	)
	.await?;

	write_file(
		directory.join("init.server.luau"),
		r#"-- you can require packages with requireM("path") where path is a file inside of pkg (no extension)"#,
	)
	.await?;

	write_file(directory.join("README.md"), r#"# stuff here"#).await?;

	write_file(
		directory.join("types.d.luau"),
		r#"declare loadstringEnabled: boolean
declare owner: Player
declare arguments: { any }

declare isolatedStorage: {
  get: (name: string) -> any,
  set: (name: string, value: any?) -> ()
}

declare immediateSignals: boolean
declare NLS: (source: string, parent: Instance?) -> LocalScript
declare requireM: (moduleName: string) -> any

declare LoadAssets: (assetId: number) -> {
  Get: (asset: string) -> Instance,
  Exists: (asset: string) -> boolean,
  GetNames: () -> { string },
  GetArray: () -> { Instance },
  GetDictionary: () -> { [string]: Instance }
}"#,
	)
	.await?;

	write_file(
		directory.join("fumosync.json"),
		&serde_json::to_string_pretty(&Configuration {
			script_name: directory
				.file_name()
				.unwrap_or(OsStr::new("unknown"))
				.to_string_lossy()
				.to_string(),
			script_id: "???".to_owned(),
			whitelist: Vec::new(),
			is_public: false,
		})
		.context("failed serializing example fumosync.json")?,
	)
	.await?;

	Ok(())
}

/// Pulls a project from fumosclub and links it via fumosync.json.
pub async fn pull_project(script_id: String, project_directory: PathBuf) -> anyhow::Result<()> {
	let client = Client::new(get_session_secrets().await?);

	// setup initial file structure for hydration
	init(project_directory.clone())
		.await
		.context("failed initing project")?;

	let script_info = client.get_editor(&script_id).await?.script_info;

	write_file(
		project_directory.join("README.md"),
		&script_info.description,
	)
	.await?;

	write_file(
		project_directory.join("init.server.luau"),
		&script_info.source.main,
	)
	.await?;

	write_file(
		project_directory.join("fumosync.json"),
		&serde_json::to_string_pretty(&Configuration {
			script_name: script_info.name,
			script_id,
			whitelist: script_info.whitelist,
			is_public: script_info.is_public,
		})?,
	)
	.await?;

	for (name, source) in script_info.source.modules {
		write_file(
			project_directory.join("pkg").join(format!("{name}.luau")),
			&source,
		)
		.await?;
	}

	Ok(())
}

pub async fn push_project() -> anyhow::Result<()> {
	let configuration = read_configuration().await?;
	let whitelist = configuration.whitelist.iter().map(|x| x.as_str()).collect();

	let description = &tokio::fs::read_to_string("README.md")
		.await
		.context("failed reading description (README.md)")?;

	let main_source = &tokio::fs::read_to_string("init.server.luau")
		.await
		.context("failed reading main source (init.server.luau)")?;

	let mut actions: Vec<EditorUpdate> = Vec::from([
		EditorUpdate::Name(&configuration.script_name),
		EditorUpdate::Whitelist(whitelist),
		EditorUpdate::Publicity(configuration.is_public),
		EditorUpdate::Description(description),
		EditorUpdate::MainSource(main_source),
	]);

	let mut modules: Vec<(String, String)> = Vec::new();

	let mut stream = tokio::fs::read_dir("pkg")
		.await
		.context("failed reading pkg (modules)")?;

	while let Some(module) = stream.next_entry().await? {
		if let Ok(file_type) = module.file_type().await {
			if file_type.is_file()
				&& module
					.path()
					.extension()
					.unwrap_or(OsStr::new(""))
					.to_string_lossy()
					== "luau"
			{
				let name = module
					.file_name()
					.into_string()
					.expect("failed converting file name to unicode characters");
				let source = tokio::fs::read_to_string(module.path())
					.await
					.with_context(|| format!("failed reading module: {}", module.path().display()))?;
				modules.push((name, source));
			}
		} else {
			println!("failed getting file type for {}", module.path().display());
		}
	}

	// use .iter() to force items to have a lifetime bounded by the function
	for (name, source) in modules.iter() {
		actions.push(EditorUpdate::Module { name, source });
	}

	let client = Client::new(get_session_secrets().await?);
	client
		.set_editor(&configuration.script_id, &actions)
		.await?;
	Ok(())
}
