use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("NoTry")]
    NoTry,

    #[error("TryOver")]
    TryOver,

    #[error("Stop")]
    Stop,
}
