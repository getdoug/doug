#[macro_use]
extern crate clap;
extern crate chrono;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate colored;


use std::env;
use std::fs;
use std::fs::{OpenOptions, File, Metadata};
use std::io::{Write, ErrorKind};
use std::path::PathBuf;
use std::collections::{HashMap};
use std::process::Command;

use chrono::{DateTime, Date, Utc, Local, Duration};
use clap::{App, Arg, AppSettings, SubCommand};
use serde_json::Error;
use colored::*;

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
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
            .about("Display aggregate time from projects"))
        .subcommand(SubCommand::with_name("amend")
            .about("Change name of currently running project")
            .arg(Arg::with_name("project")
                .help("new project name")
                .required(true)))
        .subcommand(SubCommand::with_name("edit")
            .about("Edit last frame or currently running frame"))
        .subcommand(SubCommand::with_name("delete")
            .about("Delete all intervals for project")
            .arg(Arg::with_name("project")
                .help("new project name")
                .required(true)))
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

    if let Some(matches) = matches.subcommand_matches("delete") {
        delete(matches.value_of("project").expect("missing project name"));
    }

    match matches.subcommand_name() {
        Some("status") => status(),
        Some("stop") => stop(),
        Some("cancel") => cancel(),
        Some("restart") => restart(),
        Some("log") => log(),
        Some("report") => report(),
        Some("edit") => edit(),
        _ => {},
    }
}


fn start(project_name: &str) {
    let mut periods = get_periods();

    if !periods.is_empty() {
        let last_index = periods.len() - 1;
        if let Some(period) = periods.get_mut(last_index) {
            if period.end_time.is_none() {
                let message = format!("project {} is being tracked", period.project);
                eprintln!("Error: {}",  message.red());
                return eprintln!("Try stopping your current project with {} first.", "stop".blue());
            }
        }
    } 
    let current_period = create_period(project_name);
    // store current period in file
    print!("Started tracking project {} at {}", current_period.project.blue(), humanize_time(current_period.start_time));
    periods.push(current_period);
    save_periods(periods.to_vec());
}

fn status() {
    let mut periods = get_periods();
    if let Some(period) = periods.pop() {
        if period.end_time.is_none() {
            let diff = Utc::now().signed_duration_since(period.start_time);
            return println!("Project {} started {} ({})", period.project.magenta(), humanize_duration(diff), humanize_datetime(period.start_time).blue());
        }
    }
    eprintln!("No running project");
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
                let diff = Utc::now().signed_duration_since(period.start_time);
                println!("Stopped project {}, started {}", period.project.blue(), humanize_duration(diff));
            }
        }
    }
    if updated_period {
        save_periods(periods);
    } else {
        eprintln!("Error: {}", "No project started.".red());
    }
}

fn cancel() {
    let mut periods = get_periods();
    if let Some(period) = periods.pop() {
        if period.end_time.is_none() {
            save_periods(periods);
            let diff = Utc::now().signed_duration_since(period.start_time);
            println!("Canceled project {}, started {}", period.project.blue(), humanize_duration(diff));
            return
        }
    }
    eprintln!("Error: {}", "No project started".red());
}

fn restart() {
    let periods = get_periods();
    let mut new_periods = periods.to_vec();
    if let Some(period) = periods.last() {
        if !period.end_time.is_none() {
            let new_period = create_period(&period.project);
            new_periods.push(new_period);
            save_periods(new_periods);
            return println!("Tracking last running project: {}", period.project.blue());
        } else {
            let message = format!("No project to restart. Project {} is being tracked", period.project);
            eprintln!("Error: {}",  message.red());
            return eprintln!("Try stopping your current project with {} first.", "stop".blue());
        }
    }
    eprintln!("Error: {}", "No previous project to restart".red());
}

