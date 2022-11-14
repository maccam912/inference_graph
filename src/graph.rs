use futures::stream::FuturesUnordered;
use futures::{Future, StreamExt};
use std::error::Error;
use std::pin::Pin;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tokio::sync::broadcast::{channel, Receiver, Sender};

type BoxedFuture<T = String> = Pin<Box<dyn Future<Output = T>>>;

/// An `OpFn` is a regular function that returns a `Pin<Box<dyn Future<Output = String>>>`. This
/// is because Rust gets upset if I try to create a type alias of an `async fn`. A macro `wrap!` is provided
/// that will turn an `async fn(Vec<String>) -> String` into the `OpFn` type for you.
type OpFn = fn(Vec<String>) -> BoxedFuture;

/// A `Node` contains a `name` that other nodes use to refer to it, `inputs` to list the other `Node`s that it will require input from, and an operation `op`
/// that will run when all inputs are ready. The `Node` lists the `name`s of other `Node`s and the order they should be in. The `op` must be a function
/// that accepts a single argument of type `Vec<String>` which returns a `String`. This way, the other `Node`s referenced in `inputs`, when they have run,
/// will have single `String`s that will be passed in as part of the `Vec<String>` input to this `Node`s op.
pub struct Node {
    name: String,
    inputs: Vec<String>,
    op: OpFn,
    sender: Sender<String>,
}

impl Node {
    pub fn new(name: String, inputs: Vec<String>, op: OpFn, sender: Sender<String>) -> Self {
        Self {
            name,
            inputs,
            op,
            sender,
        }
    }
}

async fn run_node(node: &Rc<RefCell<Node>>, receivers: Vec<Receiver<String>>) {
    let mut inputs: Vec<String> = vec![];
    for mut r in receivers {
        if let Ok(i) = r.recv().await {
            inputs.push(i);
        } else {
            unreachable!();
        }
    }
    let t = (node.clone().borrow().op)(inputs);
    let result = t.await;
    let _ = node.borrow().sender.send(result);
}

/// A `Graph` stores a bunch of `Node`s (added with `stage_node`). It also has the `run` method, which will
/// let you pass in a `String` value to send to nodes referencing `entrypoint`, and let you request a final response
/// from a `Node` by referencing it with `output_name`.
#[derive(Default)]
pub struct Graph {
    graph: HashMap<String, Rc<RefCell<Node>>>,
    channels: HashMap<String, Sender<String>>,
}

impl<'a> Graph {
    /// `stage_node` lets you add a `Node` to the graph by providing the `name`, a list of other `Node`s (referenced by their `name`)
    /// that will be input to this `Node`s `op`, and finally the `op`. The simplest way to specify an `op` is to have an
    /// `async fn(Vec<String>) -> String` and wrap it with the `wrap!` macro.
    ///
    /// *At least one of the nodes needs to have only a single input named `entrypoint` which is where the rest of the inference graph
    /// will start.*
    pub fn stage_node(&mut self, name: String, inputs: Vec<String>, op: OpFn) {
        let (tx, _) = channel(1);
        let node = Rc::new(RefCell::new(Node::new(
            name.clone(),
            inputs,
            op,
            tx.clone(),
        )));
        self.graph.insert(name.clone(), node);
        self.channels.insert(name, tx);
    }

    /// `run` lets you pass in a `String` that will be sent to any nodes referencing `entrypoint` in their inputs. You must also pass in
    /// the `output_name` to reference the `Node` of that name as the final step in this run of the graph. Once that node has a value
    /// from its `op`, it will be returned to you in the `Result`.
    pub async fn run(
        &mut self,
        entrypoint_value: String,
        output_name: String,
    ) -> Result<String, Box<dyn Error>> {
        let (entrypoint_tx, _) = channel(1);

        self.channels
            .insert("entrypoint".into(), entrypoint_tx.clone());

        let mut tasks = FuturesUnordered::new();

        let mut my_receiver = self
            .channels
            .get(&output_name)
            .unwrap_or_else(|| panic!("Output node of name {output_name} does not exist"))
            .subscribe();

        for node in self.graph.values() {
            let parent_node_name = node.borrow().name.clone();
            let senders: Vec<Sender<String>> = node
                .borrow()
                .inputs
                .iter()
                .map(|name| {
                    self.channels
                        .get(name)
                        .unwrap_or_else(|| {
                            panic!("Node {parent_node_name} does not have {name} as an input")
                        })
                        .clone()
                })
                .collect();
            let receivers: Vec<Receiver<String>> = senders
                .iter()
                .map(tokio::sync::broadcast::Sender::subscribe)
                .collect();

            let task = run_node(node, receivers);
            tasks.push(task);
        }
        entrypoint_tx.send(entrypoint_value)?;

        while let Some(()) = tasks.next().await {}
        let result = my_receiver
            .recv()
            .await
            .expect("Could not receive anything on the output channel");
        Ok(result)
    }
}

/// The `wrap!` macro lets you pass in an `async fn(Vec<String>) -> String` function and it will converg
/// it to the right type for a `Node`s `op` field.
/// ```
/// # use inference_graph::wrap;
/// async fn concat_all(x: Vec<String>) -> String {
///   x.concat()
/// }
///
/// let wrapped_concat_all = wrap!(concat_all);
/// ```
#[macro_export]
macro_rules! wrap {
    ($x:expr) => {
        |x: Vec<String>| Box::pin(async move { $x(x).await })
    };
}
