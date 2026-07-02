use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub title: String,
    pub detail: String,
    pub r#type: String,
    pub instance: Option<String>,
    pub extra: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(status: StatusCode, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            status,
            title: title.into(),
            detail: detail.into(),
            r#type: "about:blank".into(),
            instance: None,
            extra: None,
        }
    }

    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "Bad Request", detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "Not Found", detail)
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            detail,
        )
    }

    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "Unauthorized", detail)
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "Forbidden", detail)
    }

    pub fn too_many_requests(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "Too Many Requests",
            detail,
        )
    }

    pub fn with_instance(mut self, instance: &str) -> Self {
        self.instance = Some(instance.to_string());
        self
    }

    pub fn with_extra(mut self, extra: serde_json::Value) -> Self {
        self.extra = Some(extra);
        self
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({
            "type": self.r#type,
            "title": self.title,
            "status": self.status.as_u16(),
            "detail": self.detail,
            "instance": self.instance,
            "extra": self.extra,
        });

        (self.status, axum::Json(body)).into_response()
    }
}

impl From<&nova_executor::PipelineError> for ApiError {
    fn from(err: &nova_executor::PipelineError) -> Self {
        let status = match err.code {
            nova_executor::ErrorCode::ParseError => StatusCode::BAD_REQUEST,
            nova_executor::ErrorCode::ValidationError => StatusCode::UNPROCESSABLE_ENTITY,
            nova_executor::ErrorCode::AuthorizationError => StatusCode::FORBIDDEN,
            nova_executor::ErrorCode::AuthenticationError => StatusCode::UNAUTHORIZED,
            nova_executor::ErrorCode::NotFound => StatusCode::NOT_FOUND,
            nova_executor::ErrorCode::Conflict => StatusCode::CONFLICT,
            nova_executor::ErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            nova_executor::ErrorCode::CircuitBreakerOpen => StatusCode::SERVICE_UNAVAILABLE,
            nova_executor::ErrorCode::DeadlineExceeded => StatusCode::GATEWAY_TIMEOUT,
            nova_executor::ErrorCode::Cancelled => {
                StatusCode::from_u16(499).unwrap_or(StatusCode::BAD_REQUEST)
            }
            nova_executor::ErrorCode::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            nova_executor::ErrorCode::Unprocessable => StatusCode::UNPROCESSABLE_ENTITY,
            nova_executor::ErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            nova_executor::ErrorCode::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            nova_executor::ErrorCode::NotImplemented => StatusCode::NOT_IMPLEMENTED,
            nova_executor::ErrorCode::InsufficientStorage => StatusCode::INSUFFICIENT_STORAGE,
        };
        ApiError::new(status, &err.message, &err.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_new() {
        let err = ApiError::new(StatusCode::BAD_REQUEST, "Bad Request", "something went wrong");
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert_eq!(err.title, "Bad Request");
        assert_eq!(err.detail, "something went wrong");
        assert_eq!(err.r#type, "about:blank");
        assert!(err.instance.is_none());
        assert!(err.extra.is_none());
    }

    #[test]
    fn test_bad_request() {
        let err = ApiError::bad_request("invalid input");
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert_eq!(err.title, "Bad Request");
        assert_eq!(err.detail, "invalid input");
    }

    #[test]
    fn test_not_found() {
        let err = ApiError::not_found("resource not found");
        assert_eq!(err.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_internal() {
        let err = ApiError::internal("server error");
        assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_unauthorized() {
        let err = ApiError::unauthorized("login required");
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_forbidden() {
        let err = ApiError::forbidden("no access");
        assert_eq!(err.status, StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_too_many_requests() {
        let err = ApiError::too_many_requests("slow down");
        assert_eq!(err.status, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_with_instance() {
        let err = ApiError::bad_request("bad").with_instance("/api/v1/data");
        assert_eq!(err.instance, Some("/api/v1/data".to_string()));
    }

    #[test]
    fn test_with_extra() {
        let extra = serde_json::json!({"field": "value"});
        let err = ApiError::bad_request("bad").with_extra(extra.clone());
        assert_eq!(err.extra, Some(extra));
    }

    #[test]
    fn test_into_response() {
        let err = ApiError::bad_request("invalid input");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_into_response_with_instance() {
        let err = ApiError::not_found("missing").with_instance("/api/v1/data");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_from_pipeline_error_not_found() {
        let perr = nova_executor::PipelineError::new(
            nova_executor::ErrorCode::NotFound,
            "entity not found",
        );
        let aerr = ApiError::from(&perr);
        assert_eq!(aerr.status, StatusCode::NOT_FOUND);
        assert_eq!(aerr.title, "entity not found");
        assert_eq!(aerr.detail, "entity not found");
    }

    #[test]
    fn test_from_pipeline_error_rate_limited() {
        let perr = nova_executor::PipelineError::new(
            nova_executor::ErrorCode::RateLimited,
            "rate limited",
        );
        let aerr = ApiError::from(&perr);
        assert_eq!(aerr.status, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_from_pipeline_error_cancelled() {
        let perr = nova_executor::PipelineError::new(
            nova_executor::ErrorCode::Cancelled,
            "cancelled",
        );
        let aerr = ApiError::from(&perr);
        assert_eq!(aerr.status.as_u16(), 499);
    }

    #[test]
    fn test_from_pipeline_error_all_variants() {
        use nova_executor::ErrorCode;
        let cases = [
            (ErrorCode::ParseError, StatusCode::BAD_REQUEST),
            (ErrorCode::ValidationError, StatusCode::UNPROCESSABLE_ENTITY),
            (ErrorCode::AuthorizationError, StatusCode::FORBIDDEN),
            (ErrorCode::AuthenticationError, StatusCode::UNAUTHORIZED),
            (ErrorCode::NotFound, StatusCode::NOT_FOUND),
            (ErrorCode::Conflict, StatusCode::CONFLICT),
            (ErrorCode::RateLimited, StatusCode::TOO_MANY_REQUESTS),
            (ErrorCode::CircuitBreakerOpen, StatusCode::SERVICE_UNAVAILABLE),
            (ErrorCode::DeadlineExceeded, StatusCode::GATEWAY_TIMEOUT),
            (ErrorCode::Cancelled, StatusCode::from_u16(499).unwrap()),
            (ErrorCode::PayloadTooLarge, StatusCode::PAYLOAD_TOO_LARGE),
            (ErrorCode::Unprocessable, StatusCode::UNPROCESSABLE_ENTITY),
            (ErrorCode::InternalError, StatusCode::INTERNAL_SERVER_ERROR),
            (ErrorCode::ServiceUnavailable, StatusCode::SERVICE_UNAVAILABLE),
            (ErrorCode::NotImplemented, StatusCode::NOT_IMPLEMENTED),
            (ErrorCode::InsufficientStorage, StatusCode::INSUFFICIENT_STORAGE),
        ];
        for (code, expected) in cases {
            let perr = nova_executor::PipelineError::new(code, "test");
            let aerr = ApiError::from(&perr);
            assert_eq!(aerr.status, expected, "mismatch for {code:?}");
        }
    }

    #[test]
    fn test_from_pipeline_error_with_stage() {
        let perr = nova_executor::PipelineError::new(
            nova_executor::ErrorCode::InternalError,
            "internal error",
        );
        let aerr = ApiError::from(&perr);
        assert_eq!(aerr.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(aerr.detail, "internal error");
    }
}