fn log() {
    let periods = get_periods();
    let mut days: HashMap<Date<chrono::Local>, Vec<Period>> = HashMap::new();
    let mut project_periods = Vec::new();
    let mut max_diff_len = 0;

    for period in periods.iter() {
        let time = period.start_time.with_timezone(&Local).date();
        days.entry(time).or_insert(Vec::new()).push(period.clone());
    }
    for (date, day) in days.iter() {
        let d = day.into_iter().fold(Duration::zero(), |acc, ref x| acc + (x.end_time.unwrap_or(Utc::now()).signed_duration_since(x.start_time)));
        println!("{date} ({duration})", date=date.with_timezone(&Local).format("%A %-d %B %Y").to_string().green(), duration=format_duration(d).bold());
        for period in day.iter() {
            match period.end_time {
                Some(end_time) => {
                    let diff = end_time.signed_duration_since(period.start_time);
                    project_periods.push((period.start_time, end_time, diff, period.project.clone()));
                    if format_duration(diff).len() > max_diff_len {
                        max_diff_len = format_duration(diff).len()
                    }
                },
                None => {
                    let diff = Utc::now().signed_duration_since(period.start_time);
                    project_periods.push((period.start_time, Utc::now(), diff, period.project.clone()));
                    if format_duration(diff).len() > max_diff_len {
                        max_diff_len = format_duration(diff).len()
                    }
                },
            }
        }
        project_periods.sort();
        for &(start, end, diff, ref project) in project_periods.iter() {
            println!("    {start} to {end} {diff:>width$} {project}",
                start=humanize_time(start),
                end=humanize_time(end),
                diff=format_duration(diff),
                project=project.blue(),
                width=max_diff_len);
        }
    }
}

fn report() {
    let periods = get_periods();
    let mut days: HashMap<String, Vec<Period>> = HashMap::new();
    let mut start_date = Utc::now().with_timezone(&Local).date();
    let mut results: Vec<(String, Duration)> = Vec::new();
    let mut max_proj_len = 0;
    let mut max_diff_len = 0;

    for period in periods.iter() {
        days.entry(period.project.clone()).or_insert(Vec::new()).push(period.clone());
    }
    for (project, intervals) in days.iter() {
        let diff = intervals.into_iter().fold(Duration::zero(), |acc, ref x| acc + (x.end_time.unwrap_or(Utc::now()).signed_duration_since(x.start_time)));
        for x in intervals.iter() {
            if x.start_time.with_timezone(&Local).date() < start_date {
                start_date = x.start_time.with_timezone(&Local).date();
            }
        }
        if project.to_string().len() > max_proj_len {
            max_proj_len = project.to_string().len();
        }
        if format_duration(diff).len() > max_diff_len {
            max_diff_len = format_duration(diff).len();
        }
        results.push((project.clone(), diff));
    }
    println!("{start} -> {end}",
        start=start_date.format("%A %-d %B %Y").to_string().blue(),
        end=Utc::now().format("%A %-d %B %Y").to_string().blue());
    results.sort();
    for &(ref project, ref duration) in results.iter() {
        println!("{project:pwidth$} {duration:>dwidth$}", project=project.green(),
            duration=format_duration(duration.clone()).bold(),
            pwidth=max_proj_len,
            dwidth=max_diff_len);
    }
}

fn amend(project_name: &str) {
    let mut periods = get_periods();
    if let Some(mut period) = periods.pop() {
        if period.end_time.is_none() {
            let old_name = period.project.clone();
            period.project = String::from(project_name);
            println!("Renamed tracking project {old} -> {new}", old=old_name.red(), new=period.project.green());
            periods.push(period);
            save_periods(periods);
            return
        }
    }
    eprintln!("Error: {}", "No project started".red());
}


fn edit() {
    let path = get_data_file_path();
    println!("File: {}", path.to_str().unwrap().blue());
    if let Some(editor) = env::var_os("EDITOR") {
        let mut edit = Command::new(editor);
        edit.arg(path.clone());
        let status = edit.status();
        if !status.is_ok() {
            eprintln!("Error: {}", "Problem with editing.".red());
        }
    } else {
        eprintln!("Error: {}", "Couldn't open editor".red());
    }
}

