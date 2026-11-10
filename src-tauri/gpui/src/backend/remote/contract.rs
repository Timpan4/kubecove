//! Check the native host's serialized requests against ADR 0025's declaration.
//! This catches missing required keys and misspelled optional keys before send.
use kubecove_lib::models::*;
use serde_json::Value;

macro_rules! remote_arguments {
    ($($transport:ident $mode:ident fn $name:ident($($key:ident: $ty:ty),* $(,)?)
        $(with $($host:ident $(<$state:ty>)?),+)?;)*) => {
        pub(super) fn validate(command: &str, value: &Value) -> Result<(), AppError> {
            let args = CommandArgs::new(value)?;
            match command {
                $(stringify!($name) => {
                    let keys: &[&str] = &[$(stringify!($key)),*];
                    if value.as_object().expect("CommandArgs requires an object").keys().any(|key| key != "channel" && !keys.contains(&key.as_str())) {
                        return Err(AppError::new("Unexpected remote command argument", AppErrorKind::Validation));
                    }
                    $(let _: $ty = args.get(stringify!($key))?;)*
                    Ok(())
                })*
                _ => Err(AppError::new("Unknown remote command", AppErrorKind::Validation)),
            }
        }
    }
}
kubecove_lib::models::command_contract!(remote_arguments);
