use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::{AuthUrl, ClientId, ClientSecret, TokenResponse, TokenUrl};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tonic::metadata::MetadataValue;
use tonic::service::Interceptor;
use tonic::Status;

use crate::Error;

#[derive(Clone)]
pub struct AuthConfig {
    client_id: String,
    client_secret: String,
    token_audience: Option<String>,
    authorization_server_url: String,
}

impl AuthConfig {
    pub fn from_env() -> crate::Result<Self> {
        let client_id = std::env::var("ZEEBE_CLIENT_ID")
            .map_err(|_error| Error::InvalidParameters("ZEEBE_CLIENT_ID not set"))?;
        let client_secret = std::env::var("ZEEBE_CLIENT_SECRET")
            .map_err(|_error| Error::InvalidParameters("ZEEBE_CLIENT_SECRET not set"))?;
        let token_audience = std::env::var("ZEEBE_TOKEN_AUDIENCE").ok();
        let authorization_server_url = std::env::var("ZEEBE_AUTHORIZATION_SERVER_URL")
            .unwrap_or("https://login.cloud.camunda.io/oauth/token/".to_owned());
        Ok(AuthConfig {
            client_id,
            client_secret,
            token_audience,
            authorization_server_url,
        })
    }
}

pub struct TokenResponseWithExpiration {
    pub response: BasicTokenResponse,
    pub expires_at: std::time::SystemTime,
}

pub struct TokenProvider {
    pub oauth2_client: BasicClient,
    auth_config: AuthConfig,
    cached_token: std::sync::RwLock<Option<TokenResponseWithExpiration>>,
}

#[derive(Clone)]
pub struct TokenRefresher {
    pub token_provider: Option<Arc<TokenProvider>>,
    timer: Arc<JoinHandle<()>>,
}

impl TokenRefresher {
    fn get_access_token(&self) -> Result<String, Error> {
        if let Some(cached_token) = self
            .token_provider
            .as_ref()
            .and_then(|provider| provider.access_token().ok())
        {
            return Ok(cached_token);
        }
        return Err(Error::GRPC(Status::new(
            tonic::Code::Unauthenticated,
            "No valid token available",
        )));
    }
}

#[derive(Clone)]
pub struct AuthInterceptor {
    pub token_refresher: Arc<TokenRefresher>,
}

impl TokenProvider {
    fn access_token(&self) -> Result<String, Error> {
        let lock = self
            .cached_token
            .read()
            .map_err(|error| Error::Auth("Could not get auth lock".to_owned()))?;

        lock.as_ref()
            .map(|token| token.response.access_token().clone())
            .map(|access_token| access_token.secret().clone())
            .ok_or(Error::Auth("Not authed yet".to_owned()))
    }

    fn cached_token_is_expired(&self) -> Result<bool, Error> {
        let lock = self
            .cached_token
            .read()
            .map_err(|error| Error::Auth("Could not get auth lock".to_owned()))?;

        let is_expired = if let Some(token) = lock.as_ref() {
            if token.expires_at > std::time::SystemTime::now() {
                false
            } else {
                true
            }
        } else {
            true
        };

        Ok(is_expired)
    }

    async fn refresh_token(&self) -> Result<(), Error> {
        tracing::trace!("Exchanging OAuth Token");

        if !self.cached_token_is_expired()? {
            return Ok(());
        }

        let token_request = self.oauth2_client.exchange_client_credentials();

        let token_request = if let Some(audience) = &self.auth_config.token_audience {
            token_request.add_extra_param("audience", audience)
        } else {
            token_request
        };

        tracing::trace!("Sending token request: {:?}", token_request);

        let response = token_request
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "Error getting oauth token");
                Status::permission_denied(format!("{}", error))
            })?;

        tracing::trace!("Got Oauth token");

        let expires_at = std::time::SystemTime::now()
            + response
                .expires_in()
                .unwrap_or(std::time::Duration::from_secs(0));
        let responser_with_expiration = TokenResponseWithExpiration {
            response,
            expires_at,
        };

        let mut lock = self
            .cached_token
            .write()
            .map_err(|error| Error::Auth(format!("Could not get auth lock: {}", error)))?;

        lock.replace(responser_with_expiration);

        Ok(())
    }
}

impl Default for AuthInterceptor {
    fn default() -> Self {
        let auth_config = AuthConfig::from_env().ok();
        let token_provider = if let Some(auth_config) = auth_config {
            let oauth2_client = BasicClient::new(
                ClientId::new(auth_config.client_id.clone()),
                Some(ClientSecret::new(auth_config.client_secret.clone())),
                AuthUrl::new(auth_config.authorization_server_url.clone()).unwrap(),
                Some(TokenUrl::new(auth_config.authorization_server_url.clone()).unwrap()),
            )
            .set_auth_type(oauth2::AuthType::RequestBody);

            let token_provider = TokenProvider {
                oauth2_client,
                auth_config,
                cached_token: std::sync::RwLock::new(None),
            };

            Some(Arc::new(token_provider))
        } else {
            None
        };

        let timer_client = token_provider.clone();
        let timer = tokio::task::spawn(async {
            if let Some(token_provider) = timer_client {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
                loop {
                    match token_provider.clone().refresh_token().await {
                        Err(error) => {
                            tracing::warn!(error = %error, "Could not refresh access token");
                        }
                        Ok(token) => {
                            tracing::debug!("Refreshed access token");
                        }
                    }
                    interval.tick().await;
                }
            }
        });

        let token_refresher = TokenRefresher {
            token_provider,
            timer: Arc::new(timer),
        };

        AuthInterceptor {
            token_refresher: Arc::new(token_refresher),
        }
    }
}

impl Interceptor for AuthInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> std::result::Result<tonic::Request<()>, Status> {
        let access_token = self
            .token_refresher
            .get_access_token()
            .map_err(|error| Status::permission_denied(format!("{}", error)))?;

        let value = MetadataValue::from_str(format!("Bearer {}", access_token).as_str())
            .map_err(|error| Status::permission_denied(format!("{}", error)))?;

        tracing::debug!("Setting authorization header: {:?}", access_token);

        request.metadata_mut().insert("authorization", value);

        Ok(request)
    }
}
