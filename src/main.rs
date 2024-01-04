use anyhow::Result;
use chrono::{DateTime, Local};
use colored::Colorize;
use rustyline::{error::ReadlineError, history::FileHistory, DefaultEditor, Editor};
use serde::{Deserialize, Serialize};
use std::{
    env::{self},
    fs::{self, File},
    path::Path,
    process::{Child, Command, Stdio},
    str::FromStr,
};

#[derive(Debug)]
enum Builtin {
    History,
    Cd,
    Pwd,
    Clear,
    Exit,
    ClearHistory,
    Other(String),
}

impl FromStr for Builtin {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "history" => Ok(Builtin::History),
            "cd" => Ok(Builtin::Cd),
            "pwd" => Ok(Builtin::Pwd),
            "clear" => Ok(Builtin::Clear),
            "exit" => Ok(Builtin::Exit),
            "clearHistory" => Ok(Builtin::ClearHistory),
            _ => Ok(Builtin::Other(s.to_owned())),
        }
    }
}

#[derive(Debug)]
enum FilePipe<'a> {
    ReadFile(&'a str),
    WriteFile(&'a str),
}

#[derive(Debug, Serialize, Deserialize)]
struct History {
    command: String,
    date: DateTime<Local>,
}

fn main() -> Result<()> {
    let mut rl = DefaultEditor::new()?;

    if rl.load_history("history.txt").is_err() {
        println!("{}", "No previous history.".red());
    }
    let mut currct_path = "/".to_owned();
    loop {
        let readline = rl.readline(&format!(
            "{} {}{}",
            "$".green(),
            currct_path.green(),
            "/ : ".green()
        ));

        match readline {
            Ok(line) => {
                rl.add_history_entry(&line)?;
                if let Err(err) = handel_command(line, &mut rl, &mut currct_path) {
                    eprintln!("{}", err.to_string().red());
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("{}", "CTRL-C, Bye".blue());
                break;
            }
            Err(err) => {
                println!("{} {:?}", "Error: ".red(), err);
                break;
            }
        }
    }
    rl.save_history("./history.txt")?;
    Ok(())
}

fn handel_command(
    input: String,
    rl: &mut Editor<(), FileHistory>,
    curruct_path: &mut String,
) -> Result<()> {
    let mut commands = input.trim().split(" | ").peekable();
    let mut previous_command = None;

    while let Some(command) = commands.next() {
        let (file, mut parts) = if command.contains("<<") {
            let mut parts = command.split("<<");
            let com = parts.next().unwrap().split_whitespace();
            (Some(FilePipe::ReadFile(parts.next().unwrap().trim())), com)
        } else if command.contains(">>") {
            let mut parts = command.split(">>");
            let com = parts.next().unwrap().split_whitespace();
            (Some(FilePipe::WriteFile(parts.next().unwrap().trim())), com)
        } else {
            (None, command.split_whitespace())
        };
        let command = parts.next().unwrap();
        let args = parts;

        match Builtin::from_str(command).unwrap() {
            Builtin::History => rl
                .history()
                .iter()
                .for_each(|history| println!("{}", history.purple())),
            Builtin::Cd => {
                let new_dir = args.peekable().peek().map_or("/", |x| *x);
                let root = Path::new(new_dir);
                if let Err(e) = env::set_current_dir(&root) {
                    eprintln!("{}", e);
                } else {
                    *curruct_path = new_dir.to_owned();
                }
                previous_command = None;
            }
            Builtin::Pwd => println!("{}", curruct_path.purple()),
            Builtin::Clear => rl.clear_screen()?,
            Builtin::Exit => break,
            Builtin::ClearHistory => {
                rl.clear_history()?;
                fs::write("./history.txt", "#V2\n")?;
                println!("{}", "history cleared".purple());
            }
            Builtin::Other(command) => {
                let stdin = if let Some(FilePipe::ReadFile(file)) = file {
                    Stdio::from(File::open(file)?)
                } else {
                    previous_command.map_or(Stdio::inherit(), |output: Child| {
                        Stdio::from(output.stdout.unwrap())
                    })
                };

                let stdout = if let Some(FilePipe::WriteFile(file)) = file {
                    Stdio::from(File::options().write(true).create(true).open(file)?)
                } else if commands.peek().is_some() {
                    Stdio::piped()
                } else {
                    Stdio::inherit()
                };

                let output = Command::new(command)
                    .args(args)
                    .stdin(stdin)
                    .stdout(stdout)
                    .spawn();

                match output {
                    Ok(output) => {
                        previous_command = Some(output);
                    }
                    Err(e) => {
                        previous_command = None;
                        eprintln!("{}{}", "command failed to start : ".red(), e);
                    }
                };
            }
        }
    }

    if let Some(mut final_command) = previous_command {
        final_command.wait()?;
    }

    Ok(())
}
