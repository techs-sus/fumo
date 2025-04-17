use crate::{
	client::{Client, EditorUpdate},
	error::{Context, Error},
	login::get_session_secrets,
};
use notify_debouncer_full::{
	DebounceEventResult, new_debouncer,
	notify::{EventKind, RecursiveMode, event::ModifyKind},
};
use serde::{Deserialize, Serialize};
use std::{
	ffi::OsStr,
	path::{Component, Path, PathBuf},
	sync::Arc,
	time::Duration,
};
use tokio::sync::{Mutex, Notify};
use tracing::{Instrument, info, warn};

pub const SYNC_CONFIGURATION_FILE: &str = "fumosync.json";
pub const MAIN_SCRIPT_FILE: &str = "init.server.luau";
pub const PACKAGE_DIRECTORY: &str = "pkg";
pub const DESCRIPTION_FILE: &str = "README.md";

/// fumosync.json
#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
	pub script_name: String,
	pub script_id: String,
	pub whitelist: Vec<String>,
	pub is_public: bool,
}

pub async fn write_file<T: AsRef<Path>>(path: T, contents: &str) -> Result<(), Error> {
	match tokio::fs::write(path.as_ref(), contents).await {
		Ok(value) => Ok(value),
		Err(io_error) => Err(Error::CreateFile(path.as_ref().to_path_buf(), io_error)),
	}
}

async fn create_directory<T: AsRef<Path>>(path: T) -> Result<(), Error> {
	match tokio::fs::create_dir(path.as_ref()).await {
		Ok(value) => Ok(value),
		Err(io_error) => Err(Error::CreateDirectory(
			path.as_ref().to_path_buf(),
			io_error,
		)),
	}
}

pub async fn read_configuration<T: AsRef<Path>>(
	project_directory: T,
) -> Result<Configuration, Error> {
	Ok(serde_json::from_str(
		&read_file(project_directory.as_ref().join(SYNC_CONFIGURATION_FILE)).await?,
	)?)
}

/// Initializes a project for syncing within fumosclub.
pub async fn init(directory: PathBuf) -> Result<(), Error> {
	if directory.exists() {
		return Err(Error::DirectoryAlreadyExists(directory));
	}

	create_directory(directory.clone()).await?;
	create_directory(directory.join(PACKAGE_DIRECTORY)).await?;
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

	write_file(directory.join(DESCRIPTION_FILE), r"# stuff here").await?;

	write_file(
		directory.join("types.d.luau"),
		r"declare loadstringEnabled: boolean
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
}",
	)
	.await?;

	write_file(
		directory.join(SYNC_CONFIGURATION_FILE),
		&serde_json::to_string_pretty(&Configuration {
			script_name: directory
				.file_name()
				.unwrap_or_else(|| OsStr::new("unknown"))
				.to_string_lossy()
				.to_string(),
			script_id: "???".to_owned(),
			whitelist: Vec::new(),
			is_public: false,
		})?,
	)
	.await?;

	Ok(())
}

/// Pulls a project from fumosclub and links it via fumosync.json.
pub async fn pull(script_id: String, project_directory: PathBuf) -> Result<(), Error> {
	let client = Client::new(get_session_secrets().await?);

	// setup initial file structure for hydration
	match init(project_directory.clone()).await {
		Ok(()) => {}
		Err(e) => return Err(Error::ProjectDidntInitialize(Box::new(e))),
	};

	let script_info = client.get_editor(&script_id).await?.script_info;

	write_file(
		project_directory.join(DESCRIPTION_FILE),
		&script_info.description,
	)
	.await?;

	write_file(
		project_directory.join(MAIN_SCRIPT_FILE),
		&script_info.source.main,
	)
	.await?;

	write_file(
		project_directory.join(SYNC_CONFIGURATION_FILE),
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
			project_directory
				.join(PACKAGE_DIRECTORY)
				.join(format!("{name}.luau")),
			&source,
		)
		.await?;
	}

	Ok(())
}

pub async fn read_file<T: AsRef<Path>>(path: T) -> Result<String, Error> {
	match tokio::fs::read_to_string(path.as_ref()).await {
		Ok(value) => Ok(value),
		Err(io_error) => Err(Error::ReadFile(path.as_ref().to_path_buf(), io_error)),
	}
}

fn get_module_from_path<T: Into<PathBuf>>(file_name: T) -> String {
	let path_without_extension = file_name.into().with_extension("");
	path_without_extension.to_string_lossy().to_string()
}

fn get_editor_updates_from_configuration(configuration: &Configuration) -> [EditorUpdate<'_>; 3] {
	let whitelist = configuration.whitelist.iter().map(String::as_str).collect();

	[
		EditorUpdate::Name(&configuration.script_name),
		EditorUpdate::Whitelist(whitelist),
		EditorUpdate::Publicity(configuration.is_public),
	]
}

