use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("TryOver")]
    TryOver,

    #[error("Stop")]
    Stop,
}
