use futures::stream::FuturesUnordered;
use futures::{Future, StreamExt};
use std::error::Error;
use std::pin::Pin;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tokio::sync::broadcast::{channel, Receiver, Sender};

type BoxedFuture<T = String> = Pin<Box<dyn Future<Output = T>>>;

type OpFn = fn(Vec<String>) -> BoxedFuture;

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

#[derive(Default)]
pub struct Graph {
    graph: HashMap<String, Rc<RefCell<Node>>>,
    entrypoints: Vec<Rc<RefCell<Node>>>,
    channels: HashMap<String, Sender<String>>,
}

impl<'a> Graph {
    pub fn stage_node(&mut self, name: String, inputs: Vec<String>, op: OpFn) {
        let (tx, _) = channel(1);
        let node = Rc::new(RefCell::new(Node::new(
            name.clone(),
            inputs,
            op,
            tx.clone(),
        )));
        self.entrypoints.push(node.clone());
        self.graph.insert(name.clone(), node);
        self.channels.insert(name, tx);
    }

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

#[macro_export]
macro_rules! wrap {
    ($x:expr) => {
        |x: Vec<String>| Box::pin(async move { $x(x).await })
    };
}
