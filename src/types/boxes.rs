use crate::{
    registry, ContextSelectionSet, InputValueResult, InputValueType, OutputValueType, Pos, Result,
    Type, Value,
};

impl<T: InputValueType + Send + Sync> InputValueType for Box<T> {
    fn parse(value: Value) -> InputValueResult<Self> {
        Ok(Box::new(T::parse(value)?))
    }
}