#[macro_use]
extern crate clap;
extern crate chrono;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

use std::env;
use std::fs;
use std::fs::{OpenOptions, File};
use std::io::{Write, ErrorKind};
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use clap::{App, Arg, AppSettings, SubCommand};
use serde_json::Error;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Period {
    project: String,
    start_time: DateTime<Utc>,
    end_time: Option<DateTime<Utc>>,
}

fn main() {
    let matches = App::new("Doug")
        .version(crate_version!())
        .about("A time tracking command-line utility")
        .author(crate_authors!())
        .settings(&[
            AppSettings::DeriveDisplayOrder,
            AppSettings::GlobalVersion,
            AppSettings::SubcommandRequiredElseHelp,
            AppSettings::VersionlessSubcommands,
            AppSettings::DisableHelpSubcommand,
        ])
        .subcommand(SubCommand::with_name("start")
            .about("Track new or existing project")
            .arg(Arg::with_name("project")
                .help("project to track")
                .required(true)))
        .subcommand(SubCommand::with_name("status")
            .about("Display elapsed time, start time, and running project name"))
        .subcommand(SubCommand::with_name("stop")
            .about("Stop any running projects"))
        .subcommand(SubCommand::with_name("cancel")
            .about("Stop running project and remove most recent time interval"))
        .subcommand(SubCommand::with_name("restart")
            .about("Track last running project"))
        .subcommand(SubCommand::with_name("log")
            .about("Display time intervals across all projects"))
        .subcommand(SubCommand::with_name("report")
            .about("Display aggregate time from last week"))
        .subcommand(SubCommand::with_name("amend")
            .about("Change name of currently running project")
            .arg(Arg::with_name("project")
                .help("new project name")
                .required(true)))
        .subcommand(SubCommand::with_name("edit")
            .about("Edit last frame or currently running frame")
            .arg(Arg::with_name("project")
                .help("project to track")))
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("start") {
        if matches.is_present("project") {
            start(matches.value_of("project").unwrap());
        }
    }

    if let Some(matches) = matches.subcommand_matches("amend") {
        if matches.is_present("project") {
            amend(matches.value_of("project").expect("missing project name"));
        }
    }

    if let Some(matches) = matches.subcommand_matches("edit") {
        edit(matches.value_of("project"));
    }

    match matches.subcommand_name() {
        Some("stop") => stop(),
        Some("cancel") => cancel(),
        Some("restart") => restart(),
        Some("log") => log(),
        Some("report") => report(),
        _ => {},
    }
}


fn start(project_name: &str) {

    let mut periods = get_periods();

    if !periods.is_empty() {
        let last_index = periods.len() - 1;
        if let Some(period) = periods.get_mut(last_index) {
            if period.end_time.is_none() {
                eprintln!("Sorry, you're currently tracking project `{}`, started `{}`", period.project, period.start_time);
                eprintln!("Try stopping your current project with `stop` first.`");
                return
            }
        }
    } 
    let current_period = create_period(project_name);
    // store current period in file
    print!("Started tracking project '{}'", current_period.project);
    periods.push(current_period);
    save_periods(periods.to_vec());
}

fn stop() {
    let mut periods = get_periods();
    let mut updated_period = false;
    let last_index = periods.len() - 1;
    if periods.len() > 0 {
        if let Some(period) = periods.get_mut(last_index) {
            if period.end_time.is_none() {
                period.end_time = Some(Utc::now());
                updated_period = true;
                println!("Stopped project: `{}`", period.project);
            }
        }
    }
    if updated_period {
        save_periods(periods);
    } else {
        eprintln!("No running project to stop.");
    }
}

fn cancel() {
    unimplemented!();
}

fn restart() {
    unimplemented!();
}

fn log() {
    unimplemented!();
}

fn report() {
    unimplemented!();
}

fn amend(project_name: &str) {
    println!("Amend project: {:?}", project_name);
    unimplemented!();
}


fn edit(project_name: Option<&str>) {
    if let Some(name) = project_name {
        println!("Edit project: {}", name);
    }
    unimplemented!();
}

fn create_period(project: &str) -> Period {
    Period {
        project: String::from(project),
        start_time: Utc::now(),
        end_time: None,
    }
}

fn get_config_folder() -> PathBuf {
    let home_dir = env::var("HOME").expect("Failed to find home directory from environment 'HOME'");
    let mut config_folder = PathBuf::from(home_dir);
    config_folder.push(".config/doug");
    config_folder
}

fn get_config_file_path() -> PathBuf {
    let config_folder = get_config_folder();
    let mut config_file = PathBuf::from(&config_folder);
    config_file.push("periods.json");
    config_file
}

fn get_config_back_file_path() -> PathBuf {
    let config_file = get_config_file_path();
    config_file.with_extension("json-backup")
}

fn get_config_file() -> File {
    let config_file = get_config_file_path();
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&config_file)
        .expect(&format!("Couldn't open configuration file: {:?}", config_file))
}


fn get_periods() -> Vec<Period> {
    let config_folder = get_config_folder();

    match fs::create_dir(&config_folder) {
        Err(ref error) if error.kind() == ErrorKind::AlreadyExists => {},
        Err(error) => panic!("There was a problem creating the config directory: {:?}", error),
        Ok(_) => {},
    }

    let file = get_config_file();

    let periods: Result<Vec<Period>, Error> = serde_json::from_reader(file);
    let periods = match periods {
        Ok(p) => p,
        Err(ref error) if error.is_eof() => Vec::new(),
        Err(error) => panic!("There was a serialization issue: {:?}", error),
    };
    periods
}

fn save_periods(periods: Vec<Period>) {
    let serialized = serde_json::to_string(&periods).expect("Couldn't serialize data to string");
    let config_file = get_config_file_path();
    let config_file_backup = get_config_back_file_path();
    fs::copy(&config_file, &config_file_backup).expect("Couldn't create backup file");
    let mut file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&config_file)
                    .expect("Couldn't open file for saving period.");

    file.write_all(serialized.as_bytes()).expect("Couldn't write serialized data to file");
}
