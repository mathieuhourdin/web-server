use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    entities_v2::error::{ErrorType, PpdcError},
    environment,
};

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const APPLE_JWKS_URL: &str = "https://appleid.apple.com/auth/keys";

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExternalAuthProvider {
    Google,
    Apple,
}

#[derive(Debug, Clone)]
pub struct VerifiedExternalIdentity {
    pub provider: ExternalAuthProvider,
    pub subject: String,
    pub email: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
}

#[derive(Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    n: String,
    e: String,
}

#[derive(Deserialize)]
struct IdentityTokenClaims {
    iss: String,
    aud: Value,
    sub: String,
    email: Option<String>,
    email_verified: Option<Value>,
    given_name: Option<String>,
    family_name: Option<String>,
}

fn invalid_identity_token(message: impl Into<String>) -> PpdcError {
    PpdcError::new(401, ErrorType::ApiError, message.into())
}

fn configured_audiences(provider: ExternalAuthProvider) -> Result<Vec<String>, PpdcError> {
    let audiences = match provider {
        ExternalAuthProvider::Google => environment::get_google_auth_client_ids(),
        ExternalAuthProvider::Apple => environment::get_apple_auth_client_ids(),
    };
    if audiences.is_empty() {
        return Err(PpdcError::new(
            503,
            ErrorType::InternalError,
            "External authentication provider is not configured".to_string(),
        ));
    }
    Ok(audiences)
}

fn claim_audiences(value: &Value) -> Vec<&str> {
    match value {
        Value::String(value) => vec![value.as_str()],
        Value::Array(values) => values.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    }
}

fn claim_is_verified(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Bool(true)))
        || matches!(value, Some(Value::String(value)) if value == "true")
}

async fn fetch_jwk_set(url: &str) -> Result<JwkSet, PpdcError> {
    reqwest::get(url)
        .await
        .map_err(|_| {
            PpdcError::new(
                503,
                ErrorType::InternalError,
                "Identity provider is unavailable".to_string(),
            )
        })?
        .error_for_status()
        .map_err(|_| {
            PpdcError::new(
                503,
                ErrorType::InternalError,
                "Identity provider is unavailable".to_string(),
            )
        })?
        .json::<JwkSet>()
        .await
        .map_err(|_| {
            PpdcError::new(
                503,
                ErrorType::InternalError,
                "Identity provider key response is invalid".to_string(),
            )
        })
}

pub async fn verify_id_token(
    provider: ExternalAuthProvider,
    id_token: &str,
) -> Result<VerifiedExternalIdentity, PpdcError> {
    let id_token = id_token.trim();
    if id_token.is_empty() {
        return Err(invalid_identity_token("Identity token is required"));
    }

    let header = decode_header(id_token)
        .map_err(|_| invalid_identity_token("Identity token header is invalid"))?;
    let kid = header
        .kid
        .ok_or_else(|| invalid_identity_token("Identity token has no key identifier"))?;
    let jwks_url = match provider {
        ExternalAuthProvider::Google => GOOGLE_JWKS_URL,
        ExternalAuthProvider::Apple => APPLE_JWKS_URL,
    };
    let jwk = fetch_jwk_set(jwks_url)
        .await?
        .keys
        .into_iter()
        .find(|key| key.kid == kid && key.kty == "RSA")
        .ok_or_else(|| invalid_identity_token("Identity token signing key is unknown"))?;
    let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|_| invalid_identity_token("Identity token signing key is invalid"))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_aud = false;
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    let claims = decode::<IdentityTokenClaims>(id_token, &key, &validation)
        .map_err(|_| invalid_identity_token("Identity token is invalid or expired"))?
        .claims;

    let valid_issuer = match provider {
        ExternalAuthProvider::Google => {
            claims.iss == "https://accounts.google.com" || claims.iss == "accounts.google.com"
        }
        ExternalAuthProvider::Apple => claims.iss == "https://appleid.apple.com",
    };
    if !valid_issuer {
        return Err(invalid_identity_token("Identity token issuer is invalid"));
    }

    let audiences = configured_audiences(provider)?;
    if !claim_audiences(&claims.aud)
        .iter()
        .any(|audience| audiences.iter().any(|allowed| allowed == audience))
    {
        return Err(invalid_identity_token("Identity token audience is invalid"));
    }

    let email = claims
        .email
        .map(|email| email.trim().to_lowercase())
        .filter(|email| !email.is_empty())
        .ok_or_else(|| invalid_identity_token("Identity token does not contain an email"))?;
    if !claim_is_verified(claims.email_verified.as_ref()) {
        return Err(invalid_identity_token(
            "Identity provider email is not verified",
        ));
    }
    if claims.sub.trim().is_empty() {
        return Err(invalid_identity_token("Identity token subject is invalid"));
    }

    Ok(VerifiedExternalIdentity {
        provider,
        subject: claims.sub,
        email,
        given_name: claims.given_name,
        family_name: claims.family_name,
    })
}
