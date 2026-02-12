//! # Error Handling Macros
//!
//! Convenience macros for error handling in CIS.
//!
//! ## Macros
//!
//! - [`bail!`] - Early return with an error
//! - [`ensure!`] - Conditional check with error
//! - [`context!`] - Add context to an error
//! - [`try_err!`] - Convert Option to Result with error
//!
//! ## Usage
//!
//! ```rust
//! use cis_core::error::{CisError, Result};
//! use cis_core::error::macros::{bail, ensure, context};
//!
//! fn example_function(value: i32) -> Result<i32> {
//!     // Early return with error
//!     if value < 0 {
//!         bail!(CisError::invalid_input("value", "must be non-negative"));
//!     }
//!
//!     // Conditional check
//!     ensure!(value > 10, CisError::invalid_input("value", "must be > 10"));
//!
//!     // Add context to error
//!     let result = dangerous_operation()
//!         .map_err(|e| context!(e, "operation", "dangerous_operation"))?;
//!
//!     Ok(result)
//! }
//! ```

/// Early return with an error.
///
/// This macro creates an error and returns it immediately.
///
/// # Examples
///
/// ```rust
/// use cis_core::error::{CisError, Result};
/// use cis_core::error::macros::bail;
///
/// fn validate(value: i32) -> Result<()> {
///     if value < 0 {
///         bail!(CisError::invalid_input("value", "must be non-negative"));
///     }
///     Ok(())
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! bail {
    ($err:expr) => {
        return Err($err);
    };
    ($category:expr, $code:expr, $msg:expr) => {{
        use $crate::error::unified::{CisError, ErrorCategory};
        return Err(CisError::new($category, $code, $msg));
    }};
    ($category:expr, $code:expr, $msg:expr, $($key:expr => $value:expr),+ $(,)?) => {{
        use $crate::error::unified::{CisError, ErrorCategory};
        let mut err = CisError::new($category, $code, $msg);
        $(
            err = err.with_context($key, $value);
        )+
        return Err(err);
    }};
}

/// Conditional check with error.
///
/// This macro checks a condition and returns an error if it's false.
///
/// # Examples
///
/// ```rust
/// use cis_core::error::{CisError, Result};
/// use cis_core::error::macros::ensure;
///
/// fn divide(a: i32, b: i32) -> Result<i32> {
///     ensure!(b != 0, CisError::invalid_input("b", "division by zero"));
///     Ok(a / b)
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! ensure {
    ($cond:expr, $err:expr) => {
        if !$cond {
            return Err($err);
        }
    };
    ($cond:expr, $err:expr, $msg:expr) => {
        if !$cond {
            use $crate::error::unified::{CisError, ErrorCategory};
            return Err(CisError::new($err, $msg));
        }
    };
}

/// Add context to an error.
///
/// This macro adds key-value context to an error, typically used in `map_err`.
///
/// # Examples
///
/// ```rust
/// use cis_core::error::{CisError, Result};
/// use cis_core::error::macros::context;
///
/// fn read_config() -> Result<String> {
///     std::fs::read_to_string("config.toml")
///         .map_err(|e| context!(e, "operation", "read_config", "path", "config.toml"))
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! context {
    ($err:expr, $($key:expr => $value:expr),+ $(,)?) => {{
        use $crate::error::unified::{CisError, ErrorCategory};
        let mut base_err = $err;
        // If it's already a CisError, add context
        if let Some(cis_err) = base_err.downcast_ref::<$crate::error::CisError>() {
            let mut err = cis_err.clone();
            $(
                err = err.with_context($key, $value);
            )+
            err
        } else {
            // Otherwise, wrap in a generic error
            let mut err = CisError::internal_error(base_err.to_string());
            $(
                err = err.with_context($key, $value);
            )+
            err.with_source(base_err)
        }
    }};
}

/// Convert an Option to a Result with an error if None.
///
/// # Examples
///
/// ```rust
/// use cis_core::error::{CisError, Result};
/// use cis_core::error::macros::try_err;
///
/// fn get_value(map: &std::collections::HashMap<String, i32>) -> Result<i32> {
///     try_err!(map.get("key"), CisError::not_found("key"))
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! try_err {
    ($opt:expr, $err:expr) => {
        match $opt {
            Some(val) => val,
            None => return Err($err),
        }
    };
}

/// Ensure a value is within a range, returning an error otherwise.
///
/// # Examples
///
/// ```rust
/// use cis_core::error::{CisError, Result};
/// use cis_core::error::macros::ensure_range;
///
/// fn set_volume(level: u8) -> Result<()> {
///     ensure_range!(level, 0..=100, CisError::invalid_input("level", "must be 0-100"));
///     Ok(())
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! ensure_range {
    ($value:expr, $range:expr, $err:expr) => {
        if !$range.contains(&$value) {
            return Err($err);
        }
    };
}

/// Retry an operation with exponential backoff.
///
/// # Examples
///
/// ```rust
/// use cis_core::error::{CisError, Result};
/// use cis_core::error::macros::retry;
/// use std::time::Duration;
///
/// async fn connect() -> Result<()> {
///     retry!(3, Duration::from_millis(100), {
///         attempt_connect().await
///     })
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! retry {
    ($max_attempts:expr, $delay:expr, $block:expr) => {{
        let mut attempts = 0;
        let mut delay = $delay;
        let result = loop {
            attempts += 1;
            match $block {
                Ok(result) => break Ok(result),
                Err(err) if err.is_retryable() && attempts < $max_attempts => {
                    if attempts < $max_attempts {
                        tokio::time::sleep(delay).await;
                        delay = delay.mul_f32(2.0); // Exponential backoff
                    }
                }
                Err(err) => break Err(err),
            }
        };
        result
    }};
}

