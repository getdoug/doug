#[macro_use]
extern crate clap;

use clap::{App, Arg, AppSettings, SubCommand};

fn main() {
    App::new("Rus")
        .version(crate_version!())
        .about("A time tracking command-line utility")
        .author(crate_authors!())
        .settings(&[
            AppSettings::DeriveDisplayOrder,
            AppSettings::GlobalVersion,
            AppSettings::SubcommandRequiredElseHelp,
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
}
