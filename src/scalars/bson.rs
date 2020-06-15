use crate::{InputValueError, InputValueResult, ScalarType, Value};
use super::DateTimeUtc;
use async_graphql_derive::Scalar;
use bson::{oid::ObjectId, DateTime as UtcDateTime};

#[Scalar(internal)]
impl ScalarType for ObjectId {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(s) => Ok(ObjectId::with_string(&s)?),
            _ => Err(InputValueError::ExpectedType(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}

#[Scalar(internal, name = "DateTime")]
impl ScalarType for UtcDateTime {
    fn parse(value: Value) -> InputValueResult<Self> {
        DateTimeUtc::parse(value).map(|v| UtcDateTime::from(v.0))
    }

    fn to_value(&self) -> Value {
        DateTimeUtc(**self).to_value()
    }
}