/// Wrap an error with additional context and source location.
///
/// # Examples
///
/// ```rust
/// use cis_core::error::{CisError, Result};
/// use cis_core::error::macros::wrap_err;
///
/// fn read_file() -> Result<String> {
///     std::fs::read_to_string("file.txt")
///         .map_err(|e| wrap_err!(e, "Failed to read file", "path", "file.txt"))
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! wrap_err {
    ($source:expr, $msg:expr) => {{
        use $crate::error::unified::CisError;
        CisError::internal_error($msg)
            .with_source($source)
    }};
    ($source:expr, $msg:expr, $($key:expr => $value:expr),+ $(,)?) => {{
        use $crate::error::unified::CisError;
        let mut err = CisError::internal_error($msg)
            .with_source($source);
        $(
            err = err.with_context($key, $value);
        )+
        err
    }};
}

/// Create a not found error with the current location.
///
/// # Examples
///
/// ```rust
/// use cis_core::error::{CisError, Result};
/// use cis_core::error::macros::not_found;
///
/// fn get_user(id: &str) -> Result<User> {
///     let user = database.find_user(id)?;
///     not_found!(user, "User", id)
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! not_found {
    ($value:expr, $entity:expr, $id:expr) => {{
        use $crate::error::unified::CisError;
        return if $value.is_some() {
            Ok($value.unwrap())
        } else {
            Err(CisError::not_found(format!("{} with id {}", $entity, $id)))
        };
    }};
}

/// Create an invalid input error with details.
///
/// # Examples
///
/// ```rust
/// use cis_core::error::{CisError, Result};
/// use cis_core::error::macros::invalid_input;
///
/// fn parse_age(input: &str) -> Result<u32> {
///     let age = input.parse::<u32>()
///         .map_err(|_| invalid_input!("age", "not a valid number", input))?;
///     Ok(age)
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! invalid_input {
    ($field:expr, $reason:expr) => {{
        use $crate::error::unified::CisError;
        CisError::invalid_input($field, $reason)
    }};
    ($field:expr, $reason:expr, $value:expr) => {{
        use $crate::error::unified::CisError;
        CisError::invalid_input($field, $reason)
            .with_context("provided_value", $value)
    }};
}

/// Ensure a pre-condition is met.
///
/// Similar to `assert!` but returns an error instead of panicking.
///
/// # Examples
///
/// ```rust
/// use cis_core::error::{CisError, Result};
/// use cis_core::error::macros::precondition;
///
/// fn divide(a: i32, b: i32) -> Result<i32> {
///     precondition!(b != 0, "division by zero");
///     Ok(a / b)
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! precondition {
    ($cond:expr, $msg:expr) => {
        if !$cond {
            use $crate::error::unified::CisError;
            return Err(CisError::internal_error(format!(
                "Precondition failed: {}",
                $msg
            )));
        }
    };
    ($cond:expr, $msg:expr, $($key:expr => $value:expr),+ $(,)?) => {{
        if !$cond {
            use $crate::error::unified::CisError;
            let mut err = CisError::internal_error(format!(
                "Precondition failed: {}",
                $msg
            ));
            $(
                err = err.with_context($key, $value);
            )+
            return Err(err);
        }
    }};
}

/// Log and convert an error.
///
/// Useful for logging errors while still propagating them.
///
/// # Examples
///
/// ```rust
/// use cis_core::error::{CisError, Result};
/// use cis_core::error::macros::log_err;
///
/// fn process() -> Result<()> {
///     risky_operation()
///         .map_err(|e| log_err!(e, "Risky operation failed"))?;
///     Ok(())
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! log_err {
    ($err:expr, $msg:expr) => {{
        tracing::error!("{}: {:?}", $msg, $err);
        $err
    }};
    ($err:expr, $msg:expr, $($key:expr => $value:expr),+ $(,)?) => {{
        let context = vec![$((stringify!($key), $value)),+];
        tracing::error!("{}: {:?} | context: {:?}", $msg, $err, context);
        $err
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::unified::{CisError, ErrorCategory};

    #[test]
    fn test_bail_macro() {
        fn test_fn() -> Result<()> {
            bail!(CisError::invalid_input("test", "bail test"));
        }

        let result = test_fn();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("test"));
    }

    #[test]
    fn test_ensure_macro() {
        fn test_fn(value: i32) -> Result<()> {
            ensure!(value > 0, CisError::invalid_input("value", "must be positive"));
            Ok(())
        }

        assert!(test_fn(0).is_err());
        assert!(test_fn(1).is_ok());
    }

    #[test]
    fn test_ensure_range_macro() {
        fn test_fn(value: i32) -> Result<()> {
            ensure_range!(value, 0..=100, CisError::invalid_input("value", "out of range"));
            Ok(())
        }

        assert!(test_fn(-1).is_err());
        assert!(test_fn(50).is_ok());
        assert!(test_fn(101).is_err());
    }

    #[test]
    fn test_invalid_input_macro() {
        let err = invalid_input!("test_field", "invalid value", "bad_input");
        assert_eq!(err.category, ErrorCategory::Validation);
        assert!(err.to_string().contains("test_field"));
    }

    #[test]
    fn test_precondition_macro() {
        fn test_fn(a: i32, b: i32) -> Result<i32> {
            precondition!(b != 0, "division by zero");
            Ok(a / b)
        }

        assert!(test_fn(10, 0).is_err());
        assert!(test_fn(10, 2).is_ok());
    }
}
