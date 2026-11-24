//! Every email the app sends, composed here, text only, and handed to
//! the `mailer`, in the language given: its English is in `t!`, see
//! `helpers::translations`, and which language a mail goes in is the
//! sender's, see `accounts::preferences::mail_locale_in`. The app's name
//! is `app::identity::APP_NAME`, the `{app}` in each text.

use crate::app::identity::APP_NAME;
use crate::helpers::translations::t;
use crate::services::mailer::Mail;

/// The code a sign-in is finished with.
pub fn sign_in_code(locale: &str, to: &str, code: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: t!(locale, "Your {app} code", app = APP_NAME),
        text: t!(
            locale,
            "Your code is {code}. It expires in ten minutes.\n\n\
             If you did not ask for it, ignore this email.",
            code = code,
        ),
    }
}

/// To an email with no account someone signed in with on a server that
/// takes no new sign-ups; the sign-in answered as if it had one, see
/// `accounts::sign_in`.
pub fn no_account(locale: &str, to: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: t!(locale, "No {app} account for this email", app = APP_NAME),
        text: t!(
            locale,
            "Someone, likely you, tried to log in to {app} with this email. \
             There is no account for it, and {app} is not taking new sign-ups \
             right now: log in with the email you signed up with.\n\n\
             If this was not you, ignore this email.",
            app = APP_NAME,
        ),
    }
}

/// The code that confirms deleting an account, see
/// `accounts::deletions::code::create`.
pub fn delete_code(locale: &str, to: &str, code: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: t!(
            locale,
            "Your code to delete your {app} account",
            app = APP_NAME
        ),
        text: t!(
            locale,
            "Your code is {code}. It expires in ten minutes. Entering it deletes \
             your account and everything in it, and cannot be undone.\n\n\
             If you did not ask for it, someone is logged in to your account: \
             log in and change your password.",
            code = code,
        ),
    }
}

/// To the address an account had, once it was deleted.
pub fn account_deleted(locale: &str, to: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: t!(locale, "Your {app} account was deleted", app = APP_NAME),
        text: t!(
            locale,
            "Your {app} account and everything in it were deleted, as you asked. \
             A plan bought in the App Store is billed by Apple until you cancel it \
             in your Apple ID's subscriptions.\n\n\
             You can sign up again with this email at any time.",
            app = APP_NAME,
        ),
    }
}

/// To an account whose password was made or changed, see
/// `accounts::password::update`.
pub fn password_changed(locale: &str, to: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: t!(locale, "Your password was changed"),
        text: t!(
            locale,
            "The password of your {app} account was just set.\n\n\
             If this was not you, log in with a code sent to this email and \
             change it, then check who else can read your email.",
            app = APP_NAME,
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
    use crate::helpers::locales;

    #[test]
    fn mails_sign_in_code_carries_the_code_and_code_in_reads_it_back() {
        let mail = sign_in_code(locales::DEFAULT, "alice@example.com", "042137");
        assert_eq!(mail.to, "alice@example.com");
        assert!(mail.subject.contains(APP_NAME));
        assert_eq!(code_in(&mail), Some("042137".to_string()));
        assert_eq!(
            code_in(&no_account(locales::DEFAULT, "alice@example.com")),
            None
        );
    }

    #[test]
    fn mails_go_in_the_language_given_with_their_placeholders_filled() {
        let mail = sign_in_code("es", "alice@example.com", "042137");
        assert!(mail.subject.contains(APP_NAME));
        assert!(!mail.subject.contains("{app}"));
        assert!(
            !mail.subject.starts_with("Your"),
            "in Spanish: {}",
            mail.subject
        );
        assert_eq!(code_in(&mail), Some("042137".to_string()));
    }
}
