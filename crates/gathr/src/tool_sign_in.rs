//! A tool that signs in through the browser (OAuth, as MCP's authorization spec has it). rmcp's own
//! client does the work: it finds where the tool signs in, registers Gathr there, makes the PKCE
//! exchange and renews the sign-in. Gathr keeps the sign-in (the tokens and the client it registered
//! as) sealed in the tool's row, and one copy of it per tool in memory, so two connections to a tool
//! never renew it at once.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use rmcp::transport::auth::{
    AuthError, AuthorizationManager, AuthorizationRequest, AuthorizationSession,
    CredentialRefreshGuard, CredentialStore, InMemoryCredentialStore, StoredCredentials,
};

use crate::writer::Writer;
use crate::{Error, seal, web_tools};

/// Why a sign-in didn't work.
#[derive(Debug)]
pub enum Refused {
    /// The sign-in service takes no app it doesn't know already.
    NoRegistering,
    /// The person, or the service, said no.
    Denied,
    /// Anything else, with the library's words for the record.
    Failed(String),
}

impl Refused {
    /// In one plain sentence, for the tool called `name`.
    pub fn said(&self, name: &str) -> String {
        match self {
            Refused::NoRegistering => format!("{name} doesn’t let apps register themselves."),
            Refused::Denied => format!("{name} refused access."),
            Refused::Failed(_) => format!("{name} didn’t let Gathr sign in."),
        }
    }

    /// What the record gets: the sentence's reason, and the library's words when there are some.
    pub fn detail(&self) -> &str {
        match self {
            Refused::NoRegistering => "no registration endpoint",
            Refused::Denied => "access denied",
            Refused::Failed(why) => why,
        }
    }
}

fn failed(e: impl std::fmt::Display) -> Refused {
    Refused::Failed(e.to_string())
}

/// The name a tool's sign-in is sealed under: never its id, which its key uses.
pub fn sealed_as(id: &str) -> String {
    format!("{id} sign-in")
}

/// A manager for the tool at `link` that knows where it signs in, and whether Gathr can register
/// there. Every address it names must be https, or http on this computer, as a tool's own link.
async fn discovered(link: &str) -> Result<(AuthorizationManager, bool), Refused> {
    let mut manager = AuthorizationManager::new(link.trim())
        .await
        .map_err(failed)?;
    let metadata = manager.resolve_metadata().await.map_err(failed)?.metadata;
    let addresses = [
        Some(&metadata.authorization_endpoint),
        Some(&metadata.token_endpoint),
        metadata.registration_endpoint.as_ref(),
    ];
    for address in addresses.into_iter().flatten() {
        web_tools::checked(address).map_err(|_| failed(format!("not https: {address}")))?;
    }
    let registers = metadata.registration_endpoint.is_some();
    manager.set_metadata(metadata);
    Ok((manager, registers))
}

/// A sign-in waiting for the browser to come back.
pub struct Started {
    /// The page the person signs in on.
    pub url: String,
    /// The `state` the page sends back: the one way the come-back is admitted.
    pub state: String,
    session: AuthorizationSession,
    kept: InMemoryCredentialStore,
}

/// Registers Gathr where the tool at `link` signs in, and makes the page to sign in on, which comes
/// back to `back`. Registered again at each sign-in, so a changed port or a forgotten client never
/// stops one.
pub async fn start(link: &str, back: &str) -> Result<Started, Refused> {
    let (mut manager, registers) = discovered(link).await?;
    if !registers {
        return Err(Refused::NoRegistering);
    }
    let kept = InMemoryCredentialStore::new();
    manager.set_credential_store(kept.clone());
    let request = AuthorizationRequest::new(back).with_client_name("Gathr");
    let session = AuthorizationSession::new(manager, request)
        .await
        .map_err(|(_, e)| failed(e))?;
    let url = session.get_authorization_url().to_string();
    let query = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let state = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("state="))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| failed("no state in the sign-in page's address"))?
        .to_string();
    Ok(Started {
        url,
        state,
        session,
        kept,
    })
}

/// Swaps the code the browser brought back for the sign-in: the manager that holds it, for a first
/// connection, and its text, to be sealed.
pub async fn finish(
    started: Started,
    code: &str,
    issuer: Option<&str>,
) -> Result<(AuthorizationManager, String), Refused> {
    let session = started.session;
    session
        .handle_callback_with_issuer(code, &started.state, issuer)
        .await
        .map_err(failed)?;
    let credentials = started.kept.load().await.map_err(failed)?;
    let credentials = credentials.ok_or_else(|| failed("nothing was kept"))?;
    let text = serde_json::to_string(&credentials).map_err(failed)?;
    Ok((session.auth_manager, text))
}

