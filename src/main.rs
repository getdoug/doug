extern crate atty;
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
use std::io::{Write, stdout};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::process::Command;

use atty::Stream;
use chrono::{Date, DateTime, Duration, Local, Utc};
use clap::{App, AppSettings, Arg, SubCommand, Shell};
use serde_json::Error;
use colored::*;

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
struct Period {
    project: String,
    start_time: DateTime<Utc>,
    end_time: Option<DateTime<Utc>>,
}

arg_enum!{
    #[derive(PartialEq, Debug)]
    pub enum Shells {
        Bash,
        Zsh,
        Fish,
        PowerShell,
    }
}

fn main() {
    let mut cli = App::new("Doug")
        .version(crate_version!())
        .about("A time tracking command-line utility")
        .author(crate_authors!())
        .settings(
            &[
                AppSettings::DeriveDisplayOrder,
                AppSettings::GlobalVersion,
                AppSettings::SubcommandRequiredElseHelp,
                AppSettings::VersionlessSubcommands,
                AppSettings::DisableHelpSubcommand,
            ],
        )
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
                .about("Display elapsed time, start time, and running project name")
                .arg(Arg::with_name("t").short("t").long("time").help(
                    "Print time for currently tracked project.",
                ))
                .arg(Arg::with_name("s").short("s").long("simple").help(
                    "Print running project name or nothing if there isn't a running project.",
                )),
        )
        .subcommand(SubCommand::with_name("stop").about(
            "Stop any running projects",
        ))
        .subcommand(SubCommand::with_name("cancel").about(
            "Stop running project and remove most recent time interval",
        ))
        .subcommand(SubCommand::with_name("restart").about(
            "Track last running project",
        ))
        .subcommand(SubCommand::with_name("log").about(
            "Display time intervals across all projects",
        ))
        .subcommand(
            SubCommand::with_name("report")
                .about("Display aggregate time from projects")
                .arg(
                    Arg::with_name("year")
                        .short("y")
                        .long("year")
                        .help(
                            "Limit report to past year. Use multiple to increase interval.",
                        )
                        .overrides_with_all(&["month", "week", "day"])
                        .multiple(true),
                )
                .arg(
                    Arg::with_name("month")
                        .short("m")
                        .long("month")
                        .help(
                            "Limit report to past month. Use multiple to increase interval.",
                        )
                        .overrides_with_all(&["year", "week", "day"])
                        .multiple(true),
                )
                .arg(
                    Arg::with_name("week")
                        .short("w")
                        .long("week")
                        .help(
                            "Limit report to past week. Use multiple to increase interval.",
                        )
                        .overrides_with_all(&["year", "month", "day"])
                        .multiple(true),
                )
                .arg(
                    Arg::with_name("day")
                        .short("d")
                        .long("day")
                        .help(
                            "Limit report to past day. Use multiple to increase interval.",
                        )
                        .overrides_with_all(&["year", "month", "week"])
                        .multiple(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("amend")
                .about("Change name of currently running project")
                .arg(
                    Arg::with_name("project")
                        .help("new project name")
                        .required(true),
                ),
        )
        .subcommand(SubCommand::with_name("edit").about(
            "Edit last frame or currently running frame",
        ))
        .subcommand(
            SubCommand::with_name("generate-completions")
                .about("Generate completions")
                .arg(
                    Arg::with_name("shell")
                        .help("shell to generate completion for (default: bash).")
                        .short("s")
                        .long("shell")
                        .possible_values(&Shells::variants())
                        .case_insensitive(true)
                        .default_value("bash")
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("delete")
                .about("Delete all intervals for project")
                .arg(
                    Arg::with_name("project")
                        .help("new project name")
                        .required(true),
                ),
        );

    let matches = cli.clone().get_matches();

    let time_periods = periods();

    if !atty::is(Stream::Stdout) {
        colored::control::set_override(false);
    }

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
            &time_periods.clone(),
            save_periods,
        );
    }

    if let Some(matches) = matches.subcommand_matches("status") {
        status(
            &time_periods,
            matches.is_present("s"),
            matches.is_present("t"),
        );
    }

    if let Some(matches) = matches.subcommand_matches("report") {
        report(
            &time_periods,
            (
                matches.is_present("year"),
                matches.occurrences_of("year") as i32,
            ),
            (
                matches.is_present("month"),
                matches.occurrences_of("month") as i32,
            ),
            (
                matches.is_present("week"),
                matches.occurrences_of("week") as i32,
            ),
            (
                matches.is_present("day"),
                matches.occurrences_of("day") as i32,
            ),
        );
    }

    if let Some(matches) = matches.subcommand_matches("generate-completions") {
        if matches.is_present("shell") {
            match matches.value_of("shell") {
                Some("bash") => cli.gen_completions_to("doug", Shell::Bash, &mut stdout()),
                Some("zsh") => cli.gen_completions_to("doug", Shell::Zsh, &mut stdout()),
                Some("fish") => cli.gen_completions_to("doug", Shell::Fish, &mut stdout()),
                Some("powershell") => {
                    cli.gen_completions_to("doug", Shell::PowerShell, &mut stdout())
                }
                _ => eprintln!("Invalid option"),
            }
        }
    }

    match matches.subcommand_name() {
        Some("stop") => stop(time_periods, save_periods),
        Some("cancel") => cancel(time_periods, save_periods),
        Some("restart") => restart(&time_periods, save_periods),
        Some("log") => log(&time_periods),
        Some("edit") => edit(),
        _ => {}
    }
}