fn delete(project_name: &str) {
    let periods = get_periods();
    let filtered_periods = periods.clone().into_iter().filter(|x| x.project != project_name).collect();
    if filtered_periods == periods {
        eprintln!("Error: {}", "Project not found.".red());
    } else {
        save_periods(filtered_periods);
        println!("Deleted project {project}", project=project_name.blue());
    }
}

fn humanize_datetime(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local).format("%F %H:%M").to_string()
}

fn humanize_time(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local).format("%H:%M").to_string()
}

fn format_duration(duration: Duration) -> String{
    let days = duration.num_days();
    let hours = duration.num_hours();
    let minutes = duration.num_minutes();
    let seconds = duration.num_seconds();
    if minutes == 0 {
        return format!("{}s", seconds)
    } else if hours == 0 {
        return format!("{minutes}m {seconds}s",
            minutes=minutes,
            seconds=seconds % 60)
    } else if days == 0 {
        return format!("{hours}h {minutes}m {seconds}s",
            hours=hours,
            minutes=minutes % 60,
            seconds=seconds % 60)
    } else {
        return format!("{days}d {hours}h {minutes}m {seconds}s",
            days=days,
            hours=hours % 24,
            minutes=minutes % 60,
            seconds=seconds % 60)
    }
}

fn humanize_duration(time: Duration) -> String {
    let hours = time.num_hours();
    let minutes = time.num_minutes();
    let seconds = time.num_seconds();
    if minutes == 0 {
        if seconds < 5 {
            return String::from("just now")
        }
        return String::from("seconds ago")
    } else if hours == 0 {
        if minutes == 1 {
            return format!("{minutes} minute ago", minutes=minutes)
        } else {
            return format!("{minutes} minutes ago", minutes=minutes)
        }
    } else {
        return format!("{hours}:{minutes}", hours=hours, minutes=minutes)
    }
}

fn create_period(project: &str) -> Period {
    Period {
        project: String::from(project),
        start_time: Utc::now(),
        end_time: None,
    }
}

fn get_data_folder() -> PathBuf {
    let home_dir = env::var("HOME").expect("Failed to find home directory from environment 'HOME'");
    let mut data_folder = PathBuf::from(home_dir);
    data_folder.push(".doug");
    data_folder
}

fn get_data_file_path() -> PathBuf {
    let data_folder = get_data_folder();
    let mut data_file = PathBuf::from(&data_folder);
    data_file.push("periods.json");
    data_file
}

fn get_data_back_file_path() -> PathBuf {
    let data_file = get_data_file_path();
    data_file.with_extension("json-backup")
}

fn get_data_file() -> (File, Metadata) {
    let data_file = get_data_file_path();
    (OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&data_file)
        .expect(&format!("Couldn't open datafile: {:?}", data_file)),
    fs::metadata(&data_file).expect("Couldn't access datafile metadata")
    )
}


fn get_periods() -> Vec<Period> {
    let data_folder = get_data_folder();

    match fs::create_dir(&data_folder) {
        Err(ref error) if error.kind() == ErrorKind::AlreadyExists => {},
        Err(error) => panic!("There was a problem creating the data directory: {:?}", error),
        Ok(_) => {},
    }

    let (file, metadata) = get_data_file();

    let periods: Result<Vec<Period>, Error> = serde_json::from_reader(file);
    let periods = match periods {
        Ok(p) => p,
        Err(_) if metadata.len() > 0 => Vec::new(),
        Err(ref error) if error.is_eof() => Vec::new(),
        Err(error) => panic!("There was a serialization issue: {:?}", error),
    };
    periods
}

fn save_periods(periods: Vec<Period>) {
    let serialized = serde_json::to_string(&periods).expect("Couldn't serialize data to string");
    let data_file = get_data_file_path();
    let data_file_backup = get_data_back_file_path();
    fs::copy(&data_file, &data_file_backup).expect("Couldn't create backup file");
    let mut file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&data_file)
                    .expect("Couldn't open file for saving period.");

    file.write_all(serialized.as_bytes()).expect("Couldn't write serialized data to file");
}
