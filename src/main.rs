#![cfg_attr(test, deny(warnings))]

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
use std::process;

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
                SubCommand::with_name("edit")
                    .about("Edit last frame or currently running frame")
                    .arg(
                        Arg::with_name("start")
                            .short("s")
                            .long("start")
                            .help("starting date")
                            .takes_value(true),
                    ).arg(
                        Arg::with_name("end")
                            .short("e")
                            .long("end")
                            .help("ending date")
                            .takes_value(true),
                    ),
            ).subcommand(
                SubCommand::with_name("generate-completions")
                    .about("Generate completions")
                    .arg(
                        Arg::with_name("shell")
                            .help("shell to generate completion for (default: bash).")
                            .short("s")
                            .long("shell")
                            .possible_values(&["bash", "zsh", "fish", "powershell"])
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

    let mut doug = match Doug::new() {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1)
        }
    };

    if !atty::is(Stream::Stdout) {
        colored::control::set_override(false);
    }

    let results: Result<(), String> = if let Some(matches) = matches.subcommand_matches("start") {
        if matches.is_present("project") {
            doug.start(matches.value_of("project").unwrap())
        } else {
            // Restart last project if not argument is provided
            doug.restart()
        }
    } else if let Some(matches) = matches.subcommand_matches("amend") {
        if let Some(project) = matches.value_of("project") {
                doug.amend(project)
        } else {
            Err("Missing project name".to_string())
        }
    } else if let Some(matches) = matches.subcommand_matches("delete") {
        doug.delete(matches.value_of("project").expect("missing project name"))
    } else if let Some(matches) = matches.subcommand_matches("status") {
        doug.status(matches.is_present("s"), matches.is_present("t"))
    } else if let Some(matches) = matches.subcommand_matches("report") {
        doug.report(
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
        )
    } else if let Some(matches) = matches.subcommand_matches("generate-completions") {
        if matches.is_present("shell") {
            match matches.value_of("shell") {
                Some("bash") => {
                    cli.gen_completions_to("doug", Shell::Bash, &mut stdout());
                    Ok(())
                },
                Some("zsh") => {
                    cli.gen_completions_to("doug", Shell::Zsh, &mut stdout());
                    Ok(())
                },
                Some("fish") => {
                    cli.gen_completions_to("doug", Shell::Fish, &mut stdout());
                    Ok(())
                },
                Some("powershell") => {
                    cli.gen_completions_to("doug", Shell::PowerShell, &mut stdout());
                    Ok(())
                }
                _ => Err("Invalid option".to_string()),
            }
        } else {
            Ok(())
        }
    } else if let Some(matches) = matches.subcommand_matches("edit") {
        doug.edit(matches.value_of("start"), matches.value_of("end"))
    } else {
        match matches.subcommand_name() {
            Some("stop") => doug.stop(),
            Some("cancel") => doug.cancel(),
            Some("restart") => doug.restart(),
            Some("log") => doug.log(),
            _ => Ok(())
        }
    };

    match results {
        Ok(x) => {
            // println!("{}", x);
            process::exit(0)
        },
        Err(e) => {
            eprintln!("{}", e);
            process::exit(0)
        }
    }
}
