//! `app_web accounts users ...`: accounts made, their passwords reset and
//! deleted from the server's command line, for a server that takes no
//! sign-ups, see `app_core::accounts::users::create`,
//! `accounts::password::update_from_command_line` and
//! `accounts::deletions::create_from_command_line`. Nothing is mailed. A
//! password is asked for twice without echo at a terminal, or read as one
//! line from standard input, so it never sits in the shell's history; a
//! delete asks first at a terminal.

use std::io::{self, BufRead, IsTerminal, Write};

use app_core::accounts::{deletions, password, users};
use app_core::deployment::Deployment;

const COMMANDS: &str = "commands: accounts users create <email>, accounts users password <email>, \
                        accounts users delete <email>";

/// `create <email>`, `password <email>` and `delete <email>`.
pub async fn run(deployment: &Deployment, words: &[&str]) -> Result<(), String> {
    match words {
        ["create", email] => {
            let password = password_read().await?;
            let account = users::create(deployment, email, &password)
                .await
                .map_err(|error| error.to_string())?;
            println!("{} can sign in", account.email);
            Ok(())
        }
        ["password", email] => {
            let new_password = password_read().await?;
            password::update_from_command_line(deployment, email, &new_password)
                .await
                .map_err(|error| error.to_string())?;
            println!("{email} has a new password");
            Ok(())
        }
        ["delete", email] => {
            let question = format!("Delete {email} and everything of theirs? Type yes: ");
            if !confirmed(question).await? {
                return Err("nothing deleted".to_string());
            }
            deletions::create_from_command_line(deployment, email)
                .await
                .map_err(|error| error.to_string())?;
            println!("{email} is deleted");
            Ok(())
        }
        _ => Err(COMMANDS.to_string()),
    }
}

async fn password_read() -> Result<String, String> {
    tokio::task::spawn_blocking(password_read_blocking)
        .await
        .map_err(|error| error.to_string())?
}

/// The new password: typed twice, hidden, at a terminal; else the first
/// line of standard input.
fn password_read_blocking() -> Result<String, String> {
    if !io::stdin().is_terminal() {
        return line_read();
    }
    let password = rpassword::prompt_password("Password: ")
        .map_err(|error| format!("could not read the password: {error}"))?;
    let again = rpassword::prompt_password("Password again: ")
        .map_err(|error| format!("could not read the password: {error}"))?;
    if password != again {
        return Err("the two passwords differ".to_string());
    }
    Ok(password)
}

/// Whether the person typed yes at a terminal; true with no terminal.
async fn confirmed(question: String) -> Result<bool, String> {
    tokio::task::spawn_blocking(move || {
        if !io::stdin().is_terminal() {
            return Ok(true);
        }
        print!("{question}");
        io::stdout().flush().map_err(|error| error.to_string())?;
        Ok(line_read()?.trim() == "yes")
    })
    .await
    .map_err(|error| error.to_string())?
}

fn line_read() -> Result<String, String> {
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|error| format!("could not read standard input: {error}"))?;
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}
