
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

    pub fn prompt_choice(options: &[&str], prompt: &str) -> usize {
        let mut answer = String::with_capacity(options.len().ilog(10) as usize + 3); // +1 for offset, +1 '\n'; +1 to not silently cut bad numbers (e.x. 20 => 2)
        loop {
            {
                let mut stdout = std::io::stdout().lock();
                stdout.write_all(prompt.as_bytes()).unwrap();
                stdout.write_all(b"\n=> ").unwrap();
                for (index, option) in options.iter().enumerate() {
                    stdout.write_all(format!("{index}: {option}\n").as_bytes()).unwrap();
                }
                stdout.flush().unwrap();
            }

            {
                let mut stdin = std::io::stdin().lock();
                let r = stdin.read_line(&mut answer);
                r.expect("could not process user input");
            }

            if let Ok(index) = answer.trim().parse::<usize>() {
                if index < options.len() { return index }
            }
            println!(r#"Invalid input! Enter a number from 0 to {}."#, options.len() - 1);
            println!();
            answer.clear();
        }
    }

    #[cfg(feature = "discord")]
    pub mod discord {
        use super::*;
        use crate::status_backend::discord;

        pub async fn prompt(config: &mut Option<discord::Config>, force_enable: bool) {
            if force_enable || prompt_bool("Enable Discord Rich Presence?") {
                if let Some(config) = config.as_mut() {
                    config.enabled = true;
                } else {
                    *config = Some(discord::Config::default());
                }
                let config = config.as_mut().unwrap();
    
                use discord::EnumeratedApplicationIdentifier;
                let mut options = Vec::<&str>::with_capacity(EnumeratedApplicationIdentifier::VARIANT_COUNT + 1);
                options.push("Other (requires a custom application ID)");
                for id in EnumeratedApplicationIdentifier::VARIANTS.iter() {
                    options.push(id.get_display_text());
                }
    
                let choice = prompt_choice(&options, "How should your activity display? (\"Listening to _________\")");
                config.application_id = match prompt_choice(&options, "How should your activity display? (\"Listening to _________\")") {
                    0 => {
                        const MAX_U64_LENGTH_IN_BASE_TEN: usize = 20;
                        let id = super::prompt("Enter your custom application ID:", MAX_U64_LENGTH_IN_BASE_TEN + '\n'.len_utf8());
                        id.trim().parse().expect("could not parse custom application ID")
                    },
                    index => EnumeratedApplicationIdentifier::VARIANTS[choice - index].get_id()
                }
            } else if let Some(config) = config.as_mut() {
                config.enabled = false;
            }
        }
    }

    #[cfg(feature = "lastfm")]
    pub mod lastfm {
        use super::*;
        use crate::status_backend::lastfm::{self, *};

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
            let client = &crate::status_backend::lastfm::DEFAULT_CLIENT_IDENTITY;
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
                    Ok(key) => Some(crate::status_backend::lastfm::Config {
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
        use crate::status_backend::listenbrainz;

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
                        break Some(crate::status_backend::listenbrainz::Config {
                            enabled: true,
                            program_info: crate::status_backend::listenbrainz::DEFAULT_PROGRAM_INFO.clone(),
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
