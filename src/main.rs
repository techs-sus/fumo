mod client;
mod login;
mod project;

use clap::{Parser, Subcommand};
use client::Client;
use login::{get_config_directory, get_session_secrets, save_session_secrets, use_browser_token};
use project::{init, pull_project, push_project};
use std::path::PathBuf;

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
	// TODO: Add create script command, key gen
	/// Logs into fumosclub.
	Login,
	/// Initializes a project in the directory.
	Init { project_directory: PathBuf },
	/// Lists all projects under this fumosclub account.
	List,
	/// Pulls down a script from fumosclub (the script must be editable).
	Pull {
		script_id: String,
		project_directory: PathBuf,
	},
	/// Push the script in current directory to fumosclub using the fumosync.json file.
	Push,
}

/// Fumosync allows you to push and pull local projects to fumosclub.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
	#[command(subcommand)]
	command: Command,
}

async fn ensure_config_directory_exists() {
	if !get_config_directory()
		.try_exists()
		.expect("failed verifying the existance of config directory")
	{
		tokio::fs::create_dir_all(get_config_directory())
			.await
			.expect("failed creating config directory");
	}
}

#[tokio::main]
async fn main() {
	ensure_config_directory_exists().await;
	let args = Args::parse();

	match args.command {
		Command::Init { project_directory } => init(project_directory)
			.await
			.expect("failed initializing project"),
		Command::Login => save_session_secrets(use_browser_token())
			.await
			.expect("failed to write session"),
		Command::List => {
			let client = Client::new(
				get_session_secrets()
					.await
					.expect("failed getting session secrets"),
			);
			for script in client
				.list_scripts()
				.await
				.expect("failed getting script list")
				.scripts
			{
				println!(
					"{} {} ({}) by {} {}",
					match script.is_favorite {
						true => "★",
						false => "☆",
					},
					script.name,
					script.id,
					script.creator,
					match script.editable {
						true => "📝",
						false => "",
					}
				)
			}
		}
		Command::Pull {
			script_id,
			project_directory,
		} => {
			pull_project(script_id, project_directory)
				.await
				.expect("failed pulling project from fumosclub");
		}

		Command::Push => push_project()
			.await
			.expect("failed pushing project to fumosclub"),
	}
}
