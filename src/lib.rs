extern crate atty;
extern crate chrono;
#[macro_use]
extern crate clap;
extern crate colored;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::{DirBuilder, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::process::Command;

use chrono::{Date, DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
use colored::*;
use serde_json::Error;

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct Period {
    project: String,
    start_time: DateTime<Utc>,
    end_time: Option<DateTime<Utc>>,
}

pub fn start(project_name: &str, mut periods: Vec<Period>, save: fn(&[Period])) {
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

pub fn status(periods: &[Period], simple_name: bool, simple_time: bool) {
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

pub fn stop(mut periods: Vec<Period>, save: fn(&[Period])) {
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

pub fn cancel(mut periods: Vec<Period>, save: fn(&[Period])) {
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

pub fn restart(periods: &[Period], save: fn(&[Period])) {
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

pub fn log(periods: &[Period]) {
    let mut days: HashMap<Date<chrono::Local>, Vec<Period>> = HashMap::new();

    // organize periods by day
    for period in periods {
        let time = period.start_time.with_timezone(&Local).date();
        days.entry(time)
            .or_insert_with(Vec::new)
            .push(period.clone());
    }

    // order days
    let mut days: Vec<(Date<chrono::Local>, Vec<Period>)> = days.into_iter().collect();
    days.sort_by_key(|&(a, ref _b)| a);

    // count the total time tracker per day
    for &(ref date, ref day) in &days {
        let d = day.into_iter().fold(Duration::zero(), |acc, x| {
            acc + (x
                .end_time
                .unwrap_or_else(Utc::now)
                .signed_duration_since(x.start_time))
        });
        println!(
            "{date} ({duration})",
            date = date
                .with_timezone(&Local)
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

pub fn add_interval(
    start_limit: DateTime<Utc>,
    end_limit: DateTime<Utc>,
    (start_time, end_time): (DateTime<Utc>, Option<DateTime<Utc>>),
    ref mut start_date: &mut Date<Local>,
    ref mut end_date: &mut Date<Local>,
) -> Duration {
    // Accumulates intervals based on limits.
    // Finds earliest start date for printing out later
    let end_time = end_time.unwrap_or_else(Utc::now);
    // start time is within starting limit
    if start_time >= start_limit && start_time <= end_limit {
        // Find starting date
        // FIXME:
        if start_time.with_timezone(&Local).date() < **start_date {
            **start_date = start_time.with_timezone(&Local).date();
        }

        // end time is outside of end limit
        if end_time > end_limit {
            if end_limit.with_timezone(&Local).date() > **end_date {
                **end_date = end_limit.with_timezone(&Local).date();
            }
            return end_limit.signed_duration_since(start_time);
        } else {
            if end_time.with_timezone(&Local).date() > **end_date {
                **end_date = end_time.with_timezone(&Local).date();
            }
            return end_time.signed_duration_since(start_time);
        }
    } else if end_time >= start_limit && end_time <= end_limit {
        // Find starting date
        // FIXME
        if start_limit.with_timezone(&Local).date() < **start_date {
            **start_date = start_limit.with_timezone(&Local).date();
        }

        if end_time > end_limit {
            if end_limit.with_timezone(&Local).date() > **end_date {
                **end_date = end_limit.with_timezone(&Local).date();
            }

            return end_limit.signed_duration_since(start_time);
        } else {
            if end_time.with_timezone(&Local).date() > **end_date {
                **end_date = end_time.with_timezone(&Local).date();
            }

            return end_time.signed_duration_since(start_limit);
        }
    } else {
        return Duration::zero();
    }
}

pub fn report(
    periods: &[Period],
    (past_year, past_year_occur): (bool, i32),
    (past_month, past_month_occur): (bool, i32),
    (past_week, past_week_occur): (bool, i32),
    (past_day, past_day_occur): (bool, i32),
    from_date: Option<&str>,
    to_date: Option<&str>,
) {
    let mut days: HashMap<String, Vec<Period>> = HashMap::new();
    let mut start_date = Utc::today().with_timezone(&Local);
    let mut end_date = Utc
        .from_utc_date(&NaiveDate::from_ymd(1, 1, 1))
        .with_timezone(&Local);

    let mut results: Vec<(String, Duration)> = Vec::new();
    let mut max_proj_len = 0;
    let mut max_diff_len = 0;

    let one_year = Duration::days(365);
    let one_month = Duration::days(31);
    let one_week = Duration::weeks(1);
    let one_day = Duration::days(1);
    let today = Utc::now();

    let mut from_date_parsed = Local::today();
    let mut to_date_parsed = Local::today();
    let offset = to_date_parsed.offset().clone();

    if let Some(from_date_string) = from_date {
        match NaiveDate::parse_from_str(from_date_string, "%Y-%m-%d") {
            Ok(result) => {
                from_date_parsed = Date::from_utc(result, offset);
            }
            Err(_error) => {
                eprintln!("Error: {}", "Invalid date format.".red());
                eprintln!("Required format: {}", "%Y-%m-%d".blue());
                exit(1)
            }
        }
    }
    if let Some(to_date_string) = to_date {
        match NaiveDate::parse_from_str(to_date_string, "%Y-%m-%d") {
            Ok(result) => {
                to_date_parsed = Date::from_utc(result, offset);
            }
            Err(_error) => {
                eprintln!("Error: {}", "Invalid date format.".red());
                eprintln!("Required format: {}", "%Y-%m-%d".blue());
                exit(1)
            }
        }
    }

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
                return acc + add_interval(
                    start_limit,
                    Utc::now(),
                    (x.start_time, x.end_time),
                    &mut start_date,
                    &mut end_date,
                );
            } else if past_month {
                let start_limit = today - one_month * past_month_occur;
                return acc + add_interval(
                    start_limit,
                    Utc::now(),
                    (x.start_time, x.end_time),
                    &mut start_date,
                    &mut end_date,
                );
            } else if past_week {
                let start_limit = today - one_week * past_week_occur;
                return acc + add_interval(
                    start_limit,
                    Utc::now(),
                    (x.start_time, x.end_time),
                    &mut start_date,
                    &mut end_date,
                );
            } else if past_day {
                let start_limit = today - one_day * past_day_occur;
                return acc + add_interval(
                    start_limit,
                    Utc::now(),
                    (x.start_time, x.end_time),
                    &mut start_date,
                    &mut end_date,
                );
            } else if from_date.is_some() || to_date.is_some() {
                let start_limit = from_date_parsed.and_hms(0, 0, 0).with_timezone(&Utc);
                let end_limit = to_date_parsed.and_hms(0, 0, 0).with_timezone(&Utc);
                return acc + add_interval(
                    start_limit,
                    end_limit,
                    (x.start_time, x.end_time),
                    &mut start_date,
                    &mut end_date,
                );
            } else {
                let end_time = x.end_time.unwrap_or_else(Utc::now);
                if x.start_time.with_timezone(&Local).date() < start_date {
                    start_date = x.start_time.with_timezone(&Local).date();
                }
                if end_time.with_timezone(&Local).date() > end_date {
                    end_date = end_time.with_timezone(&Local).date();
                }
                acc + (x
                    .end_time
                    .unwrap_or_else(Utc::now)
                    .signed_duration_since(x.start_time))
            }
        });

        // skip projects that weren't worked on
        if duration == Duration::zero() {
            continue;
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
        end = end_date.format("%A %-d %B %Y").to_string().blue()
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

pub fn amend(project_name: &str, mut periods: Vec<Period>, save: fn(&[Period])) {
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

pub fn edit() {
    let path = Path::new(
        &env::var("HOME").expect("Failed to find home directory from environment 'HOME'"),
    ).join(".doug/periods.json");
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

pub fn delete(project_name: &str, periods: &[Period], save: fn(&[Period])) {
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

pub fn periods() -> Vec<Period> {
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

pub fn save_periods(periods: &[Period]) {
    let serialized = serde_json::to_string(&periods).expect("Couldn't serialize data to string");
    let data_file = Path::new(
        &env::var("HOME").expect("Failed to find home directory from environment 'HOME'"),
    ).join(".doug/periods.json");
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