/*!
`inference_graph` provides a few main items:
- a `Graph` definition.
- a way to add `Node`s to the `Graph` with `graph.stage_node`.
- a way to execute the `Graph` with some input.
- a `wrap!` macro to turn your async function into an op-compatible function.

The nodes also will need to specify an `op`, which is almost a
`async fn(Vec<String>) -> String`, but because of rust type aliases
not liking async functions, is not *quite* that exact type. Luckily,
we also provide a `wrap!` that lets you pass in a `async fn(Vec<String>) -> String`
and converts it to the exact type needed.

Creating a graph, adding some nodes that use an op to concatenate the strings passed in
for the argument, and retrieving the output might look something like this:
```
use inference_graph::graph::Graph;
use inference_graph::wrap;

async fn concat(x: Vec<String>) -> String {
  x.concat()
}

#[tokio::main]
async fn main() {
  let mut graph = Graph::default();
  graph.stage_node("A".into(), vec!["entrypoint".into()], wrap!(concat));
  graph.stage_node("B".into(), vec!["entrypoint".into()], wrap!(concat));
  graph.stage_node("C".into(), vec!["A".into(), "B".into()], wrap!(concat));
  let output = graph.run("hubba".into(), "C".into()).await;
  assert_eq!(output.unwrap(), "hubbahubba".to_string());
}
*/

pub mod graph;

#[cfg(test)]
mod config_tests {
    use crate::{graph, wrap};

    async fn concat(x: Vec<String>) -> String {
        x.concat()
    }

    #[tokio::test]
    async fn basic_graph() {
        let mut graph = graph::Graph::default();
        graph.stage_node("A".into(), vec!["entrypoint".into()], wrap!(concat));
        graph.stage_node("B".into(), vec!["entrypoint".into()], wrap!(concat));
        graph.stage_node("C".into(), vec!["A".into(), "B".into()], wrap!(concat));
        let output = graph.run("hubba".into(), "C".into()).await;
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "hubbahubba".to_string());
    }
}
