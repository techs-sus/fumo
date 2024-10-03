mod client;
mod error;
mod login;
mod project;

use clap::{Parser, Subcommand};
use client::Client;
use error::Error;
use login::{get_config_directory, get_session_secrets, save_session_secrets, use_browser_token};
use project::{init, pull_project, push_project, read_configuration};
use std::path::PathBuf;
use tracing::warn;

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
	/// Logs into fumosclub.
	Login,
	/// Shows infomation about the current fumosclub account.
	View,
	/// Initializes a project in the directory.
	Init { project_directory: PathBuf },
	/// Lists all projects under the logged in fumosclub account.
	List,
	/// Pulls down a script from fumosclub (the script must be editable).
	Pull {
		script_id: String,
		project_directory: PathBuf,
	},
	/// Push the script in current directory to fumosclub using the fumosync.json file.
	Push,
	/// Generates a key for a script under the logged in fumosclub account.
	Generate {
		#[arg(long)]
		id: Option<String>,
	},
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
	match main_fn().await {
		Err(e) => {
			tracing::error!("{e}")
		}
		Ok(_t) => {}
	}
}

async fn main_fn() -> Result<(), Error> {
	tracing_subscriber::fmt()
		.compact()
		.with_target(false)
		.without_time()
		.with_level(true)
		.init();
	warn!("fumosync is beta software; please report bugs to https://github.com/techs-sus/fumosync");
	let args = Args::parse();
	ensure_config_directory_exists().await;

	match args.command {
		Command::View => {
			let client = Client::new(get_session_secrets().await?);
			let details = client.get_details().await?;
			println!(
				"{} - {} - {}\n{} currently logged in sessions",
				details.name, details.roblox_user, details.id, details.num_sessions
			)
		}
		Command::Init { project_directory } => init(project_directory).await?,
		Command::Login => save_session_secrets(use_browser_token()).await?,
		Command::List => {
			let client = Client::new(get_session_secrets().await?);
			for script in client.list_scripts().await?.scripts {
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
			pull_project(script_id, project_directory).await?;
		}

		Command::Push => push_project().await?,
		Command::Generate { id } => {
			let client = Client::new(get_session_secrets().await?);
			let id = match id {
				Some(id) => id,
				None => read_configuration().await?.script_id,
			};

			println!("{}", client.generate_key(&id).await?);
		}
	}

	Ok(())
}