pub async fn push<T: AsRef<Path>>(project_directory: T) -> Result<(), Error> {
	let project_directory = project_directory.as_ref();

	let configuration = read_configuration(project_directory).await?;
	let description = &read_file(project_directory.join(DESCRIPTION_FILE)).await?;
	let main_source = &read_file(project_directory.join(MAIN_SCRIPT_FILE)).await?;

	let mut actions: Vec<EditorUpdate> = Vec::from([
		EditorUpdate::Description(description),
		EditorUpdate::MainSource(main_source),
	]);

	actions.extend(get_editor_updates_from_configuration(&configuration));

	let mut modules: Vec<(String, String)> = Vec::new();

	let pkg_path = project_directory.join(PACKAGE_DIRECTORY);
	let mut stream = match tokio::fs::read_dir(&pkg_path).await {
		Ok(value) => value,
		Err(io_error) => return Err(Error::ReadDirectory(pkg_path, io_error)),
	};

	while let Some(module) = stream.next_entry().await? {
		if let Ok(file_type) = module.file_type().await {
			if file_type.is_file()
				&& module
					.path()
					.extension()
					.unwrap_or_else(|| OsStr::new(""))
					.to_string_lossy()
					== "luau"
			{
				let source: String = read_file(module.path()).await?;
				modules.push((get_module_from_path(module.file_name()).to_string(), source));
			}
		} else {
			warn!("failed getting file type for {}", module.path().display());
		}
	}

	// use .iter() to force items to have a lifetime bounded by the function
	for (name, source) in &modules {
		actions.push(EditorUpdate::Module { name, source });
	}

	let client = Client::new(get_session_secrets().await?);
	client
		.set_editor(&configuration.script_id, &actions)
		.await?;
	Ok(())
}

#[derive(Debug)]
enum Update {
	MainSource,
	Description,
	ProjectConfiguration,
	Module(PathBuf),
}

/// Processes all of the updates, uploads them to fumosclub, and clears the vector when done.
async fn process_updates<T: AsRef<Path>>(
	project_directory: T,
	updates: &mut Vec<Update>,
) -> Result<(), Error> {
	let project_directory = project_directory.as_ref();
	/* "why use another vector... very inefficent"
		1. filesystem I/O -> String (direct ownership, no copies or clones)
		2. String -> EditorPair (rust move semantics, no clones or clones)
		3. &EditorPair -> EditorUpdate<'lt> (zero-cost moving abstraction)
	*/
	enum UpdatePair {
		MainSource(String),
		Description(String),
		ProjectConfiguration,
		Module { name: String, source: String },
	}

	let mut editor_updates: Vec<EditorUpdate<'_>> = Vec::with_capacity(updates.len());
	// we must read the project configuration eventually because we need the project's id
	let configuration: Configuration = read_configuration(project_directory).await?;
	let mut update_pairs: Vec<UpdatePair> = Vec::with_capacity(updates.len());

	for update in updates.iter() {
		let pair = match update {
			Update::MainSource => Some(UpdatePair::MainSource(
				read_file(project_directory.join(MAIN_SCRIPT_FILE)).await?,
			)),
			Update::Description => Some(UpdatePair::Description(
				read_file(project_directory.join(DESCRIPTION_FILE)).await?,
			)),
			Update::ProjectConfiguration => Some(UpdatePair::ProjectConfiguration),
			Update::Module(path_buf) => match path_buf.file_name() {
				None => {
					warn!(
						"module at {} has no file name, skipping...",
						path_buf.display()
					);

					None
				}

				Some(file_name) => Some(UpdatePair::Module {
					name: get_module_from_path(file_name),
					source: read_file(project_directory.join(path_buf)).await?,
				}),
			},
		};

		if let Some(pair) = pair {
			update_pairs.push(pair);
		}
	}

	for pair in &update_pairs {
		match pair {
			UpdatePair::MainSource(source) => editor_updates.push(EditorUpdate::MainSource(source)),
			UpdatePair::Description(description) => {
				editor_updates.push(EditorUpdate::Description(description));
			}
			UpdatePair::ProjectConfiguration => {
				editor_updates.extend(get_editor_updates_from_configuration(&configuration));
			}
			UpdatePair::Module { name, source } => {
				editor_updates.push(EditorUpdate::Module { name, source });
			}
		}
	}

	// push updates
	let client = Client::new(get_session_secrets().await?);
	client
		.set_editor(&configuration.script_id, &editor_updates)
		.await?;

	updates.clear();
	Ok(())
}

