//! Every email the app sends, composed here, text only, and handed to
//! the `mailer`. The app's name is `APP_NAME`, to change in a fork.

use crate::services::mailer::Mail;

pub const APP_NAME: &str = "Starter Pack";

/// The code a sign-in is finished with.
pub fn sign_in_code(to: &str, code: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: format!("Your {APP_NAME} code"),
        text: format!(
            "Your code is {code}. It expires in ten minutes.\n\n\
             If you did not ask for it, ignore this email."
        ),
    }
}

/// To an email with no account someone signed in with on a server that
/// takes no new sign-ups; the sign-in answered as if it had one, see
/// `accounts::sign_in`.
pub fn no_account(to: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: format!("No {APP_NAME} account for this email"),
        text: format!(
            "Someone, likely you, tried to log in to {APP_NAME} with this email. \
             There is no account for it, and {APP_NAME} is not taking new sign-ups \
             right now: log in with the email you signed up with.\n\n\
             If this was not you, ignore this email."
        ),
    }
}

/// The code that confirms deleting an account, see
/// `accounts::deletions::code::create`.
pub fn delete_code(to: &str, code: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: format!("Your code to delete your {APP_NAME} account"),
        text: format!(
            "Your code is {code}. It expires in ten minutes. Entering it deletes \
             your account and everything in it, and cannot be undone.\n\n\
             If you did not ask for it, someone is logged in to your account: \
             log in and change your password."
        ),
    }
}

/// To the address an account had, once it was deleted.
pub fn account_deleted(to: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: format!("Your {APP_NAME} account was deleted"),
        text: format!(
            "Your {APP_NAME} account and everything in it were deleted, as you asked. \
             A plan bought in the App Store is billed by Apple until you cancel it \
             in your Apple ID's subscriptions.\n\n\
             You can sign up again with this email at any time."
        ),
    }
}

/// To an account whose password was made or changed, see
/// `accounts::password::update`.
pub fn password_changed(to: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: "Your password was changed".to_string(),
        text: format!(
            "The password of your {APP_NAME} account was just set.\n\n\
             If this was not you, log in with a code sent to this email and \
             change it, then check who else can read your email."
        ),
    }
}

/// The six-digit code an email carries, for a test reading it back.
pub fn code_in(mail: &Mail) -> Option<String> {
    let digits: Vec<char> = mail.text.chars().collect();
    digits
        .windows(6)
        .find(|window| window.iter().all(char::is_ascii_digit))
        .map(|window| window.iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mails_sign_in_code_carries_the_code_and_code_in_reads_it_back() {
        let mail = sign_in_code("alice@example.com", "042137");
        assert_eq!(mail.to, "alice@example.com");
        assert!(mail.subject.contains(APP_NAME));
        assert_eq!(code_in(&mail), Some("042137".to_string()));
        assert_eq!(code_in(&no_account("alice@example.com")), None);
    }
}
