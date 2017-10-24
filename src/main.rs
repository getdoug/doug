extern crate chrono;
#[macro_use]
extern crate clap;
extern crate colored;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;


use std::env;
use std::fs;
use std::fs::{DirBuilder, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::process::Command;

use chrono::{Date, DateTime, Duration, Local, Utc};
use clap::{App, AppSettings, Arg, SubCommand};
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
        .subcommand(
            SubCommand::with_name("start")
                .about("Track new or existing project")
                .arg(
                    Arg::with_name("project")
                        .help("project to track")
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("status")
                .about("Display elapsed time, start time, and running project name"),
        )
        .subcommand(SubCommand::with_name("stop").about("Stop any running projects"))
        .subcommand(
            SubCommand::with_name("cancel")
                .about("Stop running project and remove most recent time interval"),
        )
        .subcommand(SubCommand::with_name("restart").about("Track last running project"))
        .subcommand(
            SubCommand::with_name("log").about("Display time intervals across all projects"),
        )
        .subcommand(SubCommand::with_name("report").about("Display aggregate time from projects"))
        .subcommand(
            SubCommand::with_name("amend")
                .about("Change name of currently running project")
                .arg(
                    Arg::with_name("project")
                        .help("new project name")
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("edit").about("Edit last frame or currently running frame"),
        )
        .subcommand(
            SubCommand::with_name("delete")
                .about("Delete all intervals for project")
                .arg(
                    Arg::with_name("project")
                        .help("new project name")
                        .required(true),
                ),
        )
        .get_matches();

    let time_periods = periods();

    if let Some(matches) = matches.subcommand_matches("start") {
        if matches.is_present("project") {
            start(
                matches.value_of("project").unwrap(),
                time_periods.clone(),
                save_periods,
            );
        }
    }

    if let Some(matches) = matches.subcommand_matches("amend") {
        if matches.is_present("project") {
            amend(
                matches.value_of("project").expect("missing project name"),
                time_periods.clone(),
                save_periods,
            );
        }
    }

    if let Some(matches) = matches.subcommand_matches("delete") {
        delete(
            matches.value_of("project").expect("missing project name"),
            time_periods.clone(),
            save_periods,
        );
    }

    match matches.subcommand_name() {
        Some("status") => status(time_periods),
        Some("stop") => stop(time_periods, save_periods),
        Some("cancel") => cancel(time_periods, save_periods),
        Some("restart") => restart(time_periods, save_periods),
        Some("log") => log(time_periods),
        Some("report") => report(time_periods),
        Some("edit") => edit(),
        _ => {}
    }
}


fn start(project_name: &str, mut periods: Vec<Period>, save: fn(Vec<Period>)) {
    if !periods.is_empty() {
        if let Some(period) = periods.last_mut() {
            if period.end_time.is_none() {
                let message = format!("project {} is being tracked", period.project);
                eprintln!("Error: {}", message.red());
                return eprintln!(
                    "Try stopping your current project with {} first.",
                    "stop".blue()
                );
            }
        }
    }
    let current_period = create_period(project_name);
    print!(
        "Started tracking project {} at {}",
        current_period.project.blue(),
        humanize_time(current_period.start_time)
    );
    periods.push(current_period);
    save(periods);
}

fn status(periods: Vec<Period>) {
    if let Some(period) = periods.last() {
        if period.end_time.is_none() {
            let diff = Utc::now().signed_duration_since(period.start_time);
            return println!(
                "Project {} started {} ({})",
                period.project.magenta(),
                humanize_duration(diff),
                humanize_datetime(period.start_time).blue()
            );
        }
    }
    eprintln!("No running project");
}

fn stop(mut periods: Vec<Period>, save: fn(Vec<Period>)) {
    if let Some(mut period) = periods.pop() {
        if period.end_time.is_none() {
            period.end_time = Some(Utc::now());
            let diff = Utc::now().signed_duration_since(period.start_time);
            println!(
                "Stopped project {}, started {}",
                period.project.blue(),
                humanize_duration(diff)
            );
            periods.push(period);
            return save(periods);
        }
    }
    eprintln!("Error: {}", "No project started.".red());
}

fn cancel(mut periods: Vec<Period>, save: fn(Vec<Period>)) {
    if let Some(period) = periods.pop() {
        if period.end_time.is_none() {
            save(periods);
            let diff = Utc::now().signed_duration_since(period.start_time);
            return println!(
                "Canceled project {}, started {}",
                period.project.blue(),
                humanize_duration(diff)
            );
        }
    }
    eprintln!("Error: {}", "No project started".red());
}

fn restart(periods: Vec<Period>, save: fn(Vec<Period>)) {
    let mut new_periods = periods.to_vec();
    if let Some(period) = periods.last() {
        if !period.end_time.is_none() {
            let new_period = create_period(&period.project);
            new_periods.push(new_period);
            save(new_periods);
            return println!("Tracking last running project: {}", period.project.blue());
        } else {
            let message = format!(
                "No project to restart. Project {} is being tracked",
                period.project
            );
            eprintln!("Error: {}", message.red());
            return eprintln!(
                "Try stopping your current project with {} first.",
                "stop".blue()
            );
        }
    }
    eprintln!("Error: {}", "No previous project to restart".red());
}

fn log(periods: Vec<Period>) {
    let mut days: HashMap<Date<chrono::Local>, Vec<Period>> = HashMap::new();
    let mut project_periods = Vec::new();
    let mut max_diff_len = 0;

    // organize periods by day
    for period in periods.iter() {
        let time = period.start_time.with_timezone(&Local).date();
        days.entry(time).or_insert(Vec::new()).push(period.clone());
    }
    // count the total time tracker per day
    for (date, day) in days.iter() {
        let d = day.into_iter().fold(Duration::zero(), |acc, ref x| {
            acc
                + (x.end_time
                    .unwrap_or(Utc::now())
                    .signed_duration_since(x.start_time))
        });
        println!(
            "{date} ({duration})",
            date = date.with_timezone(&Local)
                .format("%A %-d %B %Y")
                .to_string()
                .green(),
            duration = format_duration(d).bold()
        );
        // find time tracker per period
        for period in day.iter() {
            // push periods onto vector so we can could there lengths and properly align them
            match period.end_time {
                Some(end_time) => {
                    let diff = end_time.signed_duration_since(period.start_time);
                    project_periods
                        .push((period.start_time, end_time, diff, period.project.clone()));
                    if format_duration(diff).len() > max_diff_len {
                        max_diff_len = format_duration(diff).len()
                    }
                }
                None => {
                    let diff = Utc::now().signed_duration_since(period.start_time);
                    project_periods.push((
                        period.start_time,
                        Utc::now(),
                        diff,
                        period.project.clone(),
                    ));
                    if format_duration(diff).len() > max_diff_len {
                        max_diff_len = format_duration(diff).len()
                    }
                }
            }
        }
        project_periods.sort();
        for &(start, end, diff, ref project) in project_periods.iter() {
            println!(
                "    {start} to {end} {diff:>width$} {project}",
                start = humanize_time(start),
                end = humanize_time(end),
                diff = format_duration(diff),
                project = project.blue(),
                width = max_diff_len
            );
        }
    }
}

fn report(periods: Vec<Period>) {
    let mut days: HashMap<String, Vec<Period>> = HashMap::new();
    let mut start_date = Utc::now().with_timezone(&Local).date();
    let mut results: Vec<(String, Duration)> = Vec::new();
    let mut max_proj_len = 0;
    let mut max_diff_len = 0;

    // organize periods by project
    for period in periods.iter() {
        days.entry(period.project.clone())
            .or_insert(Vec::new())
            .push(period.clone());
    }
    //
    for (project, intervals) in days.iter() {
        // sum total time per project
        let duration = intervals.into_iter().fold(Duration::zero(), |acc, ref x| {
            acc
                + (x.end_time
                    .unwrap_or(Utc::now())
                    .signed_duration_since(x.start_time))
        });
        // determine start date of report period
        for x in intervals.iter() {
            if x.start_time.with_timezone(&Local).date() < start_date {
                start_date = x.start_time.with_timezone(&Local).date();
            }
        }
        // find lengths of project names for alignment
        if project.to_string().len() > max_proj_len {
            max_proj_len = project.to_string().len();
        }
        // find lengths of durations names for alignment
        if format_duration(duration).len() > max_diff_len {
            max_diff_len = format_duration(duration).len();
        }
        results.push((project.clone(), duration));
    }
    println!(
        "{start} -> {end}",
        start = start_date.format("%A %-d %B %Y").to_string().blue(),
        end = Utc::now().format("%A %-d %B %Y").to_string().blue()
    );
    results.sort();
    for &(ref project, ref duration) in results.iter() {
        println!(
            "{project:pwidth$} {duration:>dwidth$}",
            project = project.green(),
            duration = format_duration(duration.clone()).bold(),
            pwidth = max_proj_len,
            dwidth = max_diff_len
        );
    }
}

fn amend(project_name: &str, mut periods: Vec<Period>, save: fn(Vec<Period>)) {
    if let Some(mut period) = periods.pop() {
        if period.end_time.is_none() {
            let old_name = period.project.clone();
            period.project = String::from(project_name);
            println!(
                "Renamed tracking project {old} -> {new}",
                old = old_name.red(),
                new = period.project.green()
            );
            periods.push(period);
            return save(periods);
        }
    }
    eprintln!("Error: {}", "No project started".red());
}


fn edit() {
    let path = Path::new(&env::var("HOME")
        .expect("Failed to find home directory from environment 'HOME'"))
        .join(
        ".doug/periods.json",
    );
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

fn delete(project_name: &str, periods: Vec<Period>, save: fn(Vec<Period>)) {
    let filtered_periods = periods
        .clone()
        .into_iter()
        .filter(|x| x.project != project_name)
        .collect();
    if filtered_periods == periods {
        eprintln!("Error: {}", "Project not found.".red());
    } else {
        save(filtered_periods);
        println!("Deleted project {project}", project = project_name.blue());
    }
}

fn humanize_datetime(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local).format("%F %H:%M").to_string()
}

fn humanize_time(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local).format("%H:%M").to_string()
}

fn format_duration(duration: Duration) -> String {
    let days = duration.num_days();
    let hours = duration.num_hours() % 24;
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;
    if minutes == 0 {
        format!("{}s", seconds)
    } else if hours == 0 {
        format!(
            "{minutes}m {seconds}s",
            minutes = minutes,
            seconds = seconds
        )
    } else if days == 0 {
        format!(
            "{hours}h {minutes}m {seconds}s",
            hours = hours,
            minutes = minutes,
            seconds = seconds
        )
    } else {
        format!(
            "{days}d {hours}h {minutes}m {seconds}s",
            days = days,
            hours = hours,
            minutes = minutes,
            seconds = seconds
        )
    }
}

fn humanize_duration(time: Duration) -> String {
    let hours = time.num_hours();
    let minutes = time.num_minutes();
    let seconds = time.num_seconds();
    if minutes == 0 {
        if seconds < 5 {
            String::from("just now")
        } else {
            String::from("seconds ago")
        }
    } else if hours == 0 {
        if minutes == 1 {
            format!("{minutes} minute ago", minutes = minutes)
        } else {
            format!("{minutes} minutes ago", minutes = minutes)
        }
    } else {
        format!("{hours}:{minutes}", hours = hours, minutes = minutes)
    }
}

fn create_period(project: &str) -> Period {
    Period {
        project: String::from(project),
        start_time: Utc::now(),
        end_time: None,
    }
}

fn periods() -> Vec<Period> {
    let home_dir = env::var("HOME").expect("Failed to find home directory from environment 'HOME'");
    let mut folder = PathBuf::from(home_dir);
    folder.push(".doug");
    // create .doug directory
    DirBuilder::new()
        .recursive(true)
        .create(&folder)
        .expect("Couldn't create data directory");
    // create data file
    let data_file_path = folder.as_path().join("periods.json");
    let data_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&data_file_path)
        .expect(&format!("Couldn't open datafile: {:?}", data_file_path));
    // serialize periods from data file
    let periods: Result<Vec<Period>, Error> = serde_json::from_reader(data_file);
    match periods {
        Ok(p) => p,
        Err(ref error) if error.is_eof() => Vec::new(),
        Err(error) => panic!("There was a serialization issue: {:?}", error),
    }
}

fn save_periods(periods: Vec<Period>) {
    let serialized = serde_json::to_string(&periods).expect("Couldn't serialize data to string");
    let data_file = Path::new(&env::var("HOME")
        .expect("Failed to find home directory from environment 'HOME'"))
        .join(
        ".doug/periods.json",
    );
    let mut data_file_backup = data_file.clone();
    data_file_backup.set_extension("json-backup");
    fs::copy(&data_file, &data_file_backup).expect("Couldn't create backup file");
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&data_file)
        .expect("Couldn't open file for saving period.");

    file.write_all(serialized.as_bytes())
        .expect("Couldn't write serialized data to file");
}
