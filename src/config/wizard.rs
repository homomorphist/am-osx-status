
fn loose_matches(str: &str, one_of: &[&str]) -> bool {
    for check in one_of {
        if str.eq_ignore_ascii_case(check) {
            return true
        }
    }
    false
}

fn str_to_boolish(str: &str) -> Option<bool>  {
    let str = str.trim();
    if loose_matches(str, &["y", "yes", "true"]) { return Some(true) }
    if loose_matches(str, &["n", "no", "false"]) { return Some(false) }
    None
}

pub mod io {
    use std::io::{Write, BufRead};

    use crate::util::ferror;

    use super::*;
    
    pub fn prompt(prompt: &str, initial_capacity: usize) -> String {
        {
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(prompt.as_bytes()).unwrap();
            stdout.write_all(b"\n=> ").unwrap();
            stdout.flush().unwrap();
        }

        let mut str_buf = String::with_capacity(initial_capacity);
        {
            let mut stdin = std::io::stdin().lock();
            stdin.read_line(&mut str_buf).expect("could not process user input");
        }

        str_buf
    }
    
    pub fn prompt_bool(prompt: &str) -> bool {
        let mut answer = String::with_capacity(4);
        loop {
            {
                let mut stdout = std::io::stdout().lock();
                stdout.write_all(prompt.as_bytes()).unwrap();
                stdout.write_all(b" (y/n)\n=> ").unwrap();
                stdout.flush().unwrap();
            }
    
            {

                let mut stdin = std::io::stdin().lock();
                let r = stdin.read_line(&mut answer);
                r.expect("could not process user input");
            }

            if let Some(bool) = str_to_boolish(&answer) { return bool };
            println!(r#"Invalid input! Enter "yes" or "no"."#);
            println!();
            answer.clear();
        }
    }

    fn prompt_choice_maybe_optional(options: &[&str], prompt: &str, marked_optional: bool) -> Option<usize> {
        assert!(!options.is_empty(), "no options provided to `prompt_choice`");

        let mut string_size = options.len().ilog(10) as usize;
        string_size += 1; // +1 for offset
        string_size += 1; // +1 for '\n'
        string_size += 1; // +1 to not silently cut bad numbers (e.x. 20 => 2)

        let mut answer = String::with_capacity(string_size);

        loop {
            {
                let mut stdout = std::io::stdout().lock();
                stdout.write_all(prompt.as_bytes()).unwrap();

                if marked_optional {
                    stdout.write_all(b"\n(optional; press enter without any value to skip)\n").unwrap();
                }
                
                stdout.write_all(b"\n").unwrap();
                for (index, option) in options.iter().enumerate() {
                    stdout.write_all(format!("{index}: {option}\n").as_bytes()).unwrap();
                }
                stdout.write_all(b"\n=> ").unwrap();
                stdout.flush().unwrap();
            }

            {
                let mut stdin = std::io::stdin().lock();
                let r = stdin.read_line(&mut answer);
                r.expect("could not process user input");
            }

            if let Ok(index) = answer.trim().parse::<usize>() {
                if index < options.len() { return Some(index) }
            }

            if marked_optional && answer.trim().is_empty() {
                return None
            }
            
            println!(r#"Invalid input! Enter a number from 0 to {}."#, options.len() - 1);
            println!();
            answer.clear();
        }
    }

    pub fn prompt_choice(options: &[&str], prompt: &str) -> usize {
        prompt_choice_maybe_optional(options, prompt, false).expect("prompt returned `None` despite being marked as non-optional")
    }

    pub fn prompt_choice_optional(options: &[&str], prompt: &str) -> Option<usize> {
        prompt_choice_maybe_optional(options, prompt, true)
    }

    #[cfg(feature = "discord")]
    pub mod discord {
        use super::*;
        use crate::subscribers::discord::{self, DisplayedField};

        pub fn prompt(config: &mut Option<discord::Config>, force_enable: bool) {
            if force_enable || prompt_bool("Enable Discord Rich Presence?") {
                if let Some(config) = config.as_mut() {
                    config.enabled = true;
                } else {
                    *config = Some(discord::Config::default());
                }
                let config = config.as_mut().unwrap();


                if let Some(ty) = prompt_display_type() { config.displayed_field = ty; }
                if let Some(id) = prompt_application_id() { config.application_id = id; }
            } else if let Some(config) = config.as_mut() {
                config.enabled = false;
            }
        }

        pub fn prompt_display_type() -> Option<DisplayedField> {
            let options = &[
                "Listening to <activity-name> // Typically the application name, e.g. \"Apple Music\"",
                "Listening to <artist>",
                "Listening to <album>",
            ];
            prompt_choice_optional(options, "How should your activity display? (\"Listening to _________\")").map(|choice| match choice {
                0 => DisplayedField::ApplicationName,
                1 => DisplayedField::State,
                2 => DisplayedField::Details,
                index => unreachable!("`prompt_choice` returned out-of-bounds index {index}")
            })
        }

        pub fn prompt_application_id() -> Option<u64> {
            use discord::EnumeratedApplicationIdentifier;
            let mut options = Vec::<&str>::with_capacity(EnumeratedApplicationIdentifier::VARIANT_COUNT + 1);
            options.push("Other (requires a custom application ID)");
            for id in EnumeratedApplicationIdentifier::VARIANTS.iter() {
                options.push(id.get_display_text());
            }

            loop {
                if let Some(choice) = prompt_choice_optional(&options, "What should the activity name be?") {
                    match choice {
                        0 => {
                            const MAX_U64_LENGTH_IN_BASE_TEN: usize = 20;
                            let id = super::prompt("Enter your custom application ID:", MAX_U64_LENGTH_IN_BASE_TEN + '\n'.len_utf8());
                            match id.trim().parse() {
                                Ok(id) => return Some(id),
                                Err(_) => {
                                    eprintln!("could not parse application id; please try again");
                                    continue;
                                }
                            }
                        },
                        index => return Some(EnumeratedApplicationIdentifier::VARIANTS[choice - index].get_id())
                    }
                } else {
                    return None
                }
            }
        }
    }

    #[cfg(feature = "lastfm")]
    pub mod lastfm {
        use super::*;
        use crate::subscribers::lastfm::{self, *};

        pub async fn prompt(config: &mut Option<lastfm::Config>)  {
            if prompt_bool("Enable last.fm Scrobbling?") {
                if let Some(config) = config.as_mut() {
                    config.enabled = true;
                } else {
                    *config = authorize().await
                }
            } else if let Some(config) = config.as_mut() {
                config.enabled = false;
            }
        }
        

        pub async fn authorize() -> Option<lastfm::Config> {
            let client = &crate::subscribers::lastfm::DEFAULT_CLIENT_IDENTITY;
            let auth = match client.generate_authorization_token().await {
                Ok(auth) => auth,
                Err(error) => {
                    eprintln!("Error: {error}");
                    eprintln!("Continuing with last.fm support disabled. This can be reconfigured later.");
                    return None;
                }
            };
            let auth_url = auth.generate_authorization_url(client);
            println!("Continue after authorizing the application: {auth_url}");
            if prompt_bool("Have you authorized the application?") {
                match auth.generate_session_key(client).await {
                    Ok(key) => Some(crate::subscribers::lastfm::Config {
                        enabled: true,
                        identity: (*client).clone(),
                        session_key: Some(key)
                    }),
                    Err(error) => {
                        ferror!("couldn't create session key: {error}");
                    }
                }
            } else { None }
        }
    }

    #[cfg(feature = "listenbrainz")]
    pub mod listenbrainz {
        use super::*;
        use crate::subscribers::listenbrainz;

        pub async fn prompt(config: &mut Option<listenbrainz::Config>) {
            if prompt_bool("Enable ListenBrainz synchronization?") {
                if let Some(config) = config.as_mut() {
                    config.enabled = true;
                } else {
                    *config = authorize().await
                }
            } else if let Some(config) = config.as_mut() {
                config.enabled = false;
            }
        }

        pub async fn authorize() -> Option<listenbrainz::Config> {
            loop {
                const HYPHENATED_UUID_LENGTH: usize = 36;
                let token = super::prompt(r#"Paste your access token (from https://listenbrainz.org/settings/) or type "cancel":"#, HYPHENATED_UUID_LENGTH + '\n'.len_utf8());
                let token = &token[..token.len().saturating_sub('\n'.len_utf8())];
                if token == "cancel" { break None; }
                match brainz::listen::v1::UserToken::new(token).await {
                    Ok(token) => {
                        break Some(crate::subscribers::listenbrainz::Config {
                            enabled: true,
                            program_info: crate::subscribers::listenbrainz::DEFAULT_PROGRAM_INFO.clone(),
                            user_token: Some(token),
                        })
                    },
                    Err(error) => {
                        use brainz::listen::v1::token_validity::ValidTokenInstantiationError;
                        match error {
                            ValidTokenInstantiationError::Invalid(..) => eprintln!("Invalid token!"),
                            ValidTokenInstantiationError::ValidityCheckFailure(failure) => eprintln!("Network failure: {}", failure.without_url())
                        }
                    }
                }
            }
        }
    }
}