fn start(project_name: &str, mut periods: Vec<Period>, save: fn(&[Period])) {
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
    println!(
        "Started tracking project {} at {}",
        current_period.project.blue(),
        format_time(current_period.start_time)
    );
    periods.push(current_period);
    save(&periods.to_vec());
}

fn status(periods: &[Period], simple_name: bool, simple_time: bool) {
    if let Some(period) = periods.last() {
        if period.end_time.is_none() {
            let diff = Utc::now().signed_duration_since(period.start_time);
            if simple_name {
                return println!("{}", period.project);
            } else if simple_time {
                return println!("{}", format_duration(diff));
            } else {
                return println!(
                    "Project {} started {} ago ({})",
                    period.project.magenta(),
                    format_duration(diff),
                    format_datetime(period.start_time).blue()
                );
            }
        }
    }
    if !(simple_name || simple_time) {
        eprintln!("No running project");
    }
}

fn stop(mut periods: Vec<Period>, save: fn(&[Period])) {
    if let Some(mut period) = periods.pop() {
        if period.end_time.is_none() {
            period.end_time = Some(Utc::now());
            let diff = Utc::now().signed_duration_since(period.start_time);
            println!(
                "Stopped project {}, started {} ago",
                period.project.blue(),
                format_duration(diff)
            );
            periods.push(period);
            return save(&periods.to_vec());
        }
    }
    eprintln!("Error: {}", "No project started.".red());
}

fn cancel(mut periods: Vec<Period>, save: fn(&[Period])) {
    if let Some(period) = periods.pop() {
        if period.end_time.is_none() {
            save(&periods.to_vec());
            let diff = Utc::now().signed_duration_since(period.start_time);
            return println!(
                "Canceled project {}, started {} ago",
                period.project.blue(),
                format_duration(diff)
            );
        }
    }
    eprintln!("Error: {}", "No project started".red());
}

