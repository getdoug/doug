extern crate chrono;
extern crate chrono_english;
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
use std::process::Command;

use chrono::{Date, DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
use chrono_english::{parse_date_string, Dialect};
use colored::*;
use serde_json::Error;
use std::fmt;

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct Period {
    project: String,
    start_time: DateTime<Utc>,
    end_time: Option<DateTime<Utc>>,
}

impl Period {
    fn new(project: &str) -> Period {
        Period {
            project: String::from(project),
            start_time: Utc::now(),
            end_time: None,
        }
    }
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let end_time = match self.end_time {
            Some(x) => x,
            None => Utc::now(),
        };
        let diff = end_time.signed_duration_since(self.start_time);
        let start_time = format_time(self.start_time);
        let end_time = if self.end_time.is_some() {
            format_time(end_time)
        } else {
            "present".to_string()
        };
        write!(
            f,
            "{} to {} {}",
            start_time,
            end_time,
            format_duration(diff).purple()
        )
    }
}

#[derive(Default, Clone)]
pub struct Doug {
    periods: Vec<Period>,
}

type DougResult = Result<String, String>;

impl Doug {
    pub fn new() -> Result<Self, String> {
        let home_dir = match env::var("HOME") {
            Ok(x) => x,
            Err(_) => {
                return Err("Failed to find home directory from environment 'HOME'. Doug needs 'HOME' to be set to find its data.".to_string());
            }
        };
        let mut folder = PathBuf::from(home_dir);
        folder.push(".doug");
        // create .doug directory
        match DirBuilder::new().recursive(true).create(&folder) {
            Ok(_) => {}
            Err(_) => return Err(format!("Couldn't create data directory: {:?}\n", folder)),
        }
        // create data file
        let data_file_path = folder.as_path().join("periods.json");
        let data_file = match OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&data_file_path)
        {
            Ok(x) => x,
            Err(_) => {
                return Err(format!("Couldn't open datafile: {:?}\n", data_file_path));
            }
        };

        // serialize periods from data file
        let periods: Result<Vec<Period>, Error> = serde_json::from_reader(data_file);

