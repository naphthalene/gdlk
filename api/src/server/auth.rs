use crate::{
    config::{OpenIdConfig, ProviderConfig},
    error::ResponseError,
    models::NewUserProvider,
    schema::user_providers,
    util::Pool,
};
use actix_identity::Identity;
use actix_web::{get, http, post, web, HttpResponse};
use diesel::{
    Connection, ExpressionMethods, OptionalExtension, PgConnection, QueryDsl,
    RunQueryDsl,
};
use openid::{Client, DiscoveredClient, Options, Token, Userinfo};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Map of provider name to configured [Client]
pub struct ClientMap {
    pub map: HashMap<String, Client>,
}

#[derive(Deserialize, Debug)]
pub struct RedirectQuery {
    next: Option<String>,
}

/// Contents of the `state` query param that gets passed through the OpenID
/// login. These will be (de)serialized through JSON.
/// https://auth0.com/docs/protocols/oauth2/oauth-state
#[derive(Serialize, Deserialize, Debug)]
pub struct AuthState<'a> {
    /// The next param determines what page to redirect the user to after login
    next: Option<&'a str>,
    // TODO add secure token here
}

impl ClientMap {
    pub fn get_client(
        &self,
        provider_name: &str,
    ) -> Result<&Client, HttpResponse> {
        self.map
            .get(provider_name)
            .ok_or_else(|| HttpResponse::NotFound().finish())
    }
}

/// Build a map of OpenID clients, one for each provider.
pub async fn build_client_map(open_id_config: &OpenIdConfig) -> ClientMap {
    async fn make_client(
        host_url: &str,
        name: &str,
        provider_config: &ProviderConfig,
    ) -> Client {
        let redirect = Some(format!("{}/api/oidc/{}/callback", host_url, name));
        let issuer = Url::parse(&provider_config.issuer_url).unwrap();
        DiscoveredClient::discover(
            provider_config.client_id.clone(),
            provider_config.client_secret.clone(),
            redirect,
            issuer,
        )
        .await
        .unwrap()
    }

    let host_url: &str = &open_id_config.host_url;

    // Build a client for each provider
    // TODO do these in parallel
    let mut map = HashMap::new();
    for (name, provider_config) in &open_id_config.providers {
        let client = make_client(host_url, name, provider_config).await;
        map.insert(name.into(), client);
    }

    ClientMap { map }
}

/// The frontend will redirect to this before being sent off to the
/// actual openid provider
#[get("/api/oidc/{provider_name}/redirect")]
pub async fn route_authorize(
    client_map: web::Data<ClientMap>,
    params: web::Path<(String,)>,
    query: web::Query<RedirectQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let provider_name: &str = &params.0;
    let oidc_client = client_map.get_client(provider_name)?;
    let state = AuthState {
        next: query.next.as_deref(),
    };

    let auth_url = oidc_client.auth_url(&Options {
        scope: Some("email".into()),
        // Serialization shouldn't ever fail so yeet that shit outta the Result
        state: Some(serde_json::to_string(&state).unwrap()),
        ..Default::default()
    });

    Ok(HttpResponse::Found()
        .header(http::header::LOCATION, auth_url.to_string())
        .finish())
}

#[derive(Deserialize, Debug)]
pub struct LoginQuery {
    code: String,
    state: Option<String>,
}

/// Exchanges the access token from the initial login in the openid provider
/// for a normal token. The code here should come from the browser, which
/// is passed along from the provider.
async fn request_token(
    oidc_client: &Client,
    code: &str,
) -> Result<(Token, Userinfo), ResponseError> {
    let mut token: Token = oidc_client.request_token(&code).await?.into();
    if let Some(mut id_token) = token.id_token.as_mut() {
        // Decode the JWT and validate it was signed by the provider
        oidc_client.decode_token(&mut id_token)?;
        oidc_client.validate_token(&id_token, None, None)?;

        // Call to the userinfo endpoint of the provider
        let userinfo = oidc_client.request_userinfo(&token).await?;
        Ok((token, userinfo))
    } else {
        Err(ResponseError::InvalidCredentials)
    }
}

/// Provider redirects back to this route after the login
#[get("/api/oidc/{provider_name}/callback")]
pub async fn route_login(
    client_map: web::Data<ClientMap>,
    params: web::Path<(String,)>,
    query: web::Query<LoginQuery>,
    pool: web::Data<Pool>,
    identity: Identity,
) -> Result<HttpResponse, actix_web::Error> {
    let provider_name: &str = &params.0;
    let oidc_client = client_map.get_client(provider_name)?;
    let conn = &pool.get().map_err(ResponseError::from)? as &PgConnection;

    // Parse the state param
    // TODO check for a security token here
    let state: Option<AuthState> = match &query.state {
        None => None,
        Some(state_str) => Some(serde_json::from_str(state_str)?),
    };
    // This is where we'll redirect the user back to after login
    let redirect_dest = match state {
        Some(AuthState {
            next: Some(next), ..
        }) => next,
        // Default to home page
        _ => "/",
    };

    // Send the user's code to the server to authenticate it
    let (_, userinfo) = request_token(oidc_client, &query.code).await?;

    // Not sure when this can be None, hopefully never??
    let sub: &str = userinfo.sub.as_ref().unwrap();

    // Insert the sub+provider, or just return the existing one if it's already
    // in the DB. We need to do this in a transaction to prevent race conditions
    // if the provider gets deleted in another thread.
    let user_provider_id: Uuid =
        conn.transaction::<Uuid, ResponseError, _>(|| {
            // Insert, if the row already exists, just return None
            let inserted = NewUserProvider {
                sub,
                provider_name,
                user_id: None,
            }
            .insert()
            .on_conflict_do_nothing()
            .returning(user_providers::columns::id)
            .get_result(conn)
            .optional()
            .map_err(ResponseError::from)?;

            match inserted {
                // Insert didn't return anything, which means the row is already
                // in the DB. Just select that row.
                None => user_providers::table
                    .select(user_providers::columns::id)
                    .filter(user_providers::columns::sub.eq(sub))
                    .filter(
                        user_providers::columns::provider_name
                            .eq(provider_name),
                    )
                    .get_result(conn)
                    .map_err(ResponseError::from),
                Some(inserted_id) => Ok(inserted_id),
            }
        })?;

    // Add a cookie which can be used to auth requests. We use the UserProvider
    // ID so that this works even if the User object hasn't been created yet.
    identity.remember(user_provider_id.to_string());

    // Redirect to the path specified in the OpenID state param
    Ok(HttpResponse::Found()
        .header(http::header::LOCATION, redirect_dest)
        .finish())
}

#[post("/api/logout")]
pub async fn logout_route(identity: Identity) -> HttpResponse {
    identity.forget();
    HttpResponse::Ok().finish()
}
