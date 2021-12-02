use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::{AuthUrl, ClientId, ClientSecret, TokenResponse, TokenUrl};
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

#[derive(Clone)]
pub struct AuthInterceptor {
    auth_config: Option<AuthConfig>,
    pub oauth2_client: Option<BasicClient>,
    pub cached_token: Option<BasicTokenResponse>,
}

impl AuthInterceptor {
    fn get_token(&mut self) -> Result<BasicTokenResponse, Error> {
        if let Some(oauth2_client) = &self.oauth2_client {
            tracing::trace!("Getting token: {:?}", oauth2_client);
            if let Some(token) = &self.cached_token {
                if token.expires_in().unwrap_or_default() > std::time::Duration::default() {
                    tracing::trace!("Using cached token");
                    return Ok(token.clone());
                }
            }

            tracing::trace!("Exchanging OAuth Token");

            let token_request = oauth2_client.exchange_client_credentials();

            let token_request = if let Some(audience) = self
                .auth_config
                .as_ref()
                .and_then(|config| config.token_audience.as_ref())
            {
                token_request.add_extra_param("audience", audience)
            } else {
                token_request
            };

            tracing::trace!("Sending token request: {:?}", token_request);

            let token_response = futures::executor::block_on(
                token_request.request_async(oauth2::reqwest::async_http_client),
            )
            .map_err(|error| {
                tracing::error!(error = %error, "Error getting oauth token");
                Status::permission_denied(format!("{}", error))
            })?;

            tracing::trace!("Got Oauth token");

            self.cached_token = Some(token_response.clone());

            Ok(token_response)
        } else {
            Err(Error::InvalidParameters("oauth2_client not set"))
        }
    }
}

impl Default for AuthInterceptor {
    fn default() -> Self {
        let auth_config = AuthConfig::from_env().ok();
        let oauth2_client = if let Some(auth_config) = auth_config.clone() {
            let client = BasicClient::new(
                ClientId::new(auth_config.client_id),
                Some(ClientSecret::new(auth_config.client_secret)),
                AuthUrl::new(auth_config.authorization_server_url.clone()).unwrap(),
                Some(TokenUrl::new(auth_config.authorization_server_url).unwrap()),
            )
            .set_auth_type(oauth2::AuthType::RequestBody);

            Some(client)
        } else {
            None
        };
        AuthInterceptor {
            auth_config,
            oauth2_client,
            cached_token: None,
        }
    }
}

impl Interceptor for AuthInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> std::result::Result<tonic::Request<()>, Status> {
        if self.oauth2_client.is_some() {
            let token_response = self
                .get_token()
                .map_err(|error| Status::permission_denied(format!("{}", error)))?;

            let value = MetadataValue::from_str(
                format!("Bearer {}", token_response.access_token().secret()).as_str(),
            )
            .map_err(|error| Status::permission_denied(format!("{}", error)))?;

            tracing::debug!(
                "Setting authorization header: {:?}",
                token_response.access_token()
            );

            request.metadata_mut().insert("authorization", value);
        }
        Ok(request)
    }
}
