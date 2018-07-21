extern crate linked_list;
extern crate rand;

use linked_list::LinkedList;
use rand::prelude::*;
use std::collections::VecDeque;

struct Graph {
    adj_list: Vec<Vec<usize>>,
}

impl Graph {
    fn size(&self) -> usize {
        self.adj_list.len()
    }
    fn connect(&mut self, a: usize, b: usize) {
        if a != b {
            self.adj_list[a].push(b);
            self.adj_list[b].push(a);
        }
    }
    fn neighbours<'a>(&'a self, node: usize) -> impl Iterator<Item = usize> + 'a {
        self.adj_list[node].iter().cloned()
    }
}

fn random_graph() -> Graph {
    let mut rng = thread_rng();
    let mut graph = Graph {
        adj_list: vec![Vec::new(); 128],
    };
    for _ in 0..256 {
        let a = rng.gen_range(0, graph.size());
        let b = rng.gen_range(0, graph.size());
        graph.connect(a, b);
    }
    graph
}

/// Uses the linked list as the queue used in the shortest path breadth first graph
/// algorithm.
#[test]
fn shortest_path() {
    let graph = random_graph();

    // find the shortest path from node 0 to 1
    let start = 0;
    let end = 1;

    // we perform the algorithm with two queues simultaneously, checking that they do the
    // same thing
    let mut list_queue = LinkedList::new();
    let mut vec_queue = VecDeque::new();

    let mut visited = vec![false; graph.size()];

    list_queue.push_back((start, 0));
    vec_queue.push_back((start, 0));

    while let Some((node, len)) = list_queue.pop_front() {
        assert_eq!(Some((node, len)), vec_queue.pop_front());

        assert_eq!(vec_queue.len(), list_queue.len());

        if node == end {
            println!("shortest path has length {}", len);
            return;
        }
        if visited[node] {
            continue;
        }
        visited[node] = true;

        for neighbour in graph.neighbours(node) {
            if !visited[neighbour] {
                list_queue.push_back((neighbour, len + 1));
                vec_queue.push_back((neighbour, len + 1));
            }
        }
    }
    assert!(vec_queue.is_empty());
    println!("no path");
}
/// Performs a depth first search to determine if there is a path.
#[test]
fn has_path() {
    let graph = random_graph();

    // check if there is a path from 0 to 1
    let start = 0;
    let end = 1;

    // we perform the algorithm with two stacks simultaneously, checking that they do the
    // same thing
    let mut list_stack = LinkedList::new();
    let mut vec_stack = VecDeque::new();

    let mut visited = vec![false; graph.size()];

    list_stack.push_back(start);
    vec_stack.push_back(start);

    while let Some(node) = list_stack.pop_back() {
        assert_eq!(Some(node), vec_stack.pop_back());

        assert_eq!(vec_stack.len(), list_stack.len());

        if node == end {
            println!("there is a path");
            return;
        }
        if visited[node] {
            continue;
        }
        visited[node] = true;

        for neighbour in graph.neighbours(node) {
            if !visited[neighbour] {
                list_stack.push_back(neighbour);
                vec_stack.push_back(neighbour);
            }
        }
    }
    assert!(vec_stack.is_empty());
    println!("there is no path");
}