        match periods {
            Ok(periods) => Ok(Doug { periods }),
            // No periods exist. Create a new Doug instance.
            Err(ref error) if error.is_eof() => Ok(Doug {
                periods: Vec::new(),
            }),
            Err(error) => Err(format!("There was a serialization issue: {:?}\n", error)),
        }
    }

    pub fn status(&self, simple_name: bool, simple_time: bool) -> DougResult {
        // TODO(chdsbd): Return status as String
        if let Some(period) = &self.periods.last() {
            if period.end_time.is_none() {
                let diff = Utc::now().signed_duration_since(period.start_time);
                return Ok((if simple_name {
                    format!("{}\n", period.project)
                } else if simple_time {
                    format!("{}\n", format_duration(diff))
                } else {
                    format!(
                        "Project {} started {} ago ({})\n",
                        period.project.magenta(),
                        format_duration(diff),
                        format_datetime(period.start_time).blue()
                    )
                }));
            }
        }
        if !(simple_name || simple_time) {
            Err("No running project".to_string())
        } else {
            Ok("".to_string())
        }
    }

    pub fn save(&self) -> DougResult {
        let serialized = match serde_json::to_string(&self.periods) {
            Ok(x) => x,
            Err(_) => return Err("Couldn't serialize data to string".to_string()),
        };
        let home = match env::var("HOME") {
            Ok(x) => x,
            Err(_) => return Err("Couldn't find `HOME`".to_string()),
        };
        let data_file = Path::new(&home).join(".doug/periods.json");
        let mut data_file_backup = data_file.clone();
        data_file_backup.set_extension("json-backup");
        match fs::copy(&data_file, &data_file_backup) {
            Ok(_) => {}
            Err(_) => return Err("Couldn't create backup file".to_string()),
        };
        let mut file = match OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&data_file)
        {
            Ok(x) => x,
            Err(_) => return Err("Couldn't open file for saving period.".to_string()),
        };

        match file.write_all(serialized.as_bytes()) {
            Ok(_) => Ok("".to_string()),
            Err(_) => return Err("Couldn't write serialized data to file".to_string()),
        }
    }

    pub fn start(&mut self, project_name: &str) -> DougResult {
        // TODO(chdsbd): Replace print with Result<String, &str>
        if !self.periods.is_empty() {
            if let Some(period) = self.periods.last_mut() {
                if period.end_time.is_none() {
                    let mut error = format!("project {} is being tracked\n", period.project);
                    error.push_str(
                        format!(
                            "Try stopping your current project with {} first.",
                            "stop".blue()
                        ).as_str(),
                    );
                    return Err(error);
                }
            }
        }
        let current_period = Period::new(project_name);
        let message = format!(
            "Started tracking project {} at {}\n",
            current_period.project.blue(),
            format_time(current_period.start_time)
        );
        self.periods.push(current_period);
        self.save()?;
        Ok((message))
    }

    pub fn amend(&mut self, project_name: &str) -> DougResult {
        if let Some(mut period) = self.periods.pop() {
            if period.end_time.is_none() {
                let old_name = period.project.clone();
                period.project = String::from(project_name);
                let message = format!(
                    "Renamed tracking project {old} -> {new}\n",
                    old = old_name.red(),
                    new = period.project.green()
                );
                self.periods.push(period);
                self.save()?;
                return Ok((message));

            }
        }
        Err(format!("Error: {}", "No project started".red()))
    }

    pub fn report(
        &self,
        (past_year, past_year_occur): (bool, i32),
        (past_month, past_month_occur): (bool, i32),
        (past_week, past_week_occur): (bool, i32),
        (past_day, past_day_occur): (bool, i32),
        from_date: Option<&str>,
        to_date: Option<&str>,
    ) -> DougResult {
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
        let offset = *to_date_parsed.offset();

        if let Some(from_date_string) = from_date {
            match NaiveDate::parse_from_str(from_date_string, "%Y-%m-%d") {
                Ok(result) => {
                    from_date_parsed = Date::from_utc(result, offset);
                }
                Err(_error) => {
                    let mut error = format!("Error: {}", "Invalid date format.\n".red());
                    error.push_str(format!("Required format: {}", "%Y-%m-%d".blue()).as_str());
                    return Err(error);
                }
            }
        }
        if let Some(to_date_string) = to_date {
            match NaiveDate::parse_from_str(to_date_string, "%Y-%m-%d") {
                Ok(result) => {
                    to_date_parsed = Date::from_utc(result, offset);
                }
                Err(_error) => {
                    let mut error = format!("Error: {}", "Invalid date format.\n".red());
                    error.push_str(format!("Required format: {}", "%Y-%m-%d".blue()).as_str());
                    return Err(error);
                }
            }
        }

        // organize periods by project
        for period in &self.periods {
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
                    acc + add_interval(
                        start_limit,
                        Utc::now(),
                        (x.start_time, x.end_time),
                        &mut start_date,
                        &mut end_date,
                    )
                } else if past_month {
                    let start_limit = today - one_month * past_month_occur;
                    acc + add_interval(
                        start_limit,
                        Utc::now(),
                        (x.start_time, x.end_time),
                        &mut start_date,
                        &mut end_date,
                    )
                } else if past_week {
                    let start_limit = today - one_week * past_week_occur;
                    acc + add_interval(
                        start_limit,
                        Utc::now(),
                        (x.start_time, x.end_time),
                        &mut start_date,
                        &mut end_date,
                    )
                } else if past_day {
                    let start_limit = today - one_day * past_day_occur;
                    acc + add_interval(
                        start_limit,
                        Utc::now(),
                        (x.start_time, x.end_time),
                        &mut start_date,
                        &mut end_date,
                    )
                } else if from_date.is_some() || to_date.is_some() {
                    let start_limit = from_date_parsed.and_hms(0, 0, 0).with_timezone(&Utc);
                    let end_limit = to_date_parsed.and_hms(0, 0, 0).with_timezone(&Utc);
                    acc + add_interval(
                        start_limit,
                        end_limit,
                        (x.start_time, x.end_time),
                        &mut start_date,
                        &mut end_date,
                    )
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
        let mut message = format!(
            "{start} -> {end}\n",
            start = start_date.format("%A %-d %B %Y").to_string().blue(),
            end = end_date.format("%A %-d %B %Y").to_string().blue()
        );
        results.sort();
        for &(ref project, ref duration) in &results {
            message.push_str(format!(
                "{project:pwidth$} {duration:>dwidth$}\n",
                project = project.green(),
                duration = format_duration(*duration).bold(),
                pwidth = max_proj_len,
                dwidth = max_diff_len
            ).as_str());
        }
        Ok((message))
    }
    pub fn delete(&mut self, project_name: &str) -> DougResult {
        let mut project_not_found = true;
        let mut filtered_periods = Vec::new();
        for period in &self.periods {
            if period.project == project_name {
                project_not_found = false;
            } else {
                filtered_periods.push(period.clone());
            }
        }
        if project_not_found {
            Err(format!("Error: {}", "Project not found.\n".red()))
        } else {
            self.periods = filtered_periods;
            self.save()?;
            Ok((format!("Deleted project {project}\n", project = project_name.blue())))
        }
    }

    pub fn restart(&mut self) -> DougResult {
        let mut new_periods = self.periods.to_vec();
        // TODO(sbdchd): we shouldn't need this clone
        if let Some(period) = self.periods.clone().last() {
            if period.end_time.is_some() {
                let new_period = Period::new(&period.project);
                new_periods.push(new_period);
                self.periods = new_periods.to_vec();
                self.save()?;
                return Ok((format!("Tracking last running project: {}", period.project.blue())));
            } else {
                let mut error = format!(
                    "No project to restart. Project {} is being tracked\n",
                    period.project
                );
                error.push_str(
                    format!(
                        "Try stopping your current project with {} first.",
                        "stop".blue()
                    ).as_str(),
                );
                return Err(error);
            }
        }
        Err(format!("Error: {}", "No previous project to restart".red()))
    }
    pub fn log(&self) -> DougResult {
        let mut days: HashMap<Date<chrono::Local>, Vec<Period>> = HashMap::new();

        // organize periods by day
        for period in &self.periods {
            let time = period.start_time.with_timezone(&Local).date();
            days.entry(time)
                .or_insert_with(Vec::new)
                .push(period.clone());
        }

        // order days
        let mut days: Vec<(Date<chrono::Local>, Vec<Period>)> = days.into_iter().collect();
        days.sort_by_key(|&(a, ref _b)| a);
        let mut message = String::new();
        // count the total time tracker per day
        for (date, day) in &days {
            let d = day.into_iter().fold(Duration::zero(), |acc, x| {
                acc + (x
                    .end_time
                    .unwrap_or_else(Utc::now)
                    .signed_duration_since(x.start_time))
            });
            message.push_str(format!(
                "{date} ({duration})\n",
                date = date
                    .with_timezone(&Local)
                    .format("%A %-d %B %Y")
                    .to_string()
                    .green(),
                duration = format_duration(d).bold()
            ).as_str());
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
                        message.push_str(format!(
                            "    {start} to {end} {diff:>width$} {project}\n",
                            start = format_time(period.start_time),
                            end = format_time(end_time),
                            diff = format_duration(diff),
                            project = period.project.clone().blue(),
                            width = 11
                        ).as_str());
                    }
                    None => {
                        let diff = Utc::now().signed_duration_since(period.start_time);
                        message.push_str(format!(
                            "    {start} to {end} {diff:>width$} {project}\n",
                            start = format_time(period.start_time),
                            end = format_time(Utc::now()),
                            diff = format_duration(diff),
                            project = period.project.clone().blue(),
                            width = 11
                        ).as_str());
                    }
                }
            }
        }
        Ok((message))
    }

    pub fn cancel(&mut self) -> DougResult {
        if let Some(period) = self.periods.pop() {
            if period.end_time.is_none() {
                self.save()?;
                let diff = Utc::now().signed_duration_since(period.start_time);
                return Ok((format!(
                    "Canceled project {}, started {} ago",
                    period.project.blue(),
                    format_duration(diff)
                )));
            }
        }
        Err("No project started".to_string())
    }

    pub fn stop(&mut self) -> DougResult {
        if let Some(mut period) = self.periods.pop() {
            if period.end_time.is_none() {
                period.end_time = Some(Utc::now());
                let diff = Utc::now().signed_duration_since(period.start_time);
                let messaage = format!(
                    "Stopped project {}, started {} ago",
                    period.project.blue(),
                    format_duration(diff)
                );
                self.periods.push(period);
                self.save()?;
                return Ok((messaage));
            }
        }
        Err(format!("Error: {}", "No project started.".red()))
    }

    /// Retrieve last active (including current) period
    fn last_period(&mut self) -> Option<&mut Period> {
        self.periods.last_mut()
    }

    pub fn edit(&mut self, start: Option<&str>, end: Option<&str>) -> DougResult {
        if let Some(start) = start {
            match parse_date_string(start, Local::now(), Dialect::Us) {
                Ok(x) => {
                    {
                        let period = match self.last_period() {
                            Some(x) => x,
                            None => {
                                return Err("no period to edit".to_string());
                            }
                        };
                        period.start_time = x.with_timezone(&Utc);
                    }
                    self.save()?;
                    if let Some(last_period) = self.clone().last_period() {
                        return Ok((format!("{}", last_period)));
                    } else {
                        return Err("Error: Couldn't find last period.".to_string());
                    }
                }
                Err(_) => {
                    return Err(format!("Couldn't parse date {}", start));
                }
            };
        }
        if let Some(end) = end {
            match parse_date_string(end, Local::now(), Dialect::Us) {
                Ok(x) => {
                    {
                        let period = match self.last_period() {
                            Some(x) => x,
                            None => return Err("no period to edit".to_string()),
                        };
                        period.end_time = Some(x.with_timezone(&Utc));
                    }
                    self.save()?;
                    if let Some(last_period) = self.clone().last_period() {
                        return Ok((format!("{}", last_period)));
                    } else {
                        return Err("Error: Couldn't find last period.".to_string());
                    }
                }
                Err(_) => {
                    return Err(format!("Couldn't parse date {}", end));
                }
            };
        }
        let home = match env::var("HOME") {
            Ok(x) => x,
            Err(_) => {
                return Err("Failed to find home directory from environment 'HOME'".to_string())
            }
        };
        let path = Path::new(&home).join(".doug/periods.json");
        let message = format!("File: {}\n", path.to_str().unwrap_or("none").blue());
        if let Some(editor) = env::var_os("EDITOR") {
            let mut edit = Command::new(editor);
            edit.arg(path.clone());
            let status = edit.status();
            if status.is_err() {
                return Err(format!("Error: {}", "Problem with editing.".red()));
            }
        } else {
            return Err(format!("Error: {}", "Couldn't open editor".red()));
        }
        Ok((message))
    }
}

