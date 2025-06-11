use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error<ErrorResponse> {
    #[error("NoTry")]
    NoTry,

    #[error("TryOver")]
    TryOver(ErrorResponse),

    #[error("Stop")]
    Stop(ErrorResponse),
}
