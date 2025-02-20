use crate::app_state::AppState;
use crate::entity::scopes::Scope;
use actix_web::web;
use rauthy_common::constants::CACHE_NAME_12HR;
use rauthy_common::error_response::ErrorResponse;
use redhac::{cache_get, cache_get_from, cache_get_value, cache_put};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The struct for the `.well-known` endpoint for automatic OIDC discovery
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct WellKnown {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub introspection_endpoint: String,
    pub userinfo_endpoint: String,
    pub end_session_endpoint: String,
    pub jwks_uri: String,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub registration_endpoint: Option<String>,
    // pub check_session_iframe: String,
    pub grant_types_supported: Vec<String>,
    pub response_types_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
    pub token_endpoint_auth_signing_alg_values_supported: Vec<String>,
    pub claims_supported: Vec<String>,
    pub scopes_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
    pub dpop_signing_alg_values_supported: Vec<String>,
}

const IDX: &str = ".well-known";

impl WellKnown {
    pub async fn json(data: &web::Data<AppState>) -> Result<String, ErrorResponse> {
        if let Some(wk) = cache_get!(
            String,
            CACHE_NAME_12HR.to_string(),
            IDX.to_string(),
            &data.caches.ha_cache_config,
            false
        )
        .await?
        {
            Ok(wk)
        } else {
            let scopes = Scope::find_all(data)
                .await?
                .into_iter()
                .map(|s| s.name)
                .collect::<Vec<String>>();
            let slf = Self::new(&data.issuer, scopes);
            let json = serde_json::to_string(&slf).unwrap();

            cache_put(
                CACHE_NAME_12HR.to_string(),
                IDX.to_string(),
                &data.caches.ha_cache_config,
                &json,
            )
            .await?;

            Ok(json)
        }
    }

    /// Rebuilds the WellKnown, serializes it as json and updates it inside the cache.
    /// Should be called after any update on the Scopes.
    pub async fn rebuild(data: &web::Data<AppState>) -> Result<(), ErrorResponse> {
        let scopes = Scope::find_all(data)
            .await?
            .into_iter()
            .map(|s| s.name)
            .collect::<Vec<String>>();
        let slf = Self::new(&data.issuer, scopes);
        let json = serde_json::to_string(&slf).unwrap();

        cache_put(
            CACHE_NAME_12HR.to_string(),
            IDX.to_string(),
            &data.caches.ha_cache_config,
            &json,
        )
        .await?;

        Ok(())
    }
}

impl WellKnown {
    pub fn new(issuer: &str, scopes_supported: Vec<String>) -> Self {
        let authorization_endpoint = format!("{}/oidc/authorize", issuer);
        let token_endpoint = format!("{}/oidc/token", issuer);
        let introspection_endpoint = format!("{}/oidc/tokenInfo", issuer);
        let userinfo_endpoint = format!("{}/oidc/userinfo", issuer);
        let end_session_endpoint = format!("{}/oidc/userinfo", issuer);
        let jwks_uri = format!("{}/oidc/certs", issuer);
        let grant_types_supported = vec![
            "authorization_code".to_string(),
            "client_credentials".to_string(),
            "password".to_string(),
            "refresh_token".to_string(),
        ];
        let response_types_supported = vec!["code".to_string()];
        let id_token_signing_alg_values_supported = vec![
            "RS256".to_string(),
            "RS384".to_string(),
            "RS512".to_string(),
            "EdDSA".to_string(),
        ];
        let token_endpoint_auth_signing_alg_values_supported = vec![
            "RS256".to_string(),
            "RS384".to_string(),
            "RS512".to_string(),
            "EdDSA".to_string(),
        ];
        let claims_supported = vec![
            "iss".to_string(),
            "azp".to_string(),
            "amr".to_string(),
            "sub".to_string(),
            "preferred_username".to_string(),
            "email".to_string(),
            "email_verified".to_string(),
            "given_name".to_string(),
            "family_name".to_string(),
            "roles".to_string(),
            "groups".to_string(),
            "custom".to_string(),
        ];
        // TODO to not confuse users when static clients will not be able to use the scope,
        // `webid` should be added manually in the UI to make it fully work for ephemeral as
        // well as for static clients.
        // if *ENABLE_WEB_ID {
        //     claims_supported.push("webid".to_string());
        // }
        let code_challenge_methods_supported = vec!["plain".to_string(), "S256".to_string()];
        let dpop_signing_alg_values_supported = vec![
            "RS256".to_string(),
            "RS384".to_string(),
            "RS512".to_string(),
            "EdDSA".to_string(),
        ];

        WellKnown {
            issuer: String::from(issuer),
            authorization_endpoint,
            token_endpoint,
            introspection_endpoint,
            userinfo_endpoint,
            end_session_endpoint,
            jwks_uri,
            grant_types_supported,
            response_types_supported,
            id_token_signing_alg_values_supported,
            token_endpoint_auth_signing_alg_values_supported,
            claims_supported,
            scopes_supported,
            code_challenge_methods_supported,
            dpop_signing_alg_values_supported,
        }
    }
}
