use std::fmt::Debug;

use serde::de::DeserializeOwned;
pub use workspaces::result::CallExecutionDetails;

use anyhow::{Error, Result};

/// Some logic behind it:
/// To return Results that later get unwrapped in the test.
/// This way we know what invocation caused unwrap to panic on Err
pub trait StatusCheck {
    fn successful<T: DeserializeOwned>(self) -> Result<T>;
    fn assert_eq<T: DeserializeOwned + PartialEq + Debug>(self, expected_value: T) -> Result<T>;

    fn failure(&self) -> Result<()>;
    fn assert_err(&self, expected_error: &str) -> Result<()>;
}

impl StatusCheck for Result<CallExecutionDetails> {
    fn successful<T>(self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        match self {
            Ok(details) => {
                if details.is_success() {
                    Ok(details.json()?)
                } else {
                    let errors = details.failures();
                    match errors.len() {
                        1 => Err(Error::msg(format!(
                            "got error (expected success):\n{:?}\n",
                            errors
                        ))),
                        2.. => Err(Error::msg(format!(
                            "got errors (expected success):\n{:?}\n\n",
                            errors
                        ))),
                        _ => Err(Error::msg(
                            "the call is not successful, but there are no failures🤨",
                        )),
                    }
                }
            }
            Err(err) => panic!("got an error: {:?}", err),
        }
    }

    fn assert_eq<T>(self, expected_value: T) -> Result<T>
    where
        T: DeserializeOwned + PartialEq + Debug,
    {
        let outcome = self?;
        let value = outcome.json()?;
        if value == expected_value {
            Ok(value)
        } else {
            Err(Error::msg(format!(
                "assert_eq failed: expected '{:?}' got '{:?}'",
                expected_value, value
            )))
        }
    }

    fn failure(&self) -> Result<()> {
        if let Err(err) = self {
            println!("got error as expected:\n{:?}\n", err);
            Ok(())
        } else {
            Err(Error::msg("got success (expected error)"))
        }
    }

    fn assert_err(&self, expected_error: &str) -> Result<()> {
        match self {
            Ok(details) => {
                if details.is_success() && details.receipt_failures().is_empty() {
                    Err(Error::msg(format!(
                        "got success, expected error '{}'",
                        expected_error
                    )))
                } else {
                    let errors = details.receipt_failures();

                    if !errors
                        .iter()
                        .any(|error| format!("{:?}", error).contains(expected_error))
                    {
                        let err_list = if !errors.is_empty() {
                            format!(", got this error: '{:?}'", errors[0])
                        } else {
                            String::new()
                        };
                        Err(Error::msg(format!(
                            "didn't got expected error ('{}'){}",
                            expected_error, err_list
                        )))
                    } else {
                        Ok(())
                    }
                }
            }
            Err(err) => {
                if err.to_string().contains(expected_error) {
                    println!("got error as expected:\n'{}'\n", err);
                    Ok(())
                } else {
                    Err(Error::msg(format!(
                        "got an error, but it does not contain the expected_error substring:
    expected error:
'{}'
    received:
'{}'\n",
                        expected_error, err
                    )))
                }
            }
        }
    }
}

// panics if status is not successful
// pub fn assert_successful<T: DeserializeOwned>(status: FinalExecutionStatus) -> Result<T>{
//     match status{
//         NotStarted => panic!("got 'NotStarted'"),
//         Started => panic!("got 'Started'"),
//         Failure(err) => panic!("got error (expected success):\n{:?}\n", err),
//         SuccessValue(val) => {
//             let decoded = near_primitives::serialize::from_base64(&val).unwrap();
//             Ok(from_slice(&decoded)?)
//             }
//         }
// }