pub fn add_interval(
    start_limit: DateTime<Utc>,
    end_limit: DateTime<Utc>,
    (start_time, end_time): (DateTime<Utc>, Option<DateTime<Utc>>),
    start_date: &mut Date<Local>,
    end_date: &mut Date<Local>,
) -> Duration {
    // Accumulates intervals based on limits.
    // Finds earliest start date for printing out later
    let end_time = end_time.unwrap_or_else(Utc::now);
    // start time is within starting limit
    if start_time >= start_limit && start_time <= end_limit {
        // Find starting date
        // FIXME:
        if start_time.with_timezone(&Local).date() < *start_date {
            *start_date = start_time.with_timezone(&Local).date();
        }

        // end time is outside of end limit
        if end_time > end_limit {
            if end_limit.with_timezone(&Local).date() > *end_date {
                *end_date = end_limit.with_timezone(&Local).date();
            }
            end_limit.signed_duration_since(start_time)
        } else {
            if end_time.with_timezone(&Local).date() > *end_date {
                *end_date = end_time.with_timezone(&Local).date();
            }
            end_time.signed_duration_since(start_time)
        }
    } else if end_time >= start_limit && end_time <= end_limit {
        // Find starting date
        // FIXME
        if start_limit.with_timezone(&Local).date() < *start_date {
            *start_date = start_limit.with_timezone(&Local).date();
        }

        if end_time > end_limit {
            if end_limit.with_timezone(&Local).date() > *end_date {
                *end_date = end_limit.with_timezone(&Local).date();
            }

            end_limit.signed_duration_since(start_time)
        } else {
            if end_time.with_timezone(&Local).date() > *end_date {
                *end_date = end_time.with_timezone(&Local).date();
            }

            end_time.signed_duration_since(start_limit)
        }
    } else {
        Duration::zero()
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
