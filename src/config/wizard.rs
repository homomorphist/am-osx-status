
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
    use super::*;
    
    pub fn prompt(prompt: &str, initial_capacity: usize) -> String {
        {
            use std::io::Write;
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(prompt.as_bytes()).unwrap();
            stdout.write_all(b"\n=> ").unwrap();
            stdout.flush().unwrap();
        }

        let mut str_buf = String::with_capacity(initial_capacity);
        {
            use std::io::BufRead;
            let mut stdin = std::io::stdin().lock();
            stdin.read_line(&mut str_buf).expect("could not process user input");
        }

        str_buf
    }

    pub fn prompt_bool(prompt: &str) -> bool {
        let mut answer = String::with_capacity(4);
        loop {
            {
                use std::io::Write;
                let mut stdout = std::io::stdout().lock();
                stdout.write_all(prompt.as_bytes()).unwrap();
                stdout.write_all(b" (y/n)\n=> ").unwrap();
                stdout.flush().unwrap();
            }
    
            {
                use std::io::BufRead;
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

    pub async fn prompt_lastfm() -> Option<crate::status_backend::lastfm::Config> {
        if prompt_bool("Enable last.fm Scrobbling?") {
            interrogate_lastfm().await
        } else {
            None
        }
    }

    pub async fn interrogate_lastfm() -> Option<crate::status_backend::lastfm::Config> {
        let client = &crate::status_backend::lastfm::DEFAULT_CLIENT_IDENTITY;
        let auth = match client.generate_authorization_token().await {
            Ok(auth) => auth,
            Err(err) => {
                use lastfm::auth::AuthorizationTokenGenerationError;
                match err {
                    AuthorizationTokenGenerationError::NetworkError(failure) => {
                        eprintln!("Network failure: {}", failure.without_url());
                        eprintln!("Continuing with last.fm support disabled. This can be reconfigured later.");
                        return None;
                    }
                }
            }
        };
        let auth_url = auth.generate_authorization_url(client);
        println!("Continue after authorizing the application: {}", auth_url);
        if prompt_bool("Have you authorized the application?") {
            let session_key = auth.generate_session_key(client).await.expect("could not create session key");
            Some(crate::status_backend::lastfm::Config {
                enabled: true,
                identity: (*client).clone(),
                session_key: Some(session_key)
            })
        } else { None }
    }

    pub async fn prompt_listenbrainz() -> Option<crate::status_backend::listenbrainz::Config> {
        if prompt_bool("Enable ListenBrainz synchronization?") {
            interrogate_listenbrainz().await
        } else {
            None
        }
    }

    pub async fn interrogate_listenbrainz() -> Option<crate::status_backend::listenbrainz::Config> {
        loop {
            const HYPHENATED_UUID_LENGTH: usize = 36;
            let token = prompt(r#"Paste your access token (from https://listenbrainz.org/settings/) or type "cancel":"#, HYPHENATED_UUID_LENGTH + '\n'.len_utf8());
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