fn restart(periods: &[Period], save: fn(&[Period])) {
    let mut new_periods = periods.to_vec();
    if let Some(period) = periods.last() {
        if !period.end_time.is_none() {
            let new_period = create_period(&period.project);
            new_periods.push(new_period);
            save(&new_periods.to_vec());
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

fn log(periods: &[Period]) {
    let mut days: HashMap<Date<chrono::Local>, Vec<Period>> = HashMap::new();

    // organize periods by day
    for period in periods {
        let time = period.start_time.with_timezone(&Local).date();
        days.entry(time).or_insert_with(Vec::new).push(
            period.clone(),
        );
    }

    // order days
    let mut days: Vec<(Date<chrono::Local>, Vec<Period>)> = days.into_iter().collect();
    days.sort_by_key(|&(a, ref _b)| a);

    // count the total time tracker per day
    for &(ref date, ref day) in &days {
        let d = day.into_iter().fold(Duration::zero(), |acc, x| {
            acc +
                (x.end_time.unwrap_or_else(Utc::now).signed_duration_since(
                    x.start_time,
                ))
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
        let mut project_periods = Vec::new();
        for period in day.iter() {
            // push periods onto vector so we can could there lengths and properly align them
            match period.end_time {
                Some(end_time) => {
                    let diff = end_time.signed_duration_since(period.start_time);
                    project_periods.push((
                        period.start_time,
                        end_time,
                        diff,
                        period.project.clone(),
                    ));
                    println!(
                        "    {start} to {end} {diff:>width$} {project}",
                        start = format_time(period.start_time),
                        end = format_time(end_time),
                        diff = format_duration(diff),
                        project = period.project.clone().blue(),
                        width = 11
                    )
                }
                None => {
                    let diff = Utc::now().signed_duration_since(period.start_time);
                    println!(
                        "    {start} to {end} {diff:>width$} {project}",
                        start = format_time(period.start_time),
                        end = format_time(Utc::now()),
                        diff = format_duration(diff),
                        project = period.project.clone().blue(),
                        width = 11
                    )
                }
            }
        }
    }
}

fn add_interval(
    start_limit: DateTime<Utc>,
    (start_time, end_time): (DateTime<Utc>, Option<DateTime<Utc>>),
    ref mut start_date: &mut Date<Local>,
) -> Duration {
    // Accumulates intervals based on limits.
    // Finds earliest start date for printing out later
    if start_time > start_limit {
        if start_time.with_timezone(&Local).date() < **start_date {
            **start_date = start_time.with_timezone(&Local).date();
        }
        return end_time.unwrap_or_else(Utc::now).signed_duration_since(
            start_time,
        );
    } else if end_time.unwrap_or_else(Utc::now) >= start_limit {
        if start_limit.with_timezone(&Local).date() < **start_date {
            **start_date = start_limit.with_timezone(&Local).date();
        }
        return end_time.unwrap_or_else(Utc::now).signed_duration_since(
            start_limit,
        );
    } else {
        return Duration::zero();
    }
}


fn report(
    periods: &[Period],
    (past_year, past_year_occur): (bool, i32),
    (past_month, past_month_occur): (bool, i32),
    (past_week, past_week_occur): (bool, i32),
    (past_day, past_day_occur): (bool, i32),
) {
    let mut days: HashMap<String, Vec<Period>> = HashMap::new();
    let mut start_date = Utc::now().with_timezone(&Local).date();
    let mut results: Vec<(String, Duration)> = Vec::new();
    let mut max_proj_len = 0;
    let mut max_diff_len = 0;

    let one_year = Duration::days(365);
    let one_month = Duration::days(31);
    let one_week = Duration::weeks(1);
    let one_day = Duration::days(1);
    let today = Utc::now();

    // organize periods by project
    for period in periods {
        days.entry(period.project.clone())
            .or_insert_with(Vec::new)
            .push(period.clone());
    }

    for (project, intervals) in &days {
        // sum total time per project
        let duration = intervals.into_iter().fold(Duration::zero(), |acc, x| {
            // if start time is beyond our limit, but end time is within
            // add duration of time within boundaries to total
            if past_year {
                let start_limit = today - one_year * past_year_occur;
                return acc + add_interval(start_limit, (x.start_time, x.end_time), &mut start_date);
            } else if past_month {
                let start_limit = today - one_month * past_month_occur;
                return acc + add_interval(start_limit, (x.start_time, x.end_time), &mut start_date);
            } else if past_week {
                let start_limit = today - one_week * past_week_occur;
                return acc + add_interval(start_limit, (x.start_time, x.end_time), &mut start_date);
            } else if past_day {
                let start_limit = today - one_day * past_day_occur;
                return acc + add_interval(start_limit, (x.start_time, x.end_time), &mut start_date);
            } else {
                if x.start_time.with_timezone(&Local).date() < start_date {
                    start_date = x.start_time.with_timezone(&Local).date();
                }
                acc +
                    (x.end_time.unwrap_or_else(Utc::now).signed_duration_since(
                        x.start_time,
                    ))
            }

        });
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
    for &(ref project, ref duration) in &results {
        println!(
            "{project:pwidth$} {duration:>dwidth$}",
            project = project.green(),
            duration = format_duration(*duration).bold(),
            pwidth = max_proj_len,
            dwidth = max_diff_len
        );
    }
}

fn amend(project_name: &str, mut periods: Vec<Period>, save: fn(&[Period])) {
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
            return save(&periods.to_vec());
        }
    }
    eprintln!("Error: {}", "No project started".red());
}


fn edit() {
    let path = Path::new(&env::var("HOME").expect(
        "Failed to find home directory from environment 'HOME'",
    )).join(".doug/periods.json");
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

fn delete(project_name: &str, periods: &[Period], save: fn(&[Period])) {
    let mut project_not_found = true;
    let mut filtered_periods = Vec::new();
    for period in periods {
        if period.project == project_name {
            project_not_found = false;
        } else {
            filtered_periods.push(period.clone());
        }
    }
    if project_not_found {
        eprintln!("Error: {}", "Project not found.".red());
    } else {
        save(&filtered_periods);
        println!("Deleted project {project}", project = project_name.blue());
    }
}

fn format_datetime(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local).format("%F %H:%M").to_string()
}

fn format_time(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local).format("%H:%M").to_string()
}

fn format_duration(duration: Duration) -> String {
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;

    if duration.num_minutes() == 0 {
        format!("{}s", seconds)
    } else if duration.num_hours() == 0 {
        format!(
            "{minutes}m {seconds:>2}s",
            minutes = minutes,
            seconds = seconds
        )
    } else {
        format!(
            "{hours}h {minutes:>2}m {seconds:>2}s",
            hours = hours,
            minutes = minutes,
            seconds = seconds
        )
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
    DirBuilder::new().recursive(true).create(&folder).expect(
        "Couldn't create data directory",
    );
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

fn save_periods(periods: &[Period]) {
    let serialized = serde_json::to_string(&periods).expect("Couldn't serialize data to string");
    let data_file = Path::new(&env::var("HOME").expect(
        "Failed to find home directory from environment 'HOME'",
    )).join(".doug/periods.json");
    let mut data_file_backup = data_file.clone();
    data_file_backup.set_extension("json-backup");
    fs::copy(&data_file, &data_file_backup).expect("Couldn't create backup file");
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&data_file)
        .expect("Couldn't open file for saving period.");

    file.write_all(serialized.as_bytes()).expect(
        "Couldn't write serialized data to file",
    );
}
