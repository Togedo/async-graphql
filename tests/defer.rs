use async_graphql::*;

#[async_std::test]
pub async fn test_defer() {
    struct Query {
        value: i32,
    }

    #[Object]
    impl Query {
        async fn value_ref(&self) -> &i32 {
            &self.value
        }

        async fn value_owned(&self) -> i32 {
            20
        }
    }

    let schema = Schema::new(Query { value: 10 }, EmptyMutation, EmptySubscription);
    let query = r#"{
        valueRef @defer
        valueOwned @defer
    }"#;
    assert_eq!(
        schema.execute(&query).await.unwrap().data,
        serde_json::json!({
            "valueRef": 10,
            "valueOwned": 20
        })
    );
}
