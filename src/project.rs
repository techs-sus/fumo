use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, path::PathBuf};

/// fumosync.json
#[derive(Deserialize, Serialize)]
pub struct Configuration {
	#[serde(rename = "scriptName")]
	pub script_name: String,
}

/// secrets.json
#[derive(Deserialize, Serialize)]
pub struct Secrets {
	pub session: String,
}

pub async fn init(directory: PathBuf) {
	if directory.exists() {
		eprintln!("the directory already exists");
		return;
	}

	tokio::fs::create_dir(directory.as_path())
		.await
		.expect("failed creating directory");

	tokio::fs::create_dir(directory.as_path().join("pkg"))
		.await
		.unwrap_or_else(|_| panic!("failed creating {}{}pkg",
			directory.display(),
			std::path::MAIN_SEPARATOR));

	tokio::fs::create_dir(directory.as_path().join(".vscode"))
		.await
		.unwrap_or_else(|_| panic!("failed creating {}{}.vscode",
			directory.display(),
			std::path::MAIN_SEPARATOR));

	tokio::fs::write(
		directory.as_path().join(".vscode").join("settings.json"),
		r#"{
	"luau-lsp.types.robloxSecurityLevel": "None",
	"luau-lsp.types.definitionFiles": ["types.d.luau"]
}"#,
	)
	.await
	.unwrap_or_else(|_| panic!("failed creating {}{}.vscode{}settings.json",
		directory.display(),
		std::path::MAIN_SEPARATOR,
		std::path::MAIN_SEPARATOR));

	tokio::fs::write(
		directory.as_path().join("init.server.luau"),
		r#"-- you can require packages with requireM("path") where path is a file inside of pkg (no extension)"#,
	)
	.await
	.unwrap_or_else(|_| panic!("failed creating {}{}init.server.luau",
		directory.display(),
		std::path::MAIN_SEPARATOR));

	tokio::fs::write(
		directory.as_path().join("types.d.luau"),
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
	.await
	.unwrap_or_else(|_| {
		panic!(
			"failed creating {}{}types.d.luau",
			directory.display(),
			std::path::MAIN_SEPARATOR
		)
	});

	tokio::fs::write(
		directory.as_path().join("fumosync.json"),
		serde_json::to_string_pretty(&Configuration {
			script_name: directory
				.file_name()
				.unwrap_or(OsStr::new("unknown"))
				.to_string_lossy()
				.to_string(),
		})
		.expect("failed serializing example fumosync.json"),
	)
	.await
	.unwrap_or_else(|_| {
		panic!(
			"failed creating {}{}fumosync.json",
			directory.display(),
			std::path::MAIN_SEPARATOR
		)
	});
}
