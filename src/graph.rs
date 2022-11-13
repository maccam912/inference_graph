use futures::stream::FuturesUnordered;
use futures::{Future, StreamExt};
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
    let name = node.borrow().name.clone();
    println!("Running node {name}");
    let mut inputs: Vec<String> = vec![];
    for mut r in receivers {
        inputs.push(r.recv().await.unwrap());
        println!("Got receiver value in {name}");
    }
    println!("Got all receiver values in {name}");
    let t = (node.clone().borrow().op)(inputs);
    let result = t.await;
    println!("Sending result in {name}");
    let _ = node.borrow().sender.send(result);
    println!("Done in {name}");
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
    ) -> Result<String, tokio::sync::broadcast::error::RecvError> {
        let (entrypoint_tx, _) = channel(1);

        self.channels
            .insert("entrypoint".into(), entrypoint_tx.clone());

        let mut tasks = FuturesUnordered::new();

        let mut my_receiver = self.channels.get(&output_name).unwrap().subscribe();

        for node in self.graph.values() {
            let senders: Vec<Sender<String>> = node
                .borrow()
                .inputs
                .iter()
                .map(|name| self.channels.get(name).unwrap().clone())
                .collect();
            let receivers: Vec<Receiver<String>> =
                senders.iter().map(tokio::sync::broadcast::Sender::subscribe).collect();

            let task = run_node(node, receivers);
            tasks.push(task);
        }
        entrypoint_tx.send(entrypoint_value).unwrap();

        while let Some(()) = tasks.next().await {}
        println!("Done with FuturesUnordered");
        let result = my_receiver.recv().await;
        result
    }
}

#[macro_export]
macro_rules! wrap {
    ($x:expr) => {
        |x: Vec<String>| Box::pin(async move { $x(x).await })
    };
}
