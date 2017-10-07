#[macro_use]
extern crate clap;
extern crate chrono;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

use std::fs;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use chrono::{DateTime, Utc};
use clap::{App, Arg, AppSettings, SubCommand};
use serde_json::Error;

#[derive(Serialize, Deserialize, Debug)]
struct Period<'a> {
    project: &'a str,
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
            .arg(Arg::with_name("repo")
                .help("project to track")))
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("start") {
        if matches.is_present("project") {
            start_project(matches.value_of("project").unwrap());
        }
    }
}


fn start_project(project_name: &str) {
    let path = Path::new("doug.json");
    let path_backup = Path::new("doug-backup.json");

    let mut file = OpenOptions::new()
                    .create(true)
                    .read(true)
                    .write(true)
                    .open(path)
                    .unwrap();

    let mut s = String::new();
    file.read_to_string(&mut s).unwrap();

    let string_length = s.chars().count();
    let mut periods = match string_length {
        0 => Vec::new(),
        _ => {
            let result: Result<Vec<Period>, Error> = serde_json::from_str(&s);
            let periods: Vec<Period> = match result {
                Ok(result) => result,
                Err(error) => panic!("Couldn't deserialize data. Error: {:?}", error),
            };
            periods
        },
    };

    if !periods.is_empty() {
        let last_index = periods.len() - 1;
        if let Some(period) = periods.get_mut(last_index) {
            if period.end_time.is_none() {
                eprintln!("Sorry, you're currently tracking project: {}", period.project);
                eprintln!("Try stopping your current project with `stop` first.`");
                return
            }
        }
    } 
    let current_period = create_period(project_name);
    // store current period in file
    print!("Started tracking project '{}'", current_period.project);
    periods.push(current_period);

    let serialized = serde_json::to_string(&periods).unwrap();
    
    fs::copy(path, path_backup).unwrap();
    let mut file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(path)
                    .unwrap();

    file.write_all(serialized.as_bytes()).unwrap();
}

fn create_period(project: &str) -> Period {
    return Period {
        project: project,
        start_time: Utc::now(),
        end_time: None,
    }
}