pub async fn watch(project_directory: PathBuf) -> Result<(), Error> {
	let project_directory = std::fs::canonicalize(project_directory)?;
	push(&project_directory).await?;

	let (sender, mut receiver) = tokio::sync::mpsc::channel(32);

	let mut debouncer = new_debouncer(
		Duration::from_secs(2),
		None,
		move |result: DebounceEventResult| match result {
			Ok(events) => sender
				.blocking_send(events)
				.expect("failed sending event to async task loop"),
			Err(errors) => errors
				.iter()
				.for_each(|error| tracing::error!("got error from debouncer: {error}")),
		},
	)
	.unwrap();

	// Add a path to be watched. All files and directories at that path and
	// below will be monitored for changes.
	debouncer
		.watch(&project_directory, RecursiveMode::NonRecursive)
		.unwrap();

	// Add a path to be watched. All files and directories at that path and
	// below will be monitored for changes.
	debouncer
		.watch(
			project_directory.join(PACKAGE_DIRECTORY),
			RecursiveMode::Recursive,
		)
		.unwrap();

	let updates: Arc<Mutex<Vec<Update>>> = Arc::new(Mutex::new(Vec::with_capacity(16)));
	let notify = Arc::new(Notify::new());

	let updates_arc = updates.clone();
	let notify_arc = notify.clone();

	let update_project_directory = project_directory.clone();
	tokio::spawn(async move {
		loop {
			// wait for updates
			notify_arc.notified().await;
			// by this time, the lock would've already been released
			let mut lock = updates_arc.lock().await;
			// if the lock is empty (which it shouldnt be), we don't clear it
			if !lock.is_empty() {
				let sync_span = tracing::info_span!("sync");

				async {
					info!(
						"processing {} update{}...",
						lock.len(),
						if lock.len() == 1 { "" } else { "s" }
					);

					match process_updates(&update_project_directory, &mut lock).await {
						Ok(..) => {
							info!("synced successfully!");
						}
						Err(e) => warn!("error whilst processing: {e}"),
					};

					// drop lock to prevent deadlocks
					drop(lock);
				}
				.instrument(sync_span)
				.await;
			}
		}
	});

	info!("watcher is ready to receive events");

	while let Some(events) = receiver.recv().await {
		let mut updates = updates.lock().await;
		let starting_len = updates.len();
		for event in events {
			// neovim and other editors send access events every 2 seconds...
			// so we skip events that are useless
			match event.kind {
				EventKind::Other | EventKind::Access(..) | EventKind::Modify(ModifyKind::Metadata(..)) => {
					continue;
				}

				EventKind::Any | EventKind::Modify(..) | EventKind::Create(..) | EventKind::Remove(..) => {}
			};

			for path in &event.paths {
				let watcher_span = tracing::info_span!("watcher");
				// diff the paths to get a relative PathBuf
				let path = diff_paths(path, &project_directory).context(Error::PathDiffFailed)?;
				let is_package = path.parent().is_some_and(|parent| {
					parent
						.file_name()
						.is_some_and(|name| name == PACKAGE_DIRECTORY)
				});

				async {
					let update = if is_package && !path.is_dir() {
						// this is a package file
						info!("got package update at {}", path.display());
						Some(Update::Module(path))
					} else if !is_package && path.is_file() {
						if path == Path::new(MAIN_SCRIPT_FILE) {
							info!("got main source update");
							Some(Update::MainSource)
						} else if path == Path::new(DESCRIPTION_FILE) {
							info!("got description update");
							Some(Update::Description)
						} else if path == Path::new(SYNC_CONFIGURATION_FILE) {
							info!("got project configuration update");
							Some(Update::ProjectConfiguration)
						} else {
							None
						}
					} else {
						None
					};

					if let Some(update) = update {
						updates.push(update);
					}
				}
				.instrument(watcher_span)
				.await;
			}
		}

		// Prevent unnessacary notifications
		if updates.len() > starting_len {
			notify.notify_one();
		}

		drop(updates); // prevent deadlocks
	}

	Ok(())
}

// Licensed under the Apache License, Version 2.0; used under the MIT license as permitted
// https://docs.rs/pathdiff/latest/src/pathdiff/lib.rs.html#32-74
pub fn diff_paths<P, B>(path: P, base: B) -> Option<PathBuf>
where
	P: AsRef<Path>,
	B: AsRef<Path>,
{
	let path = path.as_ref();
	let base = base.as_ref();

	if path.is_absolute() != base.is_absolute() {
		if path.is_absolute() {
			Some(PathBuf::from(path))
		} else {
			None
		}
	} else {
		let mut ita = path.components();
		let mut itb = base.components();
		let mut comps: Vec<Component> = vec![];
		loop {
			match (ita.next(), itb.next()) {
				(None, None) => break,
				(Some(a), None) => {
					comps.push(a);
					comps.extend(ita.by_ref());
					break;
				}
				(None, _) => comps.push(Component::ParentDir),
				(Some(a), Some(b)) if comps.is_empty() && a == b => (),
				(Some(a), Some(Component::CurDir)) => comps.push(a),
				(Some(_), Some(Component::ParentDir)) => return None,
				(Some(a), Some(_)) => {
					comps.push(Component::ParentDir);
					for _ in itb {
						comps.push(Component::ParentDir);
					}
					comps.push(a);
					comps.extend(ita.by_ref());
					break;
				}
			}
		}
		Some(comps.iter().map(|c| c.as_os_str()).collect())
	}
}
