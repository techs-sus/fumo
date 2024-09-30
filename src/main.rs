mod client;
mod login;
mod project;

use clap::{Parser, Subcommand};
use client::Client;
use login::{get_config_directory, get_session_secrets, save_session_secrets, use_browser_token};
use project::init;
use std::path::PathBuf;

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
	/// Logs into fumosclub, and writes the SESSION variable to the env file.
	Login,
	/// Sync's the current directory to fumosclub using the credentials stored in the current directory.
	Sync,
	/// Initializes a project in the directory.
	Init { project_directory: PathBuf },
	/// Lists all projects under this fumosclub account.
	List,
}

/// Fumosync allows you to sync local projects to fumosclub.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
	#[command(subcommand)]
	command: Command,
}

#[tokio::main]
async fn main() {
	if !get_config_directory()
		.try_exists()
		.expect("failed verifying the existance of config dir")
	{
		tokio::fs::create_dir_all(get_config_directory())
			.await
			.expect("failed creating config directory");
	}
	let args = Args::parse();

	match args.command {
		Command::Init { project_directory } => init(project_directory).await,
		Command::Sync => {
			// TODO: implement sync
		}
		Command::Login => save_session_secrets(use_browser_token())
			.await
			.expect("failed to write session"),
		Command::List => {
			let client = Client::new(get_session_secrets().await);
			for script in client
				.list_scripts()
				.await
				.expect("failed getting script list")
				.scripts
			{
				println!(
					"{} {} by {} {}",
					match script.editable {
						true => "★",
						false => "☆",
					},
					script.name,
					script.creator,
					match script.editable {
						true => "📝",
						false => "",
					}
				)
			}
		}
	}
}
