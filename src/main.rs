#![forbid(unsafe_code)]

mod client;
mod error;
mod login;
mod project;

use clap::{Parser, Subcommand};
use client::Client;
use error::Error;
use login::{
	get_config_directory, get_session_secrets, save_session_secrets, use_browser_token,
	use_headful_chrome,
};
use project::{init, pull, push, read_configuration, watch};
use std::path::PathBuf;
use tracing::warn;

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
	/// Login to fumosclub (overwrites existing secrets)
	Login {
		/// Whether or not to spawn an instance of Chrome/Chromium in order to login to fumosclub.
		#[arg(short, long, default_value_t = false)]
		spawn_chromium: bool,
	},
	/// Shows infomation about the logged in account
	View,
	/// Initializes a project in the specified directory
	Init { project_directory: PathBuf },
	/// Lists all projects under the logged in account
	List,
	/// Pulls down a script via the fumosclub API (the script must be editable).
	Pull {
		script_id: String,
		project_directory: PathBuf,
	},
	/// Pushes the script in current directory to fumosclub; data is sourced from [current_directory]/fumosync.json
	Push,
	/// Watches the current directory for changes, and pushes them to fumosclub
	Watch,
	/// Generates a key for a script under the logged in fumosclub account
	Generate {
		/// Id of the script; defaults to the script id in [current_directory]/fumosync.json
		#[arg(long)]
		id: Option<String>,
	},
}

/// fumo is a cli tool built for fumosclub <https://fumosclubv1.vercel.app>
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
	#[command(subcommand)]
	command: Command,
}

async fn ensure_config_directory_exists() {
	if !get_config_directory()
		.expect("failed getting config directory")
		.try_exists()
		.expect("failed verifying the existance of config directory")
	{
		tokio::fs::create_dir_all(get_config_directory().expect("failed getting config directory"))
			.await
			.expect("failed creating config directory");
	}
}

#[tokio::main]
async fn main() {
	if let Err(error) = main_fn().await {
		tracing::error!("{error}");
	}
}

async fn main_fn() -> Result<(), Error> {
	tracing_subscriber::fmt()
		.compact()
		.with_target(false)
		.without_time()
		.with_level(true)
		.init();
	warn!("fumo is alpha software; please report bugs to https://github.com/techs-sus/fumo",);

	let args = Args::parse();

	ensure_config_directory_exists().await;

	match args.command {
		Command::View => {
			let client = Client::new(get_session_secrets().await?);
			let details = client.get_details().await?;
			println!(
				"{} - {} - {}\n{} currently logged in sessions",
				details.name, details.roblox_user, details.id, details.num_sessions
			);
		}
		Command::Init { project_directory } => init(project_directory).await?,
		Command::Login { spawn_chromium } => {
			save_session_secrets(if spawn_chromium {
				use_headful_chrome()
			} else {
				use_browser_token().await
			})
			.await?
		}
		Command::List => {
			let client = Client::new(get_session_secrets().await?);
			for script in client.list_scripts().await?.scripts {
				println!(
					"{} {} ({}) by {} {}",
					if script.is_favorite { "★" } else { "☆" },
					script.name,
					script.id,
					script.creator,
					if script.editable { "🔓" } else { "🔐" }
				);
			}
		}
		Command::Pull {
			script_id,
			project_directory,
		} => {
			pull(script_id, project_directory).await?;
		}

		Command::Push => push().await?,
		Command::Generate { id } => {
			let client = Client::new(get_session_secrets().await?);
			let id = match id {
				Some(id) => id,
				None => read_configuration().await?.script_id,
			};

			println!("{}", client.generate_key(&id).await?);
		}

		Command::Watch => {
			watch().await?;
		}
	}

	Ok(())
}
