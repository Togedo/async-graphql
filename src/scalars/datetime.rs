use crate::{InputValueError, InputValueResult, ScalarType, Value};
use async_graphql_derive::Scalar;
use chrono::{DateTime, Utc};
use chrono_english::{parse_date_string,Dialect};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
/// DateTime<Utc> wrapper struct
pub struct DateTimeUtc(pub DateTime<Utc>);

/// Implement the DateTime<Utc> scalar
///
/// The input/output is a string in RFC3339 format.
#[Scalar(internal, name = "DateTimeUtc")]
impl ScalarType for DateTimeUtc {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(s) => Ok(DateTimeUtc(if s == "NOW" {
                Utc::now()
            } else {
                parse_date_string(&s, Utc::now(), Dialect::Us)?
            })),
            _ => Err(InputValueError::ExpectedType(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.to_rfc3339())
    }
}

impl Default for DateTimeUtc {
    fn default() -> DateTimeUtc {
        DateTimeUtc(Utc::now())
    }
}