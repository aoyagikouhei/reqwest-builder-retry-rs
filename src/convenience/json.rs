use reqwest::{header::HeaderMap, Response, StatusCode};

use crate::{convenience::check_status_code, RetryType};

// エラー時のレスポンスのデータ
#[derive(Debug)]
pub struct ResponseSuccess<T> {
    pub status_code: StatusCode,
    pub data: T,
    pub headers: HeaderMap,
}

// エラー時のレスポンスのデータ
#[derive(Debug)]
pub struct ResponseData {
    pub status_code: StatusCode,
    pub body: String,
    pub headers: HeaderMap,
}

// エラー情報、reqwestのエラーかそれ以外
#[derive(Debug)]
pub struct ResponseError {
    pub error: Option<crate::reqwest::Error>,
    pub response_data: Option<ResponseData>,
}

// レスポンスのチェック
pub async fn check_done<T>(
    response: Result<Response, crate::reqwest::Error>,
    retryable_status_codes: &[StatusCode],
) -> Result<ResponseSuccess<T>, (RetryType, ResponseError)>
where
    T: serde::de::DeserializeOwned,
{
    let response = response.map_err(|err| {
        (
            RetryType::Retry,
            ResponseError {
                error: Some(err),
                response_data: None,
            },
        )
    })?;

    let status_code = response.status();
    let headers = response.headers().clone();
    let body = response.text().await.unwrap_or_else(|_| "".to_string());
    let response_data = ResponseData {
        status_code,
        body,
        headers,
    };

    if let Some(retry_type) = check_status_code(status_code, retryable_status_codes).await {
        return Err((
            retry_type,
            ResponseError {
                error: None,
                response_data: Some(response_data),
            },
        ));
    }

    match serde_json::from_str::<T>(&response_data.body) {
        Ok(result) => Ok(ResponseSuccess { status_code, data: result, headers: response_data.headers }),
        Err(_) => Err((
            RetryType::Retry,
            ResponseError {
                error: None,
                response_data: Some(response_data),
            },
        )),
    }
}