/// The text sealed under the tool `id`'s sign-in name. Sealing may start a program: never on the
/// writer's thread.
pub async fn sealed(id: &str, text: &str) -> Result<Vec<u8>, Error> {
    let (name, text) = (sealed_as(id), text.to_string());
    tokio::task::spawn_blocking(move || seal::seal(&name, &text))
        .await
        .map_err(|e| Error::Other(e.to_string()))?
}

/// One tool's sign-in, shared by every connection to it.
pub struct SignIn {
    id: String,
    writer: Writer,
    now: tokio::sync::Mutex<Option<StoredCredentials>>,
    renewing: Arc<tokio::sync::Mutex<()>>,
}

/// Each tool's sign-in read since it last changed, by id.
static SIGN_INS: Mutex<Option<HashMap<String, Arc<SignIn>>>> = Mutex::new(None);

/// The tool `id`'s sign-in: the one in memory, else `sealed` unsealed.
pub fn kept(id: &str, writer: &Writer, sealed: &[u8]) -> Result<Arc<SignIn>, Error> {
    let mut all = SIGN_INS.lock().unwrap_or_else(|e| e.into_inner());
    let all = all.get_or_insert_with(HashMap::new);
    if let Some(sign_in) = all.get(id) {
        return Ok(sign_in.clone());
    }
    let text = seal::unseal(&sealed_as(id), sealed)?;
    let credentials = serde_json::from_str(&text).map_err(|e| Error::Other(e.to_string()))?;
    let sign_in = Arc::new(SignIn {
        id: id.to_string(),
        writer: writer.clone(),
        now: tokio::sync::Mutex::new(Some(credentials)),
        renewing: Arc::default(),
    });
    all.insert(id.to_string(), sign_in.clone());
    Ok(sign_in)
}

/// Drops the tool's sign-in from memory: it's read again at its next use.
pub fn forget(id: &str) {
    let mut all = SIGN_INS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(all) = all.as_mut() {
        all.remove(id);
    }
}

/// A manager for the tool at `link` that holds its sign-in and renews it when it runs out: `None`
/// when there's no sign-in it can use.
pub async fn manager(
    link: &str,
    sign_in: &Arc<SignIn>,
) -> Result<Option<AuthorizationManager>, Refused> {
    let (mut manager, _) = discovered(link).await?;
    manager.set_credential_store(Store(sign_in.clone()));
    match manager.initialize_from_store().await.map_err(failed)? {
        true => Ok(Some(manager)),
        false => Ok(None),
    }
}

type Answer<'a, T> = Pin<Box<dyn Future<Output = Result<T, AuthError>> + Send + 'a>>;

/// rmcp's store for a sign-in, written out by hand as its `async_trait` would make it: Gathr has no
/// such macro of its own.
struct Store(Arc<SignIn>);

impl CredentialStore for Store {
    fn load<'a, 'b>(&'a self) -> Answer<'b, Option<StoredCredentials>>
    where
        'a: 'b,
        Self: 'b,
    {
        Box::pin(async move { Ok(self.0.now.lock().await.clone()) })
    }

    /// Kept sealed in the tool's row before memory has it, so a renewal is never only in memory.
    fn save<'a, 'b>(&'a self, credentials: StoredCredentials) -> Answer<'b, ()>
    where
        'a: 'b,
        Self: 'b,
    {
        Box::pin(async move {
            let kept = |e: Error| AuthError::CredentialStoreError(e.to_string());
            let text = serde_json::to_string(&credentials)
                .map_err(|e| AuthError::CredentialStoreError(e.to_string()))?;
            let sealed = sealed(&self.0.id, &text).await.map_err(kept)?;
            let id = self.0.id.clone();
            let saved = self
                .0
                .writer
                .call(move |conn| web_tools::set_sign_in(conn, &id, &sealed))
                .await;
            match saved {
                Some(Ok(())) => {}
                Some(Err(e)) => return Err(kept(e)),
                None => return Err(kept(Error::Other("the database didn't answer".into()))),
            }
            *self.0.now.lock().await = Some(credentials);
            Ok(())
        })
    }

    /// Only from memory: the row keeps it, so Sign in again is still offered.
    fn clear<'a, 'b>(&'a self) -> Answer<'b, ()>
    where
        'a: 'b,
        Self: 'b,
    {
        Box::pin(async move {
            *self.0.now.lock().await = None;
            Ok(())
        })
    }

    fn acquire_refresh_guard<'a, 'b>(&'a self) -> Answer<'b, Option<CredentialRefreshGuard>>
    where
        'a: 'b,
        Self: 'b,
    {
        Box::pin(async move {
            let guard = self.0.renewing.clone().lock_owned().await;
            Ok(Some(CredentialRefreshGuard::new(guard)))
        })
    }
}
