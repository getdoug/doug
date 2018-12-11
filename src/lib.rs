#![allow(clippy::new_ret_no_self)]

pub mod format;
pub mod settings;

use std::cmp::{max, min};
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::fs;
use std::fs::{DirBuilder, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use chrono::{Date, DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
use chrono_english::{parse_date_string, Dialect};
use colored::*;
use serde_derive::{Deserialize, Serialize};
use serde_json::Error;

type ProjectName = String;

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct Period {
    project: ProjectName,
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
        let end_time = self.end_time.unwrap_or_else(Utc::now);
        let diff = end_time.signed_duration_since(self.start_time);
        let start_time = format::time(self.start_time);
        let end_time = match self.end_time {
            Some(time) => format::time(time),
            None => "present".to_string(),
        };
        write!(
            f,
            "{} to {} {}",
            start_time,
            end_time,
            format::duration(diff).purple()
        )
    }
}

/// Doug, a time tracking command-line utility.
///
/// This is the backend where all the logic for Doug is kept.
/// The current implementation uses `$HOME/.doug/` for storing data,
/// while the CLI stuff is handled by [clap] in `main.rs`.
#[derive(Clone)]
pub struct Doug {
    periods: Vec<Period>,
    /// Path to settings.json file
    settings: settings::Settings,
    settings_location: PathBuf,
}

type DougResult = Result<Option<String>, String>;

impl Doug {
    /// Initialize a new Doug instance
    ///
    /// If missing, the data file will be created at `$HOME/.doug/periods.json`.
    ///
    /// # Arguments
    /// * `path` — an optional path to the root of the data folder.
    ///
    /// # Examples
    /// ```
    /// # extern crate tempfile;
    /// # extern crate doug;
    /// # use doug::*;
    /// # let tempdir = tempfile::tempdir().unwrap().into_path();
    /// # let tempdir = tempdir.to_string_lossy();
    /// #
    /// // Create a new Doug instance with default data location
    /// let doug = Doug::new(None).unwrap();
    ///
    /// let doug = Doug::new(Some(&tempdir)).unwrap();
    /// ```
    pub fn new(path: Option<&str>) -> Result<Self, String> {
        let folder = match path {
            Some(path) => PathBuf::from(path),
            None => {
                let home_dir = env::var("HOME").map_err(|_| "Failed to find home directory from environment 'HOME'. Doug needs 'HOME' to be set to find its data.".to_string())?;
                let mut folder = PathBuf::from(home_dir);
                folder.push(".doug");
                folder
            }
        };

        let settings = settings::Settings::new(&folder)?;

        // create .doug directory
        DirBuilder::new()
            .recursive(true)
            .create(&settings.data_location)
            .map_err(|_| format!("Couldn't create data directory: {:?}\n", folder))?;

        // create data file
        let location = settings.data_location.as_path().join("periods.json");
        let data_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&location)
            .map_err(|_| format!("Couldn't open datafile: {:?}\n", location))?;

        // serialize periods from data file
        let periods: Result<Vec<Period>, Error> = serde_json::from_reader(data_file);

        match periods {
            Ok(periods) => Ok(Doug {
                periods,
                settings,
                settings_location: folder,
            }),
            // No periods exist. Create a new Doug instance.
            Err(ref error) if error.is_eof() => Ok(Doug {
                periods: Vec::new(),
                settings,
                settings_location: folder,
            }),
            Err(error) => Err(format!("There was a serialization issue: {:?}\n", error)),
        }
    }

    /// Log currently running project, duration of current period, and the datetime tracking
    /// started.
    ///
    /// If there is no running project, logs `No running project`. The CLI returns exit code 1.
    ///
    /// See arguments to refine the output of this command.
    ///
    /// # Arguments
    /// * `simple_name` — Print just the name of the currently running project,
    /// or nothing.
    /// * `simple_time` — Print just the current time, formated with [format::duration].
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate tempfile;
    /// # extern crate doug;
    /// # let tempdir = tempfile::tempdir().unwrap().into_path();
    /// # let tempdir = tempdir.to_string_lossy();
    /// # use doug::*;
    /// // We don't want to mess with existing installations
    /// # let mut doug = Doug::new(Some(&tempdir)).unwrap();
    /// #
    /// // with no running project, this will return Err
    /// doug.status(false, false).expect_err("No running project");
    ///
    /// doug.start("test");
    ///
    /// // no args
    /// doug.status(false, false).expect("Should return Ok");
    ///
    /// // simple_name
    /// doug.status(true, false).expect("Should return Ok");
    ///
    /// // simple_time
    /// doug.status(false, true).expect("Should be fine too");
    ///
    /// # doug.stop();
    /// ```
    pub fn status(&self, simple_name: bool, simple_time: bool) -> DougResult {
        if let Some(period) = &self.periods.last() {
            if period.end_time.is_none() {
                let diff = Utc::now().signed_duration_since(period.start_time);
                let message = if simple_name {
                    format!("{}\n", period.project)
                } else if simple_time {
                    format!("{}\n", format::duration(diff))
                } else {
                    format!(
                        "Project {} started {} ago ({})\n",
                        period.project.magenta(),
                        format::duration(diff),
                        format::datetime(period.start_time).blue()
                    )
                };
                return Ok(Some(message));
            }
        }
        if !(simple_name || simple_time) {
            Err("No running project".to_string())
        } else {
            Ok(None)
        }
    }

    pub fn settings(&mut self, path: Option<&str>, clear: bool) -> DougResult {
        if clear {
            self.settings.clear(&self.settings_location)?;
            return Ok(Some("Cleared settings file".to_string()));
        }
        if let Some(path) = path {
            DirBuilder::new()
                .recursive(true)
                .create(&path)
                .map_err(|err| format!("Couldn't create data directory: {:?}\n", err))?;
            self.settings.data_location = PathBuf::from(path);
            self.settings.save(&self.settings_location)?;
            self.save()?;
        }
        Ok(Some(format!(
            "{}:\n{:#?}",
            self.settings_location.to_string_lossy(),
            self.settings
        )))
    }

    /// Save period data to file.
    ///
    /// A backup of the data file will be made before serializing the data.
    pub fn save(&self) -> DougResult {
        let serialized = serde_json::to_string(&self.periods)
            .map_err(|_| "Couldn't serialize data to string".to_string())?;
        let mut location_backup = self.data_location();
        location_backup.set_extension("json-backup");
        fs::copy(&self.data_location(), &location_backup)
            .map_err(|err| format!("Couldn't create backup file: {:?}", err))?;
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.data_location())
            .map_err(|err| format!("Couldn't open file for saving period: {:?}", err))?;
        file.write_all(serialized.as_bytes())
            .map_err(|_| "Couldn't write serialized data to file".to_string())?;
        Ok(None)
    }

    fn data_location(&self) -> PathBuf {
        self.settings.data_location.clone().join("periods.json")
    }

    /// Start tracking a project.
    ///
    /// In the CLI, we call [Doug::restart] if no `project_name` is provided.
    ///
    /// # Arguments
    /// * `project_name` — name of project to start tracking a new period with.
    pub fn start(&mut self, project_name: &str) -> DougResult {
        if !self.periods.is_empty() {
            if let Some(period) = self.periods.last_mut() {
                if period.end_time.is_none() {
                    let mut error = format!("project {} is being tracked\n", period.project);
                    error.push_str(
                        format!(
                            "Try stopping your current project with {} first.",
                            "stop".blue()
                        )
                        .as_str(),
                    );
                    return Err(error);
                }
            }
        }
        let current_period = Period::new(project_name);
        let message = format!(
            "Started tracking project {} at {}\n",
            current_period.project.blue(),
            format::time(current_period.start_time)
        );
        self.periods.push(current_period);
        self.save()?;
        Ok(Some(message))
    }

    /// Change name of currently running period.
    ///
    /// Will exit 1 if there isn't any running project.
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
                return Ok(Some(message));
            }
        }
        Err("No project started".to_string())
    }

    /// Aggregate periods per project.
    pub fn report(
        &self,
        past_years: i32,
        past_months: i32,
        past_weeks: i32,
        past_days: i32,
        from_date: Option<&str>,
        to_date: Option<&str>,
    ) -> DougResult {
        let (from_date, to_date): (Date<Local>, Date<Local>) =
            if past_years > 0 || past_months > 0 || past_weeks > 0 || past_days > 0 {
                let duration = Duration::weeks((52_i32 * past_years).into())
                    + Duration::weeks((4_i32 * past_months).into())
                    + Duration::weeks(past_weeks.into())
                    + Duration::days(past_days.into());
                let today = Local::now().date();
                let start = today - duration;

                (start, today)
            } else {
                let from_date_parsed: Date<Local> = {
                    if let Some(from) = from_date {
                        parse_date_string(&from, Local::now(), Dialect::Us)
                            .map_err(|_| format!("Couldn't parse date {}", from))?
                            .date()
                    } else {
                        Utc.from_utc_date(&NaiveDate::from_ymd(1, 1, 1))
                            .with_timezone(&Local)
                    }
                };
                let to_date_parsed: Date<Local> = {
                    if let Some(to) = to_date {
                        parse_date_string(&to, Local::now(), Dialect::Us)
                            .map_err(|_| format!("Couldn't parse date {}", to))?
                            .date()
                    } else {
                        Local::now().date()
                    }
                };

                (from_date_parsed, to_date_parsed)
            };

        let mut days: HashMap<ProjectName, Vec<Period>> = HashMap::new();
        // organize periods by project
        for period in &self.periods {
            days.entry(period.project.clone())
                .or_insert_with(Vec::new)
                .push(period.clone());
        }

        let mut results: Vec<(ProjectName, Duration)> = Vec::new();

        let mut max_proj_len = 0;
        let mut max_diff_len = 0;

        // start of the earliest interval
        let mut min_start_date = Local::now().date();

        for (project, intervals) in &days {
            // sum total time per project
            let duration = intervals.into_iter().fold(Duration::zero(), |acc, period| {
                let period_start_time = period.start_time.with_timezone(&Local);

                let period_duration: Duration = period
                    .end_time
                    .unwrap_or_else(Utc::now)
                    .signed_duration_since(period_start_time);

                let is_valid_start =
                    from_date <= period_start_time.date() && period_start_time.date() <= to_date;

                if is_valid_start {
                    min_start_date = min(min_start_date, period_start_time.date());
                    acc + period_duration
                } else {
                    acc
                }
            });

            // skip projects that weren't worked on
            if duration == Duration::zero() {
                continue;
            }

            // find lengths of project names for alignment
            max_proj_len = max(project.to_string().len(), max_proj_len);
            // find lengths of durations names for alignment
            max_diff_len = max(
                format::duration(duration).len(),
                format::duration(duration).len(),
            );

            results.push((project.clone(), duration));
        }
        let mut message = format!(
            "{start} -> {end}\n",
            start = min_start_date.format("%A %-d %B %Y").to_string().blue(),
            end = to_date.format("%A %-d %B %Y").to_string().blue()
        );
        results.sort();
        for (project, duration) in &results {
            message.push_str(
                format!(
                    "{project:pwidth$} {duration:>dwidth$}\n",
                    project = project.green(),
                    duration = format::duration(*duration).bold(),
                    pwidth = max_proj_len,
                    dwidth = max_diff_len
                )
                .as_str(),
            );
        }
        Ok(Some(message))
    }

    /// Remove all periods for a project
    ///
    /// # Arguments
    /// * `project_name` — project to remove
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
            Err("Project not found.\n".to_string())
        } else {
            self.periods = filtered_periods;
            self.save()?;
            Ok(Some(format!(
                "Deleted project {project}\n",
                project = project_name.blue()
            )))
        }
    }

    /// Restart last running period
    pub fn restart(&mut self) -> DougResult {
        let mut new_periods = self.periods.to_vec();
        if let Some(period) = self.periods.clone().last() {
            if period.end_time.is_some() {
                let new_period = Period::new(&period.project);
                new_periods.push(new_period);
                self.periods = new_periods.to_vec();
                self.save()?;
                return Ok(Some(format!(
                    "Tracking last running project: {}",
                    period.project.blue()
                )));
            } else {
                let mut error = format!(
                    "No project to restart. Project {} is being tracked\n",
                    period.project
                );
                error.push_str(
                    format!(
                        "Try stopping your current project with {} first.",
                        "stop".blue()
                    )
                    .as_str(),
                );
                return Err(error);
            }
        }
        Err("No previous project to restart".to_string())
    }

    /// List periods in chronological order
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
            message.push_str(
                format!(
                    "{date} ({duration})\n",
                    date = date
                        .with_timezone(&Local)
                        .format("%A %-d %B %Y")
                        .to_string()
                        .green(),
                    duration = format::duration(d).bold()
                )
                .as_str(),
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
                        message.push_str(
                            format!(
                                "    {start} to {end} {diff:>width$} {project}\n",
                                start = format::time(period.start_time),
                                end = format::time(end_time),
                                diff = format::duration(diff),
                                project = period.project.clone().blue(),
                                width = 11
                            )
                            .as_str(),
                        );
                    }
                    None => {
                        let diff = Utc::now().signed_duration_since(period.start_time);
                        message.push_str(
                            format!(
                                "    {start} to {end} {diff:>width$} {project}\n",
                                start = format::time(period.start_time),
                                end = format::time(Utc::now()),
                                diff = format::duration(diff),
                                project = period.project.clone().blue(),
                                width = 11
                            )
                            .as_str(),
                        );
                    }
                }
            }
        }
        Ok(Some(message))
    }

    /// Stop current period and remove log entry
    pub fn cancel(&mut self) -> DougResult {
        match self.periods.pop() {
            Some(ref mut period) if period.end_time.is_none() => {
                self.save()?;
                let diff = Utc::now().signed_duration_since(period.start_time);
                Ok(Some(format!(
                    "Canceled project {}, started {} ago",
                    period.project.blue(),
                    format::duration(diff)
                )))
            }
            _ => Err("No project started.".to_string()),
        }
    }

    /// Stop current period and save stop time
    pub fn stop(&mut self) -> DougResult {
        match self.periods.pop() {
            Some(ref mut period) if period.end_time.is_none() => {
                period.end_time = Some(Utc::now());
                let diff = Utc::now().signed_duration_since(period.start_time);
                let messaage = format!(
                    "Stopped project {}, started {} ago",
                    period.project.blue(),
                    format::duration(diff)
                );
                self.periods.push(period.clone());
                self.save()?;
                Ok(Some(messaage))
            }
            _ => Err("No project started.".to_string()),
        }
    }

    /// Retrieve last active (including current) period
    fn last_period(&mut self) -> Option<&mut Period> {
        self.periods.last_mut()
    }

    /// Edit last running period.
    ///
    /// If no arguments are provided, the data file is open `$EDITOR`.
    ///
    /// # Arguments
    /// * `start` — date to set start time of last period.
    /// * `end` — date to set end time of last period.
    ///
    /// Both arguments accept humanized dates (e.g. `thursday 9:00am`, `today 12:15pm`)
    pub fn edit(&mut self, start: Option<&str>, end: Option<&str>) -> DougResult {
        if let Some(start) = start {
            let date = parse_date_string(start, Local::now(), Dialect::Us)
                .map_err(|_| format!("Couldn't parse date {}", start))?;
            let period = self
                .last_period()
                .ok_or_else(|| "no period to edit".to_string())?;
            period.start_time = date.with_timezone(&Utc);
        }

        if let Some(end) = end {
            let date = parse_date_string(end, Local::now(), Dialect::Us)
                .map_err(|_| format!("Couldn't parse date {}", end))?;
            let period = self
                .last_period()
                .ok_or_else(|| "no period to edit".to_string())?;
            period.end_time = Some(date.with_timezone(&Utc));
        }
        if start.is_some() || end.is_some() {
            self.save()?;
            return Ok(Some(
                self.clone()
                    .last_period()
                    .ok_or_else(|| "Couldn't find last period.".to_string())?
                    .to_string(),
            ));
        }
        let message = format!(
            "File: {}\n",
            self.data_location().to_str().ok_or("Invalid path")?.blue()
        );
        let editor = env::var("EDITOR").map_err(|_| "Couldn't open editor".to_string())?;
        let mut edit = Command::new(editor);
        edit.arg(self.data_location().clone());
        edit.status()
            .map_err(|_| "Problem with editing.".to_string())?;
        Ok(Some(message))
    }
}
