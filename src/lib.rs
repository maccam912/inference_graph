pub mod graph;

#[cfg(test)]
mod config_tests {
    use crate::*;

    async fn sleep_concat(x: Vec<String>) -> String {
        x.concat()
    }

    #[tokio::test]
    async fn basic_graph() {
        console_subscriber::init();
        let mut graph = graph::Graph::default();
        graph.stage_node("A".into(), vec!["entrypoint".into()], wrap!(sleep_concat));
        graph.stage_node("B".into(), vec!["entrypoint".into()], wrap!(sleep_concat));
        graph.stage_node(
            "C".into(),
            vec!["A".into(), "B".into()],
            wrap!(sleep_concat),
        );
        let output = graph.run("hubba".into(), "C".into()).await;
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "hubbahubba".to_string());
    }
}
