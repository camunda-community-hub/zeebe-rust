use futures::TryFutureExt;
use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::{AuthUrl, ClientId, ClientSecret, TokenResponse, TokenUrl};
use std::env;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::timeout;
use tonic::metadata::MetadataValue;
use tonic::service::Interceptor;
use tonic::Status;

use crate::{Error, Result};

const CLIENT_ID_VAR: &str = "ZEEBE_CLIENT_ID";
const CLIENT_SECRET_VAR: &str = "ZEEBE_CLIENT_SECRET";
const TOKEN_AUDIENCE_VAR: &str = "ZEEBE_TOKEN_AUDIENCE";
const AUTHORIZATION_SERVER_URL_VAR: &str = "ZEEBE_AUTHORIZATION_SERVER_URL";
const AUTH_REQUEST_TIMEOUT_VAR: &str = "ZEEBE_AUTH_REQUEST_TIMEOUT";

/// The expected default URL for this credentials provider, the Camunda Cloud endpoint.
const DEFAULT_AUTH_SERVER_URL: &str = "https://login.cloud.camunda.io/oauth/token/";

/// The default timeout for OAuth requests
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Offset from access token expiration time at which clients will start requesting new tokens.
const CLOCK_SKEW_BUFFER: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct OAuthConfig {
    client_id: String,
    client_secret: String,
    token_audience: Option<String>,
    authorization_server_url: String,
    timeout: Duration,
}

impl OAuthConfig {
    /// Check if auth env vars set
    pub fn should_use_env_config() -> bool {
        env::var(CLIENT_ID_VAR).is_ok() || env::var(CLIENT_SECRET_VAR).is_ok()
    }

    pub fn from_env() -> Result<Self> {
        let client_id = env::var(CLIENT_ID_VAR)
            .map_err(|_error| Error::InvalidParameters("ZEEBE_CLIENT_ID not set"))?;
        let client_secret = env::var(CLIENT_SECRET_VAR)
            .map_err(|_error| Error::InvalidParameters("ZEEBE_CLIENT_SECRET not set"))?;
        let token_audience = env::var(TOKEN_AUDIENCE_VAR).ok();
        let authorization_server_url = env::var(AUTHORIZATION_SERVER_URL_VAR)
            .unwrap_or_else(|_| DEFAULT_AUTH_SERVER_URL.to_string());
        let timeout = env::var(AUTH_REQUEST_TIMEOUT_VAR)
            .ok()
            .and_then(|timeout| Some(Duration::from_millis(timeout.parse().ok()?)))
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT);

        Ok(OAuthConfig {
            client_id,
            client_secret,
            token_audience,
            authorization_server_url,
            timeout,
        })
    }
}

impl fmt::Debug for OAuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"****")
            .field("token_audience", &self.token_audience)
            .field("authorization_server_url", &self.authorization_server_url)
            .field("timeout", &self.timeout)
            .finish()
    }
}

struct TokenResponseWithExpiration {
    pub response: BasicTokenResponse,
    pub expires_at: std::time::SystemTime,
}

struct TokenProvider {
    oauth2_client: BasicClient,
    audience: Option<String>,
    request_timeout: Duration,
    cached_token: Mutex<Option<TokenResponseWithExpiration>>,
    init_sender: broadcast::Sender<()>,
}

impl TokenProvider {
    async fn auth_initialized(&self) -> Result<()> {
        broadcast::Sender::subscribe(&self.init_sender)
            .recv()
            .map_err(|err| Error::Auth(err.to_string()))
            .await
    }

    fn access_token(&self) -> Result<String> {
        let lock = self
            .cached_token
            .lock()
            .map_err(|_| Error::Auth("Could not get auth lock".to_owned()))?;

        lock.as_ref()
            .map(|token| token.response.access_token().clone())
            .map(|access_token| access_token.secret().clone())
            .ok_or_else(|| Error::Auth("Not authed yet".to_owned()))
    }

