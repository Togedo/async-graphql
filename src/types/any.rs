use crate::{InputValueResult, Scalar, ScalarType, Value};

/// Any scalar (For [Apollo Federation](https://www.apollographql.com/docs/apollo-server/federation/introduction))
///
/// The `Any` scalar is used to pass representations of entities from external services into the root `_entities` field for execution.
#[derive(Clone, PartialEq, Debug)]
pub struct Any(pub Value);

/// The `_Any` scalar is used to pass representations of entities from external services into the root `_entities` field for execution.
#[Scalar(internal, name = "_Any")]
impl ScalarType for Any {
    fn parse(value: Value) -> InputValueResult<Self> {
        Ok(Self(value))
    }

    fn is_valid(_value: &Value) -> bool {
        true
    }

    fn to_value(&self) -> Value {
        self.0.clone()
    }
}

impl<T: Into<Value>> From<T> for Any {
    fn from(value: T) -> Any {
        Any(value.into())
    }
}

impl serde::Serialize for Any {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serde_json::to_string(&serde_json::Value::from(self.0.clone()))
            .unwrap()
            .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Any {
    fn deserialize<D>(deserializer: D) -> Result<Any, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        serde::Deserialize::deserialize(deserializer)
            .map(|s: String| Any::from(serde_json::from_str::<serde_json::Value>(&s).unwrap()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_conversion_ok() {
        let value = Value::List(vec![
            Value::Number(1.into()),
            Value::Boolean(true),
            Value::Null,
        ]);
        let expected = Any(value.clone());
        let output: Any = value.into();
        assert_eq!(output, expected);
    }
}
