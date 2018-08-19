extern crate atty;
extern crate chrono;
#[macro_use]
extern crate clap;
extern crate colored;
extern crate doug;
extern crate serde;
extern crate serde_json;

use std::io::stdout;

use atty::Stream;
use clap::{App, AppSettings, Arg, Shell, SubCommand};

use doug::*;

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
    let mut cli =
        App::new("Doug")
            .version(crate_version!())
            .about("A time tracking command-line utility")
            .author(crate_authors!())
            .settings(&[
                AppSettings::DeriveDisplayOrder,
                AppSettings::GlobalVersion,
                AppSettings::SubcommandRequiredElseHelp,
                AppSettings::VersionlessSubcommands,
                AppSettings::DisableHelpSubcommand,
            ]).subcommand(
                SubCommand::with_name("start")
                    .about("Track new or existing project")
                    .arg(Arg::with_name("project").help(
                        "project to track. If missing, start subcommand behaves like restart.",
                    )),
            ).subcommand(
                SubCommand::with_name("status")
                    .about("Display elapsed time, start time, and running project name")
                    .arg(
                        Arg::with_name("t")
                            .short("t")
                            .long("time")
                            .help("Print time for currently tracked project."),
                    ).arg(Arg::with_name("s").short("s").long("simple").help(
                        "Print running project name or nothing if there isn't a running project.",
                    )),
            ).subcommand(SubCommand::with_name("stop").about("Stop any running projects"))
            .subcommand(
                SubCommand::with_name("cancel")
                    .about("Stop running project and remove most recent time interval"),
            ).subcommand(SubCommand::with_name("restart").about("Track last running project"))
            .subcommand(
                SubCommand::with_name("log").about("Display time intervals across all projects"),
            ).subcommand(
                SubCommand::with_name("report")
                    .about("Display aggregate time from projects")
                    .arg(
                        Arg::with_name("year")
                            .short("y")
                            .long("year")
                            .help("Limit report to past year. Use multiple to increase interval.")
                            .overrides_with_all(&["month", "week", "day", "from", "to"])
                            .multiple(true),
                    ).arg(
                        Arg::with_name("month")
                            .short("m")
                            .long("month")
                            .help("Limit report to past month. Use multiple to increase interval.")
                            .overrides_with_all(&["year", "week", "day", "from", "to"])
                            .multiple(true),
                    ).arg(
                        Arg::with_name("week")
                            .short("w")
                            .long("week")
                            .help("Limit report to past week. Use multiple to increase interval.")
                            .overrides_with_all(&["year", "month", "day", "from", "to"])
                            .multiple(true),
                    ).arg(
                        Arg::with_name("day")
                            .short("d")
                            .long("day")
                            .help("Limit report to past day. Use multiple to increase interval.")
                            .overrides_with_all(&["year", "month", "week", "from", "to"])
                            .multiple(true),
                    ).arg(
                        Arg::with_name("from")
                            .short("f")
                            .long("from")
                            .help("Date when report should start (e.g. 2018-1-1)")
                            .overrides_with_all(&["year", "month", "week", "day"])
                            .takes_value(true),
                    ).arg(
                        Arg::with_name("to")
                            .short("t")
                            .long("to")
                            .help("Date when report should end (e.g. 2018-1-20)")
                            .overrides_with_all(&["year", "month", "week", "day"])
                            .takes_value(true),
                    ),
            ).subcommand(
                SubCommand::with_name("amend")
                    .about("Change name of currently running project")
                    .arg(
                        Arg::with_name("project")
                            .help("new project name")
                            .required(true),
                    ),
            ).subcommand(
                SubCommand::with_name("edit").about("Edit last frame or currently running frame"),
            ).subcommand(
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
            ).subcommand(
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
        } else {
            // Restart last project if not argument is provided
            restart(&time_periods, save_periods);
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
            matches.value_of("from"),
            matches.value_of("to"),
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