    fn cached_token_is_expired(&self) -> Result<bool> {
        let lock = self
            .cached_token
            .lock()
            .map_err(|_| Error::Auth("Could not get auth lock".to_owned()))?;

        let is_expired = if let Some(token) = lock.as_ref() {
            token.expires_at <= std::time::SystemTime::now() + CLOCK_SKEW_BUFFER
        } else {
            true
        };

        Ok(is_expired)
    }

    async fn refresh_token(&self) {
        tracing::trace!("checking oauth token cache");

        match self.cached_token_is_expired() {
            Ok(false) => {
                tracing::trace!("access token still valid");
                return;
            }
            Err(err) => {
                tracing::error!(%err, "error checking access token status");
                return;
            }
            _ => {}
        }

        tracing::debug!("requesting new oauth token");

        let token_request = self.oauth2_client.exchange_client_credentials();
        let token_request = if let Some(audience) = &self.audience {
            token_request.add_extra_param("audience", audience)
        } else {
            token_request
        };

        tracing::trace!(req = ?token_request, "sending request");

        let response = match timeout(
            self.request_timeout,
            token_request
                .request_async(oauth2::reqwest::async_http_client)
                .map_err(|err| Error::Auth(err.to_string())),
        )
        .await
        .map_err(|_| {
            Error::Auth(format!(
                "timed out waiting for oauth token after {} milliseconds",
                self.request_timeout.as_millis()
            ))
        })
        .and_then(|res| res)
        {
            Ok(response) => response,
            Err(err) => {
                tracing::error!(%err, "error getting oauth token");
                return;
            }
        };

        tracing::trace!(?response, "got oauth token");

        let expires_at = std::time::SystemTime::now() + response.expires_in().unwrap_or_default();
        let response_with_expiration = TokenResponseWithExpiration {
            response,
            expires_at,
        };

        if let Err(err) = self
            .cached_token
            .lock()
            .map(|mut lock| lock.replace(response_with_expiration))
        {
            tracing::error!(%err, "Could not get auth lock")
        }

        // Notify all subscribers that tokens are available
        // Ignore errors if no subscribers exist.
        let _ = self.init_sender.send(());

        tracing::debug!("updated cached access token");
    }
}

#[derive(Clone, Default)]
pub(crate) struct AuthInterceptor {
    token_provider: Option<Arc<TokenProvider>>,
}

impl fmt::Debug for AuthInterceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthInterceptor")
            .field("configured", &self.token_provider.is_some())
            .finish()
    }
}

impl AuthInterceptor {
    pub(crate) fn init(config: OAuthConfig) -> Result<Self> {
        let oauth2_client = BasicClient::new(
            ClientId::new(config.client_id),
            Some(ClientSecret::new(config.client_secret)),
            AuthUrl::new(config.authorization_server_url.clone()).unwrap(),
            Some(TokenUrl::new(config.authorization_server_url).unwrap()),
        )
        .set_auth_type(oauth2::AuthType::RequestBody);

        let token_provider = Arc::new(TokenProvider {
            oauth2_client,
            audience: config.token_audience,
            request_timeout: config.timeout,
            cached_token: Mutex::new(None),
            init_sender: broadcast::channel(1).0,
        });

        let background_provider = Arc::clone(&token_provider);
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                background_provider.refresh_token().await;
                interval.tick().await;
            }
        });

        Ok(AuthInterceptor {
            token_provider: Some(token_provider),
        })
    }

    pub(crate) async fn auth_initialized(&self) -> Result<()> {
        if let Some(provider) = &self.token_provider {
            tracing::debug!("awaiting auth initialized event");
            provider.auth_initialized().await
        } else {
            tracing::warn!("oauth provider not configured, skipping initialization");
            Ok(())
        }
    }
}

impl Interceptor for AuthInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> std::result::Result<tonic::Request<()>, Status> {
        if let Some(token_provider) = &self.token_provider {
            let access_token = token_provider
                .access_token()
                .map_err(|_| Status::permission_denied("No valid token available"))?;

            let value = MetadataValue::from_str(format!("Bearer {}", access_token).as_str())
                .map_err(|error| Status::permission_denied(format!("{}", error)))?;

            request.metadata_mut().insert("authorization", value);
        }

        Ok(request)
    }
}
